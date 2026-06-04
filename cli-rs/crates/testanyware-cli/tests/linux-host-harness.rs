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
//! Leaf `010` landed the reusable skeleton + the **endpoint-free band** (no
//! golden, no forward, no OCR). Leaf `020` (this) bolts on the **macOS golden +
//! the in-process host→golden TCP forward** and runs the **endpoint-driven band
//! minus OCR**: `agent` HTTP actions, `input *`, `screen capture`/`size`, and
//! `screen record`→mp4 — the runtime proof of the `170` ffmpeg-8 encoder on
//! aarch64-linux. Leaf `030` adds `screen find-text` (OCR) on the same machinery.
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
//! # 2. run the harness. It clones a stock Ubuntu ARM64 HUT, brings up a macOS
//! #    golden (`testanyware vm start --platform macos`), forwards the golden's
//! #    agent + VNC through the host, and runs both bands:
//! TESTANYWARE_LINUX_HARNESS=1 cargo test -p testanyware-cli \
//!   --test linux-host-harness -- --ignored linux_host_harness
//! ```
//!
//! ## The golden + the in-process forward (`020`)
//!
//! The endpoint-driven band needs a live agent/VNC endpoint. We bring up a real
//! kept-built tart macOS golden via the host CLI subprocess (`vm start
//! --platform macos --json`, matching `live-vm-gate.rs`), read its per-VM spec
//! ([`VmSpec`]) for the golden's `agent {host,port}` + `vnc {host,port,password}`,
//! and stand up an **in-process tokio TCP proxy** ([`PortForward`]) that binds
//! `0.0.0.0:0` on the host and splices to the golden. The guest cannot route to
//! the golden directly, but its default gateway *is* the host (ADR-0009), so the
//! in-guest CLI targets `--agent <gateway>:<afwd>` / `--vnc <gateway>:<vfwd>`
//! (+ `TESTANYWARE_VNC_PASSWORD`). The gateway is discovered in-guest via
//! [`parse_default_gateway`] over `ip route show default`. The forward, the
//! gateway discovery, the band driver, and the endpoint targeting are the shared
//! machinery the deferred Windows harness reuses — only [`ProvisionChannel`]
//! differs there (ssh → in-VM agent).
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
//!     `input *`, `screen capture`/`size`/`record` → **this leaf (`020`)**;
//!     `find-text` (OCR) → `030`.
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

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use testanyware_vm::{tart, ExecOutput, SshSession, VmPaths, VmSpec};

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

/// Build the in-guest invocation: `<envs…> LD_LIBRARY_PATH=<dir>
/// <dir>/testanyware <args…>`. `LD_LIBRARY_PATH` (vs a build-time `$ORIGIN`
/// rpath) keeps the binary itself untouched and the staging visible at the call
/// site. The `envs` prefix carries per-case environment — the endpoint-driven
/// band passes `TESTANYWARE_VNC_PASSWORD` here so the password never lands in
/// the argv (and matches how `resolve.rs` sources it).
fn build_invocation(run_dir: &str, envs: &[(&str, &str)], args: &[&str]) -> String {
    let mut prefix = String::new();
    for (k, v) in envs {
        prefix.push_str(k);
        prefix.push('=');
        prefix.push_str(v);
        prefix.push(' ');
    }
    format!(
        "{prefix}LD_LIBRARY_PATH={dir} {dir}/testanyware {args}",
        dir = run_dir,
        args = args.join(" "),
    )
}

