//! End-to-end VM lifecycle orchestrator. Port of `VMLifecycle.swift`
//! (QEMU paths only; tart is a separate backlog task).

use std::path::PathBuf;
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;

use crate::error::VmError;
use crate::health::wait_for_agent;
#[cfg(target_os = "macos")]
use crate::health::wait_for_agent_with_oracle;
use crate::id::generate_id;
use crate::meta::{VmMeta, VmTool};
use crate::paths::VmPaths;
use crate::process::{child_alive, process_alive};
use crate::qemu::{
    scan_clones_dir, scan_golden_dir, GoldenImage, QemuRunner, QemuStartOptions, RunningClone,
    StartArtifacts,
};
use crate::spec::{AgentEndpoint, LogicalSize, VmSpec, VncEndpoint};
use crate::qemu_profile::host_profile;
use crate::start_outcome::{
    decide_start, read_log_tail, AddressObservation, AgentObservation, Backend, BootObservation,
    Disposition, ProcessState, StartDecision, VncObservation,
};
use crate::tpm::{decide_tpm_on_host, TpmDecision};

/// Guest platform. Ports the `Platform` enum + extension in `VMTypes.swift`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Linux,
    Windows,
}

impl Platform {
    /// Parse a `--platform` string. Errors with `InvalidPlatform`.
    pub fn parse(value: &str) -> Result<Self, VmError> {
        match value {
            "macos" => Ok(Platform::Macos),
            "linux" => Ok(Platform::Linux),
            "windows" => Ok(Platform::Windows),
            other => Err(VmError::InvalidPlatform { value: other.to_string() }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Macos => "macos",
            Platform::Linux => "linux",
            Platform::Windows => "windows",
        }
    }

    /// Default golden-image name. Ports `Platform.defaultBase`.
    pub fn default_base(self) -> &'static str {
        match self {
            Platform::Macos => "testanyware-golden-macos-27",
            Platform::Linux => "testanyware-golden-linux-24.04",
            Platform::Windows => "testanyware-golden-windows-11",
        }
    }
}

/// Inputs for `VmLifecycle::start`. Ports `VMStartOptions`.
#[derive(Debug, Clone)]
pub struct VmStartOptions {
    pub platform: Platform,
    pub base: String,
    pub id: String,
    /// The backend display value (`tart set --display` / QEMU). Never carries
    /// `@2x` — under HiDPI it is the translated `WxHpt` (ADR-0016 D3).
    pub display: Option<String>,
    /// The HiDPI logical target `(width, height)`, set only by a `@2x` opt-in
    /// (ADR-0016 D2/D3). Drives the guest-side 2× mode switch and is persisted
    /// into the spec so later command connections present the logical surface.
    pub logical: Option<(u32, u32)>,
    pub open_viewer: bool,
    /// On a failed start, leave a still-running VM (with its sidecars) for
    /// inspection instead of tearing it down (`--keep-on-failure`). Only a VM
    /// whose backend process is still alive can be kept.
    pub keep_on_failure: bool,
}

impl VmStartOptions {
    /// Build start options from a parsed [`DisplayRequest`](crate::display_request::DisplayRequest):
    /// the backend display value and the HiDPI logical target are split apart
    /// by the `--display` parser, so the `@2x` translation never leaks past the
    /// CLI boundary.
    pub fn new(
        platform: Platform,
        base: Option<String>,
        id: Option<String>,
        display: crate::display_request::DisplayRequest,
        open_viewer: bool,
    ) -> Self {
        Self {
            platform,
            base: base.unwrap_or_else(|| platform.default_base().to_string()),
            id: id.unwrap_or_else(generate_id),
            display: display.backend_display,
            logical: display.logical,
            open_viewer,
            keep_on_failure: false,
        }
    }
}

/// Result of a successful `VmLifecycle::start`.
#[derive(Debug, Clone)]
pub struct VmStartResult {
    pub id: String,
    pub platform: Platform,
    pub spec: VmSpec,
    pub spec_path: PathBuf,
    pub meta_path: PathBuf,
    /// The TPM decision this start was made under. `None` for a tart-backed
    /// start, which has no such decision to report.
    pub tpm: Option<TpmDecision>,
}

/// What `vm start --dry-run` established without side effects.
#[derive(Debug, Clone, Copy)]
pub struct StartPlan {
    /// The backend `start` would route to (`"tart"` or `"qemu"`).
    pub backend: &'static str,
    /// The TPM decision `start` would be made under; `None` on the tart backend.
    pub tpm: Option<TpmDecision>,
}

/// A running clone enriched from its spec/meta sidecars.
#[derive(Debug, Clone)]
pub struct RunningEntry {
    pub id: String,
    pub platform: String,
    pub backend: &'static str,
    pub pid: Option<i32>,
    pub vnc: Option<String>,
    pub agent: Option<String>,
}

/// `vm list` output: goldens + running clones.
#[derive(Debug, Clone, Default)]
pub struct VmListing {
    pub goldens: Vec<GoldenImage>,
    pub running: Vec<RunningEntry>,
}

/// Lifecycle entry points.
pub struct VmLifecycle;

impl VmLifecycle {
    /// Start a VM end-to-end, routing to the backend that serves the
    /// requested platform + golden on this host. macOS guests, and Linux
    /// guests whose base is a tart golden, use the tart backend (macOS
    /// host only); Linux-on-qcow2 and Windows use QEMU. Ports
    /// `VMLifecycle.start` (backend dispatch) + `startQEMU` / `startTart`.
    pub async fn start(opts: &VmStartOptions, paths: &VmPaths) -> Result<VmStartResult, VmError> {
        std::fs::create_dir_all(paths.vms_dir())
            .map_err(|e| VmError::Io(format!("create {}: {e}", paths.vms_dir().display())))?;

        #[cfg(target_os = "macos")]
        if wants_tart(opts.platform, &opts.base, paths) {
            return Self::start_tart(opts, paths).await;
        }

        // No tart backend reachable here: a macOS guest cannot be served.
        if opts.platform == Platform::Macos {
            return Err(VmError::BackendUnsupported { platform: "macos".into() });
        }

        // The one TPM decision, before anything is cloned or booted. The host
        // gate comes first so a Windows host reports `VM_HOST_UNSUPPORTED`
        // rather than failing to run the probe.
        crate::preflight::check_host_supports_local_qemu()?;
        let tpm = decide_tpm_on_host(opts.platform, &host_profile())?;
        Self::start_qemu(opts, paths, tpm).await
    }

