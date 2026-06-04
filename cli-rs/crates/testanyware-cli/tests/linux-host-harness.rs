//! linux-host-harness.rs — the self-hosted host-CLI verification harness
//! (grove `190-linux-verification-harness`, ADR-0009), Linux-first.
//!
//! A green cross-*build* is not proof the cross binary *runs*: the dynamic
//! loader, glibc floor, and the `ffmpeg-next` libav link can all fail only at
//! runtime, on the target OS/arch. This harness runs the **cross-compiled
//! `testanyware`** binary *inside a real native-arch (aarch64) Linux guest* and
//! asserts it executes and emits correct contract envelopes — "test the host
//! CLI with the product."
//!
//! Leaf `010` lands the reusable skeleton + the **endpoint-free band**: no macOS
//! golden, no port-forward, no OCR. Leaves `020`/`030` bolt the golden + the
//! in-process forward + the endpoint-driven / OCR bands onto the same machinery.
//!
//! ## How to run
//!
//! ```text
//! # 1. cross-build the aarch64-linux binary (BtbN ffmpeg-8 sysroot — see
//! #    docs/research/170-ffmpeg-cross-link.md):
//! export PKG_CONFIG_ALLOW_CROSS=1
//! export PKG_CONFIG_LIBDIR=/tmp/taw-ffmpeg-sr/aarch64-linux/lib/pkgconfig
//! cargo zigbuild -p testanyware-cli --bin testanyware \
//!   --target aarch64-unknown-linux-gnu --release
//!
//! # 2. run the harness (it clones a stock Ubuntu ARM64 HUT and provisions it):
//! TESTANYWARE_LINUX_HARNESS=1 cargo test -p testanyware-cli \
//!   --test linux-host-harness -- --ignored linux_host_harness
//! ```
//!
//! Two independent guards keep the normal suite VM-free, mirroring
//! `live-vm-gate.rs`: the test is `#[ignore]`d, and even under `--ignored` it
//! early-returns unless `TESTANYWARE_LINUX_HARNESS=1`.
//!
//! Inputs (env, with defaults):
//!   * `TESTANYWARE_LINUX_BIN`     — the aarch64-linux `testanyware` to verify
//!     (default: `target/aarch64-unknown-linux-gnu/release/testanyware`).
//!   * `TESTANYWARE_FFMPEG_LIB_DIR` — dir holding the ffmpeg-8 runtime `.so`s
//!     (default: `/tmp/taw-ffmpeg-sr/aarch64-linux/lib`). See the libav note.
//!
//! ## The three bands (ADR-0009)
//!
//! [`classify_band`] is the durable split this harness is organised around and
//! that `020`/`030` extend:
//!   * **endpoint-free** — no target: `capabilities`, `schema`,
//!     `llm-instructions`, `doctor`, plus `--help`/`--version`/dry-runs. Proves
//!     the binary execs, links resolve, and envelopes emit. **This leaf (`010`).**
//!   * **endpoint-driven** — against a forwarded golden: `agent` HTTP actions,
//!     `input *`, `screen capture`/`size`/`record` → `020`; `find-text` (OCR)
//!     → `030`.
//!   * **build/compile-only** — never run in-guest: `vm start/stop/list/delete`,
//!     `vm create-golden` (nested virt / host-orchestration). Asserted by the
//!     macOS cross-build, not exercised here.
//!
//! ## CRITICAL — libav is a *load-time* dependency
//!
//! `testanyware-video` does `use ffmpeg_next` (a normal link, not `dlopen`), so
//! the ELF carries hard `NEEDED libavcodec.so.62 / libavformat.so.62 /
//! libavutil.so.60 / libswscale.so.9` (ffmpeg **8.1** sonames). Stock Ubuntu
//! 24.04 ships ffmpeg **6.1** (`libav*.so.60`), so the loader fails to resolve
//! `NEEDED` **before `main`** — even `testanyware --version` will not exec until
//! the ffmpeg-8 `.so`s are staged beside the binary. Staging the BtbN
//! `linuxarm64-gpl-shared` `.so` bundle is therefore a *baseline* requirement,
//! not a `record`-only one. The harness uploads [`REQUIRED_SONAMES`] and runs
//! the binary with `LD_LIBRARY_PATH` pointed at them; the first in-guest command
//! ([the `--version` canary][canary]) confirms the staging is correct.
//!
//! [canary]: linux_host_harness
//!
//! ## Arch coverage — x86_64 is build-verified ONLY (logged, not silently covered)
//!
//! This Mac boots only **ARM64** guests natively (tart Ubuntu ARM64). An x86_64
//! ELF cannot run on an ARM64 guest, so **only aarch64 builds are verified at
//! runtime here.** The `x86_64-unknown-linux-gnu` build is link-verified by the
//! cross-build (`docs/research/170-ffmpeg-cross-link.md`) but its *runtime* is
//! **unverified on this host** — closable later only with a real x86_64 box or
//! TCG emulation, if it earns the cost (ADR-0009 no-silent-caps). The harness
//! prints this gap in its final summary so a reader never mistakes a green run
//! for x86_64 coverage.
//!
//! ## Reuse seam (for the deferred Windows harness)
//!
//! Built once here, swapped there: the **provisioning channel**
//! ([`ProvisionChannel`]) — Linux uses ssh ([`SshChannel`]); the Windows leaf
//! adds an agent (`file upload`/`exec`) impl, since Windows ships no sshd.
//! Shared unchanged: the band driver ([`run_band`]) and the band classifier.

