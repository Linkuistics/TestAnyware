//! Windows golden-image creation: **unattended install + agent provisioning**.
//!
//! Full Rust port of `provisioner/scripts/vm-create-golden-windows.sh` (grove
//! leaf `220/020`), the analogue of node `110`'s macOS port. Unlike macOS — which
//! clones a pre-built vanilla image and provisions a *running* system over SSH —
//! the Windows golden boots a **blank disk from a Microsoft evaluation ISO**
//! alongside an **autounattend USB**, waits 20–40 min for a fully unattended
//! install, then provisions over the **in-VM agent's HTTP surface**: Windows
//! ships no sshd, so the agent (started on first logon via a Task Scheduler
//! task) is the only in-guest control channel (`200`-Q2, ADR-0009).
//!
//! ## What this reuses vs. what is net-new
//!
//! Reused from the QEMU runtime path (`010` confirmed `vm start --platform
//! windows` already works): [`crate::qemu_profile`] (accelerator/machine/cpu +
//! UEFI resolution), swtpm + TPM-2.0 wiring, [`crate::monitor`] (port discovery,
//! VNC password, `sendkey`), [`crate::detached::spawn_detached`], and
//! [`crate::process`] teardown. Net-new: the **install-boot** QEMU arg vector
//! (ISO + autounattend USB + blank NVMe, vs. a backing-file clone) and the
//! **agent-channel provisioning** (`/health`, `/exec`).
//!
//! ## Embedded media
//!
//! The autounattend answer file and its post-install scripts are
//! [`include_str!`]-embedded so the command is self-contained once the shell
//! script is deleted — exactly the policy `golden.rs` uses for the macOS plist /
//! wallpaper helper. The agent binary and the VirtIO ARM64 drivers are *not*
//! embedded: the agent is resolved at run time (brew bundle or override) and the
//! drivers are extracted from a cached `virtio-win.iso` ([[minimal-images]] — the
//! media is staged into a throwaway USB image, never baked anywhere durable).
//!
//! **macOS-host only** (`#[cfg(target_os = "macos")]` at the crate root, like
//! `golden`/`finalize`): the FAT32 autounattend media is built with `hdiutil`,
//! and golden creation is a macOS-host operation in this project (`200`-Q2).
//!
//! ## Progress narration
//!
//! Like the macOS golden, this multi-minute flow emits its running narration to
//! **stderr** via `eprintln!`, keeping `--json` stdout clean. The pure
//! path/arg-vector helpers are unit-tested; the live QEMU + agent orchestration
//! is verified by actually creating a golden on the Mac (cheap to re-clone, but
//! the *install* is the long pole — [[vm-costs]]).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use testanyware_agent_client::{AgentClient, AgentConfig};
use testanyware_protocol::{ExecRequest, ExecResult};

use crate::error::VmError;
use crate::golden::GoldenOptions;
use crate::id::generate_id;
use crate::monitor::QemuMonitorClient;
use crate::paths::VmPaths;
use crate::process::{pgrep_first, process_alive, terminate};
use crate::qemu_profile::{host_profile, resolve_uefi_code, which, QemuProfile};

// ---- embedded media (self-contained once the shell script is deleted) ----

/// The unattended answer file Windows Setup reads from the USB media. Embedded
/// from `provisioner/helpers/` so the command needs no `provisioner/` tree.
const AUTOUNATTEND_XML: &str = include_str!("../../../../provisioner/helpers/autounattend.xml");
/// Post-OOBE SYSTEM script: installs VirtIO drivers, copies the agent, registers
/// the logon task + firewall rule. Copied onto the USB media.
const SETUP_COMPLETE_CMD: &str = include_str!("../../../../provisioner/helpers/SetupComplete.cmd");
/// First-interactive-logon RunOnce script: wallpaper + desktop-clutter removal,
/// signals completion via a marker file. Copied onto the USB media.
const DESKTOP_SETUP_PS1: &str = include_str!("../../../../provisioner/helpers/desktop-setup.ps1");

/// UEFI Shell fallback so a stray drop to the shell still boots the installer.
/// Ports the script's inline `startup.nsh` heredoc (lines ~253–258).
const STARTUP_NSH: &str = "FS0:\\efi\\boot\\bootaa64.efi\n\
                           FS1:\\efi\\boot\\bootaa64.efi\n\
                           FS2:\\efi\\boot\\bootaa64.efi\n\
                           FS3:\\efi\\boot\\bootaa64.efi\n";

/// Agent TCP port forwarded guest→host (`hostfwd=tcp::0-:8648`). Same port the
/// runtime `vm start` path uses.
const AGENT_GUEST_PORT: u16 = 8648;

// ---- pure helpers (unit-tested) -----------------------------------------

/// Default golden name for a Windows `version`. Ports
/// `testanyware-golden-windows-$_VERSION`.
pub fn golden_name(version: &str) -> String {
    format!("testanyware-golden-windows-{version}")
}

/// The cached Microsoft evaluation ISO path for `version`. Ports
/// `$_CACHE_DIR/windows-${_VERSION}-arm64-eval.iso`.
pub fn cached_iso_path(cache_dir: &Path, version: &str) -> PathBuf {
    cache_dir.join(format!("windows-{version}-arm64-eval.iso"))
}

/// The cached virtio-win driver ISO. Ports `$_CACHE_DIR/virtio-win.iso`.
pub fn virtio_iso_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("virtio-win.iso")
}

/// The brew-bundled Windows agent under a `brew --prefix testanyware`. Ports
/// `$_BREW_PREFIX/share/testanyware/agents/windows/testanyware-agent.exe`.
pub fn agent_exe_under_prefix(brew_prefix: &Path) -> PathBuf {
    brew_prefix.join("share/testanyware/agents/windows/testanyware-agent.exe")
}

