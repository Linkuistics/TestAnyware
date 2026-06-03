//! recovery-live.rs — live-VM fail-fast verification for the recovery driver
//! (grove leaf `110/030/010`, ADR-0008). **macOS host only.**
//!
//! Hardens the high-risk recovery automation *standalone*, before any finalize
//! code exists: it drives `recovery_boot_csrutil(setup, "csrutil disable")`
//! against a real setup VM and asserts that, after the normal reboot,
//! `csrutil status` over SSH reports SIP **disabled** — the authoritative
//! out-of-band gate (the in-recovery OCR/settle waits need not be perfect).
//!
//! ## How to run
//!
//! ```text
//! TESTANYWARE_LIVE_RECOVERY=1 cargo test -p testanyware-vm --test recovery-live \
//!     -- --ignored --nocapture recovery_disable_sip_live
//! ```
//!
//! Two guards keep the normal suite VM-free: the test is `#[ignore]`d, and even
//! under `--ignored` it early-returns unless `TESTANYWARE_LIVE_RECOVERY=1`.
//!
//! ## What setup VM it uses
//!
//! The recovery driver only needs a setup VM that is (a) SSH-reachable via the
//! host pubkey — for the graceful stop and the post-reboot wait — and (b)
//! bootable into Recovery. None of boot-1's Homebrew/Xcode/agent provisioning
//! touches the recovery path, so this harness builds a **minimal** setup VM
//! (clone the Cirrus vanilla image → boot → install the host pubkey) rather
//! than running the slow full `provision_boot1`. That isolates *this leaf's*
//! deliverable; the full boot-1 → recovery → finalize integration is verified
//! by `020-tcc-and-finalize` / the live-vm gate.
//!
//! The setup VM uses a **fixed id** (`testanyware-setup-rectest`) and is left
//! running on success, so re-runs reuse it (fast OCR/timing iteration —
//! ADR-0008 budgets live-VM iteration). Set `TESTANYWARE_RECOVERY_CLEANUP=1` to
//! tear it down at the end.

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::time::Duration;

use testanyware_vm::golden::{
    private_key_for, provision_boot1, ssh_pubkey_candidates, vanilla_image, GoldenOptions, SetupVm,
};
use testanyware_vm::paths::VmPaths;
use testanyware_vm::recovery::recovery_boot_csrutil;
use testanyware_vm::ssh::SshSession;
use testanyware_vm::{tart, process};

/// Vanilla Cirrus first-contact credentials (admin/admin until the pubkey is in).
const VANILLA_USER: &str = "admin";
const VANILLA_PASS: &str = "admin";
/// Fixed id so iterative re-runs reuse one setup VM.
const SETUP_ID: &str = "testanyware-setup-rectest";

fn opt_in() -> bool {
    std::env::var("TESTANYWARE_LIVE_RECOVERY").as_deref() == Ok("1")
}

fn version() -> String {
    std::env::var("TESTANYWARE_RECOVERY_VERSION").unwrap_or_else(|_| "tahoe".into())
}

/// The host pubkey + matching private key (the recovery driver authenticates
/// with the private half post-reboot). Panics if neither pair is present —
/// the whole flow depends on key auth.
fn host_keypair() -> (PathBuf, PathBuf) {
    let home = std::env::var("HOME").expect("HOME set");
    for pubkey in ssh_pubkey_candidates(std::path::Path::new(&home)) {
        let private = private_key_for(&pubkey);
        if pubkey.is_file() && private.is_file() {
            return (pubkey, private);
        }
    }
    panic!("no ~/.ssh/id_ed25519[.pub] or id_rsa[.pub] keypair found");
}

