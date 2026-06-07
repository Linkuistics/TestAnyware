//! windows-host-harness.rs — the self-hosted host-CLI verification harness for
//! **Windows aarch64** (grove `220-windows-arc/040-windows-harness`, ADR-0009).
//!
//! Sibling of `linux-host-harness.rs`. Same premise — a green cross-*build* is
//! not proof the cross binary *runs*: the dynamic loader, the `ffmpeg-next`
//! libav DLL link, the OCR daemon, and the RFB client can all fail only at
//! runtime, on the target OS/arch. This harness runs the **cross-compiled
//! `testanyware.exe`** *inside a real Windows 11 ARM64 guest* and asserts it
//! executes and emits correct contract envelopes — "test the host CLI with the
//! product." It proves the `030-windows-host-pass` `#[cfg(windows)]` facilities
//! actually run on the target, which a compiling `cargo-zigbuild` cannot.
//!
//! ## What this reuses from `190` — and what it swaps
//!
//! The node decided (grove 220, "standalone duplicate") **not** to extract a
//! shared module: this file is self-contained and duplicates `190`'s machinery
//! verbatim where it is platform-agnostic (the in-process host→golden TCP
//! forward, the band driver, the macOS-golden endpoint, `find_text_hit`), and
//! adapts it where the Windows guest genuinely differs from the Linux one. The
//! five real divergences, all flowing from "QEMU Win11-ARM64 guest" vs "tart
//! Ubuntu guest":
//!   1. **HUT lifecycle** — the Windows HUT is the **Windows agent-golden**,
//!      launched via the host CLI's `vm start --platform windows --json` (a
//!      CLI-managed QEMU+swtpm clone), exactly like the macOS golden is
//!      launched — *not* a manual `tart::clone` of a stock image. So the HUT
//!      uses the golden-launch pattern, not the Linux tart-`Hut` pattern.
//!   2. **Provisioning channel** — the reuse seam. Linux ssh ([`russh`]) →
//!      Windows **in-VM agent** `/upload` + `/exec` ([`AgentChannel`]). Windows
//!      ships no sshd, so the agent is the only in-guest control channel. The
//!      agent also exposes `/download`, so this harness reads build artifacts
//!      (PNG/MP4) straight back to the host rather than re-encoding them
//!      in-guest with `od` as the minimal ssh seam forced `190` to.
//!   3. **Host-gateway** — a QEMU user-mode (slirp) guest reaches the host at
//!      the **fixed gateway `10.0.2.2`** ([`QEMU_HOST_GATEWAY`]); there is no
//!      `ip route` discovery step as on tart's bridged NAT.
//!   4. **Invocation** — `"C:\…\testanyware.exe" <args>` with the ffmpeg DLLs
//!      **co-located beside the .exe** (Windows searches the image's own
//!      directory for DLLs); no `LD_LIBRARY_PATH`. Per-case env rides as
//!      `cmd.exe` `set "K=V" && …` because the agent runs `cmd.exe /c <command>`.
//!   5. **OCR engine** — Linux's `screen find-text` routes through the EasyOCR
//!      Python daemon; Windows uses **in-process native `Windows.Media.Ocr`**
//!      (ADR-0011, engine token `windows_media_ocr`), so the OCR band needs **no
//!      provisioning at all** — no Python, no venv, no model download. The engine
//!      is compiled into the cross binary already staged by [`stage_binary`], so
//!      `check_find_text` is just `screen find-text File --vnc … --json` over the
//!      forward, exactly like `screen size`. This *replaces* the earlier deferred
//!      EasyOCR gap: EasyOCR is uninstallable on win-arm64 (`opencv-python-headless`
//!      has no `win_arm64` wheel), and the `215` docker-host-unification spike
//!      rejected containerizing the host, so leaf `220/060` decided the native
//!      WinRT engine (ADR-0011) and `220/070` built it.
//!
//! ## Band coverage on aarch64-windows: 3/3 runtime-GREEN
//!
//! Endpoint-free + endpoint-driven (incl. `screen record` → ffmpeg-8 libx264 and
//! **`screen find-text` → native `windows_media_ocr`**) run GREEN in-guest — the
//! cross binary execs, the ffmpeg DLLs load + encode, the WinRT OCR engine
//! recognizes the forwarded golden's menu bar, and the RFB/agent/input clients
//! speak the wire. Windows is now at OCR parity (3/3) with Linux and macOS.
//! x86_64-windows stays build/link-verified only.
//!
//! ## How to run
//!
//! ```text
//! # 1. cross-build the aarch64-windows binary (BtbN ffmpeg-8 winarm64 sysroot —
//! #    see docs/research/170-ffmpeg-cross-link.md):
//! export PKG_CONFIG_ALLOW_CROSS=1
//! export PKG_CONFIG_LIBDIR=/tmp/taw-ffmpeg-sr/aarch64-windows/lib/pkgconfig
//! export BINDGEN_EXTRA_CLANG_ARGS=--target=aarch64-pc-windows-gnu
//! cargo zigbuild -p testanyware-cli --bin testanyware \
//!   --target aarch64-pc-windows-gnullvm --release
//!
//! # 2. run the harness. It boots the Windows agent-golden as a QEMU HUT,
//! #    agent-provisions the cross binary + ffmpeg DLLs, brings up a macOS golden
//! #    (`testanyware vm start --platform macos`), forwards the golden's agent +
//! #    VNC through the host, and runs all three bands (endpoint-free,
//! #    endpoint-driven, and OCR via the native in-process engine):
//! TESTANYWARE_WINDOWS_HARNESS=1 cargo test -p testanyware-cli \
//!   --test windows-host-harness -- --ignored windows_host_harness
//! ```
//!
//! Inputs (env, with defaults):
//!   * `TESTANYWARE_WINDOWS_BIN`        — the aarch64-windows `testanyware.exe`
//!     (default: `target/aarch64-pc-windows-gnullvm/release/testanyware.exe`).
//!   * `TESTANYWARE_WINDOWS_FFMPEG_DIR` — dir holding the ffmpeg-8 runtime
//!     `.dll`s (default: `/tmp/taw-ffmpeg-sr/aarch64-windows/bin` — BtbN puts the
//!     DLLs in `bin/`, the import libs in `lib/`).
//!
//! ## CRITICAL — libav is a *load-time* dependency (the `190` lesson, on PE)
//!
//! `testanyware-video` does `use ffmpeg_next` (a normal link, not `LoadLibrary`),
//! so the PE carries import-table entries for `avcodec-62.dll` / `avformat-62.dll`
//! / `avutil-60.dll` / `swscale-9.dll` (ffmpeg **8.1**). Windows resolves these
//! **before `main`**, so even `testanyware.exe --version` will not start unless
//! the DLLs are on the loader's search path. Co-locating them in the binary's own
//! directory (the image-directory search, always first after the known-DLLs set)
//! satisfies this with no env var. [`stage_binary`] uploads [`REQUIRED_DLLS`]
//! beside the .exe; the [`--version` canary][canary] confirms it.
//!
//! [canary]: windows_host_harness
//!
//! ## Arch coverage — x86_64 is build-verified ONLY (logged, not silently covered)
//!
//! This Apple-Silicon Mac boots only **ARM64** guests natively (QEMU+swtpm
//! Win11 ARM64). An x86_64 PE cannot run on an ARM64 guest, so **only aarch64
//! builds are verified at runtime here.** The `x86_64-pc-windows-gnu` build is
//! link-verified by the cross-build but its *runtime* is **unverified on this
//! host** — closable later only with a real x86_64 Windows box (ADR-0009
//! no-silent-caps). The harness prints this gap in its summary so a green run is
//! never mistaken for x86_64 coverage.