/// The no-env convenience used by the `--version` canary and the offline tests.
fn taw_cmd(run_dir: &str, args: &[&str]) -> String {
    build_invocation(run_dir, &[], args)
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

/// One smoke case: a label, the args after `testanyware`, optional per-case env
/// (e.g. `TESTANYWARE_VNC_PASSWORD`), and a check over the command's output. The
/// check returns `Ok(note)` or `Err(reason)` so one red case never masks the
/// rest (à la `live-vm-gate.rs`). Args/envs are owned `String`s because the
/// endpoint-driven band interpolates the runtime-discovered forward endpoints.
struct BandCase {
    name: &'static str,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    check: fn(&ExecOutput) -> Result<String, String>,
}

/// Construct an env-free case from string slices (the endpoint-free band).
fn case(
    name: &'static str,
    args: &[&str],
    check: fn(&ExecOutput) -> Result<String, String>,
) -> BandCase {
    BandCase {
        name,
        args: args.iter().map(|s| s.to_string()).collect(),
        envs: Vec::new(),
        check,
    }
}

/// Construct a case carrying per-invocation env (the endpoint-driven VNC band
/// passes the framebuffer password this way).
fn case_env(
    name: &'static str,
    args: &[&str],
    envs: &[(&str, &str)],
    check: fn(&ExecOutput) -> Result<String, String>,
) -> BandCase {
    BandCase {
        name,
        args: args.iter().map(|s| s.to_string()).collect(),
        envs: envs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        check,
    }
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
        let arg_refs: Vec<&str> = case.args.iter().map(String::as_str).collect();
        let env_refs: Vec<(&str, &str)> =
            case.envs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let cmd = build_invocation(run_dir, &env_refs, &arg_refs);
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
        case("help", &["--help"], |o| {
            if o.exit_code != 0 {
                return Err(format!("exit {}; stderr: {}", o.exit_code, o.stderr.trim()));
            }
            if !o.stdout.contains("testanyware") {
                return Err("--help did not mention the binary name".into());
            }
            Ok("--help exits 0 and names the binary".into())
        }),
        case("capabilities", &["capabilities", "--json"], |o| {
            let body = expect_ok_envelope(o)?;
            let subs = body
                .get("subcommands")
                .and_then(Value::as_array)
                .ok_or("capabilities.subcommands missing/!array")?;
            if subs.is_empty() {
                return Err("capabilities.subcommands is empty".into());
            }
            Ok(format!("ok envelope; {} subcommand groups", subs.len()))
        }),
        // `schema vm list` → a JSON Schema document ($schema/type).
        case("schema", &["schema", "vm", "list"], |o| {
            if o.exit_code != 0 {
                return Err(format!("exit {}; stderr: {}", o.exit_code, o.stderr.trim()));
            }
            let body = parse_json(o)?;
            let obj = body.as_object().ok_or("schema output is not a JSON object")?;
            if !(obj.contains_key("$schema") || obj.contains_key("type")) {
                return Err(format!("not a JSON Schema (no $schema/type); got: {body}"));
            }
            Ok("schema vm list emits a JSON Schema".into())
        }),
        case("llm-instructions", &["llm-instructions"], |o| {
            if o.exit_code != 0 {
                return Err(format!("exit {}; stderr: {}", o.exit_code, o.stderr.trim()));
            }
            if o.stdout.trim().is_empty() {
                return Err("llm-instructions produced empty stdout".into());
            }
            Ok(format!("emitted {} bytes of guide", o.stdout.len()))
        }),
        // doctor is read-only; it exits 0 (healthy) or 1 (a check failed —
        // expected on a bare Ubuntu host with no tart/qemu). Assert it *runs and
        // emits a valid envelope*, not that every check passes.
        case("doctor", &["doctor", "--json"], |o| {
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
        }),
        // A mutating command's dry-run short-circuits before any network I/O
        // (cf. cli-contract.rs::each_mutating_command_supports_dry_run), so it is
        // endpoint-free: exit 0 with `dry_run: true`.
        case("dry-run", &["input", "key", "a", "--dry-run", "--json"], |o| {
            if o.exit_code != 0 {
                return Err(format!("exit {} (want 0); stderr: {}", o.exit_code, o.stderr.trim()));
            }
            let body = parse_json(o)?;
            if body.get("dry_run").and_then(Value::as_bool) != Some(true) {
                return Err(format!("missing `dry_run: true`; got: {body}"));
            }
            Ok("input key --dry-run plans without mutating".into())
        }),
    ]
}

// ===========================================================================
// 020 — the macOS golden, the in-process forward, and the endpoint-driven band
// ===========================================================================

// --- host-gateway discovery (pure parse; unit-tested) ----------------------