/// The three throwaway setup-VM artefacts under the cache dir, keyed by a
/// unique `setup_id`. Mirrors the script's `${_SETUP_PREFIX}.qcow2` /
/// `-efivars.fd` / `-tpm` trio (which the script keeps in `$_CACHE_DIR`).
#[derive(Debug, Clone)]
struct SetupArtifacts {
    qcow2: PathBuf,
    efivars: PathBuf,
    tpm_dir: PathBuf,
}

impl SetupArtifacts {
    fn for_id(cache_dir: &Path, setup_id: &str) -> Self {
        Self {
            qcow2: cache_dir.join(format!("{setup_id}.qcow2")),
            efivars: cache_dir.join(format!("{setup_id}-efivars.fd")),
            tpm_dir: cache_dir.join(format!("{setup_id}-tpm")),
        }
    }
}

/// The golden image's three on-disk artefacts under the golden dir.
#[derive(Debug, Clone)]
struct GoldenArtifacts {
    qcow2: PathBuf,
    efivars: PathBuf,
    tpm_dir: PathBuf,
}

impl GoldenArtifacts {
    fn for_name(golden_dir: &Path, name: &str) -> Self {
        Self {
            qcow2: golden_dir.join(format!("{name}.qcow2")),
            efivars: golden_dir.join(format!("{name}-efivars.fd")),
            tpm_dir: golden_dir.join(format!("{name}-tpm")),
        }
    }
}

/// Per-boot files + sockets feeding [`build_install_qemu_args`].
#[derive(Debug, Clone)]
struct InstallBootSpec {
    uefi_code: PathBuf,
    efivars: PathBuf,
    setup_qcow2: PathBuf,
    tpm_socket: PathBuf,
    monitor_socket: PathBuf,
    iso: PathBuf,
    autounattend_img: PathBuf,
    serial_log: PathBuf,
}

/// Build the QEMU argument vector for the **install boot**. Pure — depends only
/// on the host profile and the per-boot spec. Ports the script's
/// `qemu-system-aarch64 …` invocation (lines ~327–354): boots the blank NVMe
/// disk from the install ISO + autounattend USB, with a dynamic agent forward
/// (`tcp::0-:8648`) and a password-gated VNC. Differs from
/// [`crate::qemu::build_qemu_args`] (the runtime clone path): a fresh ISO/USB
/// install rather than a backing-file clone, and `ramfb`-only video (the VirtIO
/// GPU driver is not installed until SetupComplete.cmd runs).
fn build_install_qemu_args(profile: &QemuProfile, spec: &InstallBootSpec) -> Vec<String> {
    let s = |p: &Path| p.display().to_string();
    vec![
        "-machine".into(), profile.machine.into(),
        "-accel".into(), profile.accelerator.into(),
        "-cpu".into(), profile.cpu.into(),
        "-smp".into(), "4".into(),
        "-m".into(), "4096".into(),
        "-drive".into(), format!("if=pflash,format=raw,file={},readonly=on", s(&spec.uefi_code)),
        "-drive".into(), format!("if=pflash,format=raw,file={}", s(&spec.efivars)),
        "-chardev".into(), format!("socket,id=chrtpm,path={}", s(&spec.tpm_socket)),
        "-tpmdev".into(), "emulator,id=tpm0,chardev=chrtpm".into(),
        "-device".into(), "tpm-tis-device,tpmdev=tpm0".into(),
        "-drive".into(), format!("file={},if=none,id=hd0,format=qcow2", s(&spec.setup_qcow2)),
        "-device".into(), "nvme,drive=hd0,serial=boot,bootindex=0".into(),
        "-device".into(), "ramfb".into(),
        "-device".into(), "qemu-xhci".into(),
        "-device".into(), "usb-kbd".into(),
        "-device".into(), "usb-tablet".into(),
        "-drive".into(), format!("file={},if=none,id=cd0,media=cdrom,readonly=on", s(&spec.iso)),
        "-device".into(), "usb-storage,drive=cd0,bootindex=1".into(),
        "-drive".into(), format!("file={},if=none,id=unattend,format=raw", s(&spec.autounattend_img)),
        "-device".into(), "usb-storage,drive=unattend,removable=on".into(),
        "-device".into(), "virtio-net-pci,netdev=net0".into(),
        "-netdev".into(), format!("user,id=net0,hostfwd=tcp::0-:{AGENT_GUEST_PORT}"),
        "-vnc".into(), "localhost:0,to=99,password=on".into(),
        "-monitor".into(), format!("unix:{},server,nowait", s(&spec.monitor_socket)),
        "-serial".into(), format!("file:{}", s(&spec.serial_log)),
        "-d".into(), "guest_errors".into(),
        "-display".into(), "none".into(),
    ]
}

/// The agent `/exec` probe for the desktop-setup completion marker. Ports the
/// script's `if exist C:\Windows\Setup\Scripts\desktop-setup-done.txt echo DONE`.
fn desktop_setup_done_probe() -> &'static str {
    "if exist C:\\Windows\\Setup\\Scripts\\desktop-setup-done.txt echo DONE"
}

/// Whether an `/exec` result signals the desktop-setup marker is present.
fn desktop_setup_done(result: &ExecResult) -> bool {
    result.stdout.contains("DONE")
}

