//! ipsw-base-live.rs — live-VM verification for the IPSW base VM
//! (grove leaf `ipsw-base-vm-k14`). **macOS host only.**
//!
//! Both tests are `#[ignore]`d and, even under `--ignored`, return early unless
//! `TESTANYWARE_LIVE_IPSW_BASE=1`.
//!
//! ## Iterate on the Setup Assistant driver (minutes, no reinstall)
//!
//! Installing macOS from the IPSW is a one-off of a few minutes; walking Setup
//! Assistant is the part that needs tuning. Make a fresh, never-booted VM once:
//!
//! ```text
//! tart create my-fresh-27 --from-ipsw ~/.local/share/testanyware/cache/macos-27.0.1-26A434.ipsw
//! ```
//!
//! then run the driver against a clone of it, as often as needed:
//!
//! ```text
//! TESTANYWARE_LIVE_IPSW_BASE=1 TESTANYWARE_IPSW_FRESH_VM=my-fresh-27 \
//!   cargo test -p testanyware-vm --test ipsw-base-live -- --ignored --nocapture configure_fresh_clone_live
//! ```
//!
//! ## The whole path, as `vm create-golden --version 27` runs it
//!
//! ```text
//! TESTANYWARE_LIVE_IPSW_BASE=1 \
//!   cargo test -p testanyware-vm --test ipsw-base-live -- --ignored --nocapture ensure_base_vm_live
//! ```
//!
//! Resolves the IPSW from Apple's catalogue, downloads it (resumable, cached),
//! installs, configures, and leaves `testanyware-base-macos-27` stopped. A
//! no-op if that VM already exists.
//!
//! Each test ends by *booting* the result and logging in over SSH as
//! `admin`/`admin` — the base's whole contract — then shutting it down.

#![cfg(target_os = "macos")]

use std::time::Duration;

use testanyware_vm::ipsw_base::{base_vm_name, configure_fresh_vm, ensure_base_vm};
use testanyware_vm::paths::VmPaths;
use testanyware_vm::ssh::SshSession;
use testanyware_vm::{process, tart};

fn opt_in() -> bool {
    std::env::var("TESTANYWARE_LIVE_IPSW_BASE").as_deref() == Ok("1")
}

/// Boot `vm`, log in over SSH with the password, check the base's contract, and
/// shut it down again.
async fn verify_base(vm: &str, paths: &VmPaths) {
    let (pid, log) = tart::run_detached(vm, &paths.vms_dir()).expect("tart run");
    assert!(
        tart::poll_vnc_url(&log, 60, Duration::from_secs(1)).is_some(),
        "{vm} did not boot"
    );
    let ip = tart::poll_ip(vm, 60, Duration::from_secs(3)).expect("guest IP");
    eprintln!("{vm} is at {ip}");

    let ssh = SshSession::wait_for_password(&ip, 22, "admin", "admin", 60, Duration::from_secs(3))
        .await
        .expect("ssh admin/admin");
    let run = |cmd: &'static str| {
        let ssh = &ssh;
        async move { ssh.exec(cmd).await.expect(cmd) }
    };

    let ver = run("sw_vers -productVersion").await;
    eprintln!("macOS {}", ver.stdout.trim());
    assert!(ver.stdout.trim().starts_with("27."), "not macOS 27: {:?}", ver.stdout);
    assert_eq!(run("sudo -n true").await.exit_code, 0, "sudo is not passwordless");
    assert_eq!(
        run("sudo test -s /etc/kcpassword").await.exit_code,
        0,
        "no auto-login password stored"
    );
    let login = run("defaults read /Library/Preferences/com.apple.loginwindow autoLoginUser").await;
    assert_eq!(login.stdout.trim(), "admin", "auto-login user not set");
    let sleep = run("pmset -g | grep displaysleep").await;
    assert!(sleep.stdout.split_whitespace().last() == Some("0"), "display sleep on: {:?}", sleep.stdout);

    let _ = ssh.exec("sudo shutdown -h now").await;
    for _ in 0..60 {
        if !process::process_alive(pid) {
            return;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    tart::stop(vm);
}

#[tokio::test]
#[ignore = "live VM: macOS install clone + Setup Assistant"]
async fn configure_fresh_clone_live() {
    if !opt_in() {
        return;
    }
    let fresh = std::env::var("TESTANYWARE_IPSW_FRESH_VM")
        .expect("set TESTANYWARE_IPSW_FRESH_VM to a never-booted VM made by `tart create --from-ipsw`");
    let paths = VmPaths::from_process_env();
    let work = "testanyware-base-livetest";
    tart::remove_existing(work);
    tart::clone(&fresh, work).expect("tart clone fresh VM");

    configure_fresh_vm(work, &paths).await.expect("configure_fresh_vm");
    verify_base(work, &paths).await;

    if std::env::var("TESTANYWARE_IPSW_KEEP").as_deref() != Ok("1") {
        tart::remove_existing(work);
    }
}

#[tokio::test]
#[ignore = "live VM: full IPSW base build (downloads ~25 GB on first run)"]
async fn ensure_base_vm_live() {
    if !opt_in() {
        return;
    }
    let paths = VmPaths::from_process_env();
    let name = ensure_base_vm(27, &paths).await.expect("ensure_base_vm");
    assert_eq!(name, base_vm_name(27));
    verify_base(&name, &paths).await;
}