/// Extract the default gateway from `ip route show default` output. The guest's
/// default route *is* the host on tart's NAT (ADR-0009), so this is the address
/// the in-guest CLI dials to reach the host-side forward. The line looks like
/// `default via 192.168.64.1 dev enp0s1 proto dhcp src 192.168.64.7 metric 100`
/// — we take the token after `via`. Returns `None` if no default route is seen.
fn parse_default_gateway(ip_route_output: &str) -> Option<String> {
    for line in ip_route_output.lines() {
        let mut head = line.split_whitespace();
        if head.next() != Some("default") {
            continue;
        }
        let mut toks = line.split_whitespace();
        while let Some(tok) = toks.next() {
            if tok == "via" {
                return toks.next().map(str::to_string);
            }
        }
    }
    None
}

/// Parse `od -An -tx1` hex output (space-separated two-digit bytes) into bytes.
/// Used to read a guest file's magic number back over the channel without
/// widening [`ProvisionChannel`] with a download op (keeps the Windows reuse
/// seam minimal). Non-hex tokens (none expected from `od -An`) are skipped.
fn parse_od_hex(od_output: &str) -> Vec<u8> {
    od_output
        .split_whitespace()
        .filter_map(|t| u8::from_str_radix(t, 16).ok())
        .collect()
}

/// Discover the in-guest host-gateway address over the channel.
async fn discover_gateway(ch: &impl ProvisionChannel) -> Result<String, String> {
    let out = ch.exec("ip route show default").await?;
    if out.exit_code != 0 {
        return Err(format!(
            "`ip route show default` exited {}: {}",
            out.exit_code,
            out.stderr.trim()
        ));
    }
    parse_default_gateway(&out.stdout).ok_or_else(|| {
        format!("no default gateway in `ip route show default`: {:?}", out.stdout.trim())
    })
}

// --- the in-process host→golden TCP forward (the reusable machinery) --------

/// An in-process tokio TCP proxy: binds `0.0.0.0:0` on the host and splices each
/// inbound connection to `target` (a golden endpoint). Bound on `0.0.0.0` — not
/// `127.0.0.1` — so the guest reaches it via the host-gateway. Dropping it
/// signals shutdown and aborts the accept loop, so no listener leaks across
/// tests. This is pure Rust (no `socat`/`ssh -L`) and is exactly the machinery
/// the deferred Windows harness reuses unchanged.
struct PortForward {
    local_port: u16,
    shutdown: watch::Sender<bool>,
    accept_task: tokio::task::JoinHandle<()>,
    label: &'static str,
}

impl PortForward {
    async fn spawn(label: &'static str, target: SocketAddr) -> Result<Self, String> {
        let listener = TcpListener::bind(("0.0.0.0", 0u16))
            .await
            .map_err(|e| format!("bind forward [{label}]: {e}"))?;
        let local_port = listener
            .local_addr()
            .map_err(|e| format!("local_addr [{label}]: {e}"))?
            .port();
        let (tx, rx) = watch::channel(false);
        let accept_task = tokio::spawn(forward_accept_loop(label, listener, target, rx));
        eprintln!("[forward] {label}: 0.0.0.0:{local_port} -> {target}");
        Ok(Self { local_port, shutdown: tx, accept_task, label })
    }
}

impl Drop for PortForward {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        self.accept_task.abort();
        eprintln!("[forward] {} shut down", self.label);
    }
}

/// Accept connections until shutdown, spawning a bidirectional splice per conn.
async fn forward_accept_loop(
    label: &'static str,
    listener: TcpListener,
    target: SocketAddr,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok((inbound, peer)) => {
                    let sd = shutdown.clone();
                    tokio::spawn(async move {
                        if let Err(e) = forward_splice(inbound, target, sd).await {
                            eprintln!("[forward] {label} conn from {peer}: {e}");
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[forward] {label} accept error: {e}");
                    break;
                }
            },
        }
    }
}

/// Connect to `target` and `copy_bidirectional` until either side EOFs or
/// shutdown fires.
async fn forward_splice(
    mut inbound: TcpStream,
    target: SocketAddr,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), String> {
    let mut outbound = TcpStream::connect(target)
        .await
        .map_err(|e| format!("connect {target}: {e}"))?;
    tokio::select! {
        r = tokio::io::copy_bidirectional(&mut inbound, &mut outbound) => {
            r.map(|_| ()).map_err(|e| format!("splice: {e}"))
        }
        _ = shutdown.changed() => Ok(()),
    }
}

// --- the macOS golden endpoint (host CLI subprocess + spec read) ------------