#![cfg(target_os = "macos")]
// The harness drives tart (the macOS golden) and the host CLI's QEMU runner,
// both macOS-host only, and runs on the same Mac that builds the release — the
// only place a plain `cargo test` ever runs (no Windows CI). So the whole file,
// including the offline unit tests, compiles and runs only on the macOS host.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::Value;
use testanyware_agent_client::{AgentClient, AgentConfig};
use testanyware_protocol::ExecRequest;
use testanyware_vm::{ExecOutput, VmPaths, VmSpec};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

// ---------------------------------------------------------------------------
// Gating + config
// ---------------------------------------------------------------------------

/// The harness only drives VMs when explicitly opted in.
fn gate_enabled() -> bool {
    std::env::var("TESTANYWARE_WINDOWS_HARNESS").as_deref() == Ok("1")
}

/// Path to the aarch64-windows `testanyware.exe` under test. Defaults to the
/// conventional `cargo zigbuild` output relative to this crate's manifest. The
/// `gnullvm` triple is the first-class Windows target (`030`); msvc cannot cross
/// from a Mac, gnu/gnullvm can.
fn windows_bin_path() -> PathBuf {
    if let Ok(p) = std::env::var("TESTANYWARE_WINDOWS_BIN") {
        return PathBuf::from(p);
    }
    // CARGO_MANIFEST_DIR = <repo>/cli-rs/crates/testanyware-cli; the target dir
    // is two levels up at <repo>/cli-rs/target.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("aarch64-pc-windows-gnullvm")
        .join("release")
        .join("testanyware.exe")
}

/// Dir holding the ffmpeg-8 runtime `.dll` bundle (BtbN `winarm64-gpl-shared`).
/// BtbN ships the runtime DLLs in `bin/` and the import libs (`.lib`/`.dll.a`)
/// in `lib/`; the loader needs the former.
fn ffmpeg_dll_dir() -> PathBuf {
    std::env::var("TESTANYWARE_WINDOWS_FFMPEG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/taw-ffmpeg-sr/aarch64-windows/bin"))
}

/// The ffmpeg-8 DLLs the cross binary imports, plus their bundled transitive
/// dep (`avcodec` → `swresample`). x264/x265 are statically linked into BtbN's
/// `avcodec-62.dll`, so they are not separate DLLs. All five must be co-located
/// for the binary to *load* (see the libav note above). Same five-lib set as the
/// Linux harness's `REQUIRED_SONAMES`, with the Windows soname spelling.
const REQUIRED_DLLS: &[&str] = &[
    "avcodec-62.dll",
    "avformat-62.dll",
    "avutil-60.dll",
    "swscale-9.dll",
    "swresample-6.dll",
];

/// Host path to the **release `.zip`** under test by [`windows_dist_zip_smoke`]
/// — `scripts/release-build.sh`'s `testanyware-v<ver>-aarch64-pc-windows-gnullvm.zip`.
/// `None` (the env var unset) skips the smoke: unlike the build-output binary
/// the main harness defaults to, there is no conventional zip path to assume, and
/// the smoke only makes sense against an actual built artifact.
fn dist_zip_path() -> Option<PathBuf> {
    std::env::var("TESTANYWARE_WINDOWS_DIST_ZIP").ok().map(PathBuf::from)
}

/// Absolute in-guest dir the binary, DLLs, venv and artifacts are provisioned
/// into. `C:\Users\Public` is world-writable with no elevation, so the agent
/// (running as the autologin user via its logon task) can always write here —
/// avoiding both a `%USERPROFILE%` discovery round-trip and the elevation a
/// `C:\` root write would need.
const RUN_DIR: &str = r"C:\Users\Public\taw";

/// Where [`windows_dist_zip_smoke`] uploads the release `.zip` and extracts it.
/// Deliberately *separate* from [`RUN_DIR`]: the distribution smoke proves the
/// **shipped artifact** unzips into a clean prefix and runs from there with the
/// DLLs the zip itself carries (image-directory search) — not the loose build
/// outputs `stage_binary` provisions. The extracted tree is
/// `{DIST_PREFIX_GUEST}\testanyware-v<ver>-<triple>\bin\{testanyware.exe,*.dll}`.
const DIST_ZIP_GUEST: &str = r"C:\Users\Public\taw-dist.zip";
const DIST_PREFIX_GUEST: &str = r"C:\Users\Public\taw-dist";

/// The address a QEMU user-mode (slirp) guest uses to reach the **host**. slirp
/// always presents the host as the network's gateway at `10.0.2.2` and proxies
/// guest→`10.0.2.2` to the host loopback, so anything the harness binds on the
/// host (the [`PortForward`]s, on `0.0.0.0`) is reachable from the guest here.
/// This is the Windows analogue of the Linux harness's `parse_default_gateway`
/// over `ip route` — but a fixed constant, because slirp's gateway is fixed
/// (qemu.rs wires plain `-netdev user,…`, no custom net range).
const QEMU_HOST_GATEWAY: &str = "10.0.2.2";

// ---------------------------------------------------------------------------
// Pure helpers (offline-unit-tested below)
// ---------------------------------------------------------------------------

/// The three-band surface split (ADR-0009) — identical to the Linux harness;
/// the band a command belongs to is a property of the command surface, not the
/// OS. Kept here so the offline test pins it on the Windows side too.
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

/// Build the in-guest `cmd.exe` invocation: `set "K=V" && … call "<dir>\testanyware.exe"
/// <args>`. The agent runs `cmd.exe /c <command>` (agents/windows/SystemEndpoints.cs),
/// so per-case env is set with `cmd`'s `set "K=V" && …` form — the quotes bound
/// the assignment so a value with spaces is preserved and not read as a command
/// separator. No `LD_LIBRARY_PATH`/PATH: the ffmpeg DLLs sit beside the .exe and
/// the loader searches the image directory first. The endpoint-driven band
/// passes `TESTANYWARE_VNC_PASSWORD` this way so the password never lands in argv
/// (and matches how `resolve.rs` sources it).
///
/// The exe is invoked through `call` so the command never *starts* with a `"`:
/// `cmd /c` strips the outer quote pair from any line that begins with `"` and
/// holds more than two quotes (the `cmd /?` quirk), which would corrupt an
/// invocation whose args also carry quoted values. Leading with `call` (or
/// `set`) sidesteps the strip and keeps quoting space-safe.
fn build_invocation(run_dir: &str, envs: &[(&str, &str)], args: &[&str]) -> String {
    let mut prefix = String::new();
    for (k, v) in envs {
        prefix.push_str(&format!("set \"{k}={v}\" && "));
    }
    format!(
        "{prefix}call \"{dir}\\testanyware.exe\" {args}",
        dir = run_dir,
        args = args.join(" "),
    )
}