#![cfg(target_os = "macos")]
// The harness clones a stock OCI image via `testanyware_vm::tart` directly
// (`vm start` only starts goldens), and `tart` is macOS-host only. So the whole
// file — including the offline unit tests — compiles and runs only on the macOS
// host, which is also the only place a "plain cargo test" ever runs (no Linux
// CI; local release on an arm64 Mac).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;
use testanyware_vm::{tart, ExecOutput, SshSession};

// ---------------------------------------------------------------------------
// Gating + config
// ---------------------------------------------------------------------------

/// The harness only drives a VM when explicitly opted in.
fn gate_enabled() -> bool {
    std::env::var("TESTANYWARE_LINUX_HARNESS").as_deref() == Ok("1")
}

/// Path to the aarch64-linux `testanyware` under test. Defaults to the
/// conventional `cargo zigbuild` output relative to this crate's manifest.
fn linux_bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("TESTANYWARE_LINUX_BIN") {
        return PathBuf::from(p);
    }
    // CARGO_MANIFEST_DIR = <repo>/cli-rs/crates/testanyware-cli; the target dir
    // is two levels up at <repo>/cli-rs/target.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("aarch64-unknown-linux-gnu")
        .join("release")
        .join("testanyware")
}

/// Dir holding the ffmpeg-8 runtime `.so` bundle (BtbN `linuxarm64-gpl-shared`).
fn ffmpeg_lib_dir() -> PathBuf {
    std::env::var("TESTANYWARE_FFMPEG_LIB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/taw-ffmpeg-sr/aarch64-linux/lib"))
}

/// The ffmpeg-8 sonames the cross binary `NEEDED`s, plus their bundled
/// transitive deps (`libavcodec` → `libswresample` + `libavutil`). x264/x265 are
/// statically linked into BtbN's `libavcodec`, so they are not separate `.so`s.
/// All five must be staged for the binary to *load* (see the libav note above).
const REQUIRED_SONAMES: &[&str] = &[
    "libavcodec.so.62",
    "libavformat.so.62",
    "libavutil.so.60",
    "libswscale.so.9",
    "libswresample.so.6",
];

/// Absolute in-guest dir the binary + `.so`s are provisioned into. The Cirrus
/// Ubuntu image's first user is `admin` with home `/home/admin`.
const RUN_DIR: &str = "/home/admin/taw";

// ---------------------------------------------------------------------------
// Pure helpers (offline-unit-tested below)
// ---------------------------------------------------------------------------