/// The pre-shutdown desktop-cleanup command: close any startup apps that opened
/// windows so clones inherit a clean desktop. Ports the script's
/// `powershell … Stop-Process` one-liner (line ~535).
fn clean_desktop_command() -> &'static str {
    "powershell -Command \"@('GetStarted','Video.UI','HelpPane','SearchHost',\
     'SearchApp','PhoneExperienceHost','msedge','Widgets') | ForEach-Object { \
     Get-Process -Name $_ -ErrorAction SilentlyContinue | Stop-Process -Force }\""
}

// ---- live orchestration (verified on the Mac, not unit-tested) -----------
//
// These shell out to qemu/swtpm/hdiutil and drive the guest over the agent's
// HTTP surface, so they are exercised by actually creating a golden on the Mac,
// not by unit tests — the same policy as `golden.rs`/`tart.rs`.

/// Resolve the Windows agent `.exe` to install. Honors
/// `TESTANYWARE_AGENT_BIN_OVERRIDE` (contributor builds); otherwise the
/// brew-bundled artifact under `brew --prefix testanyware`. Ports the script's
/// agent-resolution block (lines ~185–207).
fn resolve_windows_agent() -> Result<PathBuf, VmError> {
    if let Ok(over) = std::env::var("TESTANYWARE_AGENT_BIN_OVERRIDE") {
        if !over.is_empty() {
            let path = PathBuf::from(over);
            if !path.is_file() {
                return Err(VmError::GoldenCreateFailed {
                    detail: format!(
                        "TESTANYWARE_AGENT_BIN_OVERRIDE points at a missing file: {}",
                        path.display()
                    ),
                });
            }
            return Ok(path);
        }
    }
    let prefix = brew_prefix("testanyware").ok_or_else(|| VmError::GoldenCreateFailed {
        detail: "`brew --prefix testanyware` failed — install with \
                 `brew install Linkuistics/taps/testanyware`, or set \
                 TESTANYWARE_AGENT_BIN_OVERRIDE for a contributor build"
            .into(),
    })?;
    let exe = agent_exe_under_prefix(&prefix);
    if !exe.is_file() {
        return Err(VmError::GoldenCreateFailed {
            detail: format!("Windows agent binary not found at {}", exe.display()),
        });
    }
    Ok(exe)
}

/// `brew --prefix <formula>` stdout (trimmed), or `None` when brew is absent or
/// the formula is not installed. (Same shape as `golden::brew_prefix`, kept
/// local to avoid widening that module's visibility.)
fn brew_prefix(formula: &str) -> Option<PathBuf> {
    let brew = which("brew")?;
    let out = std::process::Command::new(brew).args(["--prefix", formula]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!prefix.is_empty()).then(|| PathBuf::from(prefix))
}

/// Resolve a tool path via `which`, falling back to the bare name (so QEMU's
/// own PATH lookup applies if `which` came up empty).
fn tool(name: &str) -> String {
    which(name).map(|p| p.display().to_string()).unwrap_or_else(|| name.to_string())
}

/// Run `program` synchronously, discarding stdout (helpers like `qemu-img`/
/// `hdiutil` print banners there — a `--json` envelope must own stdout) and
/// inheriting stderr. Errors on a non-zero exit. Mirrors `qemu::run_and_check`.
fn run_checked(program: &str, args: &[String], what: &str) -> Result<(), VmError> {
    let status = std::process::Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| VmError::QemuFailed { detail: format!("{what} ({program}): {e}") })?;
    if status.success() {
        Ok(())
    } else {
        Err(VmError::QemuFailed { detail: format!("{what}: {program} exited {status}") })
    }
}

/// `/exec` against the in-VM agent, **tolerating** failure with a warning
/// (matches the script's `… || true` steps). Returns the result on success.
async fn agent_exec(client: &AgentClient, command: &str, timeout_secs: i64) -> Option<ExecResult> {
    let req = ExecRequest { command: command.to_string(), timeout: timeout_secs, detach: false };
    match client.exec(&req).await {
        Ok(result) => Some(result),
        Err(e) => {
            eprintln!("  warning: agent exec failed (continuing): {e}");
            None
        }
    }
}

/// Best-effort teardown of the setup VM + its artefacts on a failure path.
/// SIGTERM→SIGKILL the qemu pid, pgrep-kill swtpm by its TPM state-dir, and
/// (unless the golden finalized) remove the setup disk/efivars/tpm. Ports the
/// script's `trap cleanup EXIT`.
fn cleanup_setup(qemu_pid: i32, art: &SetupArtifacts, golden_done: bool) {
    if qemu_pid > 0 {
        terminate(qemu_pid, Duration::from_millis(200), 10);
    }
    if let Some(swtpm_pid) = pgrep_first(&format!("swtpm.*{}", art.tpm_dir.display())) {
        terminate(swtpm_pid, Duration::from_millis(200), 5);
    }
    if !golden_done {
        let _ = std::fs::remove_file(&art.qcow2);
        let _ = std::fs::remove_file(&art.efivars);
        let _ = std::fs::remove_dir_all(&art.tpm_dir);
    }
}

/// Copy every regular file directly inside `src_dir` into `dst_dir` (created if
/// absent). Ports the script's `cp "$_VIRTIO_MNT/.../ARM64/"*` driver copies.
fn copy_dir_files(src_dir: &Path, dst_dir: &Path) -> Result<(), VmError> {
    std::fs::create_dir_all(dst_dir)
        .map_err(|e| VmError::Io(format!("create {}: {e}", dst_dir.display())))?;
    let entries = std::fs::read_dir(src_dir)
        .map_err(|e| VmError::Io(format!("read {}: {e}", src_dir.display())))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let dst = dst_dir.join(entry.file_name());
            std::fs::copy(&path, &dst)
                .map_err(|e| VmError::Io(format!("copy {}: {e}", path.display())))?;
        }
    }
    Ok(())
}