/// The built host `testanyware` (macOS arm64), driven as a subprocess to manage
/// the golden — matching `live-vm-gate.rs` (exercises the real command surface).
const HOST_BIN: &str = env!("CARGO_BIN_EXE_testanyware");

/// Platform of the golden whose endpoint the forward targets. macOS is the only
/// kept-built golden with deterministic on-screen content ([[minimal-images]]);
/// override only if a different golden is staged.
fn golden_platform() -> String {
    std::env::var("TESTANYWARE_LINUX_HARNESS_GOLDEN").unwrap_or_else(|_| "macos".into())
}

/// Run the host CLI as a subprocess off the async runtime (it blocks).
async fn host_run(args: &[&str]) -> Output {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let shown = owned.join(" ");
    tokio::task::spawn_blocking(move || Command::new(HOST_BIN).args(&owned).output())
        .await
        .expect("spawn_blocking join")
        .unwrap_or_else(|e| panic!("failed to invoke host `{HOST_BIN} {shown}`: {e}"))
}

/// The golden's host-reachable endpoints, read from its per-VM spec.
struct GoldenEndpoints {
    agent: SocketAddr,
    vnc: SocketAddr,
    vnc_password: Option<String>,
}

/// Stops the golden via `vm stop <id>` on drop, so a panicking endpoint-driven
/// check never leaks a running golden (the `010` `Hut` guard's macOS-side twin).
struct GoldenGuard {
    id: String,
}

impl Drop for GoldenGuard {
    fn drop(&mut self) {
        match Command::new(HOST_BIN).args(["vm", "stop", &self.id, "--json"]).output() {
            Ok(o) if o.status.success() => eprintln!("[golden] stopped {}", self.id),
            Ok(o) => eprintln!(
                "[golden] WARNING: vm stop {} exited {:?}: {}",
                self.id,
                o.status.code(),
                String::from_utf8_lossy(&o.stderr),
            ),
            Err(e) => eprintln!("[golden] WARNING: vm stop {} failed to spawn: {e}", self.id),
        }
    }
}

/// Start the golden via `vm start --platform <p> --json` and return its id. The
/// caller wraps the id in a [`GoldenGuard`] *before* any further fallible step.
async fn start_golden_vm() -> String {
    let platform = golden_platform();
    let out = host_run(&["vm", "start", "--platform", &platform, "--json"]).await;
    assert!(
        out.status.success(),
        "`vm start --platform {platform}` exited {:?}\nstdout: {}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let body: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!("vm start stdout not JSON ({e}): {}", String::from_utf8_lossy(&out.stdout))
    });
    let id = body
        .get("id")
        .and_then(Value::as_str)
        .expect("vm start --json must return an id")
        .to_string();
    eprintln!("[golden] started {id}");
    id
}

/// Read the golden's per-VM spec (written by `vm start`) and resolve its agent +
/// VNC endpoints to host-reachable [`SocketAddr`]s. The forward splices to these.
async fn golden_endpoints(id: &str) -> Result<GoldenEndpoints, String> {
    let spec_path = VmPaths::from_process_env().spec_path(id);
    let spec = VmSpec::load(&spec_path)
        .map_err(|e| format!("load golden spec {}: {e}", spec_path.display()))?;
    let agent_spec = spec.agent.ok_or_else(|| {
        format!("golden spec {} has no agent endpoint (agent unreachable at start?)", spec_path.display())
    })?;
    let agent = to_socket_addr(&agent_spec.host, agent_spec.port).await?;
    let vnc = to_socket_addr(&spec.vnc.host, spec.vnc.port).await?;
    Ok(GoldenEndpoints { agent, vnc, vnc_password: spec.vnc.password })
}

/// Resolve `host:port` (an IP for tart goldens) to a single [`SocketAddr`].
async fn to_socket_addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    tokio::net::lookup_host((host, port))
        .await
        .map_err(|e| format!("resolve {host}:{port}: {e}"))?
        .next()
        .ok_or_else(|| format!("no address resolved for {host}:{port}"))
}