    /// Start a QEMU-backed VM (Windows, Linux-on-qcow2) end-to-end. The runner
    /// clones, launches and discovers the VNC and agent ports; this method
    /// then waits for the agent — noticing at once if QEMU dies — and lets
    /// [`decide_start`] rule on what it saw, the same rule the tart arm
    /// follows. A start succeeds only with a Healthy agent; any other outcome
    /// is an error and the VM is torn down unless `keep_on_failure` keeps a
    /// still-running one. `tpm` is the decision `start` made for this guest.
    /// Ports `VMLifecycle.startQEMU`.
    async fn start_qemu(
        opts: &VmStartOptions,
        paths: &VmPaths,
        tpm: TpmDecision,
    ) -> Result<VmStartResult, VmError> {
        let qopts = QemuStartOptions {
            id: opts.id.clone(),
            base: opts.base.clone(),
            display: opts.display.clone(),
            tpm,
        };
        // A start in progress invalidates whatever this id's sidecars described,
        // so a launch that fails early cannot leave them pointing at a VM that
        // `QemuRunner::start` has just reclaimed.
        clear_sidecars(paths, &opts.id);
        // A failed launch has already torn its clone down.
        let artifacts = QemuRunner::start(&qopts, paths).await?;
        let pid = artifacts.pid;
        let observation =
            observe_qemu_boot(opts, &artifacts, move || child_alive(pid), QEMU_AGENT_POLLS).await;

        if let StartDecision::Failed { mut error, disposition } =
            decide_start(&observation, opts.keep_on_failure)
        {
            let vm = qemu_failed_vm(opts, &artifacts);
            let left_behind = settle_failed_start(
                paths,
                &vm,
                disposition,
                || QemuRunner::stop(artifacts.pid, &artifacts.clone_dir, paths),
                || artifacts.clone_dir.exists(),
            );
            record_left_behind(&mut error, left_behind);
            return Err(error);
        }

        let spec = qemu_spec(opts, &artifacts);
        let spec_path = paths.spec_path(&opts.id);
        let meta_path = paths.meta_path(&opts.id);
        // QEMU is now running; a sidecar-write failure must tear it down so it
        // cannot survive as an unkillable orphan.
        spec.write_atomic(&spec_path).inspect_err(|_| {
            QemuRunner::stop(artifacts.pid, &artifacts.clone_dir, paths);
        })?;

        let meta = qemu_meta(opts, &artifacts);
        meta.write_atomic(&meta_path).inspect_err(|_| {
            QemuRunner::stop(artifacts.pid, &artifacts.clone_dir, paths);
            let _ = std::fs::remove_file(&spec_path);
        })?;

        Ok(VmStartResult {
            id: opts.id.clone(),
            platform: opts.platform,
            spec,
            spec_path,
            meta_path,
            tpm: Some(tpm),
        })
    }

    /// Start a tart-backed VM end-to-end. macOS host only. The runner clones
    /// and launches; this method observes each readiness stage (VNC URL, guest
    /// IP, agent health — the agent listens on the guest IP, not a localhost
    /// forward), lets [`decide_start`] rule on what it saw, and only then
    /// writes the sidecars. A start succeeds only with a Healthy agent; any
    /// other outcome is an error and the VM is torn down unless
    /// `keep_on_failure` keeps a still-running one. Ports
    /// `VMLifecycle.startTart`.
    #[cfg(target_os = "macos")]
    async fn start_tart(opts: &VmStartOptions, paths: &VmPaths) -> Result<VmStartResult, VmError> {
        use crate::tart::{TartLaunch, TartRunner, TartStartOptions};

        let topts = TartStartOptions {
            id: opts.id.clone(),
            base: opts.base.clone(),
            display: opts.display.clone(),
        };
        // A failed launch has already removed its clone.
        let TartLaunch { pid, log_path } = TartRunner::launch(&topts, paths).await?;
        let boot = observe_tart_boot(opts, paths, pid, &log_path).await;

        let (vnc, ip) = match decide_start(&boot.observation, opts.keep_on_failure) {
            StartDecision::Ready => match (boot.vnc.clone(), boot.ip.clone()) {
                (Some(vnc), Some(ip)) => (vnc, ip),
                // Ready implies both stages passed; refuse to invent an endpoint.
                _ => {
                    TartRunner::stop(&opts.id, pid);
                    return Err(VmError::TartFailed {
                        detail: "internal error: ready start without a VNC endpoint and guest IP".into(),
                    });
                }
            },
            StartDecision::Failed { mut error, disposition } => {
                let left_behind = settle_failed_start(
                    paths,
                    &tart_failed_vm(opts, pid, &boot),
                    disposition,
                    || {
                        TartRunner::stop(&opts.id, pid);
                    },
                    || crate::tart::vm_exists(&opts.id),
                );
                record_left_behind(&mut error, left_behind);
                return Err(error);
            }
        };

        // macOS guests are guest-controlled (ADR-0014): the agent is up
        // but the framebuffer still shows the golden's 1024×768. Force it
        // to the resolved mode now — synchronously, before `vm start`
        // returns success — so consumers waiting on `vm start` are gated
        // on the switch by construction. The agent is known healthy here;
        // the switch itself stays best-effort: failures warn and leave the VM
        // started. Linux-on-tart honors the host-configured mode directly, so
        // the guest-side switch is macOS-only.
        if opts.platform == Platform::Macos {
            match opts.logical {
                // HiDPI @2x (ADR-0016 D3): select the guest's 2× Retina
                // mode for the logical target (k4 finding 3) — *replacing*
                // ADR-0014's 1× switch, not skipping it. At 2× the guest
                // advertises BOTH a 1× and a 2× 1920-logical mode, so the
                // old 1× selector would match the 1× mode and silently
                // defeat HiDPI (k4 finding 4); suppression is load-bearing.
                Some((lw, lh)) => {
                    let applied = crate::display::apply(&ip, 8648, (lw, lh), 2, paths, &opts.id).await;
                    // No 2× mode advertised ⇒ a 1× host (or HiDPI did not
                    // engage). Fall back to the 1× mode of the same logical
                    // size so the guest still reaches a sane 1920×1080
                    // (the connection auto-detects scale=1 and warns —
                    // ADR-0016 "the host yielded 1×"). Safe vs k4 finding 4:
                    // this runs only when no 2× mode exists, so it cannot
                    // defeat HiDPI on a real Retina host (where the 2×
                    // switch succeeds first and skips this).
                    if !applied {
                        eprintln!(
                            "  HiDPI 2× switch did not apply — falling back to \
                             1× {lw}x{lh} (HiDPI needs a Retina host / the \
                             deferred deterministic mechanism, ADR-0016)"
                        );
                        crate::display::apply(&ip, 8648, (lw, lh), 1, paths, &opts.id).await;
                    }
                }
                // Default 1× path (ADR-0014): force the 1920×1080-px
                // contract via the 1× mode selector.
                None => {
                    let resolved = crate::tart::resolve_display(opts.display.as_deref());
                    match crate::display::parse_target(resolved) {
                        Some(target) => {
                            crate::display::apply(&ip, 8648, target, 1, paths, &opts.id).await;
                        }
                        None => eprintln!(
                            "  warning: cannot derive a px target from display \
                             '{resolved}' — skipping macOS resolution switch"
                        ),
                    }
                }
            }
        }

        let spec = VmSpec {
            vnc: VncEndpoint { host: vnc.host.clone(), port: vnc.port, password: vnc.password.clone() },
            agent: Some(AgentEndpoint { host: ip, port: 8648 }),
            platform: opts.platform.as_str().to_string(),
            // Persist the HiDPI logical surface so every later short-lived
            // command connection presents it (ADR-0016 D2; ADR-0004 per-command
            // model). `None` on the default 1× path.
            logical: opts.logical.map(|(w, h)| LogicalSize { width: w, height: h }),
        };
        let spec_path = paths.spec_path(&opts.id);
        let meta_path = paths.meta_path(&opts.id);
        // tart is now running; a sidecar-write failure must tear it down
        // so it cannot survive untracked.
        spec.write_atomic(&spec_path).inspect_err(|_| {
            TartRunner::stop(&opts.id, pid);
        })?;

        // tart manages its own storage, so the meta carries no clone_dir;
        // `pid` is the detached `tart run` process tracked for `stop`.
        let meta = VmMeta {
            id: opts.id.clone(),
            tool: VmTool::Tart,
            pid,
            clone_dir: None,
            viewer_window_id: None,
            // The keeper started in `observe_tart_boot`, before any VNC
            // client (ADR-0018). `vm stop` signals it as a backstop.
            keeper_pid: boot.keeper_pid,
        };
        meta.write_atomic(&meta_path).inspect_err(|_| {
            TartRunner::stop(&opts.id, pid);
            let _ = std::fs::remove_file(&spec_path);
        })?;

        Ok(VmStartResult {
            id: opts.id.clone(),
            platform: opts.platform,
            spec,
            spec_path,
            meta_path,
            tpm: None,
        })
    }