/// Ensure the install ISO is available in the cache: copy a `--iso` argument in,
/// or require an already-cached one. Ports the script's ISO-locate block
/// (lines ~209–230).
fn ensure_iso(iso_arg: Option<&Path>, cache_dir: &Path, version: &str) -> Result<PathBuf, VmError> {
    let cached = cached_iso_path(cache_dir, version);
    if let Some(src) = iso_arg {
        if !src.is_file() {
            return Err(VmError::GoldenCreateFailed {
                detail: format!("ISO file not found: {}", src.display()),
            });
        }
        eprintln!("Copying ISO to cache...");
        std::fs::copy(src, &cached)
            .map_err(|e| VmError::Io(format!("copy ISO into cache: {e}")))?;
        return Ok(cached);
    }
    if cached.is_file() {
        return Ok(cached);
    }
    Err(VmError::GoldenCreateFailed {
        detail: format!(
            "no Windows ARM64 evaluation ISO available. Download the ARM64 ISO from \
             https://www.microsoft.com/en-us/software-download/windows11arm64 and re-run with \
             --iso <path> (it is cached at {} afterwards)",
            cached.display()
        ),
    })
}

/// Download virtio-win.iso to the cache if missing, mount it, and extract the
/// NetKVM + VioGPU ARM64 drivers into `<stage>/drivers/{netkvm,viogpu}`. Ports
/// the script's virtio block (lines ~260–274). The ISO is cached; the drivers
/// are staged into the throwaway media only.
fn stage_virtio_drivers(cache_dir: &Path, stage_dir: &Path) -> Result<(), VmError> {
    let iso = virtio_iso_path(cache_dir);
    if !iso.is_file() {
        eprintln!("Downloading virtio-win drivers (~600MB, cached after first run)...");
        run_checked(
            &tool("curl"),
            &[
                "-L".into(), "-o".into(), iso.display().to_string(),
                "https://fedorapeople.org/groups/virt/virtio-win/direct-downloads/stable-virtio/virtio-win.iso".into(),
            ],
            "download virtio-win.iso",
        )?;
    }
    // Mount the ISO read-only with hdiutil, copy the ARM64 drivers, detach.
    let mount = stage_dir.join("virtio-mnt");
    std::fs::create_dir_all(&mount)
        .map_err(|e| VmError::Io(format!("create {}: {e}", mount.display())))?;
    run_checked(
        "hdiutil",
        &[
            "attach".into(), iso.display().to_string(),
            "-mountpoint".into(), mount.display().to_string(),
            "-readonly".into(), "-nobrowse".into(), "-quiet".into(),
        ],
        "attach virtio-win.iso",
    )?;
    let drivers = stage_dir.join("drivers");
    let copy_result: Result<(), VmError> = (|| {
        copy_dir_files(&mount.join("NetKVM/w11/ARM64"), &drivers.join("netkvm"))?;
        copy_dir_files(&mount.join("viogpudo/w11/ARM64"), &drivers.join("viogpu"))?;
        Ok(())
    })();
    // Always detach, even if a copy failed.
    let _ = run_checked("hdiutil", &["detach".into(), mount.display().to_string(), "-quiet".into()], "detach virtio-win.iso");
    let _ = std::fs::remove_dir_all(&mount);
    copy_result?;
    eprintln!("  NetKVM + VioGPU ARM64 drivers included.");
    Ok(())
}

/// Build the FAT32 autounattend USB media: stage the embedded answer file +
/// scripts, the agent binary, and the VirtIO drivers, then create a raw disk
/// image Windows Setup mounts as removable media. Ports the script's
/// autounattend-media block (lines ~235–283). Returns the raw image path.
fn build_autounattend_media(
    cache_dir: &Path,
    stage_dir: &Path,
    agent_exe: &Path,
    img_path: &Path,
) -> Result<(), VmError> {
    std::fs::create_dir_all(stage_dir)
        .map_err(|e| VmError::Io(format!("create {}: {e}", stage_dir.display())))?;
    let payload = stage_dir.join("payload");
    std::fs::create_dir_all(&payload)
        .map_err(|e| VmError::Io(format!("create {}: {e}", payload.display())))?;

    // Embedded answer file + scripts (single source of truth, compiled in).
    std::fs::write(payload.join("autounattend.xml"), AUTOUNATTEND_XML)
        .map_err(|e| VmError::Io(format!("write autounattend.xml: {e}")))?;
    std::fs::write(payload.join("SetupComplete.cmd"), SETUP_COMPLETE_CMD)
        .map_err(|e| VmError::Io(format!("write SetupComplete.cmd: {e}")))?;
    std::fs::write(payload.join("desktop-setup.ps1"), DESKTOP_SETUP_PS1)
        .map_err(|e| VmError::Io(format!("write desktop-setup.ps1: {e}")))?;
    std::fs::write(payload.join("startup.nsh"), STARTUP_NSH)
        .map_err(|e| VmError::Io(format!("write startup.nsh: {e}")))?;

    // Agent binary (resolved at run time, not embedded).
    std::fs::copy(agent_exe, payload.join("testanyware-agent.exe"))
        .map_err(|e| VmError::Io(format!("copy agent into media: {e}")))?;

    // VirtIO ARM64 drivers extracted from the cached virtio-win.iso.
    stage_virtio_drivers(cache_dir, &payload)?;

    // FAT32 (not FAT16 — the ~150MB agent exceeds FAT16's practical limits).
    // hdiutil appends `.dmg` to the `-ov` output, so convert that to a raw image.
    let dmg = PathBuf::from(format!("{}.dmg", img_path.display()));
    let _ = std::fs::remove_file(&dmg);
    run_checked(
        "hdiutil",
        &[
            "create".into(), "-size".into(), "200m".into(),
            "-fs".into(), "MS-DOS FAT32".into(),
            "-volname".into(), "UNATTEND".into(),
            "-srcfolder".into(), payload.display().to_string(),
            "-ov".into(), img_path.display().to_string(),
            "-quiet".into(),
        ],
        "create autounattend media (hdiutil)",
    )?;
    run_checked(
        &tool("qemu-img"),
        &[
            "convert".into(), "-f".into(), "dmg".into(), "-O".into(), "raw".into(),
            dmg.display().to_string(), img_path.display().to_string(),
        ],
        "convert autounattend media to raw",
    )?;
    let _ = std::fs::remove_file(&dmg);
    Ok(())
}