/// The three-band surface split (ADR-0009). The single source of truth for
/// which commands run where; `020`/`030` consume it to pick their cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Band {
    /// No endpoint needed — proves the binary execs + emits envelopes.
    EndpointFree,
    /// Needs a live agent/VNC endpoint (driven via the forwarded golden).
    EndpointDriven,
    /// Never run in-guest (nested virt / host-orchestration); build-verified only.
    BuildOnly,
}

/// Classify a canonical command path into its verification band. Flags
/// (`--help`, `--version`, `--dry-run`) are endpoint-free regardless of the
/// command they decorate; this classifies the *command* by its happy path.
fn classify_band(path: &[&str]) -> Band {
    match path {
        ["capabilities", ..]
        | ["schema", ..]
        | ["llm-instructions", ..]
        | ["doctor", ..] => Band::EndpointFree,
        ["vm", ..] => Band::BuildOnly,
        // agent / input / screen / file all drive a live endpoint.
        _ => Band::EndpointDriven,
    }
}

/// Build the in-guest invocation: `LD_LIBRARY_PATH=<dir> <dir>/testanyware …`.
/// `LD_LIBRARY_PATH` (vs a build-time `$ORIGIN` rpath) keeps the binary itself
/// untouched and the staging visible at the call site.
fn taw_cmd(run_dir: &str, args: &[&str]) -> String {
    format!(
        "LD_LIBRARY_PATH={dir} {dir}/testanyware {args}",
        dir = run_dir,
        args = args.join(" "),
    )
}

/// Parse `out.stdout` as a JSON object, surfacing a readable error (including a
/// stderr tail) when it does not — the common shape every envelope check needs.
fn parse_json(out: &ExecOutput) -> Result<Value, String> {
    serde_json::from_str(&out.stdout).map_err(|e| {
        format!(
            "stdout did not parse as JSON ({e}); exit={} stderr: {}",
            out.exit_code,
            out.stderr.trim(),
        )
    })
}

/// Assert the standard success envelope: exit 0 and `{ok:true, schema_version:…}`.
/// Returns the parsed body for further field checks.
fn expect_ok_envelope(out: &ExecOutput) -> Result<Value, String> {
    if out.exit_code != 0 {
        return Err(format!("exit {} (want 0); stderr: {}", out.exit_code, out.stderr.trim()));
    }
    let body = parse_json(out)?;
    if body.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(format!("missing `ok: true`; got: {body}"));
    }
    if !body.get("schema_version").map(Value::is_string).unwrap_or(false) {
        return Err(format!("missing string `schema_version`; got: {body}"));
    }
    Ok(body)
}

/// Whether `err` reads as the load-time-libav failure (a missing/incompatible
/// shared object) — used to give the `--version` canary a pointed diagnostic.
fn looks_like_missing_shared_object(stderr: &str) -> bool {
    let s = stderr.to_lowercase();
    s.contains("cannot open shared object file")
        || s.contains("error while loading shared libraries")
        || (s.contains(".so") && s.contains("no such file"))
}

// ---------------------------------------------------------------------------
// Provisioning channel seam (the node's reuse seam — Windows adds a 2nd impl)
// ---------------------------------------------------------------------------

/// The in-guest control channel. Linux uses ssh ([`SshChannel`]); the deferred
/// Windows harness adds an agent-backed impl (`file upload` + `exec`) without
/// touching [`run_band`]. Kept minimal: exec a command, upload a file.
#[allow(async_fn_in_trait)] // private test trait, single static-dispatch impl
trait ProvisionChannel {
    /// Run `cmd` to completion, returning its captured streams + exit code.
    async fn exec(&self, cmd: &str) -> Result<ExecOutput, String>;
    /// Upload `local` to absolute `remote`.
    async fn upload(&self, local: &Path, remote: &str) -> Result<(), String>;
}

/// ssh-backed channel over the ADR-0007 `russh` [`SshSession`].
struct SshChannel {
    session: SshSession,
}

impl ProvisionChannel for SshChannel {
    async fn exec(&self, cmd: &str) -> Result<ExecOutput, String> {
        self.session.exec(cmd).await.map_err(|e| format!("ssh exec `{cmd}`: {e}"))
    }
    async fn upload(&self, local: &Path, remote: &str) -> Result<(), String> {
        self.session
            .upload(local, remote)
            .await
            .map_err(|e| format!("ssh upload {} -> {remote}: {e}", local.display()))
    }
}