/// Whether a snapshot shows the Finder menu bar with a `File` item — the
/// desktop-rendered signal `live-vm-gate.rs` waits on before driving the guest.
fn snapshot_menu_bar_ready(snap: &Value) -> bool {
    let Some(windows) = snap.get("windows").and_then(Value::as_array) else {
        return false;
    };
    windows.iter().any(|w| {
        w.get("windowType").and_then(Value::as_str) == Some("menuBar")
            && w.get("elements")
                .and_then(Value::as_array)
                .is_some_and(|els| {
                    els.iter()
                        .any(|e| e.get("label").and_then(Value::as_str) == Some("File"))
                })
    })
}

/// Poll `agent snapshot --vm <id>` (via the host CLI) until the Finder menu bar
/// renders, so the endpoint-driven input/screen checks have real content.
async fn wait_for_golden_ready(id: &str, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    // Assigned on every iteration before the deadline check reads it.
    let mut last: String;
    loop {
        let out = host_run(&["agent", "snapshot", "--vm", id, "--json"]).await;
        if out.status.success() {
            match serde_json::from_slice::<Value>(&out.stdout) {
                Ok(snap) if snapshot_menu_bar_ready(&snap) => return Ok(()),
                Ok(_) => last = "snapshot had no Finder menu bar `File` item".into(),
                Err(e) => last = format!("snapshot stdout not JSON: {e}"),
            }
        } else {
            last = format!(
                "agent snapshot exited {:?}: {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr),
            );
        }
        if Instant::now() >= deadline {
            return Err(format!("golden never became ready: {last}"));
        }
        tokio::time::sleep(Duration::from_millis(750)).await;
    }
}

// --- the endpoint-driven band (minus OCR) -----------------------------------

/// Agent HTTP actions over `--agent <gw>:<afwd>` — proves the cross binary's
/// HTTP client runs on aarch64-linux and reaches the forwarded golden agent.
/// Landing correctness is already covered by the macOS `live-vm-gate`; here the
/// point is that the *client runs on the target and speaks the wire*.
fn endpoint_driven_agent_cases(agent_ep: &str) -> Vec<BandCase> {
    vec![
        case("agent-health", &["agent", "health", "--agent", agent_ep, "--json"], |o| {
            expect_ok_envelope(o)?;
            Ok("agent health responds through the forward".into())
        }),
        case("agent-snapshot", &["agent", "snapshot", "--agent", agent_ep, "--json"], |o| {
            let body = expect_ok_envelope(o)?;
            let windows = body
                .get("windows")
                .and_then(Value::as_array)
                .ok_or("snapshot envelope has no `windows` array")?;
            if windows.is_empty() {
                return Err("snapshot returned zero windows".into());
            }
            Ok(format!("snapshot decoded {} windows", windows.len()))
        }),
        case("agent-windows", &["agent", "windows", "--agent", agent_ep, "--json"], |o| {
            expect_ok_envelope(o)?;
            Ok("agent windows responds through the forward".into())
        }),
        // `agent wait` is one of the 010-ported HTTP actions; it blocks until the
        // AX tree is ready, which it already is on a rendered golden.
        case("agent-wait", &["agent", "wait", "--agent", agent_ep, "--timeout", "10", "--json"], |o| {
            expect_ok_envelope(o)?;
            Ok("agent wait (ported HTTP action) responds through the forward".into())
        }),
    ]
}

/// `screen size` over `--vnc <gw>:<vfwd>` — proves the RFB client handshakes the
/// forwarded framebuffer and reads its dimensions.
fn endpoint_driven_screen_size_case(vnc_ep: &str, vnc_password: &str) -> Vec<BandCase> {
    vec![case_env(
        "screen-size",
        &["screen", "size", "--vnc", vnc_ep, "--json"],
        &[("TESTANYWARE_VNC_PASSWORD", vnc_password)],
        |o| {
            let body = expect_ok_envelope(o)?;
            let (w, h) = (
                body.get("width").and_then(Value::as_u64).unwrap_or(0),
                body.get("height").and_then(Value::as_u64).unwrap_or(0),
            );
            if w == 0 || h == 0 {
                return Err(format!("screen size returned implausible dims: {body}"));
            }
            Ok(format!("RFB handshake → {w}x{h} framebuffer"))
        },
    )]
}