/// Truncate a fresh `size_bytes` UEFI variable store. AArch64 QEMU ships no
/// vars template, so the script creates a blank 64MB file (`truncate -s 64M`).
fn create_blank_efivars(path: &Path, size_bytes: u64) -> Result<(), VmError> {
    let f = std::fs::File::create(path)
        .map_err(|e| VmError::Io(format!("create efivars {}: {e}", path.display())))?;
    f.set_len(size_bytes)
        .map_err(|e| VmError::Io(format!("size efivars {}: {e}", path.display())))?;
    Ok(())
}

/// Agent-health gate budget during install: 120 × 30s = up to 60 min for the
/// unattended install + first logon. Ports the script's `seq 1 120` loop.
const INSTALL_WAIT_ATTEMPTS: u32 = 120;
const INSTALL_WAIT_INTERVAL: Duration = Duration::from_secs(30);
/// Desktop-setup completion gate: 150 × 2s = 5 min. Ports `seq 1 150`.
const DESKTOP_WAIT_ATTEMPTS: u32 = 150;
const DESKTOP_WAIT_INTERVAL: Duration = Duration::from_secs(2);
/// Post-reboot agent re-appearance gate: 120 × 5s = 10 min. Ports `seq 1 120`.
const REBOOT_WAIT_ATTEMPTS: u32 = 120;
const REBOOT_WAIT_INTERVAL: Duration = Duration::from_secs(5);

/// Poll the agent's `/health` until reachable, bailing early if the QEMU
/// process dies. Returns `true` once healthy. Narrates elapsed time to stderr.
async fn wait_for_install_agent(
    client: &AgentClient,
    qemu_pid: i32,
    attempts: u32,
    interval: Duration,
) -> bool {
    for attempt in 0..attempts {
        if !process_alive(qemu_pid) {
            eprintln!("\n  QEMU process died during installation.");
            return false;
        }
        if client.health().await.is_ok() {
            eprintln!("\n  Agent ready.");
            return true;
        }
        let elapsed = (attempt + 1) * interval.as_secs() as u32;
        eprint!("\r  [{:02}:{:02}] waiting for agent...", elapsed / 60, elapsed % 60);
        if attempt + 1 < attempts {
            tokio::time::sleep(interval).await;
        }
    }
    eprintln!();
    false
}

/// The command entry point: produce a Windows golden image. Boots a fresh
/// unattended install, provisions over the in-VM agent, settles the desktop,
/// shuts down cleanly, and finalizes the three golden artefacts. Returns the
/// golden's name on success.
///
/// On any failure the setup VM is torn down and the partial setup artefacts are
/// removed, so a botched run never strands a VM or a half-built disk. `iso_arg`
/// is the optional `--iso` path (required on first run unless already cached).
pub async fn create_golden_windows(
    opts: &GoldenOptions,
    iso_arg: Option<&Path>,
    paths: &VmPaths,
) -> Result<String, VmError> {
    // --- Fail fast on host prerequisites before booting anything ----------
    crate::preflight::check_swtpm()?;
    let agent_exe = resolve_windows_agent()?;
    eprintln!("Using Windows agent: {}", agent_exe.display());

    let cache_dir = paths.cache_dir();
    let golden_dir = paths.golden_dir();
    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| VmError::Io(format!("create {}: {e}", cache_dir.display())))?;
    std::fs::create_dir_all(&golden_dir)
        .map_err(|e| VmError::Io(format!("create {}: {e}", golden_dir.display())))?;

    let iso = ensure_iso(iso_arg, &cache_dir, &opts.version)?;
    eprintln!("Using install ISO: {}", iso.display());

    // --- Delete any existing golden of the same name (script lines ~178) ---
    let golden = GoldenArtifacts::for_name(&golden_dir, &opts.name);
    if golden.qcow2.is_file() {
        eprintln!("Deleting existing golden image '{}'...", opts.name);
        let _ = std::fs::remove_file(&golden.qcow2);
        let _ = std::fs::remove_file(&golden.efivars);
        let _ = std::fs::remove_dir_all(&golden.tpm_dir);
    }

    // --- Stage the throwaway setup artefacts ------------------------------
    let setup_id = generate_id().replacen("testanyware-", "testanyware-setup-", 1);
    let setup = SetupArtifacts::for_id(&cache_dir, &setup_id);
    let stage_dir = cache_dir.join(format!("{setup_id}-stage"));

    // Drive the build, tearing everything down on any failure.
    match build_and_provision(&agent_exe, &iso, &setup, &setup_id, &stage_dir, paths).await {
        Ok(qemu_pid) => {
            // The VM is down; finalize by moving the setup artefacts to golden.
            eprintln!("Creating golden image '{}'...", opts.name);
            finalize_golden(&setup, &golden)?;
            cleanup_setup(qemu_pid, &setup, /* golden_done */ true);
            let _ = std::fs::remove_dir_all(&stage_dir);
            eprintln!("Golden image '{}' created successfully.", opts.name);
            Ok(opts.name.clone())
        }
        Err((qemu_pid, err)) => {
            cleanup_setup(qemu_pid, &setup, /* golden_done */ false);
            let _ = std::fs::remove_dir_all(&stage_dir);
            Err(err)
        }
    }
}