/// The no-env convenience used by the `--version` canary and the offline tests.
fn taw_cmd(run_dir: &str, args: &[&str]) -> String {
    build_invocation(run_dir, &[], args)
}

/// The `cmd.exe` line that wipes any prior `dest` then expands `zip` into it —
/// the [`windows_dist_zip_smoke`] "clean prefix" step. Driven through
/// PowerShell's `Expand-Archive` because the agent runs `cmd.exe /c`, which has
/// no native unzip. Leads with `powershell` (never a `"`), so the `cmd /c`
/// quote-strip quirk does not apply (see [`build_invocation`]); paths are
/// single-quoted inside the `-Command` string and the two statements are joined
/// with `;`. `-Force` overwrites, and the leading `Remove-Item` guarantees the
/// "clean" in clean-prefix — stale files from an earlier run cannot survive.
fn dist_extract_cmd(zip_guest: &str, dest_guest: &str) -> String {
    format!(
        "powershell -NoProfile -ExecutionPolicy Bypass -Command \
         \"if (Test-Path '{dest}') {{ Remove-Item -Recurse -Force '{dest}' }}; \
         Expand-Archive -Path '{zip}' -DestinationPath '{dest}' -Force\"",
        dest = dest_guest,
        zip = zip_guest,
    )
}