/// `input *` over `--vnc <gw>:<vfwd>` — proves the RFB *input* client runs on
/// aarch64-linux and the events are accepted through the forward. Mutating, so
/// these run last; coordinates target empty desktop to avoid opening UI.
fn endpoint_driven_input_cases(vnc_ep: &str, vnc_password: &str) -> Vec<BandCase> {
    let pw: &[(&str, &str)] = &[("TESTANYWARE_VNC_PASSWORD", vnc_password)];
    vec![
        case_env("input-key", &["input", "key", "Escape", "--vnc", vnc_ep, "--json"], pw, |o| {
            expect_ok_envelope(o)?;
            Ok("input key Escape accepted through the forward".into())
        }),
        case_env("input-type", &["input", "type", "testanyware", "--vnc", vnc_ep, "--json"], pw, |o| {
            expect_ok_envelope(o)?;
            Ok("input type accepted through the forward".into())
        }),
        case_env("input-click", &["input", "click", "400", "400", "--vnc", vnc_ep, "--json"], pw, |o| {
            expect_ok_envelope(o)?;
            Ok("input click accepted through the forward".into())
        }),
    ]
}

// --- bespoke file-producing checks (read the artifact back in-guest) --------

/// `stat -c %s` the guest file.
async fn guest_file_size(ch: &impl ProvisionChannel, path: &str) -> Result<u64, String> {
    let out = ch.exec(&format!("stat -c %s {path}")).await?;
    if out.exit_code != 0 {
        return Err(format!("stat {path} exited {}: {}", out.exit_code, out.stderr.trim()));
    }
    out.stdout
        .trim()
        .parse::<u64>()
        .map_err(|e| format!("parse size of {path} from {:?}: {e}", out.stdout.trim()))
}

/// Read the first `n` bytes of a guest file as raw bytes (via `head | od`).
async fn guest_file_head(ch: &impl ProvisionChannel, path: &str, n: usize) -> Result<Vec<u8>, String> {
    let out = ch.exec(&format!("head -c {n} {path} | od -An -v -tx1")).await?;
    if out.exit_code != 0 {
        return Err(format!("read head of {path} exited {}: {}", out.exit_code, out.stderr.trim()));
    }
    Ok(parse_od_hex(&out.stdout))
}

/// `screen capture --region …` over the forward, asserting the cross-built RFB
/// decoder produced a correctly-sized PNG in-guest.
async fn check_capture(
    ch: &impl ProvisionChannel,
    run_dir: &str,
    vnc_ep: &str,
    vnc_password: &str,
) -> Result<String, String> {
    let path = format!("{run_dir}/cap.png");
    let region = "0,0,250,28"; // static left menu-bar slice (no clock/notifications)
    let args = ["screen", "capture", "--vnc", vnc_ep, "--region", region, "-o", &path, "--json"];
    let cmd = build_invocation(run_dir, &[("TESTANYWARE_VNC_PASSWORD", vnc_password)], &args);
    let out = ch.exec(&cmd).await?;
    let body = expect_ok_envelope(&out)?;
    if body.get("width").and_then(Value::as_u64) != Some(250)
        || body.get("height").and_then(Value::as_u64) != Some(28)
    {
        return Err(format!("capture reported unexpected dims: {body}"));
    }
    let size = guest_file_size(ch, &path).await?;
    if size == 0 {
        return Err("captured PNG is zero bytes".into());
    }
    // PNG magic: 89 50('P') 4E('N') 47('G') …
    let head = guest_file_head(ch, &path, 8).await?;
    if head.first() != Some(&0x89) || head.get(1..4) != Some(b"PNG".as_slice()) {
        return Err(format!("captured file is not a PNG (head: {head:02x?})"));
    }
    Ok(format!("RFB decoder produced a {size}-byte 250x28 PNG on aarch64-linux"))
}