/// Build (or reuse) a minimal setup VM: clone the vanilla image, boot it, and
/// install the host pubkey so key auth works. Returns the [`SetupVm`] handoff.
async fn ensure_minimal_setup_vm(paths: &VmPaths) -> SetupVm {
    let (pubkey, private_key) = host_keypair();

    // Fresh clone every run keeps SIP in its vanilla (enabled) state, so
    // `csrutil disable` is a real state change to assert on.
    if tart::vm_exists(SETUP_ID) {
        eprintln!("Removing stale setup VM {SETUP_ID}...");
        tart::remove_existing(SETUP_ID);
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    let vanilla = vanilla_image(&version());
    eprintln!("Cloning {vanilla} -> {SETUP_ID}...");
    tart::clone(&vanilla, SETUP_ID).expect("tart clone vanilla");

    eprintln!("Booting setup VM...");
    let (pid, log_path) = tart::run_detached(SETUP_ID, &paths.vms_dir()).expect("tart run");
    assert!(
        tart::poll_vnc_url(&log_path, 60, Duration::from_secs(1)).is_some(),
        "setup VM did not produce a VNC URL (boot failed)"
    );

    eprint!("Waiting for guest IP...");
    let ip = tart::poll_ip(SETUP_ID, 60, Duration::from_secs(3)).expect("guest IP");
    eprintln!(" {ip}");

    eprint!("Waiting for SSH...");
    let pw = SshSession::wait_for_password(
        &ip, 22, VANILLA_USER, VANILLA_PASS, 60, Duration::from_secs(3),
    )
    .await
    .expect("password SSH to vanilla image");
    eprintln!(" ready.");

    eprintln!("Installing host pubkey...");
    pw.exec("mkdir -p ~/.ssh && chmod 700 ~/.ssh").await.expect("mk ~/.ssh");
    pw.upload(&pubkey, "/tmp/host_key.pub").await.expect("upload pubkey");
    pw.exec(
        "cat /tmp/host_key.pub >> ~/.ssh/authorized_keys && \
         chmod 600 ~/.ssh/authorized_keys && rm /tmp/host_key.pub",
    )
    .await
    .expect("append authorized_keys");
    drop(pw);

    // Verify key auth before handing off to recovery.
    let key_session = SshSession::connect_key(&ip, 22, VANILLA_USER, &private_key)
        .await
        .expect("key auth after pubkey install");
    let echo = key_session.exec("echo ok").await.expect("echo over key auth");
    assert!(echo.stdout.contains("ok"), "key-auth echo failed");
    drop(key_session);
    eprintln!("Setup VM ready (pid {pid}, ip {ip}).");

    SetupVm { id: SETUP_ID.into(), pid, ip, key_path: private_key }
}

/// Optional full-fidelity variant: build the setup VM via the real
/// `provision_boot1`. Slow (Homebrew + Xcode CLT); opt in with
/// `TESTANYWARE_RECOVERY_FULL_BOOT1=1`.
async fn full_boot1_setup_vm(paths: &VmPaths) -> SetupVm {
    let opts = GoldenOptions {
        version: version(),
        name: format!("testanyware-golden-macos-{}", version()),
    };
    provision_boot1(&opts, paths).await.expect("provision_boot1")
}

#[tokio::test]
#[ignore = "drives a real macOS VM; opt in with TESTANYWARE_LIVE_RECOVERY=1"]
async fn recovery_disable_sip_live() {
    if !opt_in() {
        eprintln!("skipped: set TESTANYWARE_LIVE_RECOVERY=1 to run the live recovery gate");
        return;
    }

    let paths = VmPaths::from_process_env();

    let setup = if std::env::var("TESTANYWARE_RECOVERY_FULL_BOOT1").as_deref() == Ok("1") {
        full_boot1_setup_vm(&paths).await
    } else {
        ensure_minimal_setup_vm(&paths).await
    };

    // The deliverable under test: drive recovery + csrutil disable end-to-end.
    let rebooted = recovery_boot_csrutil(&setup, "csrutil disable", &paths)
        .await
        .expect("recovery_boot_csrutil(csrutil disable)");

    // Authoritative out-of-band gate: csrutil status over SSH post-reboot.
    eprintln!("Checking csrutil status over SSH...");
    let session = SshSession::connect_key(&rebooted.ip, 22, VANILLA_USER, &rebooted.key_path)
        .await
        .expect("SSH to rebooted setup VM");
    let status = session.exec("csrutil status").await.expect("csrutil status");
    eprintln!("csrutil status: {}", status.stdout.trim());

    let disabled = status.stdout.to_lowercase().contains("disabled");

    if std::env::var("TESTANYWARE_RECOVERY_CLEANUP").as_deref() == Ok("1") {
        eprintln!("Cleaning up setup VM {}...", rebooted.id);
        tart::remove_existing(&rebooted.id);
        process::terminate(rebooted.pid, Duration::from_millis(200), 10);
    } else {
        eprintln!(
            "Leaving setup VM {} running for inspection / re-runs \
             (set TESTANYWARE_RECOVERY_CLEANUP=1 to tear down).",
            rebooted.id
        );
    }

    assert!(
        disabled,
        "SIP should be disabled after recovery_boot_csrutil; csrutil status was: {}",
        status.stdout.trim()
    );
}