    /// Stop a VM and remove its sidecars. Ports `VMLifecycle.stop`
    /// (QEMU branch). A `tart` meta returns `BackendUnsupported`.
    pub fn stop(id: &str, paths: &VmPaths) -> Result<(), VmError> {
        let spec_path = paths.spec_path(id);
        let meta_path = paths.meta_path(id);
        if !meta_path.is_file() {
            return Err(VmError::VmNotFound { id: id.to_string() });
        }
        let meta = VmMeta::load(&meta_path)?;
        // A corrupt qemu meta (no clone_dir) leaves nothing to tear down,
        // but the sidecars must still be removed so `vm stop` is
        // self-healing — mirrors Swift `VMLifecycle.stop`, which sets
        // `ok = false`, removes both sidecars, then throws `stopFailed`.
        let stop_error = match meta.tool {
            #[cfg(target_os = "macos")]
            VmTool::Tart => {
                // `tart stop` + `tart delete` the clone, SIGTERM the
                // detached `tart run` pid. A VM that no longer exists is a
                // stop failure (mirrors Swift `stop`'s `ok = false`).
                let stopped = crate::tart::TartRunner::stop(id, meta.pid);
                // The keeper ends itself once tart is gone (ADR-0018); this
                // signal is the backstop, and only reaches a pid that is
                // still this id's keeper.
                crate::keeper::stop_keeper(id, meta.keeper_pid);
                if stopped {
                    None
                } else {
                    Some(VmError::VmStopFailed { id: id.to_string() })
                }
            }
            // A tart meta on a non-macOS host is unreachable in practice
            // (tart never ran here) but must still compile.
            #[cfg(not(target_os = "macos"))]
            VmTool::Tart => Some(VmError::BackendUnsupported { platform: "macos (tart)".into() }),
            VmTool::Qemu => match meta.clone_dir.as_deref().filter(|d| !d.is_empty()) {
                Some(clone_dir) => {
                    QemuRunner::stop(meta.pid, std::path::Path::new(clone_dir), paths);
                    None
                }
                None => Some(VmError::VmStopFailed { id: id.to_string() }),
            },
        };
        let _ = std::fs::remove_file(&spec_path);
        let _ = std::fs::remove_file(&meta_path);
        match stop_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// Delete a QEMU golden image by name. Refuses when running clones
    /// depend on it unless `force`. Ports `VMLifecycle.delete` (QEMU
    /// branch; tart detection is backlog task 12).
    pub fn delete(name: &str, force: bool, paths: &VmPaths) -> Result<(), VmError> {
        let golden_dir = paths.golden_dir();
        let qcow2 = golden_dir.join(format!("{name}.qcow2"));

        // A tart golden (macOS) takes precedence only when no qcow2 of the
        // same name exists — QEMU stays authoritative for its own images.
        #[cfg(target_os = "macos")]
        if !qcow2.is_file() && crate::tart::golden_exists(name) {
            if !force {
                let running = crate::tart::list_running_ids();
                if !running.is_empty() {
                    // tart cannot cheaply prove which clones back this
                    // image; warn via the in-use error (no pids) unless forced.
                    return Err(VmError::GoldenInUse { name: name.to_string(), clone_pids: vec![] });
                }
            }
            if !crate::tart::delete_golden(name) {
                return Err(VmError::TartFailed { detail: format!("tart delete {name} failed") });
            }
            return Ok(());
        }

        if !qcow2.is_file() {
            return Err(VmError::GoldenNotFound { name: name.to_string() });
        }
        if !force {
            let pids = QemuRunner::running_clones_backed_by(name, paths);
            if !pids.is_empty() {
                return Err(VmError::GoldenInUse { name: name.to_string(), clone_pids: pids });
            }
        }
        QemuRunner::delete_golden(name, &golden_dir);
        Ok(())
    }

    /// Validate a `vm start --dry-run` without side effects: confirm the
    /// golden exists in whichever backend `start` would route to, and run
    /// the host preflight for the QEMU path. Returns the backend and the TPM
    /// decision so the caller can report them. Keeps the per-platform routing (and the
    /// macOS-only tart lookups) inside the crate, off the command layer.
    pub fn dry_run_validate_start(opts: &VmStartOptions, paths: &VmPaths) -> Result<StartPlan, VmError> {
        #[cfg(target_os = "macos")]
        if wants_tart(opts.platform, &opts.base, paths) {
            if !crate::tart::golden_exists(&opts.base) {
                return Err(VmError::GoldenNotFound { name: opts.base.clone() });
            }
            return Ok(StartPlan { backend: "tart", tpm: None });
        }

        if opts.platform == Platform::Macos {
            return Err(VmError::BackendUnsupported { platform: "macos".into() });
        }
        // Same host boundary as the real `start`: a Windows host cannot run
        // the local-QEMU path, so `--dry-run` reports it too (build-verified
        // only, ADR-0009) rather than validating a launch that cannot run.
        crate::preflight::check_host_supports_local_qemu()?;
        let qcow2 = paths.golden_dir().join(format!("{}.qcow2", opts.base));
        if !qcow2.is_file() {
            return Err(VmError::GoldenNotFound { name: opts.base.clone() });
        }
        crate::preflight::check_kvm()?;
        let tpm = decide_tpm_on_host(opts.platform, &host_profile())?;
        Ok(StartPlan { backend: "qemu", tpm: Some(tpm) })
    }

    /// Validate a `vm delete --dry-run`: the golden must exist as a QEMU
    /// qcow2 or (macOS) a tart golden. Mirrors the backend detection in
    /// `delete`.
    pub fn dry_run_validate_delete(name: &str, paths: &VmPaths) -> Result<(), VmError> {
        if paths.golden_dir().join(format!("{name}.qcow2")).is_file() {
            return Ok(());
        }
        #[cfg(target_os = "macos")]
        if crate::tart::golden_exists(name) {
            return Ok(());
        }
        Err(VmError::GoldenNotFound { name: name.to_string() })
    }

    /// List goldens and running clones. Ports `VMCommand.List` (QEMU
    /// entries only). Running clones are enriched from their sidecars.
    pub fn list(paths: &VmPaths) -> VmListing {
        let goldens = scan_golden_dir(&paths.golden_dir());
        let raw: Vec<RunningClone> =
            scan_clones_dir(&paths.clones_dir(), &session_root(paths));
        let running: Vec<RunningEntry> = raw
            .into_iter()
            .map(|clone| enrich_running(&clone, paths))
            .collect();

        // tart goldens + running clones (macOS host only). A running tart
        // clone is enriched from its sidecars exactly like a QEMU clone —
        // the spec carries vnc/agent/platform, the meta carries the pid.
        // Shadowed inside the cfg so the non-macOS build needs no `mut`.
        #[cfg(target_os = "macos")]
        let (goldens, running) = {
            let mut goldens = goldens;
            let mut running = running;
            goldens.extend(crate::tart::list_goldens());
            for id in crate::tart::list_running_ids() {
                let clone = RunningClone { id, platform: "unknown".into(), backend: "tart" };
                running.push(enrich_running(&clone, paths));
            }
            (goldens, running)
        };

        VmListing { goldens, running }
    }
}

/// Whether to route this start to the tart backend (macOS host only).
/// macOS guests always use tart. A Linux guest uses tart only as a
/// fallback: a same-named QEMU qcow2 wins, otherwise a tart golden of
/// that name is used (the kept-built `testanyware-golden-linux-24.04` is
/// a tart VM). Windows always uses QEMU.
#[cfg(target_os = "macos")]
fn wants_tart(platform: Platform, base: &str, paths: &VmPaths) -> bool {
    match platform {
        Platform::Macos => true,
        Platform::Windows => false,
        Platform::Linux => {
            let qcow2 = paths.golden_dir().join(format!("{base}.qcow2"));
            !qcow2.is_file() && crate::tart::vm_exists(base)
        }
    }
}

/// Readiness-stage budgets for a tart boot. The agent budget is the same 60
/// probes 2 s apart `vm start` has always used; this grove's job is to make
/// its failure honest, not to retune it.
#[cfg(target_os = "macos")]
const VNC_POLLS: (u32, Duration) = (60, Duration::from_secs(1));
#[cfg(target_os = "macos")]
const IP_POLLS: (u32, Duration) = (30, Duration::from_secs(2));
#[cfg(target_os = "macos")]
const AGENT_POLLS: (u32, Duration) = (60, Duration::from_secs(2));
/// The agent's fixed port in the guest.
#[cfg(target_os = "macos")]
const AGENT_PORT: u16 = 8648;
/// QEMU's agent wait: 120 probes 5 s apart. That is ten minutes when probes
/// fail fast and about fourteen when each also spends its 2 s request timeout
/// (observed live: 835 s). Deliberately long because Windows boots slowly; a
/// QEMU that *dies* ends the wait at once, so the long window costs nothing on
/// a failed launch.
const QEMU_AGENT_POLLS: (u32, Duration) = (120, Duration::from_secs(5));

/// What a tart boot observed, plus the endpoints it found along the way.
#[cfg(target_os = "macos")]
struct TartBoot {
    observation: BootObservation,
    vnc: Option<crate::tart::VncUrl>,
    ip: Option<String>,
    /// Pid of the VNC keeper (ADR-0018), or `None` if it could not start.
    keeper_pid: Option<i32>,
}

/// Run the readiness stages in order — VNC URL, guest IP, agent health —
/// stopping at the first that fails, and record what was seen. Each stage
/// notices a dead `tart run` at once. The process is sampled last, so a death
/// during any stage is what the decision sees.
#[cfg(target_os = "macos")]
async fn observe_tart_boot(
    opts: &VmStartOptions,
    paths: &VmPaths,
    pid: i32,
    log_path: &std::path::Path,
) -> TartBoot {
    use crate::tart::{await_guest_ip, await_vnc_url, Polled};

    let alive = move || child_alive(pid);
    let mut observation = BootObservation {
        id: opts.id.clone(),
        backend: Backend::Tart,
        platform: opts.platform,
        process: ProcessState::Running,
        vnc: VncObservation::Found,
        address: AddressObservation::NotReached,
        agent: AgentObservation::NotReached,
    };
    let mut vnc = None;
    let mut ip = None;
    let mut keeper_pid = None;

    let stage_start = Instant::now();
    match await_vnc_url(log_path, VNC_POLLS.0, VNC_POLLS.1, alive).await {
        Polled::Found(url) => {
            // Hold one silent VNC connection for the life of tart (ADR-0018),
            // so a client that connects after a pause of 60 s or more cannot
            // crash it on a macOS 27 host. Start it as soon as tart prints the
            // URL, before the first VNC client of any kind. A failure warns;
            // the guest then runs without the protection.
            keeper_pid = crate::keeper::start_keeper(&opts.id, &url.host, url.port, &paths.vms_dir());
            vnc = Some(url);
        }
        Polled::ProcessExited => {}
        Polled::TimedOut { attempts } => {
            observation.vnc = VncObservation::Missing { attempts, elapsed: stage_start.elapsed() };
        }
    }

    if vnc.is_some() {
        let stage_start = Instant::now();
        match await_guest_ip(&opts.id, IP_POLLS.0, IP_POLLS.1, alive).await {
            Polled::Found(found) => {
                observation.address = AddressObservation::Found;
                ip = Some(found);
            }
            Polled::ProcessExited => {}
            Polled::TimedOut { attempts } => {
                observation.address =
                    AddressObservation::Missing { attempts, elapsed: stage_start.elapsed() };
            }
        }
    }

    if let Some(ip) = &ip {
        // The guest is on a host-only subnet, so macOS Local Network privacy may
        // deny this process while the agent is healthy (ADR-0019).
        let wait = wait_for_agent_with_oracle(ip, AGENT_PORT, AGENT_POLLS.0, AGENT_POLLS.1, alive, || {
            crate::local_network::platform_tool_reaches(ip, AGENT_PORT)
        })
        .await;
        observation.agent = AgentObservation::Waited {
            endpoint: format!("http://{ip}:{AGENT_PORT}/health"),
            wait,
        };
    }

    if !child_alive(pid) {
        observation.process = ProcessState::Exited { log_tail: read_log_tail(log_path) };
    }
    TartBoot { observation, vnc, ip, keeper_pid }
}

/// Wait for a launched QEMU's agent and record what was seen. The VNC and
/// agent-forward stages already passed (`QemuRunner::start` fails on its own
/// if the monitor cannot name either port), so only the agent wait and the
/// final process sample remain. The process is sampled last, so a death during
/// the wait is what the decision sees — and, as in the tart arm, its log tail
/// is read here, before any teardown deletes the clone directory that holds it.
///
/// `alive` and `agent_polls` are injected so the wait is testable without QEMU.
async fn observe_qemu_boot(
    opts: &VmStartOptions,
    artifacts: &StartArtifacts,
    mut alive: impl FnMut() -> bool,
    agent_polls: (u32, Duration),
) -> BootObservation {
    let port = artifacts.agent_port;
    let wait = wait_for_agent("localhost", port, agent_polls.0, agent_polls.1, &mut alive).await;
    let process = if alive() {
        ProcessState::Running
    } else {
        ProcessState::Exited { log_tail: read_log_tail(&artifacts.log_path) }
    };
    BootObservation {
        id: opts.id.clone(),
        backend: Backend::Qemu,
        platform: opts.platform,
        process,
        vnc: VncObservation::Found,
        address: AddressObservation::Found,
        agent: AgentObservation::Waited {
            endpoint: format!("http://localhost:{port}/health"),
            wait,
        },
    }
}

/// Remove an id's spec and meta sidecars; a missing file is the normal case.
pub(crate) fn clear_sidecars(paths: &VmPaths, id: &str) {
    let _ = std::fs::remove_file(paths.spec_path(id));
    let _ = std::fs::remove_file(paths.meta_path(id));
}

/// What settling a failed start needs to know about the backend's VM. Each
/// backend builds one; [`settle_failed_start`] does the rest.
struct FailedVm {
    id: String,
    /// What `vm stop` needs to find the VM again.
    meta: VmMeta,
    /// The spec to leave beside a kept VM; `None` until a VNC endpoint is known.
    spec: Option<VmSpec>,
    /// How to remove the VM by hand, quoted in the warning when teardown fails.
    manual_reclaim: String,
}

/// The tart VM of a failed start, as far as the boot got.
#[cfg(target_os = "macos")]
fn tart_failed_vm(opts: &VmStartOptions, pid: i32, boot: &TartBoot) -> FailedVm {
    FailedVm {
        id: opts.id.clone(),
        meta: VmMeta {
            id: opts.id.clone(),
            tool: VmTool::Tart,
            pid,
            clone_dir: None,
            viewer_window_id: None,
            // A kept VM keeps its keeper; `vm stop` signals it later.
            keeper_pid: boot.keeper_pid,
        },
        spec: boot.vnc.as_ref().map(|vnc| VmSpec {
            vnc: VncEndpoint {
                host: vnc.host.clone(),
                port: vnc.port,
                password: vnc.password.clone(),
            },
            agent: boot
                .ip
                .as_ref()
                .map(|ip| AgentEndpoint { host: ip.clone(), port: AGENT_PORT }),
            platform: opts.platform.as_str().to_string(),
            logical: opts.logical.map(|(w, h)| LogicalSize { width: w, height: h }),
        }),
        manual_reclaim: format!("`tart stop {id}` and `tart delete {id}`", id = opts.id),
    }
}

/// The QEMU VM of a failed start. Its VNC port and agent forward are always
/// known (the runner refuses to return without them), so a kept QEMU VM always
/// has a spec naming the agent endpoint, healthy or not.
fn qemu_failed_vm(opts: &VmStartOptions, artifacts: &StartArtifacts) -> FailedVm {
    FailedVm {
        id: opts.id.clone(),
        meta: qemu_meta(opts, artifacts),
        spec: Some(qemu_spec(opts, artifacts)),
        manual_reclaim: format!(
            "kill process {} and remove {}",
            artifacts.pid,
            artifacts.clone_dir.display()
        ),
    }
}

/// The spec a QEMU start publishes: VNC and the agent forward, both on loopback.
fn qemu_spec(opts: &VmStartOptions, artifacts: &StartArtifacts) -> VmSpec {
    VmSpec {
        vnc: VncEndpoint {
            host: "localhost".into(),
            port: artifacts.vnc_port,
            password: Some("testanyware".into()),
        },
        agent: Some(AgentEndpoint { host: "localhost".into(), port: artifacts.agent_port }),
        platform: opts.platform.as_str().to_string(),
        // HiDPI is a macOS-VF concept (rejected for QEMU guests at the CLI
        // boundary), so this is always `None` here; carried uniformly so the
        // two backends build the spec identically.
        logical: opts.logical.map(|(w, h)| LogicalSize { width: w, height: h }),
    }
}

/// The meta `vm stop` needs to reclaim a QEMU VM.
fn qemu_meta(opts: &VmStartOptions, artifacts: &StartArtifacts) -> VmMeta {
    VmMeta {
        id: opts.id.clone(),
        tool: VmTool::Qemu,
        pid: artifacts.pid,
        clone_dir: Some(artifacts.clone_dir.display().to_string()),
        // `viewer_window_id` tracked the Swift CLI's *external* AppleScript
        // viewer window; the Rust embedded viewer (ADR-0005) owns its window
        // in-process, so this Swift-compat field is permanently null.
        viewer_window_id: None,
        keeper_pid: None,
    }
}

/// The decision predicts `kept` from the flag alone; settlement knows whether a
/// VM really remains for `vm stop` to reclaim.
fn record_left_behind(error: &mut VmError, left_behind: bool) {
    if let VmError::VmBootTimeout { timeout, .. } = error {
        timeout.kept = left_behind;
    }
}

/// Act on a failed start's disposition, for either backend. Returns whether a
/// VM was left behind for the caller to reclaim with `vm stop`.
///
/// `Teardown` stops and deletes the VM and, once it is really gone, removes
/// the sidecars. If the VM survives the teardown the meta is kept (or written)
/// so `vm stop` can retry, because the meta is the only handle it has.
/// `Keep` leaves the running VM: the meta always, the spec when the backend has
/// one to give (naming the agent endpoint if it is known, so commands against
/// the kept VM report the real connection error). A kept VM that cannot be
/// given its meta could never be reclaimed, so it is torn down instead.
///
/// `teardown` and `vm_exists` are the backend's own stop-and-delete and
/// "is it still there?", injected so the cleanup failure paths are testable
/// without tart or QEMU.
fn settle_failed_start(
    paths: &VmPaths,
    vm: &FailedVm,
    disposition: Disposition,
    teardown: impl FnOnce(),
    vm_exists: impl Fn() -> bool,
) -> bool {
    let spec_path = paths.spec_path(&vm.id);
    let meta_path = paths.meta_path(&vm.id);

    if disposition == Disposition::Keep {
        match vm.meta.write_atomic(&meta_path) {
            Ok(()) => {
                if let Some(spec) = &vm.spec {
                    if let Err(err) = spec.write_atomic(&spec_path) {
                        eprintln!("warning: could not write the kept VM's spec: {err}");
                    }
                }
                return true;
            }
            Err(err) => eprintln!(
                "warning: could not write the kept VM's meta, so `vm stop` could not reclaim it; tearing it down instead: {err}"
            ),
        }
    }

    teardown();
    if !vm_exists() {
        clear_sidecars(paths, &vm.id);
        return false;
    }
    // The VM survived the teardown: keep the handle `vm stop` needs.
    match vm.meta.write_atomic(&meta_path) {
        Ok(()) => {
            eprintln!(
                "warning: could not remove the failed start's VM; reclaim it with `testanyware vm stop {}`",
                vm.id
            );
            true
        }
        Err(err) => {
            eprintln!(
                "warning: could not remove the failed start's VM, and could not record it for `vm stop` ({err}); reclaim it by hand: {}",
                vm.manual_reclaim
            );
            false
        }
    }
}

/// The `$TMPDIR` root for socket session dirs — `session_dir` of any id
/// shares the same parent, so the parent of an arbitrary id's session dir
/// is that root.
fn session_root(paths: &VmPaths) -> PathBuf {
    paths
        .session_dir("_")
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Enrich a bare running clone with spec (vnc/agent) and meta (pid) data.
fn enrich_running(clone: &RunningClone, paths: &VmPaths) -> RunningEntry {
    let spec = VmSpec::load(&paths.spec_path(&clone.id)).ok();
    let meta = VmMeta::load(&paths.meta_path(&clone.id)).ok();
    let pid = meta.as_ref().map(|m| m.pid).filter(|p| process_alive(*p));
    let vnc = spec
        .as_ref()
        .map(|s| format!("{}:{}", s.vnc.host, s.vnc.port));
    let agent = spec
        .as_ref()
        .and_then(|s| s.agent.as_ref())
        .map(|a| format!("{}:{}", a.host, a.port));
    // The spec is authoritative for `platform`. A clone id is random hex
    // (`testanyware-<hex8>`) with no platform substring, so the
    // name-derived `clone.platform` is always "unknown" for clones — fall
    // back to it only when the spec sidecar is missing or unreadable.
    let platform = spec
        .as_ref()
        .map(|s| s.platform.clone())
        .unwrap_or_else(|| clone.platform.clone());
    RunningEntry {
        id: clone.id.clone(),
        platform,
        backend: clone.backend,
        pid,
        vnc,
        agent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn paths_in(dir: &std::path::Path) -> VmPaths {
        let mut env = HashMap::new();
        env.insert("XDG_STATE_HOME".into(), dir.join("state").display().to_string());
        env.insert("XDG_DATA_HOME".into(), dir.join("data").display().to_string());
        env.insert("TMPDIR".into(), dir.join("tmp").display().to_string());
        VmPaths::from_env(&env)
    }

    #[test]
    fn platform_parses_the_three_known_values() {
        assert_eq!(Platform::parse("linux").unwrap(), Platform::Linux);
        assert_eq!(Platform::parse("windows").unwrap(), Platform::Windows);
        assert_eq!(Platform::parse("macos").unwrap(), Platform::Macos);
        assert!(Platform::parse("bsd").is_err());
    }

    #[test]
    fn platform_default_base_matches_golden_naming() {
        assert_eq!(Platform::Linux.default_base(), "testanyware-golden-linux-24.04");
        assert_eq!(Platform::Windows.default_base(), "testanyware-golden-windows-11");
        assert_eq!(Platform::Macos.default_base(), "testanyware-golden-macos-27");
    }

    #[test]
    fn start_options_fill_in_base_and_id_defaults() {
        use crate::display_request::parse_display_request;
        let opts = VmStartOptions::new(
            Platform::Windows,
            None,
            None,
            parse_display_request(None).unwrap(),
            false,
        );
        assert_eq!(opts.base, "testanyware-golden-windows-11");
        assert!(opts.id.starts_with("testanyware-"));
        assert_eq!(opts.logical, None);
        let explicit = VmStartOptions::new(
            Platform::Linux,
            Some("custom-base".into()),
            Some("testanyware-fixedid".into()),
            parse_display_request(Some("800x600")).unwrap(),
            false,
        );
        assert_eq!(explicit.base, "custom-base");
        assert_eq!(explicit.id, "testanyware-fixedid");
        assert_eq!(explicit.display.as_deref(), Some("800x600"));
        assert_eq!(explicit.logical, None);
    }

    #[test]
    fn start_options_carry_the_hidpi_logical_target() {
        use crate::display_request::parse_display_request;
        let opts = VmStartOptions::new(
            Platform::Macos,
            None,
            None,
            parse_display_request(Some("1920x1080@2x")).unwrap(),
            false,
        );
        // The backend sees the translated pt value; @2x never leaks past here.
        assert_eq!(opts.display.as_deref(), Some("1920x1080pt"));
        assert_eq!(opts.logical, Some((1920, 1080)));
    }

    // On a macOS host the tart backend serves macOS guests, so this
    // rejection only holds where tart is unreachable (non-macOS). The
    // macOS tart `start` path is covered by the leaf's manual
    // `vm start --platform macos` verification, not a unit test (it
    // shells out to a real `tart`).
    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn start_rejects_the_macos_platform_as_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let opts = VmStartOptions::new(
            Platform::Macos,
            None,
            None,
            crate::display_request::DisplayRequest::default(),
            false,
        );
        let err = VmLifecycle::start(&opts, &paths_in(dir.path())).await.unwrap_err();
        assert!(matches!(err, VmError::BackendUnsupported { .. }));
    }

    #[test]
    fn delete_reports_golden_not_found_for_an_absent_image() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(paths.golden_dir()).unwrap();
        let err = VmLifecycle::delete("testanyware-golden-ghost", false, &paths).unwrap_err();
        assert!(matches!(err, VmError::GoldenNotFound { .. }));
    }

    #[test]
    fn stop_reports_vm_not_found_when_no_meta_exists() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        let err = VmLifecycle::stop("testanyware-ghost", &paths).unwrap_err();
        assert!(matches!(err, VmError::VmNotFound { .. }));
    }