/// `screen record --duration 2 --fps 10` over the forward — **the runtime proof
/// of the ffmpeg-8 libx264 encoder on aarch64-linux** (`170` could only link).
/// Asserts a plausible MP4 (ISO-BMFF `ftyp`) with frames ≥ ~fps.
async fn check_record(
    ch: &impl ProvisionChannel,
    run_dir: &str,
    vnc_ep: &str,
    vnc_password: &str,
) -> Result<String, String> {
    let path = format!("{run_dir}/rec.mp4");
    let (fps, secs) = (10u32, 2u32);
    let args = [
        "screen", "record", "--vnc", vnc_ep, "--fps", "10", "--duration", "2", "-o", &path, "--json",
    ];
    let cmd = build_invocation(run_dir, &[("TESTANYWARE_VNC_PASSWORD", vnc_password)], &args);
    let out = ch.exec(&cmd).await?;
    let body = expect_ok_envelope(&out).map_err(|e| {
        // The most likely failure here is the encoder, not the wire: a bundle
        // missing libx264 makes ffmpeg.rs error "is this ffmpeg built with
        // libx264/libx265?". Surface that hint.
        format!("{e}\n  ↑ if this mentions a missing libav encoder, the staged \
                 ffmpeg-8 bundle lacks libx264 — confirm the BtbN gpl-shared variant")
    })?;
    let frames = body.get("frames").and_then(Value::as_u64).unwrap_or(0);
    if frames == 0 {
        return Err(format!("record wrote zero frames: {body}"));
    }
    if frames < u64::from(fps) {
        return Err(format!(
            "record wrote only {frames} frames for {secs}s @ {fps}fps — stream looks stalled"
        ));
    }
    let size = guest_file_size(ch, &path).await?;
    if size < 1000 {
        return Err(format!("recorded MP4 implausibly small: {size} bytes"));
    }
    // ISO-BMFF: bytes 4..8 == "ftyp".
    let head = guest_file_head(ch, &path, 8).await?;
    if head.get(4..8) != Some(b"ftyp".as_slice()) {
        return Err(format!("recorded file is not an MP4 (head: {head:02x?})"));
    }
    Ok(format!(
        "ffmpeg-8 libx264 encoded a {frames}-frame {size}-byte MP4 (ftyp) on aarch64-linux"
    ))
}

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
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

    // ---- Endpoint-driven band (020): golden + in-process forward ----------
    //
    // Bring up the macOS golden, forward its agent + VNC through the host, and
    // drive the endpoint-driven surface (minus OCR) from inside the HUT. Setup
    // failures (golden, gateway, forward) are infrastructure, not band cases —
    // they panic (the guards still tear down both VMs); individual command
    // failures are collected so one red case never masks the rest.
    eprintln!("\nlinux_host_harness: bringing up the {} golden + forward…", golden_platform());
    let golden_id = start_golden_vm().await;
    let _golden_guard = GoldenGuard { id: golden_id.clone() };

    wait_for_golden_ready(&golden_id, Duration::from_secs(120))
        .await
        .unwrap_or_else(|e| panic!("golden readiness wait failed: {e}"));
    let endpoints = golden_endpoints(&golden_id)
        .await
        .unwrap_or_else(|e| panic!("reading the golden endpoints failed: {e}"));

    let gateway = discover_gateway(&channel)
        .await
        .unwrap_or_else(|e| panic!("host-gateway discovery failed: {e}"));
    eprintln!("[forward] guest reaches the host-gateway at {gateway}");

    let agent_fwd = PortForward::spawn("agent", endpoints.agent)
        .await
        .unwrap_or_else(|e| panic!("agent forward setup failed: {e}"));
    let vnc_fwd = PortForward::spawn("vnc", endpoints.vnc)
        .await
        .unwrap_or_else(|e| panic!("vnc forward setup failed: {e}"));

    let agent_ep = format!("{gateway}:{}", agent_fwd.local_port);
    let vnc_ep = format!("{gateway}:{}", vnc_fwd.local_port);
    let vnc_pw = endpoints.vnc_password.clone().unwrap_or_default();

    // Order: agent HTTP, then read-only screen (size/capture/record), then the
    // mutating input family last (mirrors live-vm-gate's read-before-write).
    let mut driven: Vec<(&str, Result<String, String>)> = Vec::new();
    driven.extend(run_band(&channel, RUN_DIR, &endpoint_driven_agent_cases(&agent_ep)).await);
    driven.extend(
        run_band(&channel, RUN_DIR, &endpoint_driven_screen_size_case(&vnc_ep, &vnc_pw)).await,
    );
    driven.push(("screen-capture", check_capture(&channel, RUN_DIR, &vnc_ep, &vnc_pw).await));
    driven.push(("screen-record", check_record(&channel, RUN_DIR, &vnc_ep, &vnc_pw).await));
    driven.extend(run_band(&channel, RUN_DIR, &endpoint_driven_input_cases(&vnc_ep, &vnc_pw)).await);

    // Forwards have served their purpose; tear them down before asserting so a
    // failure does not leave them bound while the panic unwinds.
    drop(vnc_fwd);
    drop(agent_fwd);

    eprintln!("\nlinux_host_harness — endpoint-driven band (minus OCR):");
    let mut driven_failures = Vec::new();
    for (name, res) in &driven {
        match res {
            Ok(note) => eprintln!("  ✓ {name}: {note}"),
            Err(reason) => {
                eprintln!("  ✗ {name}: {reason}");
                driven_failures.push(format!("{name}: {reason}"));
            }
        }
    }

    assert!(
        driven_failures.is_empty(),
        "{} of {} endpoint-driven checks failed:\n  - {}",
        driven_failures.len(),
        driven.len(),
        driven_failures.join("\n  - "),
    );
    eprintln!(
        "\nlinux_host_harness: all {} endpoint-driven checks passed — incl. screen record \
         (ffmpeg-8 libx264 runtime-proven on aarch64-linux).",
        driven.len()
    );
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
            let cmd: Vec<&str> = case
                .args
                .iter()
                .map(String::as_str)
                .filter(|a| !a.starts_with('-'))
                .collect();
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

    #[test]
    fn build_invocation_prepends_env_then_ld_path() {
        // No env → identical to the bare LD_LIBRARY_PATH form.
        assert_eq!(
            build_invocation("/home/admin/taw", &[], &["screen", "size", "--json"]),
            "LD_LIBRARY_PATH=/home/admin/taw /home/admin/taw/testanyware screen size --json"
        );
        // The VNC password rides as an env prefix, never in the argv.
        assert_eq!(
            build_invocation(
                "/home/admin/taw",
                &[("TESTANYWARE_VNC_PASSWORD", "s3cr3t")],
                &["screen", "size", "--vnc", "10.0.0.1:55", "--json"],
            ),
            "TESTANYWARE_VNC_PASSWORD=s3cr3t LD_LIBRARY_PATH=/home/admin/taw \
             /home/admin/taw/testanyware screen size --vnc 10.0.0.1:55 --json"
        );
    }

    #[test]
    fn parses_default_gateway_from_ip_route() {
        let out = "default via 192.168.64.1 dev enp0s1 proto dhcp src 192.168.64.7 metric 100\n\
                   192.168.64.0/24 dev enp0s1 proto kernel scope link src 192.168.64.7";
        assert_eq!(parse_default_gateway(out).as_deref(), Some("192.168.64.1"));
        // No default route → None (don't misread a non-default line).
        assert_eq!(
            parse_default_gateway("10.0.0.0/24 dev eth0 proto kernel scope link src 10.0.0.5"),
            None
        );
        assert_eq!(parse_default_gateway(""), None);
    }

    #[test]
    fn parses_od_hex_into_bytes() {
        // `head -c 8 file | od -An -v -tx1` of a PNG header.
        let bytes = parse_od_hex(" 89 50 4e 47 0d 0a 1a 0a\n");
        assert_eq!(bytes, vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(&bytes[1..4], b"PNG");
        // An MP4 `ftyp` box: bytes 4..8 spell "ftyp".
        let mp4 = parse_od_hex("00 00 00 20 66 74 79 70");
        assert_eq!(&mp4[4..8], b"ftyp");
        assert!(parse_od_hex("").is_empty());
    }

    #[test]
    fn detects_rendered_menu_bar() {
        let ready = serde_json::json!({
            "windows": [
                { "windowType": "systemDialog", "elements": [] },
                { "windowType": "menuBar", "elements": [
                    { "role": "menu-item", "label": "Apple" },
                    { "role": "menu-item", "label": "File" }
                ] }
            ]
        });
        assert!(snapshot_menu_bar_ready(&ready));
        // A menu bar without a File item is not yet ready.
        let not_ready = serde_json::json!({
            "windows": [ { "windowType": "menuBar", "elements": [
                { "role": "menu-item", "label": "Apple" }
            ] } ]
        });
        assert!(!snapshot_menu_bar_ready(&not_ready));
        assert!(!snapshot_menu_bar_ready(&serde_json::json!({})));
    }
}