/// Run a channel command and require exit 0 — for provisioning steps (`mkdir`,
/// `chmod`) whose failure should abort, unlike a band case's soft assertion.
async fn exec_ok(ch: &impl ProvisionChannel, cmd: &str) -> Result<(), String> {
    let out = ch.exec(cmd).await?;
    if out.exit_code != 0 {
        return Err(format!(
            "`{cmd}` exited {} (want 0); stderr: {}",
            out.exit_code,
            out.stderr.trim(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// HUT lifecycle — a throwaway stock Ubuntu ARM64 clone with a teardown guard
// ---------------------------------------------------------------------------

/// The stock base image cloned for each HUT (already pulled locally).
const HUT_BASE: &str = "ghcr.io/cirruslabs/ubuntu:24.04";

/// A running host-under-test clone. Dropping it stops + deletes the clone, so a
/// panicking assertion never leaks a VM (mirrors `live-vm-gate.rs::VmGuard`).
struct Hut {
    id: String,
    ip: String,
}

impl Hut {
    /// Clone the stock Ubuntu base to a throwaway id, boot it detached, and
    /// poll (state-gated, [[tart-ip-lies]]) for the guest IP.
    fn launch() -> Result<Self, String> {
        // testanyware-hut-<hex8>: distinct from goldens and from `vm`-managed
        // clones, but still under the `testanyware-` prefix the runner expects.
        let id = format!(
            "testanyware-hut-{}",
            testanyware_vm::generate_id()
                .strip_prefix("testanyware-")
                .unwrap_or("clone")
        );
        if tart::vm_exists(&id) {
            tart::remove_existing(&id);
        }
        tart::clone(HUT_BASE, &id).map_err(|e| format!("tart clone {HUT_BASE} {id}: {e}"))?;

        let log_dir = std::env::temp_dir().join("testanyware-linux-harness");
        let (_pid, _log) = tart::run_detached(&id, &log_dir)
            .map_err(|e| format!("tart run {id}: {e}"))?;

        let ip = tart::poll_ip(&id, 60, Duration::from_secs(2)).ok_or_else(|| {
            tart::remove_existing(&id);
            format!("HUT {id} never reported a running IP")
        })?;
        eprintln!("[hut] {id} up at {ip}");
        Ok(Self { id, ip })
    }
}

impl Drop for Hut {
    fn drop(&mut self) {
        eprintln!("[hut] tearing down {}", self.id);
        tart::remove_existing(&self.id);
    }
}

// ---------------------------------------------------------------------------
// Provisioning: ssh in, install pubkey, key-auth, stage the binary + ffmpeg
// ---------------------------------------------------------------------------

/// Provision the HUT over ssh and return a key-authed [`SshChannel`] plus the
/// tempdir holding the generated key (kept alive by the caller).
///
/// First-contact auth on the stock Cirrus image is `admin`/`admin` (the brief's
/// claim, verified here at run time). We connect once with the password to
/// install a freshly generated pubkey, then reconnect with key auth — exactly
/// the macOS-golden pattern, and the one place the Linux harness exercises
/// `SshSession::connect_key` on a Linux sshd.
async fn provision_ssh(ip: &str) -> Result<(SshChannel, tempfile::TempDir), String> {
    // Generate a throwaway ed25519 keypair with the system ssh-keygen.
    let keydir = tempfile::tempdir().map_err(|e| format!("keydir: {e}"))?;
    let key_path = keydir.path().join("id_ed25519");
    let status = std::process::Command::new("ssh-keygen")
        .args(["-t", "ed25519", "-N", "", "-q", "-f"])
        .arg(&key_path)
        .status()
        .map_err(|e| format!("spawn ssh-keygen: {e}"))?;
    if !status.success() {
        return Err(format!("ssh-keygen exited {status}"));
    }
    let pub_path = key_path.with_extension("pub");
    let pubkey = std::fs::read_to_string(&pub_path)
        .map_err(|e| format!("read {}: {e}", pub_path.display()))?
        .trim()
        .to_string();

    // Password session (verifies admin/admin first-contact auth).
    let pw = SshSession::wait_for_password(ip, 22, "admin", "admin", 60, Duration::from_secs(2))
        .await
        .map_err(|e| format!("password auth admin/admin failed (first-contact): {e}"))?;
    eprintln!("[provision] password auth ok; installing pubkey");
    install_pubkey(&pw, &pubkey).await?;
    drop(pw);

    // Reconnect with key auth — exercises the russh pubkey path on Linux sshd.
    let keyed = SshSession::connect_key(ip, 22, "admin", &key_path)
        .await
        .map_err(|e| format!("pubkey auth after install failed: {e}"))?;
    eprintln!("[provision] pubkey auth ok");
    Ok((SshChannel { session: keyed }, keydir))
}

/// Append `pubkey` to the remote's `authorized_keys` over an authenticated
/// session. `single-quoted` because an OpenSSH pubkey contains no single quotes.
async fn install_pubkey(session: &SshSession, pubkey: &str) -> Result<(), String> {
    let cmd = format!(
        "mkdir -p ~/.ssh && chmod 700 ~/.ssh && \
         printf '%s\\n' '{pubkey}' >> ~/.ssh/authorized_keys && \
         chmod 600 ~/.ssh/authorized_keys"
    );
    let out = session.exec(&cmd).await.map_err(|e| format!("install pubkey: {e}"))?;
    if out.exit_code != 0 {
        return Err(format!("install pubkey exited {}: {}", out.exit_code, out.stderr.trim()));
    }
    Ok(())
}

/// Upload the cross binary + the ffmpeg `.so` bundle into [`RUN_DIR`] and mark
/// the binary executable (SFTP does not preserve the exec bit).
async fn stage_binary(ch: &impl ProvisionChannel) -> Result<(), String> {
    let bin = linux_bin_path();
    if !bin.is_file() {
        return Err(format!(
            "cross binary not found at {} — cross-build it first (see the module docs)",
            bin.display(),
        ));
    }
    let lib_dir = ffmpeg_lib_dir();

    exec_ok(ch, &format!("mkdir -p {RUN_DIR}")).await?;
    ch.upload(&bin, &format!("{RUN_DIR}/testanyware")).await?;
    exec_ok(ch, &format!("chmod +x {RUN_DIR}/testanyware")).await?;

    for soname in REQUIRED_SONAMES {
        let local = lib_dir.join(soname);
        if !local.exists() {
            return Err(format!(
                "ffmpeg runtime lib {} missing — stage the BtbN linuxarm64-gpl-shared \
                 bundle (see docs/research/170-ffmpeg-cross-link.md)",
                local.display(),
            ));
        }
        // `upload` reads through the soname symlink to the real versioned `.so`.
        ch.upload(&local, &format!("{RUN_DIR}/{soname}")).await?;
    }
    eprintln!("[provision] staged binary + {} ffmpeg libs into {RUN_DIR}", REQUIRED_SONAMES.len());
    Ok(())
}

// ---------------------------------------------------------------------------
// Band driver — band-agnostic so 020/030 add bands without rewriting it
// ---------------------------------------------------------------------------

/// One smoke case: a label, the args after `testanyware`, and a check over the
/// command's output. The check returns `Ok(note)` or `Err(reason)` so one red
/// case never masks the rest (à la `live-vm-gate.rs`).
struct BandCase {
    name: &'static str,
    args: Vec<&'static str>,
    check: fn(&ExecOutput) -> Result<String, String>,
}

/// Run each case over the channel and collect outcomes. Generic over the
/// channel so the Windows harness reuses it verbatim with its agent impl.
async fn run_band(
    ch: &impl ProvisionChannel,
    run_dir: &str,
    cases: &[BandCase],
) -> Vec<(&'static str, Result<String, String>)> {
    let mut out = Vec::with_capacity(cases.len());
    for case in cases {
        let cmd = taw_cmd(run_dir, &case.args);
        let result = match ch.exec(&cmd).await {
            Ok(exec) => (case.check)(&exec),
            Err(e) => Err(format!("channel exec failed: {e}")),
        };
        out.push((case.name, result));
    }
    out
}

/// The endpoint-free band (this leaf). Each case needs no target — it proves
/// the binary runs, links resolve, and the contract envelopes emit on aarch64.
fn endpoint_free_cases() -> Vec<BandCase> {
    vec![
        BandCase {
            name: "help",
            args: vec!["--help"],
            check: |o| {
                if o.exit_code != 0 {
                    return Err(format!("exit {}; stderr: {}", o.exit_code, o.stderr.trim()));
                }
                if !o.stdout.contains("testanyware") {
                    return Err("--help did not mention the binary name".into());
                }
                Ok("--help exits 0 and names the binary".into())
            },
        },
        BandCase {
            name: "capabilities",
            args: vec!["capabilities", "--json"],
            check: |o| {
                let body = expect_ok_envelope(o)?;
                let subs = body
                    .get("subcommands")
                    .and_then(Value::as_array)
                    .ok_or("capabilities.subcommands missing/!array")?;
                if subs.is_empty() {
                    return Err("capabilities.subcommands is empty".into());
                }
                Ok(format!("ok envelope; {} subcommand groups", subs.len()))
            },
        },
        BandCase {
            name: "schema",
            // `schema vm list` → a JSON Schema document ($schema/type).
            args: vec!["schema", "vm", "list"],
            check: |o| {
                if o.exit_code != 0 {
                    return Err(format!("exit {}; stderr: {}", o.exit_code, o.stderr.trim()));
                }
                let body = parse_json(o)?;
                let obj = body.as_object().ok_or("schema output is not a JSON object")?;
                if !(obj.contains_key("$schema") || obj.contains_key("type")) {
                    return Err(format!("not a JSON Schema (no $schema/type); got: {body}"));
                }
                Ok("schema vm list emits a JSON Schema".into())
            },
        },
        BandCase {
            name: "llm-instructions",
            args: vec!["llm-instructions"],
            check: |o| {
                if o.exit_code != 0 {
                    return Err(format!("exit {}; stderr: {}", o.exit_code, o.stderr.trim()));
                }
                if o.stdout.trim().is_empty() {
                    return Err("llm-instructions produced empty stdout".into());
                }
                Ok(format!("emitted {} bytes of guide", o.stdout.len()))
            },
        },
        BandCase {
            name: "doctor",
            args: vec!["doctor", "--json"],
            // doctor is read-only; it exits 0 (healthy) or 1 (a check failed —
            // expected on a bare Ubuntu host with no tart/qemu). Assert it
            // *runs and emits a valid envelope*, not that every check passes.
            check: |o| {
                if !matches!(o.exit_code, 0 | 1) {
                    return Err(format!("exit {} (want 0|1); stderr: {}", o.exit_code, o.stderr.trim()));
                }
                let body = parse_json(o)?;
                if !body.get("schema_version").map(Value::is_string).unwrap_or(false) {
                    return Err(format!("doctor missing string schema_version; got: {body}"));
                }
                if !body.get("ok").map(Value::is_boolean).unwrap_or(false) {
                    return Err(format!("doctor `ok` not boolean; got: {body}"));
                }
                if !body.get("checks").map(Value::is_object).unwrap_or(false) {
                    return Err(format!("doctor `checks` not an object; got: {body}"));
                }
                Ok(format!("valid report envelope (exit {}, ok={})", o.exit_code, body["ok"]))
            },
        },
        BandCase {
            name: "dry-run",
            // A mutating command's dry-run short-circuits before any network I/O
            // (cf. cli-contract.rs::each_mutating_command_supports_dry_run), so
            // it is endpoint-free: exit 0 with `dry_run: true`.
            args: vec!["input", "key", "a", "--dry-run", "--json"],
            check: |o| {
                if o.exit_code != 0 {
                    return Err(format!("exit {} (want 0); stderr: {}", o.exit_code, o.stderr.trim()));
                }
                let body = parse_json(o)?;
                if body.get("dry_run").and_then(Value::as_bool) != Some(true) {
                    return Err(format!("missing `dry_run: true`; got: {body}"));
                }
                Ok("input key --dry-run plans without mutating".into())
            },
        },
    ]
}

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "live VM: TESTANYWARE_LINUX_HARNESS=1 cargo test --test linux-host-harness -- --ignored linux_host_harness"]
async fn linux_host_harness() {
    if !gate_enabled() {
        eprintln!(
            "linux_host_harness: skipped — set TESTANYWARE_LINUX_HARNESS=1 to clone a \
             stock Ubuntu ARM64 HUT and verify the aarch64 cross binary."
        );
        return;
    }

    // tart drives the HUT lifecycle; run_detached/poll_ip block, which is fine
    // for a serial test on a multi-thread runtime.
    let hut = Hut::launch().unwrap_or_else(|e| panic!("HUT launch failed: {e}"));
    let _guard = &hut; // dropped at scope end → stop + delete

    let (channel, _keydir) = provision_ssh(&hut.ip)
        .await
        .unwrap_or_else(|e| panic!("provisioning failed: {e}"));
    stage_binary(&channel)
        .await
        .unwrap_or_else(|e| panic!("staging the cross binary failed: {e}"));

    // Canary: the FIRST in-guest command proves the binary execs — i.e. the
    // load-time libav `NEEDED` resolves. A failure here is almost always a
    // mis-staged `.so` bundle, so say so plainly (see the libav note).
    let version = channel
        .exec(&taw_cmd(RUN_DIR, &["--version"]))
        .await
        .unwrap_or_else(|e| panic!("--version canary: channel exec failed: {e}"));
    if version.exit_code != 0 {
        let hint = if looks_like_missing_shared_object(&version.stderr) {
            "\n  ↑ this is the load-time-libav failure: the ffmpeg-8 .so bundle is \
             mis-staged. Confirm REQUIRED_SONAMES are all present in TESTANYWARE_FFMPEG_LIB_DIR."
        } else {
            ""
        };
        panic!(
            "--version canary failed: the cross binary does not exec on aarch64-linux \
             (exit {}).\n  stderr: {}{hint}",
            version.exit_code,
            version.stderr.trim(),
        );
    }
    eprintln!("[canary] --version execs on aarch64-linux: {}", version.stdout.trim());

    // Endpoint-free band.
    let results = run_band(&channel, RUN_DIR, &endpoint_free_cases()).await;

    eprintln!("\nlinux_host_harness — endpoint-free band:");
    let mut failures = Vec::new();
    for (name, res) in &results {
        match res {
            Ok(note) => eprintln!("  ✓ {name}: {note}"),
            Err(reason) => {
                eprintln!("  ✗ {name}: {reason}");
                failures.push(format!("{name}: {reason}"));
            }
        }
    }

    // ADR-0009 no-silent-caps: state the arch gap so a green run is never
    // mistaken for x86_64 runtime coverage.
    eprintln!(
        "\n[arch] aarch64-linux: runtime-verified in-guest above. \
         x86_64-linux: BUILD-verified only (no native x86_64 guest on this Mac) — \
         runtime gap is open and accepted (ADR-0009)."
    );

    assert!(
        failures.is_empty(),
        "{} of {} endpoint-free checks failed:\n  - {}",
        failures.len(),
        results.len(),
        failures.join("\n  - "),
    );
    eprintln!("\nlinux_host_harness: all {} endpoint-free checks passed.", results.len());
}

// ---------------------------------------------------------------------------
// Offline unit tests for the pure helpers (run in a plain `cargo test` on the
// macOS host; no VM). These pin the contracts the live harness depends on.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod helper_tests {
    use super::*;

    fn out(stdout: &str, stderr: &str, code: i32) -> ExecOutput {
        ExecOutput { stdout: stdout.into(), stderr: stderr.into(), exit_code: code }
    }

    #[test]
    fn classify_band_splits_the_surface() {
        assert_eq!(classify_band(&["capabilities"]), Band::EndpointFree);
        assert_eq!(classify_band(&["schema", "vm", "list"]), Band::EndpointFree);
        assert_eq!(classify_band(&["llm-instructions"]), Band::EndpointFree);
        assert_eq!(classify_band(&["doctor"]), Band::EndpointFree);

        assert_eq!(classify_band(&["vm", "start"]), Band::BuildOnly);
        assert_eq!(classify_band(&["vm", "create-golden"]), Band::BuildOnly);

        assert_eq!(classify_band(&["agent", "snapshot"]), Band::EndpointDriven);
        assert_eq!(classify_band(&["input", "click"]), Band::EndpointDriven);
        assert_eq!(classify_band(&["screen", "find-text"]), Band::EndpointDriven);
        assert_eq!(classify_band(&["file", "upload"]), Band::EndpointDriven);
    }

    #[test]
    fn taw_cmd_sets_ld_library_path_and_abs_binary() {
        let cmd = taw_cmd("/home/admin/taw", &["capabilities", "--json"]);
        assert_eq!(
            cmd,
            "LD_LIBRARY_PATH=/home/admin/taw /home/admin/taw/testanyware capabilities --json"
        );
    }

    #[test]
    fn expect_ok_envelope_accepts_a_well_formed_body() {
        let o = out(r#"{"ok":true,"schema_version":"1","subcommands":["vm"]}"#, "", 0);
        let body = expect_ok_envelope(&o).expect("well-formed envelope");
        assert_eq!(body["subcommands"][0], "vm");
    }

    #[test]
    fn expect_ok_envelope_rejects_nonzero_exit_and_bad_shape() {
        assert!(expect_ok_envelope(&out("{}", "boom", 1)).is_err(), "nonzero exit");
        assert!(expect_ok_envelope(&out(r#"{"ok":false,"schema_version":"1"}"#, "", 0)).is_err(), "ok:false");
        assert!(expect_ok_envelope(&out(r#"{"ok":true}"#, "", 0)).is_err(), "no schema_version");
        assert!(expect_ok_envelope(&out("not json", "", 0)).is_err(), "non-JSON");
    }

    #[test]
    fn parse_json_error_includes_stderr_tail() {
        let e = parse_json(&out("garbage", "loader: cannot open shared object", 127)).unwrap_err();
        assert!(e.contains("cannot open shared object"), "stderr surfaced: {e}");
    }

    #[test]
    fn detects_load_time_libav_failures() {
        assert!(looks_like_missing_shared_object(
            "testanyware: error while loading shared libraries: libavcodec.so.62: \
             cannot open shared object file: No such file or directory"
        ));
        assert!(!looks_like_missing_shared_object("usage: testanyware [OPTIONS]"));
    }

    #[test]
    fn required_sonames_cover_the_binarys_needed_set() {
        // Guards against silently dropping a soname the loader needs. The four
        // directly-NEEDED libs (170 research) plus libswresample (libavcodec's
        // bundled transitive dep) must all be present.
        for needed in ["libavcodec.so.62", "libavformat.so.62", "libavutil.so.60", "libswscale.so.9"] {
            assert!(REQUIRED_SONAMES.contains(&needed), "missing directly-NEEDED {needed}");
        }
        assert!(REQUIRED_SONAMES.contains(&"libswresample.so.6"), "missing transitive dep");
    }

    #[test]
    fn endpoint_free_cases_are_all_endpoint_free() {
        // Each endpoint-free case is one of: a global flag only (`--help`), the
        // dry-run carve-out (an endpoint-driven command short-circuited offline
        // by `--dry-run`), or a genuinely endpoint-free *command*. Only the
        // last must classify as EndpointFree — the band's defining property.
        for case in endpoint_free_cases() {
            let cmd: Vec<&str> = case.args.iter().copied().filter(|a| !a.starts_with('-')).collect();
            if cmd.is_empty() || case.name == "dry-run" {
                continue;
            }
            assert_eq!(
                classify_band(&cmd),
                Band::EndpointFree,
                "case {} command {cmd:?} is not endpoint-free",
                case.name,
            );
        }
    }
}