/// The in-guest `bin\` dir of an extracted release zip: `dest\<stem>\bin`, where
/// `stem` is the zip's filename without its `.zip` (the bundle's single
/// top-level dir, `testanyware-v<ver>-<triple>`). Derived from the filename so no
/// in-guest `dir` round-trip is needed to find the extracted binary.
fn dist_bin_dir(dest_guest: &str, stem: &str) -> String {
    format!(r"{dest_guest}\{stem}\bin")
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

/// Whether `(stderr, exit_code)` reads as the load-time-libav failure — a DLL
/// the loader could not resolve before `main`. The Windows analogue of Linux's
/// `looks_like_missing_shared_object`: the loader returns the NTSTATUS
/// `STATUS_DLL_NOT_FOUND`/`STATUS_ENTRYPOINT_NOT_FOUND` (surfaced as these
/// negative `i32`s), and any console text mentions a missing `.dll`.
fn looks_like_missing_dll(stderr: &str, exit_code: i32) -> bool {
    const STATUS_DLL_NOT_FOUND: i32 = -1073741515; // 0xC0000135
    const STATUS_ENTRYPOINT_NOT_FOUND: i32 = -1073741511; // 0xC0000139
    if exit_code == STATUS_DLL_NOT_FOUND || exit_code == STATUS_ENTRYPOINT_NOT_FOUND {
        return true;
    }
    let s = stderr.to_lowercase();
    s.contains(".dll")
        && (s.contains("not found") || s.contains("was not found") || s.contains("cannot proceed"))
}

// ---------------------------------------------------------------------------
// Provisioning channel seam (the node's reuse seam — the 2nd impl lives here)
// ---------------------------------------------------------------------------

/// The in-guest control channel. The Linux harness uses ssh; this is the
/// **second impl** the node exists to write: the in-VM agent's HTTP surface.
/// Richer than the minimal ssh seam — the agent natively offers `download`, so
/// artifact checks read files straight back to the host instead of re-encoding
/// them in-guest.
#[allow(async_fn_in_trait)] // private test trait, single static-dispatch impl
trait ProvisionChannel {
    /// Run `cmd` (a `cmd.exe` command line) to completion via the agent's
    /// `/exec`, returning its captured streams + exit code.
    async fn exec(&self, cmd: &str) -> Result<ExecOutput, String>;
    /// Upload `local` to absolute in-guest `remote` (the agent's `/upload`
    /// creates the parent dir, so callers need no separate `mkdir`).
    async fn upload(&self, local: &Path, remote: &str) -> Result<(), String>;
    /// Download in-guest `remote` to host `local` (the agent's `/download`).
    async fn download(&self, remote: &str, local: &Path) -> Result<(), String>;
}

/// In-guest process timeout ceiling (seconds) for one `/exec`. Generous so the
/// slow provisioning steps (Python install, `pip install torch`, EasyOCR model
/// warm) complete; band cases finish in well under this. The agent kills the
/// process tree and returns exit `-1` past this.
const EXEC_TIMEOUT_SECS: i64 = 1800;

/// Agent-backed channel over the in-VM agent's HTTP client.
struct AgentChannel {
    client: AgentClient,
}

impl ProvisionChannel for AgentChannel {
    async fn exec(&self, cmd: &str) -> Result<ExecOutput, String> {
        let req = ExecRequest {
            command: cmd.to_string(),
            timeout: EXEC_TIMEOUT_SECS,
            detach: false,
        };
        let r = self
            .client
            .exec(&req)
            .await
            .map_err(|e| format!("agent exec `{cmd}`: {e}"))?;
        // The agent's ExecResult maps 1:1 onto the harness's ExecOutput shape,
        // so every shared check (expect_ok_envelope, …) works unchanged.
        Ok(ExecOutput { stdout: r.stdout, stderr: r.stderr, exit_code: r.exit_code })
    }

    async fn upload(&self, local: &Path, remote: &str) -> Result<(), String> {
        self.client
            .upload(remote, local)
            .await
            .map(|_| ())
            .map_err(|e| format!("agent upload {} -> {remote}: {e}", local.display()))
    }

    async fn download(&self, remote: &str, local: &Path) -> Result<(), String> {
        self.client
            .download(remote, local)
            .await
            .map(|_| ())
            .map_err(|e| format!("agent download {remote} -> {}: {e}", local.display()))
    }
}

/// Run a channel command and require exit 0 — for provisioning steps whose
/// failure should abort, unlike a band case's soft assertion.
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
// VM lifecycle — both the Windows HUT and the macOS golden are CLI-managed
// ---------------------------------------------------------------------------

/// The built host `testanyware` (macOS arm64), driven as a subprocess to manage
/// both VMs — matching `live-vm-gate.rs`/`linux-host-harness.rs` (exercises the
/// real command surface).
const HOST_BIN: &str = env!("CARGO_BIN_EXE_testanyware");

/// Run the host CLI as a subprocess off the async runtime (it blocks).
async fn host_run(args: &[&str]) -> Output {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let shown = owned.join(" ");
    tokio::task::spawn_blocking(move || Command::new(HOST_BIN).args(&owned).output())
        .await
        .expect("spawn_blocking join")
        .unwrap_or_else(|e| panic!("failed to invoke host `{HOST_BIN} {shown}`: {e}"))
}

/// Start a VM via `vm start --platform <p> --json` and return its id. The caller
/// wraps the id in a [`VmGuard`] *before* any further fallible step. Used for
/// both the Windows HUT (`windows`) and the macOS golden (`macos`).
async fn start_vm(platform: &str) -> String {
    let out = host_run(&["vm", "start", "--platform", platform, "--json"]).await;
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
    eprintln!("[vm:{platform}] started {id}");
    id
}

/// Stops a VM via `vm stop <id>` on drop, so a panicking check never leaks a
/// running VM (the Windows-side twin of `linux-host-harness.rs::GoldenGuard`,
/// reused for both the HUT and the golden).
struct VmGuard {
    id: String,
    label: &'static str,
}

impl Drop for VmGuard {
    fn drop(&mut self) {
        match Command::new(HOST_BIN).args(["vm", "stop", &self.id, "--json"]).output() {
            Ok(o) if o.status.success() => eprintln!("[{}] stopped {}", self.label, self.id),
            Ok(o) => eprintln!(
                "[{}] WARNING: vm stop {} exited {:?}: {}",
                self.label,
                self.id,
                o.status.code(),
                String::from_utf8_lossy(&o.stderr),
            ),
            Err(e) => eprintln!("[{}] WARNING: vm stop {} failed to spawn: {e}", self.label, self.id),
        }
    }
}

/// Read a VM's host-reachable agent endpoint from its per-VM spec. For the
/// Windows HUT this is `127.0.0.1:<hostfwd-port>` — the dynamic host port QEMU
/// forwards to the in-guest agent's `:8648` (qemu.rs `hostfwd=tcp::0-:8648`).
fn agent_endpoint(id: &str) -> Result<(String, u16), String> {
    let spec_path = VmPaths::from_process_env().spec_path(id);
    let spec = VmSpec::load(&spec_path)
        .map_err(|e| format!("load spec {}: {e}", spec_path.display()))?;
    let a = spec.agent.ok_or_else(|| {
        format!("spec {} has no agent endpoint (agent unreachable at start?)", spec_path.display())
    })?;
    Ok((a.host, a.port))
}

/// Poll the HUT agent's `/health` until it reports accessible. `vm start`
/// returns once QEMU is up and the port is forwarded, but the in-guest agent
/// (an autologin logon task) needs a few more seconds after boot. A
/// connection-refused fails fast, so this loop retries cheaply.
async fn wait_for_hut_agent(ch: &AgentChannel, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    // Assigned on every non-returning iteration before the deadline check reads it.
    let mut last: String;
    loop {
        match ch.client.health().await {
            Ok(h) if h.accessible => return Ok(()),
            Ok(h) => last = format!("health: accessible=false, platform={}", h.platform),
            Err(e) => last = format!("health error: {e}"),
        }
        if Instant::now() >= deadline {
            return Err(format!("HUT agent never became healthy: {last}"));
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

// ---------------------------------------------------------------------------
// Provisioning: stage the cross binary + the ffmpeg DLLs beside it
// ---------------------------------------------------------------------------

/// Upload the cross binary + the ffmpeg DLL bundle into [`RUN_DIR`]. No `chmod`
/// (Windows has no exec bit) and no `mkdir` (the agent's `/upload` creates the
/// parent dir). The DLLs land *beside* the .exe so the loader's image-directory
/// search resolves them with no env var.
async fn stage_binary(ch: &impl ProvisionChannel) -> Result<(), String> {
    let bin = windows_bin_path();
    if !bin.is_file() {
        return Err(format!(
            "cross binary not found at {} — cross-build it first (see the module docs)",
            bin.display(),
        ));
    }
    let dll_dir = ffmpeg_dll_dir();

    ch.upload(&bin, &format!(r"{RUN_DIR}\testanyware.exe")).await?;

    for dll in REQUIRED_DLLS {
        let local = dll_dir.join(dll);
        if !local.exists() {
            return Err(format!(
                "ffmpeg runtime DLL {} missing — stage the BtbN winarm64-gpl-shared \
                 bundle's bin/ (see docs/research/170-ffmpeg-cross-link.md)",
                local.display(),
            ));
        }
        ch.upload(&local, &format!(r"{RUN_DIR}\{dll}")).await?;
    }
    eprintln!("[provision] staged binary + {} ffmpeg DLLs into {RUN_DIR}", REQUIRED_DLLS.len());
    Ok(())
}

/// Provision from the **shipped release zip** instead of loose build outputs:
/// upload `zip`, wipe any prior extraction, `Expand-Archive` into a clean prefix,
/// and return the extracted `bin\` dir (which holds `testanyware.exe` *and* the
/// ffmpeg DLLs the zip itself carries, co-located). This is the distribution
/// smoke's whole point — proving the artifact a user downloads unzips and runs,
/// with the DLL co-location the zip layout (not the harness) is responsible for.
async fn stage_from_zip(ch: &impl ProvisionChannel, zip: &Path) -> Result<String, String> {
    if !zip.is_file() {
        return Err(format!(
            "release zip not found at {} — build it first (scripts/release-build.sh)",
            zip.display(),
        ));
    }
    // The bundle's single top-level dir is the zip's filename without `.zip`.
    let stem = zip
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("zip path {} has no usable file stem", zip.display()))?;

    ch.upload(zip, DIST_ZIP_GUEST).await?;
    exec_ok(ch, &dist_extract_cmd(DIST_ZIP_GUEST, DIST_PREFIX_GUEST)).await?;
    let bin_dir = dist_bin_dir(DIST_PREFIX_GUEST, stem);
    eprintln!("[provision] extracted {} into {bin_dir}", zip.display());
    Ok(bin_dir)
}

// ---------------------------------------------------------------------------
// Band driver — band-agnostic, identical in shape to the Linux harness
// ---------------------------------------------------------------------------

/// One smoke case: a label, the args after `testanyware`, optional per-case env
/// (e.g. `TESTANYWARE_VNC_PASSWORD`), and a check over the command's output. The
/// check returns `Ok(note)` or `Err(reason)` so one red case never masks the
/// rest. Args/envs are owned `String`s because the endpoint-driven band
/// interpolates the runtime-discovered forward endpoints.
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

/// Run each case over the channel and collect outcomes.
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

/// The endpoint-free band. Each case needs no target — it proves the binary
/// runs, the import table resolves, and the contract envelopes emit on
/// aarch64-windows.
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
        // expected on a bare Windows host with no tart/qemu). Assert it *runs
        // and emits a valid envelope* (the `where`/no-Homebrew path from `030`),
        // not that every check passes.
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
        // A mutating command's dry-run short-circuits before any network I/O, so
        // it is endpoint-free: exit 0 with `dry_run: true`.
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

// ---------------------------------------------------------------------------
// The macOS golden endpoint + the in-process host→golden TCP forward
// (duplicated from the Linux harness — platform-agnostic machinery)
// ---------------------------------------------------------------------------

/// Platform of the golden whose endpoint the forward targets. macOS is the only
/// kept-built golden with deterministic on-screen content; override only if a
/// different golden is staged.
fn golden_platform() -> String {
    std::env::var("TESTANYWARE_WINDOWS_HARNESS_GOLDEN").unwrap_or_else(|_| "macos".into())
}

/// The golden's host-reachable endpoints, read from its per-VM spec.
struct GoldenEndpoints {
    agent: SocketAddr,
    vnc: SocketAddr,
    vnc_password: Option<String>,
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
/// desktop-rendered signal we wait on before driving the golden.
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

/// An in-process tokio TCP proxy: binds `0.0.0.0:0` on the host and splices each
/// inbound connection to `target` (a golden endpoint). Bound on `0.0.0.0` so a
/// guest reaching it via [`QEMU_HOST_GATEWAY`] (slirp → host loopback) connects.
/// Dropping it aborts the accept loop, so no listener leaks across tests. This is
/// the same pure-Rust forward the Linux harness uses, reused unchanged.
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

// ---------------------------------------------------------------------------
// The endpoint-driven band (minus OCR)
// ---------------------------------------------------------------------------

/// Agent HTTP actions over `--agent <gw>:<afwd>` — proves the cross binary's
/// HTTP client runs on aarch64-windows and reaches the forwarded golden agent.
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
/// aarch64-windows and the events are accepted through the forward. Mutating, so
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

// --- bespoke file-producing checks (download the artifact back to the host) --

/// Download an in-guest file to a fresh host temp path and return its bytes.
/// The agent's `/download` makes this far simpler than the Linux harness's
/// `stat`/`od`-over-ssh dance.
async fn fetch_guest_file(ch: &impl ProvisionChannel, remote: &str) -> Result<Vec<u8>, String> {
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let local = dir.path().join("artifact.bin");
    ch.download(remote, &local).await?;
    std::fs::read(&local).map_err(|e| format!("read downloaded {}: {e}", local.display()))
}

/// `screen capture --region …` over the forward, asserting the cross-built RFB
/// decoder produced a correctly-sized PNG in-guest.
async fn check_capture(
    ch: &impl ProvisionChannel,
    run_dir: &str,
    vnc_ep: &str,
    vnc_password: &str,
) -> Result<String, String> {
    let remote = format!(r"{run_dir}\cap.png");
    let region = "0,0,250,28"; // static left menu-bar slice (no clock/notifications)
    let args = ["screen", "capture", "--vnc", vnc_ep, "--region", region, "-o", &remote, "--json"];
    let cmd = build_invocation(run_dir, &[("TESTANYWARE_VNC_PASSWORD", vnc_password)], &args);
    let out = ch.exec(&cmd).await?;
    let body = expect_ok_envelope(&out)?;
    if body.get("width").and_then(Value::as_u64) != Some(250)
        || body.get("height").and_then(Value::as_u64) != Some(28)
    {
        return Err(format!("capture reported unexpected dims: {body}"));
    }
    let bytes = fetch_guest_file(ch, &remote).await?;
    if bytes.is_empty() {
        return Err("captured PNG is zero bytes".into());
    }
    // PNG magic: 89 50('P') 4E('N') 47('G') …
    if bytes.first() != Some(&0x89) || bytes.get(1..4) != Some(b"PNG".as_slice()) {
        return Err(format!("captured file is not a PNG (head: {:02x?})", &bytes[..bytes.len().min(8)]));
    }
    Ok(format!("RFB decoder produced a {}-byte 250x28 PNG on aarch64-windows", bytes.len()))
}

/// `screen record --duration 2 --fps 10` over the forward — **the runtime proof
/// of the ffmpeg-8 libx264 encoder on aarch64-windows** (the link was proven in
/// `030`/`170`; this proves the DLLs load + encode). Asserts a plausible MP4
/// (ISO-BMFF `ftyp`) with frames ≥ ~fps.
async fn check_record(
    ch: &impl ProvisionChannel,
    run_dir: &str,
    vnc_ep: &str,
    vnc_password: &str,
) -> Result<String, String> {
    let remote = format!(r"{run_dir}\rec.mp4");
    let (fps, secs) = (10u32, 2u32);
    let args = [
        "screen", "record", "--vnc", vnc_ep, "--fps", "10", "--duration", "2", "-o", &remote, "--json",
    ];
    let cmd = build_invocation(run_dir, &[("TESTANYWARE_VNC_PASSWORD", vnc_password)], &args);
    let out = ch.exec(&cmd).await?;
    let body = expect_ok_envelope(&out).map_err(|e| {
        // The most likely failure here is the encoder, not the wire: a bundle
        // missing libx264 makes ffmpeg.rs error "is this ffmpeg built with
        // libx264/libx265?". Surface that hint.
        format!("{e}\n  ↑ if this mentions a missing libav encoder, the staged \
                 ffmpeg-8 DLLs lack libx264 — confirm the BtbN gpl-shared variant")
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
    let bytes = fetch_guest_file(ch, &remote).await?;
    if bytes.len() < 1000 {
        return Err(format!("recorded MP4 implausibly small: {} bytes", bytes.len()));
    }
    // ISO-BMFF: bytes 4..8 == "ftyp".
    if bytes.get(4..8) != Some(b"ftyp".as_slice()) {
        return Err(format!("recorded file is not an MP4 (head: {:02x?})", &bytes[..bytes.len().min(8)]));
    }
    Ok(format!(
        "ffmpeg-8 libx264 encoded a {frames}-frame {}-byte MP4 (ftyp) on aarch64-windows",
        bytes.len()
    ))
}

// ===========================================================================
// The OCR band: native in-process Windows.Media.Ocr, then find-text
// ===========================================================================
//
// Windows routes `screen find-text` through the in-process native
// `Windows.Media.Ocr` engine (ADR-0011, engine token `windows_media_ocr`),
// compiled into the cross binary already staged by `stage_binary`. Unlike the
// Linux harness's EasyOCR daemon — and unlike this harness's earlier deferred
// EasyOCR attempt (Python 3.12 + torch + opencv, blocked by the missing
// `win_arm64` opencv wheel) — there is **nothing to provision**: no Python, no
// venv, no model download. The OCR band is therefore just one more
// endpoint-driven command over the forward, exactly like `screen size`.

/// Assert a `screen find-text File` envelope used the native Windows.Media.Ocr
/// engine and found the query with a plausible box. Pure over the parsed body
/// so it unit-tests offline (the Windows analogue of the Linux harness's
/// `find_text_hit`, which asserts `easyocr_daemon`).
fn find_text_hit(body: &Value) -> Result<String, String> {
    let engine = body.get("engine").and_then(Value::as_str).unwrap_or("");
    if engine != "windows_media_ocr" {
        return Err(format!("expected engine windows_media_ocr, got {engine:?}"));
    }
    let detections = body
        .get("detections")
        .and_then(Value::as_array)
        .ok_or("find-text envelope has no detections array")?;
    let hit = detections
        .iter()
        .find(|d| {
            d.get("text")
                .and_then(Value::as_str)
                .is_some_and(|t| t.to_lowercase().contains("file"))
        })
        .ok_or_else(|| format!("Windows.Media.Ocr did not find 'File'; detections: {detections:?}"))?;
    let w = hit.get("width").and_then(Value::as_f64).unwrap_or(0.0);
    let h = hit.get("height").and_then(Value::as_f64).unwrap_or(0.0);
    if w <= 0.0 || h <= 0.0 {
        return Err(format!("Windows.Media.Ocr 'File' hit has an implausible box: {hit}"));
    }
    Ok(format!(
        "engine=windows_media_ocr found 'File' at {w:.0}x{h:.0} px on aarch64-windows"
    ))
}

/// `screen find-text File` over the forward, routed through the in-process
/// native Windows.Media.Ocr engine. No OCR-specific env — the engine is in the
/// binary; only the VNC password is needed (like `screen size`).
async fn check_find_text(
    ch: &impl ProvisionChannel,
    run_dir: &str,
    vnc_ep: &str,
    vnc_password: &str,
) -> Result<String, String> {
    let envs = [("TESTANYWARE_VNC_PASSWORD", vnc_password)];
    let args = ["screen", "find-text", "File", "--vnc", vnc_ep, "--timeout", "20", "--json"];
    let cmd = build_invocation(run_dir, &envs, &args);
    let out = ch.exec(&cmd).await?;
    let body = expect_ok_envelope(&out).map_err(|e| {
        format!(
            "{e}\n  ↑ an OCR_* error means the native Windows.Media.Ocr engine failed \
             (no OCR language pack? decode failure?) — see the OcrBridgeError message."
        )
    })?;
    find_text_hit(&body)
}

// ---------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "live VM: TESTANYWARE_WINDOWS_HARNESS=1 cargo test --test windows-host-harness -- --ignored windows_host_harness"]
async fn windows_host_harness() {
    if !gate_enabled() {
        eprintln!(
            "windows_host_harness: skipped — set TESTANYWARE_WINDOWS_HARNESS=1 to boot the \
             Windows agent-golden as a QEMU HUT and verify the aarch64-windows cross binary."
        );
        return;
    }

    // ---- Bring up the Windows HUT (the agent-golden, via the host CLI) -----
    let hut_id = start_vm("windows").await;
    let _hut_guard = VmGuard { id: hut_id.clone(), label: "hut" };
    let (agent_host, agent_port) =
        agent_endpoint(&hut_id).unwrap_or_else(|e| panic!("reading the HUT agent endpoint failed: {e}"));
    eprintln!("[hut] agent at {agent_host}:{agent_port}");
    let channel = AgentChannel {
        client: AgentClient::new(
            AgentConfig::new(agent_host, agent_port).with_timeout(Duration::from_secs(1860)),
        )
        .unwrap_or_else(|e| panic!("building the HUT agent client failed: {e}")),
    };
    wait_for_hut_agent(&channel, Duration::from_secs(180))
        .await
        .unwrap_or_else(|e| panic!("HUT agent readiness wait failed: {e}"));
    eprintln!("[hut] agent healthy; provisioning");

    stage_binary(&channel)
        .await
        .unwrap_or_else(|e| panic!("staging the cross binary failed: {e}"));

    // Canary: the FIRST in-guest command proves the binary execs — i.e. the
    // load-time libav imports resolve. A failure here is almost always a
    // mis-staged DLL bundle, so say so plainly (see the libav note).
    let version = channel
        .exec(&taw_cmd(RUN_DIR, &["--version"]))
        .await
        .unwrap_or_else(|e| panic!("--version canary: channel exec failed: {e}"));
    if version.exit_code != 0 {
        let hint = if looks_like_missing_dll(&version.stderr, version.exit_code) {
            "\n  ↑ this is the load-time-libav failure: an ffmpeg-8 DLL is mis-staged. Confirm \
             REQUIRED_DLLS are all present in TESTANYWARE_WINDOWS_FFMPEG_DIR and co-located with the .exe."
        } else {
            ""
        };
        panic!(
            "--version canary failed: the cross binary does not exec on aarch64-windows \
             (exit {}).\n  stderr: {}{hint}",
            version.exit_code,
            version.stderr.trim(),
        );
    }
    eprintln!("[canary] --version execs on aarch64-windows: {}", version.stdout.trim());

    // ---- Endpoint-free band ------------------------------------------------
    let results = run_band(&channel, RUN_DIR, &endpoint_free_cases()).await;

    eprintln!("\nwindows_host_harness — endpoint-free band:");
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
        "\n[arch] aarch64-windows: runtime-verified in-guest above. \
         x86_64-windows: BUILD/link-verified only (no native x86_64 Windows guest on this Mac) — \
         runtime gap is open and accepted (ADR-0009)."
    );

    assert!(
        failures.is_empty(),
        "{} of {} endpoint-free checks failed:\n  - {}",
        failures.len(),
        results.len(),
        failures.join("\n  - "),
    );
    eprintln!("\nwindows_host_harness: all {} endpoint-free checks passed.", results.len());

    // ---- Endpoint-driven band: golden + in-process forward ----------------
    //
    // Bring up the macOS golden, forward its agent + VNC through the host, and
    // drive the endpoint-driven surface from inside the HUT. The guest reaches
    // the host-side forwards via the slirp gateway (a fixed constant, vs tart's
    // discovered bridged gateway). Setup failures (golden, forward) panic (the
    // guards still tear down both VMs); individual command failures are
    // collected so one red case never masks the rest.
    eprintln!("\nwindows_host_harness: bringing up the {} golden + forward…", golden_platform());
    let golden_id = start_vm(&golden_platform()).await;
    let _golden_guard = VmGuard { id: golden_id.clone(), label: "golden" };

    // 300s (vs the Linux harness's 120s): a heavy Windows QEMU+swtpm HUT runs
    // concurrently here and steals CPU from the macOS golden's boot/render, so it
    // legitimately needs a longer window to reach the Finder menu bar.
    wait_for_golden_ready(&golden_id, Duration::from_secs(300))
        .await
        .unwrap_or_else(|e| panic!("golden readiness wait failed: {e}"));
    let endpoints = golden_endpoints(&golden_id)
        .await
        .unwrap_or_else(|e| panic!("reading the golden endpoints failed: {e}"));

    let gateway = QEMU_HOST_GATEWAY;
    eprintln!("[forward] guest reaches the host at the slirp gateway {gateway}");

    let agent_fwd = PortForward::spawn("agent", endpoints.agent)
        .await
        .unwrap_or_else(|e| panic!("agent forward setup failed: {e}"));
    let vnc_fwd = PortForward::spawn("vnc", endpoints.vnc)
        .await
        .unwrap_or_else(|e| panic!("vnc forward setup failed: {e}"));

    let agent_ep = format!("{gateway}:{}", agent_fwd.local_port);
    let vnc_ep = format!("{gateway}:{}", vnc_fwd.local_port);
    let vnc_pw = endpoints.vnc_password.clone().unwrap_or_default();

    // Order: agent HTTP, then read-only screen (size/capture/record/find-text),
    // then the mutating input family last (read-before-write).
    let mut driven: Vec<(&str, Result<String, String>)> = Vec::new();
    driven.extend(run_band(&channel, RUN_DIR, &endpoint_driven_agent_cases(&agent_ep)).await);
    driven.extend(
        run_band(&channel, RUN_DIR, &endpoint_driven_screen_size_case(&vnc_ep, &vnc_pw)).await,
    );
    driven.push(("screen-capture", check_capture(&channel, RUN_DIR, &vnc_ep, &vnc_pw).await));
    driven.push(("screen-record", check_record(&channel, RUN_DIR, &vnc_ep, &vnc_pw).await));
    // OCR band — the native in-process Windows.Media.Ocr engine (ADR-0011),
    // compiled into the staged binary, so it runs unconditionally now (no
    // provisioning, no opt-in knob): `screen find-text File` over the forward.
    driven.push(("screen-find-text", check_find_text(&channel, RUN_DIR, &vnc_ep, &vnc_pw).await));
    driven.extend(run_band(&channel, RUN_DIR, &endpoint_driven_input_cases(&vnc_ep, &vnc_pw)).await);

    // Forwards have served their purpose; tear them down before asserting so a
    // failure does not leave them bound while the panic unwinds.
    drop(vnc_fwd);
    drop(agent_fwd);

    eprintln!("\nwindows_host_harness — endpoint-driven band:");
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
        "\nwindows_host_harness: all {} endpoint-driven checks passed — incl. screen record \
         (ffmpeg-8 libx264 runtime-proven) AND screen find-text (native windows_media_ocr) on \
         aarch64-windows. All THREE bands GREEN — Windows at OCR parity (3/3) with Linux and macOS.",
        driven.len(),
    );
}

// ---------------------------------------------------------------------------
// The distribution smoke — the pre-publish gate (grove 220/050)
// ---------------------------------------------------------------------------

/// Real-artifact smoke for the Windows release **zip** (grove `220/050`
/// pre-publish gate). The `040` harness proves the *build-output* binary runs;
/// this proves the **shipped artifact** does: it uploads the actual
/// `release-build.sh` zip into a Windows guest, extracts it into a **clean
/// prefix**, and runs the endpoint-free band from the extracted `bin\` — so the
/// thing under test is the zip's own layout (the 5 ffmpeg DLLs co-located beside
/// `testanyware.exe`, resolved by the PE image-directory search), not loose files
/// the harness staged. That co-location is the one thing the zip packaging — not
/// the cross-build — is responsible for, so it is exactly what a distribution
/// smoke must exercise.
///
/// Scope is deliberately endpoint-free only (no macOS golden): functional
/// correctness over a live endpoint is already `040`'s green result, and is a
/// property of the binary, not of how it was packaged. Re-running it here would
/// cost a second VM to re-prove something the zip layout cannot affect. What the
/// zip *can* break — a dropped/misplaced DLL so the loader fails before `main` —
/// is caught by the `--version` canary and every endpoint-free case.
///
/// aarch64-windows only (the sole Windows guest this Mac boots). x86_64-windows
/// stays build/link-verified — its zip is produced but not smoke-run here
/// (ADR-0009 no-silent-caps).
///
/// ```text
/// # build the artifacts (scripts/release-build.sh) then point at the zip:
/// export TESTANYWARE_WINDOWS_DIST_ZIP=target/dist/testanyware-v<ver>-aarch64-pc-windows-gnullvm.zip
/// cargo test -p testanyware-cli --test windows-host-harness \
///   -- --ignored windows_dist_zip_smoke
/// ```
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "live VM: TESTANYWARE_WINDOWS_DIST_ZIP=<zip> cargo test --test windows-host-harness -- --ignored windows_dist_zip_smoke"]
async fn windows_dist_zip_smoke() {
    let Some(zip) = dist_zip_path() else {
        eprintln!(
            "windows_dist_zip_smoke: skipped — set TESTANYWARE_WINDOWS_DIST_ZIP=<path to the \
             aarch64-windows release .zip> to boot the Windows guest and smoke the shipped artifact."
        );
        return;
    };
    eprintln!("[smoke] artifact under test: {}", zip.display());

    // Bring up the Windows HUT (the agent-golden, via the host CLI) — same boot
    // path as the main harness; no macOS golden (endpoint-free only).
    let hut_id = start_vm("windows").await;
    let _hut_guard = VmGuard { id: hut_id.clone(), label: "hut" };
    let (agent_host, agent_port) =
        agent_endpoint(&hut_id).unwrap_or_else(|e| panic!("reading the HUT agent endpoint failed: {e}"));
    eprintln!("[hut] agent at {agent_host}:{agent_port}");
    let channel = AgentChannel {
        client: AgentClient::new(
            AgentConfig::new(agent_host, agent_port).with_timeout(Duration::from_secs(600)),
        )
        .unwrap_or_else(|e| panic!("building the HUT agent client failed: {e}")),
    };
    wait_for_hut_agent(&channel, Duration::from_secs(180))
        .await
        .unwrap_or_else(|e| panic!("HUT agent readiness wait failed: {e}"));
    eprintln!("[hut] agent healthy; extracting the release zip into a clean prefix");

    let run_dir = stage_from_zip(&channel, &zip)
        .await
        .unwrap_or_else(|e| panic!("extracting the release zip failed: {e}"));

    // Canary: the first command from the *extracted* tree proves the zip's DLLs
    // resolve via image-directory search — i.e. the shipped layout loads.
    let version = channel
        .exec(&taw_cmd(&run_dir, &["--version"]))
        .await
        .unwrap_or_else(|e| panic!("--version canary: channel exec failed: {e}"));
    if version.exit_code != 0 {
        let hint = if looks_like_missing_dll(&version.stderr, version.exit_code) {
            "\n  ↑ load-time-libav failure: the zip is missing/misplacing an ffmpeg-8 DLL. The \
             zip must carry all five REQUIRED_DLLS co-located beside testanyware.exe in bin/."
        } else {
            ""
        };
        panic!(
            "--version canary failed: the shipped zip does not run on aarch64-windows \
             (exit {}).\n  stderr: {}{hint}",
            version.exit_code,
            version.stderr.trim(),
        );
    }
    eprintln!("[canary] the extracted zip's testanyware.exe execs: {}", version.stdout.trim());

    // Endpoint-free band, run from the extracted bin/ — the shipped artifact.
    let results = run_band(&channel, &run_dir, &endpoint_free_cases()).await;

    eprintln!("\nwindows_dist_zip_smoke — endpoint-free band (from the shipped zip):");
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

    eprintln!(
        "\n[arch] aarch64-windows zip: runtime-smoked in-guest above. x86_64-windows zip: \
         BUILD/link-verified only (no native x86_64 Windows guest on this Mac) — runtime gap \
         open and accepted (ADR-0009)."
    );

    assert!(
        failures.is_empty(),
        "{} of {} endpoint-free checks failed against the shipped zip:\n  - {}",
        failures.len(),
        results.len(),
        failures.join("\n  - "),
    );
    eprintln!(
        "\nwindows_dist_zip_smoke: PASS — the aarch64-windows release zip unzips into a clean \
         prefix and runs ({} endpoint-free checks green from the extracted tree).",
        results.len(),
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
    fn taw_cmd_calls_the_quoted_exe_with_no_env() {
        // Leads with `call`, never a bare `"`, to dodge the cmd /c quote-strip.
        let cmd = taw_cmd(r"C:\Users\Public\taw", &["capabilities", "--json"]);
        assert_eq!(cmd, r#"call "C:\Users\Public\taw\testanyware.exe" capabilities --json"#);
        assert!(!cmd.starts_with('"'), "must not start with a quote: {cmd}");
    }

    #[test]
    fn build_invocation_prepends_cmd_set_then_called_exe() {
        // No env → `call "exe" …`, leading with `call` not `"`.
        assert_eq!(
            build_invocation(r"C:\taw", &[], &["screen", "size", "--json"]),
            r#"call "C:\taw\testanyware.exe" screen size --json"#
        );
        // The VNC password rides as a `set "K=V" &&` prefix, never in the argv.
        assert_eq!(
            build_invocation(
                r"C:\taw",
                &[("TESTANYWARE_VNC_PASSWORD", "s3cr3t")],
                &["screen", "size", "--vnc", "10.0.2.2:55", "--json"],
            ),
            r#"set "TESTANYWARE_VNC_PASSWORD=s3cr3t" && call "C:\taw\testanyware.exe" screen size --vnc 10.0.2.2:55 --json"#
        );
        // Multiple envs chain in order (the OCR find-text case).
        assert_eq!(
            build_invocation(
                r"C:\taw",
                &[("A", "1"), ("B", "2")],
                &["screen", "find-text", "File", "--json"],
            ),
            r#"set "A=1" && set "B=2" && call "C:\taw\testanyware.exe" screen find-text File --json"#
        );
        // Neither form may start with a quote (the strip-quirk guard).
        assert!(!build_invocation(r"C:\taw", &[], &["--version"]).starts_with('"'));
    }

    #[test]
    fn dist_extract_cmd_cleans_then_expands_and_dodges_the_quote_strip() {
        let cmd = dist_extract_cmd(r"C:\Users\Public\taw-dist.zip", r"C:\Users\Public\taw-dist");
        // Leads with `powershell`, never a bare `"`, so the cmd /c outer-quote
        // strip never fires (build_invocation's quirk, same guard).
        assert!(!cmd.starts_with('"'), "must not start with a quote: {cmd}");
        // Wipes the prior prefix before expanding — the "clean" in clean-prefix.
        assert!(cmd.contains("Remove-Item -Recurse -Force 'C:\\Users\\Public\\taw-dist'"));
        assert!(cmd.contains(
            "Expand-Archive -Path 'C:\\Users\\Public\\taw-dist.zip' \
             -DestinationPath 'C:\\Users\\Public\\taw-dist' -Force"
        ));
    }

    #[test]
    fn dist_bin_dir_is_dest_stem_bin() {
        assert_eq!(
            dist_bin_dir(
                r"C:\Users\Public\taw-dist",
                "testanyware-v1.2.3-aarch64-pc-windows-gnullvm",
            ),
            r"C:\Users\Public\taw-dist\testanyware-v1.2.3-aarch64-pc-windows-gnullvm\bin",
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
        let e = parse_json(&out("garbage", "The code execution cannot proceed", 0xC0000135u32 as i32))
            .unwrap_err();
        assert!(e.contains("cannot proceed"), "stderr surfaced: {e}");
    }

    #[test]
    fn detects_load_time_dll_failures() {
        // NTSTATUS STATUS_DLL_NOT_FOUND with no console text.
        assert!(looks_like_missing_dll("", -1073741515));
        // Console text form (some shells surface it).
        assert!(looks_like_missing_dll(
            "The code execution cannot proceed because avcodec-62.dll was not found.",
            1
        ));
        // A normal nonzero exit is not a DLL-load failure.
        assert!(!looks_like_missing_dll("usage: testanyware [OPTIONS]", 2));
        assert!(!looks_like_missing_dll("", 1));
    }

    #[test]
    fn required_dlls_cover_the_binarys_imported_set() {
        // Guards against silently dropping a DLL the loader needs. The four
        // directly-imported libs (170 research) plus swresample (avcodec's
        // bundled transitive dep) must all be present.
        for needed in ["avcodec-62.dll", "avformat-62.dll", "avutil-60.dll", "swscale-9.dll"] {
            assert!(REQUIRED_DLLS.contains(&needed), "missing directly-imported {needed}");
        }
        assert!(REQUIRED_DLLS.contains(&"swresample-6.dll"), "missing transitive dep");
    }

    #[test]
    fn endpoint_free_cases_are_all_endpoint_free() {
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
    fn find_text_hit_accepts_windows_media_ocr_with_a_file_box() {
        let body = serde_json::json!({
            "engine": "windows_media_ocr",
            "detections": [
                { "text": "Finder", "x": 40, "y": 2, "width": 48, "height": 16, "confidence": 1.0 },
                { "text": "File",   "x": 96, "y": 2, "width": 24, "height": 16, "confidence": 1.0 }
            ]
        });
        let note = find_text_hit(&body).expect("a windows_media_ocr File hit");
        assert!(note.contains("windows_media_ocr"), "note names the engine: {note}");
        assert!(note.contains("aarch64-windows"), "note names the arch: {note}");
    }

    #[test]
    fn find_text_hit_rejects_wrong_engine_missing_hit_and_degenerate_box() {
        // Wrong engine (e.g. the Linux EasyOCR daemon token) must fail this
        // Windows band — the band asserts the native engine actually ran.
        let daemon = serde_json::json!({
            "engine": "easyocr_daemon",
            "detections": [{ "text": "File", "x": 0, "y": 0, "width": 24, "height": 16, "confidence": 1 }]
        });
        assert!(find_text_hit(&daemon).is_err(), "wrong engine");

        // Native engine but no detection containing 'File'.
        let no_hit = serde_json::json!({
            "engine": "windows_media_ocr",
            "detections": [{ "text": "Edit", "x": 0, "y": 0, "width": 24, "height": 16, "confidence": 1 }]
        });
        assert!(find_text_hit(&no_hit).is_err(), "no File hit");

        // A 'File' hit with a zero-area box is implausible.
        let flat = serde_json::json!({
            "engine": "windows_media_ocr",
            "detections": [{ "text": "File", "x": 0, "y": 0, "width": 0, "height": 16, "confidence": 1 }]
        });
        assert!(find_text_hit(&flat).is_err(), "degenerate box");

        // Case-insensitive substring: 'profile' contains 'file'.
        let substr = serde_json::json!({
            "engine": "windows_media_ocr",
            "detections": [{ "text": "Profile", "x": 0, "y": 0, "width": 50, "height": 16, "confidence": 1 }]
        });
        assert!(find_text_hit(&substr).is_ok(), "case-insensitive substring match");
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
        let not_ready = serde_json::json!({
            "windows": [ { "windowType": "menuBar", "elements": [
                { "role": "menu-item", "label": "Apple" }
            ] } ]
        });
        assert!(!snapshot_menu_bar_ready(&not_ready));
        assert!(!snapshot_menu_bar_ready(&serde_json::json!({})));
    }
}