/// The build body, factored out so `create_golden_windows` wraps it in a single
/// teardown-on-failure guard. Returns the live QEMU pid on success (the VM is
/// already shut down; the pid is handed back so the caller's cleanup can reap a
/// lingering process). On failure returns `(latest_pid, error)`.
#[allow(clippy::too_many_arguments)]
async fn build_and_provision(
    agent_exe: &Path,
    iso: &Path,
    setup: &SetupArtifacts,
    setup_id: &str,
    stage_dir: &Path,
    paths: &VmPaths,
) -> Result<i32, (i32, VmError)> {
    macro_rules! tri {
        ($e:expr, $pid:expr) => {
            match $e {
                Ok(v) => v,
                Err(e) => return Err(($pid, e)),
            }
        };
    }

    // --- Setup disk (64GB sparse qcow2) -----------------------------------
    eprintln!("Creating setup disk (64GB)...");
    tri!(run_checked(
        &tool("qemu-img"),
        &["create".into(), "-f".into(), "qcow2".into(),
          setup.qcow2.display().to_string(), "64G".into()],
        "create setup disk",
    ), 0);

    // --- Autounattend media (embedded answer file + agent + drivers) ------
    eprintln!("Creating autounattend media...");
    let autounattend_img = stage_dir.join("autounattend.img");
    tri!(build_autounattend_media(&paths.cache_dir(), stage_dir, agent_exe, &autounattend_img), 0);

    // --- UEFI firmware + blank vars + swtpm --------------------------------
    let profile = host_profile();
    let uefi_code = tri!(
        resolve_uefi_code(&profile.uefi_code_candidates).ok_or_else(|| VmError::UefiNotFound {
            path: profile
                .uefi_code_candidates
                .first()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<no candidate paths>".into()),
        }),
        0
    );
    eprintln!("Preparing UEFI firmware and TPM...");
    tri!(create_blank_efivars(&setup.efivars, 64 * 1024 * 1024), 0);
    tri!(std::fs::create_dir_all(&setup.tpm_dir)
        .map_err(|e| VmError::Io(format!("create {}: {e}", setup.tpm_dir.display()))), 0);
    let tpm_socket = setup.tpm_dir.join("swtpm-sock");
    tri!(run_checked(
        &tool("swtpm"),
        &["socket".into(),
          "--tpmstate".into(), format!("dir={}", setup.tpm_dir.display()),
          "--ctrl".into(), format!("type=unixio,path={}", tpm_socket.display()),
          "--tpm2".into(),
          "--log".into(), "level=0".into(),
          "--daemon".into()],
        "start swtpm",
    ), 0);
    tokio::time::sleep(Duration::from_secs(1)).await;

    // --- Boot the installer (detached QEMU) -------------------------------
    let session = paths.session_dir(setup_id);
    tri!(std::fs::create_dir_all(&session)
        .map_err(|e| VmError::Io(format!("create {}: {e}", session.display()))), 0);
    let monitor_socket = session.join("monitor.sock");
    let qemu_log = stage_dir.join("qemu.log");
    let boot = InstallBootSpec {
        uefi_code,
        efivars: setup.efivars.clone(),
        setup_qcow2: setup.qcow2.clone(),
        tpm_socket,
        monitor_socket: monitor_socket.clone(),
        iso: iso.to_path_buf(),
        autounattend_img,
        serial_log: qemu_log.clone(),
    };
    let args = build_install_qemu_args(&profile, &boot);
    eprintln!("Booting Windows VM from ISO with QEMU (install: 20–40 min)...");
    let qemu_pid = tri!(
        crate::detached::spawn_detached(&tool(profile.qemu_binary), &args, &qemu_log),
        0
    );
    tokio::time::sleep(Duration::from_secs(2)).await;
    if !process_alive(qemu_pid) {
        return Err((qemu_pid, VmError::QemuFailed {
            detail: "QEMU did not remain running after launch".into(),
        }));
    }

    // --- Monitor: set VNC password, discover agent port -------------------
    let monitor = QemuMonitorClient::new(&monitor_socket);
    monitor.set_vnc_password("admin", 3).await;
    let agent_port = match monitor.agent_port(5, Duration::from_secs(1)).await {
        Some(p) => p,
        None => return Err((qemu_pid, VmError::MonitorDiscoveryFailed)),
    };
    let vnc_port = monitor.vnc_port(3, Duration::from_millis(500)).await.unwrap_or(5900);
    eprintln!("  Agent port: {agent_port}   VNC port: {vnc_port} (password: admin)");

    // Dismiss the "Press any key to boot from CD..." prompt with a short burst
    // of Enter keypresses (script's background `sendkey ret` loop).
    for _ in 0..8 {
        let _ = monitor.send("sendkey ret", Duration::from_millis(200)).await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // --- Wait for the agent (the long pole: unattended install) -----------
    eprintln!("Waiting for agent on localhost:{agent_port} (install + first logon)...");
    let client = tri!(
        AgentClient::new(AgentConfig::new("localhost", agent_port).with_timeout(Duration::from_secs(5)))
            .map_err(|e| VmError::GoldenCreateFailed { detail: format!("agent client: {e}") }),
        qemu_pid
    );
    if !wait_for_install_agent(&client, qemu_pid, INSTALL_WAIT_ATTEMPTS, INSTALL_WAIT_INTERVAL).await {
        return Err((qemu_pid, VmError::GoldenCreateFailed {
            detail: format!("agent not reachable within {} min; connect VNC to localhost:{vnc_port} (password admin) to diagnose",
                            (INSTALL_WAIT_ATTEMPTS * INSTALL_WAIT_INTERVAL.as_secs() as u32) / 60),
        }));
    }

    // --- Diagnostics: SetupComplete.log -----------------------------------
    if let Some(r) = agent_exec(&client, "type C:\\Windows\\Setup\\Scripts\\SetupComplete.log", 30).await {
        if !r.stdout.trim().is_empty() {
            eprintln!("SetupComplete.log:\n{}", r.stdout.trim());
        }
    }

    // --- Wait for the desktop-setup RunOnce marker ------------------------
    eprint!("Waiting for desktop setup to complete...");
    let mut desktop_done = false;
    for attempt in 0..DESKTOP_WAIT_ATTEMPTS {
        if let Some(r) = agent_exec(&client, desktop_setup_done_probe(), 10).await {
            if desktop_setup_done(&r) {
                eprintln!(" done.");
                desktop_done = true;
                break;
            }
        }
        eprint!(".");
        if attempt + 1 < DESKTOP_WAIT_ATTEMPTS {
            tokio::time::sleep(DESKTOP_WAIT_INTERVAL).await;
        }
    }
    if !desktop_done {
        return Err((qemu_pid, VmError::GoldenCreateFailed {
            detail: "desktop setup script did not complete".into(),
        }));
    }

    // Let Windows settle (search indexing, app readiness, component cleanup).
    eprintln!("Waiting 60s for Windows to settle...");
    tokio::time::sleep(Duration::from_secs(60)).await;

    // --- Reboot so wallpaper/taskbar changes take full effect -------------
    eprint!("Rebooting to finalize...");
    let _ = agent_exec(&client, "shutdown /r /t 0", 10).await;
    tokio::time::sleep(Duration::from_secs(15)).await;
    let mut back = false;
    for attempt in 0..REBOOT_WAIT_ATTEMPTS {
        if client.health().await.is_ok() {
            eprintln!(" back online.");
            back = true;
            break;
        }
        eprint!(".");
        if attempt + 1 < REBOOT_WAIT_ATTEMPTS {
            tokio::time::sleep(REBOOT_WAIT_INTERVAL).await;
        }
    }
    if !back {
        return Err((qemu_pid, VmError::GoldenCreateFailed {
            detail: "VM did not come back online after the finalize reboot".into(),
        }));
    }
    eprintln!("Waiting 30s for final settle...");
    tokio::time::sleep(Duration::from_secs(30)).await;

    // --- Clean desktop, then shut down ------------------------------------
    eprintln!("Cleaning desktop state...");
    let _ = agent_exec(&client, clean_desktop_command(), 30).await;

    if !process_alive(qemu_pid) {
        return Err((qemu_pid, VmError::QemuFailed { detail: "QEMU process died before shutdown".into() }));
    }
    eprintln!("Shutting down VM...");
    let _ = agent_exec(&client, "shutdown /s /t 0", 10).await;

    eprint!("Waiting for shutdown...");
    for attempt in 0..60 {
        if !process_alive(qemu_pid) {
            eprintln!(" done.");
            break;
        }
        eprint!(".");
        if attempt + 1 < 60 {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
    if process_alive(qemu_pid) {
        eprintln!(" forcing stop.");
        terminate(qemu_pid, Duration::from_millis(200), 10);
    }
    // Stop swtpm now that QEMU is down (the TPM state is already on disk).
    if let Some(swtpm_pid) = pgrep_first(&format!("swtpm.*{}", setup.tpm_dir.display())) {
        terminate(swtpm_pid, Duration::from_millis(200), 5);
    }

    Ok(qemu_pid)
}

/// Move the three throwaway setup artefacts into their golden home. Ports the
/// script's finalize `mv`s (lines ~571–574).
fn finalize_golden(setup: &SetupArtifacts, golden: &GoldenArtifacts) -> Result<(), VmError> {
    rename_or_copy(&setup.qcow2, &golden.qcow2)?;
    rename_or_copy(&setup.efivars, &golden.efivars)?;
    // The TPM state is a directory: rename if possible, else recursive copy.
    if std::fs::rename(&setup.tpm_dir, &golden.tpm_dir).is_err() {
        copy_dir_recursive(&setup.tpm_dir, &golden.tpm_dir)?;
        let _ = std::fs::remove_dir_all(&setup.tpm_dir);
    }
    Ok(())
}

/// `rename` a file, falling back to copy+unlink when the source and
/// destination live on different filesystems (cache vs. golden may differ).
fn rename_or_copy(src: &Path, dst: &Path) -> Result<(), VmError> {
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    std::fs::copy(src, dst)
        .map_err(|e| VmError::Io(format!("move {} → {}: {e}", src.display(), dst.display())))?;
    let _ = std::fs::remove_file(src);
    Ok(())
}

/// Recursively copy `src` dir into `dst` (used as the cross-filesystem fallback
/// for the TPM state dir).
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), VmError> {
    std::fs::create_dir_all(dst)
        .map_err(|e| VmError::Io(format!("create {}: {e}", dst.display())))?;
    let entries = std::fs::read_dir(src)
        .map_err(|e| VmError::Io(format!("read {}: {e}", src.display())))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            std::fs::copy(&path, &target)
                .map_err(|e| VmError::Io(format!("copy {}: {e}", path.display())))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_name_matches_windows_naming() {
        assert_eq!(golden_name("11"), "testanyware-golden-windows-11");
    }

    #[test]
    fn cached_iso_path_matches_script_naming() {
        assert_eq!(
            cached_iso_path(Path::new("/d/cache"), "11"),
            PathBuf::from("/d/cache/windows-11-arm64-eval.iso")
        );
    }

    #[test]
    fn virtio_iso_path_is_under_cache() {
        assert_eq!(
            virtio_iso_path(Path::new("/d/cache")),
            PathBuf::from("/d/cache/virtio-win.iso")
        );
    }

    #[test]
    fn agent_exe_under_prefix_matches_brew_layout() {
        assert_eq!(
            agent_exe_under_prefix(Path::new("/opt/homebrew/opt/testanyware")),
            PathBuf::from(
                "/opt/homebrew/opt/testanyware/share/testanyware/agents/windows/testanyware-agent.exe"
            )
        );
    }

    #[test]
    fn setup_and_golden_artifacts_use_the_expected_trio() {
        let s = SetupArtifacts::for_id(Path::new("/d/cache"), "testanyware-setup-abcd1234");
        assert_eq!(s.qcow2, PathBuf::from("/d/cache/testanyware-setup-abcd1234.qcow2"));
        assert_eq!(s.efivars, PathBuf::from("/d/cache/testanyware-setup-abcd1234-efivars.fd"));
        assert_eq!(s.tpm_dir, PathBuf::from("/d/cache/testanyware-setup-abcd1234-tpm"));

        let g = GoldenArtifacts::for_name(Path::new("/d/golden"), "testanyware-golden-windows-11");
        assert_eq!(g.qcow2, PathBuf::from("/d/golden/testanyware-golden-windows-11.qcow2"));
        assert_eq!(g.efivars, PathBuf::from("/d/golden/testanyware-golden-windows-11-efivars.fd"));
        assert_eq!(g.tpm_dir, PathBuf::from("/d/golden/testanyware-golden-windows-11-tpm"));
    }

    #[test]
    fn install_qemu_args_wire_iso_usb_tpm_and_forwards() {
        let spec = InstallBootSpec {
            uefi_code: "/fw/code.fd".into(),
            efivars: "/c/setup-efivars.fd".into(),
            setup_qcow2: "/c/setup.qcow2".into(),
            tpm_socket: "/s/swtpm-sock".into(),
            monitor_socket: "/s/monitor.sock".into(),
            iso: "/c/windows-11-arm64-eval.iso".into(),
            autounattend_img: "/c/autounattend.img".into(),
            serial_log: "/c/qemu.log".into(),
        };
        let joined = build_install_qemu_args(&host_profile(), &spec).join(" ");
        // Install media wired as bootable CD + removable USB.
        assert!(joined.contains("file=/c/windows-11-arm64-eval.iso,if=none,id=cd0,media=cdrom,readonly=on"));
        assert!(joined.contains("usb-storage,drive=cd0,bootindex=1"));
        assert!(joined.contains("file=/c/autounattend.img,if=none,id=unattend,format=raw"));
        assert!(joined.contains("usb-storage,drive=unattend,removable=on"));
        // Blank NVMe boot disk + TPM + dynamic agent forward + gated VNC.
        assert!(joined.contains("nvme,drive=hd0,serial=boot,bootindex=0"));
        assert!(joined.contains("tpm-tis-device,tpmdev=tpm0"));
        assert!(joined.contains("hostfwd=tcp::0-:8648"));
        assert!(joined.contains("password=on"));
        assert!(joined.contains("unix:/s/monitor.sock,server,nowait"));
        assert!(joined.contains("file:/c/qemu.log"));
        // ramfb-only video: the VioGPU driver is not installed at install time.
        assert!(!joined.contains("virtio-gpu-pci"), "install boot must not wire virtio-gpu: {joined}");
    }

    #[test]
    fn desktop_setup_done_detects_the_marker_echo() {
        let yes = ExecResult { exit_code: 0, stdout: "DONE\r\n".into(), stderr: String::new(), timed_out: None };
        let no = ExecResult { exit_code: 0, stdout: String::new(), stderr: String::new(), timed_out: None };
        assert!(desktop_setup_done(&yes));
        assert!(!desktop_setup_done(&no));
        assert_eq!(
            desktop_setup_done_probe(),
            "if exist C:\\Windows\\Setup\\Scripts\\desktop-setup-done.txt echo DONE"
        );
    }

    #[test]
    fn clean_desktop_command_force_stops_startup_apps() {
        let cmd = clean_desktop_command();
        assert!(cmd.starts_with("powershell -Command"));
        assert!(cmd.contains("SearchHost"));
        assert!(cmd.contains("Stop-Process -Force"));
    }

    #[test]
    fn embedded_media_is_present() {
        // include_str! guards: a moved/renamed provisioner file fails the build,
        // but this catches an accidental emptying.
        assert!(AUTOUNATTEND_XML.contains("<unattend"));
        assert!(AUTOUNATTEND_XML.contains("FirstLogonCommands"));
        assert!(SETUP_COMPLETE_CMD.contains("schtasks"));
        assert!(SETUP_COMPLETE_CMD.contains("TestAnywareAgent"));
        assert!(DESKTOP_SETUP_PS1.contains("desktop-setup-done.txt"));
        assert!(STARTUP_NSH.contains("bootaa64.efi"));
    }
}