    #[test]
    fn stop_removes_sidecars_even_when_clone_dir_is_missing() {
        // A corrupt qemu meta with no clone_dir: `stop` must surface
        // VmStopFailed AND still remove the sidecars, so a retry is not
        // permanently stuck (matches Swift `VMLifecycle.stop`).
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        let id = "testanyware-abcd1234";
        let meta = VmMeta {
            id: id.into(),
            tool: VmTool::Qemu,
            pid: 0,
            clone_dir: None,
            viewer_window_id: None,
            keeper_pid: None,
        };
        meta.write_atomic(&paths.meta_path(id)).unwrap();

        let err = VmLifecycle::stop(id, &paths).unwrap_err();
        assert!(matches!(err, VmError::VmStopFailed { .. }));
        assert!(
            !paths.meta_path(id).is_file(),
            "stop must remove the meta sidecar even on a corrupt-meta failure",
        );
    }

    #[test]
    fn list_returns_goldens_and_running_clones() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(paths.golden_dir()).unwrap();
        std::fs::write(paths.golden_dir().join("testanyware-golden-linux-24.04.qcow2"), b"x").unwrap();
        let listing = VmLifecycle::list(&paths);
        // Filter to QEMU: on a macOS host `list` also merges the real tart
        // catalog, so an exact total would depend on the dev machine.
        let qemu_goldens: Vec<_> = listing.goldens.iter().filter(|g| g.backend == "qemu").collect();
        assert_eq!(qemu_goldens.len(), 1);
        assert_eq!(qemu_goldens[0].name, "testanyware-golden-linux-24.04");
        assert!(listing.running.iter().all(|r| r.backend != "qemu"), "no qemu clones started");
    }

    #[test]
    fn list_running_clone_reports_platform_from_its_spec() {
        // A clone id is random hex (`testanyware-<hex8>`) and carries no
        // platform substring, so name-derived detection yields "unknown".
        // `list` must read the authoritative `platform` from the spec
        // sidecar — regression guard for the Task 19 live-smoke finding.
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let id = "testanyware-abcd1234";
        // A running clone = a clone dir plus a monitor.sock under the
        // session dir (this is how `scan_clones_dir` detects liveness).
        std::fs::create_dir_all(paths.clone_dir(id)).unwrap();
        std::fs::create_dir_all(paths.session_dir(id)).unwrap();
        std::fs::write(paths.session_dir(id).join("monitor.sock"), b"").unwrap();
        // The spec sidecar carries the real platform.
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        let spec = VmSpec {
            vnc: VncEndpoint { host: "localhost".into(), port: 5900, password: None },
            agent: None,
            platform: "windows".into(),
            logical: None,
        };
        spec.write_atomic(&paths.spec_path(id)).unwrap();

        let listing = VmLifecycle::list(&paths);
        // Locate this fixture's clone by id — `list` may also surface
        // ambient tart clones on a macOS host.
        let clone = listing
            .running
            .iter()
            .find(|r| r.id == id)
            .expect("the clone should be detected as running");
        assert_eq!(
            clone.platform, "windows",
            "platform must come from the spec, not the name-derived guess",
        );
    }

    // ---- settling a failed start (either backend) -------------------------

    mod settle {
        use super::*;
        use std::cell::Cell;

        const ID: &str = "testanyware-settle01";

        fn vm() -> FailedVm {
            FailedVm {
                id: ID.into(),
                meta: VmMeta {
                    id: ID.into(),
                    tool: VmTool::Tart,
                    pid: 4242,
                    clone_dir: None,
                    viewer_window_id: None,
                    keeper_pid: None,
                },
                spec: Some(VmSpec {
                    vnc: VncEndpoint {
                        host: "127.0.0.1".into(),
                        port: 5901,
                        password: Some("pw".into()),
                    },
                    agent: Some(AgentEndpoint { host: "192.168.64.9".into(), port: 8648 }),
                    platform: "macos".into(),
                    logical: None,
                }),
                manual_reclaim: "by hand".into(),
            }
        }

        /// Seed both sidecars so a settlement has something to remove or keep.
        fn seed(paths: &VmPaths) {
            std::fs::create_dir_all(paths.vms_dir()).unwrap();
            std::fs::write(paths.spec_path(ID), b"{}").unwrap();
            std::fs::write(paths.meta_path(ID), b"{}").unwrap();
        }

        #[test]
        fn teardown_that_removes_the_vm_removes_the_sidecars() {
            let dir = tempfile::tempdir().unwrap();
            let paths = paths_in(dir.path());
            seed(&paths);
            let stopped = Cell::new(false);
            let left = settle_failed_start(
                &paths,
                &vm(),
                Disposition::Teardown,
                || stopped.set(true),
                || !stopped.get(),
            );
            assert!(!left);
            assert!(!paths.spec_path(ID).exists() && !paths.meta_path(ID).exists());
        }

        #[test]
        fn teardown_that_leaves_the_vm_keeps_a_meta_for_vm_stop() {
            let dir = tempfile::tempdir().unwrap();
            let paths = paths_in(dir.path());
            // No sidecars on disk: settlement must write the handle itself.
            std::fs::create_dir_all(paths.vms_dir()).unwrap();
            let left = settle_failed_start(&paths, &vm(), Disposition::Teardown, || {}, || true);
            assert!(left, "a surviving VM must be reported as left behind");
            let meta = VmMeta::load(&paths.meta_path(ID)).expect("meta must survive for `vm stop`");
            assert_eq!((meta.id.as_str(), meta.pid, meta.tool), (ID, 4242, VmTool::Tart));
        }

        #[test]
        fn surviving_vm_with_an_unwritable_meta_is_not_claimed_reclaimable() {
            let dir = tempfile::tempdir().unwrap();
            let paths = paths_in(dir.path());
            // A regular file where the `vms` directory should be: no sidecar can be written.
            std::fs::create_dir_all(paths.vms_dir().parent().unwrap()).unwrap();
            std::fs::write(paths.vms_dir(), b"").unwrap();
            let left = settle_failed_start(&paths, &vm(), Disposition::Teardown, || {}, || true);
            assert!(!left, "`vm stop` cannot find a VM with no meta");
        }

        #[test]
        fn keep_writes_the_meta_and_the_spec_and_tears_nothing_down() {
            let dir = tempfile::tempdir().unwrap();
            let paths = paths_in(dir.path());
            std::fs::create_dir_all(paths.vms_dir()).unwrap();
            let left = settle_failed_start(
                &paths,
                &vm(),
                Disposition::Keep,
                || panic!("a kept VM must not be torn down"),
                || true,
            );
            assert!(left);
            assert_eq!(VmMeta::load(&paths.meta_path(ID)).unwrap().pid, 4242);
            let spec = VmSpec::load(&paths.spec_path(ID)).unwrap();
            assert_eq!(spec.agent.unwrap().host, "192.168.64.9");
        }

        #[test]
        fn keep_without_a_spec_still_writes_the_meta() {
            let dir = tempfile::tempdir().unwrap();
            let paths = paths_in(dir.path());
            std::fs::create_dir_all(paths.vms_dir()).unwrap();
            let early = FailedVm { spec: None, ..vm() };
            let left = settle_failed_start(&paths, &early, Disposition::Keep, || {}, || true);
            assert!(left);
            assert!(paths.meta_path(ID).exists());
            assert!(!paths.spec_path(ID).exists(), "a spec cannot exist without a VNC endpoint");
        }

        #[test]
        fn keep_that_cannot_write_its_meta_tears_the_vm_down_instead() {
            let dir = tempfile::tempdir().unwrap();
            let paths = paths_in(dir.path());
            std::fs::create_dir_all(paths.vms_dir().parent().unwrap()).unwrap();
            std::fs::write(paths.vms_dir(), b"").unwrap();
            let stopped = Cell::new(false);
            let left = settle_failed_start(
                &paths,
                &vm(),
                Disposition::Keep,
                || stopped.set(true),
                || !stopped.get(),
            );
            assert!(stopped.get(), "an unreclaimable kept VM must be torn down");
            assert!(!left, "and the error must not claim it was kept");
        }

        #[test]
        fn clear_sidecars_removes_both_and_tolerates_absence() {
            let dir = tempfile::tempdir().unwrap();
            let paths = paths_in(dir.path());
            seed(&paths);
            clear_sidecars(&paths, ID);
            assert!(!paths.spec_path(ID).exists() && !paths.meta_path(ID).exists());
            clear_sidecars(&paths, ID);
        }

        #[cfg(target_os = "macos")]
        #[test]
        fn a_tart_vm_names_its_agent_endpoint_once_the_guest_ip_is_known() {
            use crate::start_outcome::{AddressObservation, AgentObservation, VncObservation};
            let opts = VmStartOptions {
                platform: Platform::Macos,
                base: "golden".into(),
                id: ID.into(),
                display: None,
                logical: None,
                open_viewer: false,
                keep_on_failure: false,
            };
            let mut boot = TartBoot {
                observation: BootObservation {
                    id: ID.into(),
                    backend: Backend::Tart,
                    platform: Platform::Macos,
                    process: ProcessState::Running,
                    vnc: VncObservation::Found,
                    address: AddressObservation::Found,
                    agent: AgentObservation::NotReached,
                },
                vnc: Some(crate::tart::VncUrl {
                    host: "127.0.0.1".into(),
                    port: 5901,
                    password: Some("pw".into()),
                }),
                ip: Some("192.168.64.9".into()),
                keeper_pid: None,
            };
            let failed = tart_failed_vm(&opts, 4242, &boot);
            assert_eq!(failed.meta.tool, VmTool::Tart);
            assert_eq!(failed.spec.unwrap().agent.unwrap().host, "192.168.64.9");
            boot.vnc = None;
            boot.ip = None;
            assert!(
                tart_failed_vm(&opts, 4242, &boot).spec.is_none(),
                "a spec cannot exist without a VNC endpoint"
            );
        }
    }

    // ---- the QEMU arm ------------------------------------------------------

    mod qemu_arm {
        use super::*;
        use crate::start_outcome::{decide_start, Disposition, StartDecision};
        use std::cell::Cell;
        use std::time::Instant;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        const ID: &str = "testanyware-qemu0001";

        fn opts(keep_on_failure: bool) -> VmStartOptions {
            VmStartOptions {
                platform: Platform::Windows,
                base: "golden".into(),
                id: ID.into(),
                display: None,
                logical: None,
                open_viewer: false,
                keep_on_failure,
            }
        }

        fn artifacts(agent_port: u16, log: &str) -> (tempfile::TempDir, StartArtifacts) {
            let dir = tempfile::tempdir().unwrap();
            let log_path = dir.path().join("qemu.log");
            std::fs::write(&log_path, log).unwrap();
            let artifacts = StartArtifacts {
                pid: 31337,
                vnc_port: 5905,
                agent_port,
                clone_dir: dir.path().join(ID),
                log_path,
            };
            (dir, artifacts)
        }

        /// A loopback port with nothing listening on it.
        fn dead_port() -> u16 {
            std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
        }

        fn failure(decision: StartDecision) -> (VmError, Disposition) {
            match decision {
                StartDecision::Failed { error, disposition } => (error, disposition),
                StartDecision::Ready => panic!("expected a failed start, got Ready"),
            }
        }

        #[tokio::test]
        async fn a_qemu_that_dies_during_the_agent_wait_ends_the_wait_at_once_with_its_log_tail() {
            let (_dir, art) = artifacts(dead_port(), "starting\nqemu: fatal: out of memory\n");
            // Alive for the first probe, dead before the second and after.
            let calls = Cell::new(0);
            let alive = || {
                calls.set(calls.get() + 1);
                calls.get() == 1
            };
            // A budget that would take ten minutes if the wait ignored the death.
            let began = Instant::now();
            let obs =
                observe_qemu_boot(&opts(true), &art, alive, (120, Duration::from_secs(5))).await;
            assert!(began.elapsed() < Duration::from_secs(4), "the wait must end at once");

            let (error, disposition) = failure(decide_start(&obs, true));
            assert_eq!(error.code(), "QEMU_FAILED");
            assert_eq!(
                error.details()["qemu_error"],
                "starting\nqemu: fatal: out of memory"
            );
            assert_eq!(disposition, Disposition::Teardown, "a dead QEMU has nothing to keep");
        }

        #[tokio::test]
        async fn a_qemu_that_is_up_but_whose_agent_never_answers_is_a_boot_timeout() {
            let port = dead_port();
            let (_dir, art) = artifacts(port, "");
            let obs =
                observe_qemu_boot(&opts(false), &art, || true, (3, Duration::from_millis(5))).await;

            let (error, disposition) = failure(decide_start(&obs, false));
            assert_eq!(error.code(), "VM_BOOT_TIMEOUT");
            let d = error.details();
            assert_eq!(d["stage"], "agent");
            assert_eq!(d["endpoint"], format!("http://localhost:{port}/health"));
            assert_eq!(d["attempts"], 3);
            assert_eq!(d["last_error"]["kind"], "connection_refused");
            assert_eq!(disposition, Disposition::Teardown);
            let (_, kept) = failure(decide_start(&obs, true));
            assert_eq!(kept, Disposition::Keep);
        }

        #[tokio::test]
        async fn a_qemu_with_a_healthy_agent_is_ready() {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/health"))
                .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "accessible": true,
                    "platform": "windows"
                })))
                .mount(&server)
                .await;
            let port: u16 = server.uri().rsplit(':').next().unwrap().parse().unwrap();
            let (_dir, art) = artifacts(port, "");
            let obs =
                observe_qemu_boot(&opts(false), &art, || true, (3, Duration::from_millis(5))).await;
            assert!(matches!(decide_start(&obs, false), StartDecision::Ready));
        }

        #[test]
        fn a_qemu_vm_is_reclaimable_by_vm_stop_and_names_its_agent_forward() {
            let (_dir, art) = artifacts(51234, "");
            let failed = qemu_failed_vm(&opts(true), &art);
            // `vm stop` needs the pid and the clone dir to tear a QEMU VM down.
            assert_eq!(failed.meta.tool, VmTool::Qemu);
            assert_eq!(failed.meta.pid, 31337);
            assert_eq!(
                failed.meta.clone_dir.as_deref(),
                Some(art.clone_dir.display().to_string().as_str())
            );
            let spec = failed.spec.expect("QEMU always knows its VNC port");
            assert_eq!(spec.vnc.port, 5905);
            let agent = spec.agent.expect("the forward is known, healthy or not");
            assert_eq!((agent.host.as_str(), agent.port), ("localhost", 51234));
            assert_eq!(spec.platform, "windows");
        }

        #[test]
        fn a_successful_qemu_start_publishes_the_same_spec_a_kept_vm_gets() {
            let (_dir, art) = artifacts(51234, "");
            assert_eq!(qemu_spec(&opts(false), &art), qemu_failed_vm(&opts(false), &art).spec.unwrap());
        }
    }
}
