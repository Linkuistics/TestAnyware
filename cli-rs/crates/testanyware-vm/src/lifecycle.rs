//! End-to-end VM lifecycle orchestrator. Port of `VMLifecycle.swift`
//! (QEMU paths only; tart is a separate backlog task).

use std::path::PathBuf;
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;

use crate::adoption::{decide, verified_pid, Adoption};
use crate::error::{StopUnproven, VmError};
use crate::health::wait_for_agent;
#[cfg(target_os = "macos")]
use crate::health::wait_for_agent_with_oracle;
use crate::id::generate_id;
use crate::identity::{read_identity, read_optional_identity, ProcessIdentity, ProcessProbe, SystemProbe};
use crate::legacy_stop::{
    find_session_dir, session_candidates, stop_through_sockets, LegacyStopProbes, SettleWait,
};
use crate::meta::{VmMeta, VmTool};
use crate::paths::VmPaths;
use crate::process::{child_alive, process_alive};
use crate::qemu::{
    scan_clones_dir, scan_golden_dir, GoldenImage, QemuRunner, QemuStartOptions, RunningClone,
    StartArtifacts,
};
use crate::spec::{AgentEndpoint, LogicalSize, VmSpec, VncEndpoint};
use crate::tools::ToolSearch;
use crate::qemu_profile::host_profile_in;
use crate::error::{AdmissionFacts, RefusedBy};
use crate::slot_gate::{Readmission, SlotAsk, SlotHold};
use crate::start_context::{Progress, StartContext};
#[cfg(target_os = "macos")]
use crate::tart::TartRunner;
#[cfg(target_os = "macos")]
use crate::start_outcome::{next_slot_step, SlotWaitReporter, SlotWaitStep, SLOT_RETRY_INTERVAL};
use crate::start_outcome::{
    decide_start, read_log_tail, AddressObservation, AgentObservation, Backend, BootObservation,
    Disposition, ProcessState, StartDecision, VncObservation,
};
use crate::tpm::{decide_tpm_on_host_in, TpmDecision};

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
    /// On a refusal by the macOS guest limit, retry until a slot frees or this
    /// long has passed, instead of failing at once (`--wait`). `None` fails at
    /// the first refusal. Only the tart backend can be refused this way.
    pub wait: Option<Duration>,
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
            wait: None,
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
    ///
    /// This is the Host CLI's own start: tools from `PATH`, progress on stderr, and a
    /// signal during `--wait` that cleans up. [`start_in`](Self::start_in) takes the
    /// context of another caller.
    pub async fn start(opts: &VmStartOptions, paths: &VmPaths) -> Result<VmStartResult, VmError> {
        Self::start_in(opts, &StartContext::direct(paths.clone())).await
    }

    /// [`start`](Self::start) on `ctx`. The TestAnyware app starts every VM of a macOS host
    /// this way, for a client that may go away: cancelling `ctx.cancel` ends the start at its
    /// next wait and cleans up as after a failed start, and then fails it with
    /// [`VmError::StartCancelled`].
    pub async fn start_in(opts: &VmStartOptions, ctx: &StartContext) -> Result<VmStartResult, VmError> {
        let paths = &ctx.paths;
        std::fs::create_dir_all(paths.vms_dir())
            .map_err(|e| VmError::Io(format!("create {}: {e}", paths.vms_dir().display())))?;

        #[cfg(target_os = "macos")]
        if wants_tart(opts.platform, &opts.base, paths, &ctx.tools) {
            return Self::start_tart(opts, ctx).await;
        }

        // No tart backend reachable here: a macOS guest cannot be served.
        if opts.platform == Platform::Macos {
            return Err(VmError::BackendUnsupported { platform: "macos".into() });
        }

        // The one TPM decision, before anything is cloned or booted. The host
        // gate comes first so a Windows host reports `VM_HOST_UNSUPPORTED`
        // rather than failing to run the probe.
        crate::preflight::check_host_supports_local_qemu()?;
        let tpm = decide_tpm_on_host_in(&ctx.tools, opts.platform, &host_profile_in(&ctx.tools))?;
        Self::start_qemu(opts, ctx, tpm).await
    }

    /// Start a QEMU-backed VM (Windows, Linux-on-qcow2) end-to-end. The runner
    /// clones, launches and discovers the VNC and agent ports; this method
    /// then waits for the agent — noticing at once if QEMU dies — and lets
    /// [`decide_start`] rule on what it saw, the same rule the tart arm
    /// follows. A start succeeds only with a Healthy agent; any other outcome
    /// is an error and the VM is torn down unless `keep_on_failure` keeps a
    /// still-running one. `tpm` is the decision `start` made for this guest.
    /// Ports `VMLifecycle.startQEMU`.
    ///
    /// A cancel is read after the launch, which tears itself down on its own failures, and
    /// during the agent wait, which is the long stage.
    async fn start_qemu(
        opts: &VmStartOptions,
        ctx: &StartContext,
        tpm: TpmDecision,
    ) -> Result<VmStartResult, VmError> {
        let paths = &ctx.paths;
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
        let artifacts = QemuRunner::start_in(&ctx.tools, &qopts, paths).await?;
        let pid = artifacts.pid;
        if ctx.cancel.is_cancelled() {
            return Err(abandon_qemu(opts, ctx, &artifacts));
        }
        let observation = tokio::select! {
            biased;
            observation = observe_qemu_boot(opts, &artifacts, move || child_alive(pid), QEMU_AGENT_POLLS) => observation,
            _ = ctx.cancel.cancelled() => return Err(abandon_qemu(opts, ctx, &artifacts)),
        };

        if let StartDecision::Failed { mut error, disposition } =
            decide_start(&observation, opts.keep_on_failure)
        {
            let vm = qemu_failed_vm(opts, &artifacts);
            let left_behind = settle_failed_start(
                paths,
                &ctx.progress,
                &vm,
                disposition,
                || QemuRunner::stop(artifacts.pid, &artifacts.clone_dir, paths),
                || artifacts.clone_dir.exists(),
            );
            record_left_behind(&mut error, left_behind);
            // A VM kept for a client that has left can never be reclaimed.
            if left_behind && ctx.cancel.is_cancelled() {
                return Err(abandon_qemu(opts, ctx, &artifacts));
            }
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

        // The client left while the sidecars were written: nobody is told the id, so the
        // VM is stopped again, not left running.
        if ctx.cancel.is_cancelled() {
            return Err(abandon_qemu(opts, ctx, &artifacts));
        }

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
    ///
    /// Everything after the clone can be cancelled at its next wait. The clone is a few
    /// seconds of `tart` that run to their end, and the cancel is read after them.
    #[cfg(target_os = "macos")]
    async fn start_tart(opts: &VmStartOptions, ctx: &StartContext) -> Result<VmStartResult, VmError> {
        use crate::tart::TartStartOptions;

        let topts = TartStartOptions {
            id: opts.id.clone(),
            base: opts.base.clone(),
            display: opts.display.clone(),
        };
        // A failed clone has already removed itself.
        TartRunner::prepare_in(&ctx.tools, &topts, &ctx.paths).await?;
        let track = StartTrack::default();
        if ctx.cancel.is_cancelled() {
            return Err(abandon_tart(opts, ctx, &track));
        }
        // The clone is ready, so the start joins the admission queue of the app (spec
        // "admission-queue": "The order"). The hold lives to the end of this function: a failed or
        // cancelled start drops it after the cleanup, so after `tart run` has exited, and a
        // successful start drops it with its meta written, which holds the slot from then on.
        let joined = Instant::now();
        let mut hold = match (&ctx.gate, opts.platform) {
            (Some(gate), Platform::Macos) => {
                let ask = SlotAsk { vm_id: &opts.id, wait: opts.wait };
                match gate.admit(&ask, &ctx.cancel, &ctx.progress) {
                    Ok(hold) => Some(hold),
                    Err(refusal) => {
                        // The wait ended before any `tart run`. The clone is the only thing to remove.
                        let cancelled = abandon_tart(opts, ctx, &track);
                        return Err(match refusal {
                            VmError::StartCancelled { .. } => cancelled,
                            other => other,
                        });
                    }
                }
            }
            _ => None,
        };
        let started = tokio::select! {
            biased;
            started = Self::start_tart_clone(opts, ctx, &topts, &track, &mut hold) => started,
            _ = ctx.cancel.cancelled() => return Err(abandon_tart(opts, ctx, &track)),
        };
        // A refusal by the framework that the queue did not take back says whose refusal it was,
        // and where the start would stand if it queued again. A refusal by the app after a try
        // has its facts from the queue already.
        let started = match (started, &hold) {
            (Err(VmError::VmLimitReached(mut limit)), Some(hold))
                if limit.admission.request_id.is_none() =>
            {
                limit.admission = AdmissionFacts {
                    request_id: Some(hold.request_id().to_string()),
                    place: Some(hold.place_if_requeued()),
                    waited_secs: joined.elapsed().as_secs(),
                    refused_by: RefusedBy::Framework,
                    unseen_possible: true,
                    tart_silence: hold.tart_silence(),
                };
                Err(VmError::VmLimitReached(limit))
            }
            (started, _) => started,
        };
        // The client left as the start ended: nobody is told the id, so stop the VM again. A start
        // that failed with `--keep-on-failure` left a VM that nobody can reclaim either.
        if ctx.cancel.is_cancelled() {
            return Err(abandon_tart(opts, ctx, &track));
        }
        started
    }

    /// The part of a tart start that follows the clone. `track` always names the processes
    /// that exist, so a cancel can remove them from outside this future.
    #[cfg(target_os = "macos")]
    async fn start_tart_clone(
        opts: &VmStartOptions,
        ctx: &StartContext,
        topts: &crate::tart::TartStartOptions,
        track: &StartTrack,
        hold: &mut Option<Box<dyn SlotHold>>,
    ) -> Result<VmStartResult, VmError> {
        let paths = &ctx.paths;
        let (pid, boot, decision) = Self::run_until_slot(opts, ctx, topts, track, hold).await?;

        let (vnc, ip) = match decision {
            StartDecision::Ready => match (boot.vnc.clone(), boot.ip.clone()) {
                (Some(vnc), Some(ip)) => (vnc, ip),
                // Ready implies both stages passed; refuse to invent an endpoint.
                _ => {
                    TartRunner::stop_in(&ctx.tools, &opts.id, pid);
                    return Err(VmError::TartFailed {
                        detail: "internal error: ready start without a VNC endpoint and guest IP".into(),
                    });
                }
            },
            StartDecision::Failed { mut error, disposition } => {
                let left_behind = settle_failed_start(
                    paths,
                    &ctx.progress,
                    &tart_failed_vm(opts, ctx.agent_port, pid, &boot),
                    disposition,
                    || {
                        TartRunner::stop_in(&ctx.tools, &opts.id, pid);
                    },
                    || crate::tart::vm_exists_in(&ctx.tools, &opts.id),
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
            let progress = &ctx.progress;
            match opts.logical {
                // HiDPI @2x (ADR-0016 D3): select the guest's 2× Retina
                // mode for the logical target (k4 finding 3) — *replacing*
                // ADR-0014's 1× switch, not skipping it. At 2× the guest
                // advertises BOTH a 1× and a 2× 1920-logical mode, so the
                // old 1× selector would match the 1× mode and silently
                // defeat HiDPI (k4 finding 4); suppression is load-bearing.
                Some((lw, lh)) => {
                    let applied = crate::display::apply(&ctx.tools, progress, &ip, ctx.agent_port, (lw, lh), 2, paths, &opts.id).await;
                    // No 2× mode advertised ⇒ a 1× host (or HiDPI did not
                    // engage). Fall back to the 1× mode of the same logical
                    // size so the guest still reaches a sane 1920×1080
                    // (the connection auto-detects scale=1 and warns —
                    // ADR-0016 "the host yielded 1×"). Safe vs k4 finding 4:
                    // this runs only when no 2× mode exists, so it cannot
                    // defeat HiDPI on a real Retina host (where the 2×
                    // switch succeeds first and skips this).
                    if !applied {
                        progress.line(&format!(
                            "  HiDPI 2× switch did not apply — falling back to \
                             1× {lw}x{lh} (HiDPI needs a Retina host / the \
                             deferred deterministic mechanism, ADR-0016)"
                        ));
                        crate::display::apply(&ctx.tools, progress, &ip, ctx.agent_port, (lw, lh), 1, paths, &opts.id).await;
                    }
                }
                // Default 1× path (ADR-0014): force the 1920×1080-px
                // contract via the 1× mode selector.
                None => {
                    let resolved = crate::tart::resolve_display(opts.display.as_deref());
                    match crate::display::parse_target(resolved) {
                        Some(target) => {
                            crate::display::apply(&ctx.tools, progress, &ip, ctx.agent_port, target, 1, paths, &opts.id).await;
                        }
                        None => progress.line(&format!(
                            "  warning: cannot derive a px target from display \
                             '{resolved}' — skipping macOS resolution switch"
                        )),
                    }
                }
            }
        }

        let spec = VmSpec {
            vnc: VncEndpoint { host: vnc.host.clone(), port: vnc.port, password: vnc.password.clone() },
            agent: Some(AgentEndpoint { host: ip, port: ctx.agent_port }),
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
            TartRunner::stop_in(&ctx.tools, &opts.id, pid);
        })?;

        // tart manages its own storage, so the meta carries no clone_dir;
        // `pid` is the detached `tart run` process tracked for `stop`.
        let meta = VmMeta {
            // The keeper started in `observe_tart_boot`, before any VNC
            // client (ADR-0018). `vm stop` signals it as a backstop.
            keeper_pid: boot.keeper_pid,
            identity: boot.identity.clone(),
            keeper_identity: boot.keeper_identity.clone(),
            ..VmMeta::new(opts.id.clone(), VmTool::Tart, pid)
        };
        meta.write_atomic(&meta_path).inspect_err(|_| {
            TartRunner::stop_in(&ctx.tools, &opts.id, pid);
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

    /// Run `tart run` on the prepared clone and observe its boot, returning the
    /// verdict. Without `--wait` that is one attempt.
    ///
    /// With the admission queue of the app (`hold`), a refusal by the framework sends the start
    /// back into the queue (`run_in_queue`). Without it, a `--wait` start runs the *same* clone
    /// again after a pause, until a slot frees or the wait is spent; the final refusal is then
    /// the verdict, with the same error as a start that never waited.
    ///
    /// A SIGINT or SIGTERM during a `--wait` start of the Host CLI removes the clone, so an
    /// interrupted wait leaks nothing, then ends the process the way the signal
    /// would have. The app installs no handler: its caller leaving cancels the whole start.
    #[cfg(target_os = "macos")]
    async fn run_until_slot(
        opts: &VmStartOptions,
        ctx: &StartContext,
        topts: &crate::tart::TartStartOptions,
        track: &StartTrack,
        hold: &mut Option<Box<dyn SlotHold>>,
    ) -> Result<(i32, TartBoot, StartDecision), VmError> {
        if let Some(hold) = hold.as_mut() {
            return Self::run_in_queue(opts, ctx, topts, track, hold).await;
        }
        let Some(timeout) = opts.wait else {
            return Self::run_and_observe(opts, ctx, topts, track, None).await;
        };
        let stop = if ctx.process_signals { StopSignals::install() } else { None };
        let Some(mut stop) = stop else {
            return Self::wait_for_slot(opts, ctx, topts, timeout, track).await;
        };
        let outcome = tokio::select! {
            outcome = Self::wait_for_slot(opts, ctx, topts, timeout, track) => outcome,
            caught = stop.caught() => {
                crate::tart::TartRunner::stop_in(&ctx.tools, &opts.id, track.pid());
                clear_sidecars(&ctx.paths, &opts.id);
                StopSignals::end_like(caught)
            }
        };
        // The wait is over; the rest of the start answers a signal as it always has.
        StopSignals::restore_defaults();
        outcome
    }

    /// The tries of a start that holds a slot of the app's queue (spec "admission-queue": "A
    /// refusal by the framework"). A refusal gives the slot back, and the queue decides whether
    /// the start has time left. If so, the queue returns the start to the place of its join, and
    /// the next try waits for the admission. If not, the refusal is the verdict.
    #[cfg(target_os = "macos")]
    async fn run_in_queue(
        opts: &VmStartOptions,
        ctx: &StartContext,
        topts: &crate::tart::TartStartOptions,
        track: &StartTrack,
        hold: &mut Box<dyn SlotHold>,
    ) -> Result<(i32, TartBoot, StartDecision), VmError> {
        loop {
            let (pid, boot, decision) =
                Self::run_and_observe(opts, ctx, topts, track, Some(&**hold)).await?;
            let StartDecision::Failed { error: VmError::VmLimitReached(limit), .. } = &decision else {
                return Ok((pid, boot, decision));
            };
            // The refused `tart run` has exited and been reaped, and its pid may be another
            // process's by the time a cancel reads it.
            track.pid.store(0, std::sync::atomic::Ordering::SeqCst);
            match hold.refused(&limit.raw, &ctx.cancel, &ctx.progress) {
                Ok(Readmission::Admitted) => {}
                Ok(Readmission::Stands) => return Ok((pid, boot, decision)),
                Err(ended) => {
                    // The wait ended in the queue. The clone is the only thing to remove.
                    let cancelled = abandon_tart(opts, ctx, track);
                    return Err(match ended {
                        VmError::StartCancelled { .. } => cancelled,
                        other => other,
                    });
                }
            }
        }
    }

    /// The retry loop behind [`run_until_slot`](Self::run_until_slot).
    #[cfg(target_os = "macos")]
    async fn wait_for_slot(
        opts: &VmStartOptions,
        ctx: &StartContext,
        topts: &crate::tart::TartStartOptions,
        timeout: Duration,
        track: &StartTrack,
    ) -> Result<(i32, TartBoot, StartDecision), VmError> {
        let waiting_since = Instant::now();
        let mut reporter = SlotWaitReporter::default();
        loop {
            let (pid, boot, decision) = Self::run_and_observe(opts, ctx, topts, track, None).await?;
            let StartDecision::Failed { error: VmError::VmLimitReached(limit), .. } = &decision else {
                return Ok((pid, boot, decision));
            };
            let elapsed = waiting_since.elapsed();
            match next_slot_step(elapsed, timeout, SLOT_RETRY_INTERVAL) {
                SlotWaitStep::GiveUp => return Ok((pid, boot, decision)),
                SlotWaitStep::RetryAfter(pause) => {
                    // The refused `tart run` has exited and been reaped, and its pid may be
                    // another process's by the time a cancel reads it.
                    track.pid.store(0, std::sync::atomic::Ordering::SeqCst);
                    if let Some(line) = reporter.note(limit, elapsed) {
                        ctx.progress.line(&line);
                    }
                    tokio::time::sleep(pause).await;
                }
            }
        }
    }

    /// One `tart run` on the prepared clone and what its boot showed. The pid
    /// goes into `track` as soon as there is one. A failed spawn has already
    /// removed the clone. `hold` hears when the framework accepts the start.
    #[cfg(target_os = "macos")]
    async fn run_and_observe(
        opts: &VmStartOptions,
        ctx: &StartContext,
        topts: &crate::tart::TartStartOptions,
        track: &StartTrack,
        hold: Option<&dyn SlotHold>,
    ) -> Result<(i32, TartBoot, StartDecision), VmError> {
        let crate::tart::TartLaunch { pid, log_path } =
            crate::tart::TartRunner::run_in(&ctx.tools, topts, &ctx.paths)?;
        track.pid.store(pid, std::sync::atomic::Ordering::SeqCst);
        let boot = observe_tart_boot(opts, ctx, pid, &log_path, track, hold).await;
        let decision = decide_start(&boot.observation, opts.keep_on_failure);
        Ok((pid, boot, decision))
    }

    /// Stop a VM and remove its sidecars. Ports `VMLifecycle.stop`
    /// (QEMU branch). A `tart` meta returns `BackendUnsupported`.
    pub fn stop(id: &str, paths: &VmPaths) -> Result<(), VmError> {
        Self::stop_with(id, paths, &ToolSearch::from_process_path())
    }

    /// [`stop`](Self::stop), with `tart` found by `tools`. The TestAnyware app stops a VM whose
    /// client left as its start succeeded.
    ///
    /// A meta with an identity signals a process only when its pid, start time and boot session
    /// all match (spec: "Lifecycle and adoption"). When the pid names another process, the stop
    /// signals nothing, removes nothing and fails. A meta with no identity keeps the stop it
    /// always had: the direct path of the Host CLI on a Linux or Windows host.
    pub fn stop_with(id: &str, paths: &VmPaths, tools: &ToolSearch) -> Result<(), VmError> {
        let meta = Self::meta_to_stop(id, paths)?;
        match decide(&meta, &SystemProbe) {
            Adoption::Legacy => Self::stop_by_pid(id, &meta, paths, tools),
            decided => Self::stop_proven(id, &meta, decided, paths, tools, &SystemProbe),
        }
    }

    /// The stop of the TestAnyware app. It sends a signal only to a process whose pid, start time
    /// and boot session all match the meta. A meta with no identity gets no signal at all:
    /// a QEMU guest stops through its control sockets, and a tart guest stops by name with
    /// `tart stop` and `tart delete` (spec: "Lifecycle and adoption").
    ///
    /// An older QEMU guest's files go only after the stop has seen that neither socket answers
    /// and no process holds the clone's disk. Otherwise nothing is removed, so a later stop can
    /// try again. A pid that names another process fails the stop in the same way.
    pub fn stop_with_probes(
        id: &str,
        paths: &VmPaths,
        tools: &ToolSearch,
        probes: &(impl LegacyStopProbes + ?Sized),
        wait: SettleWait,
    ) -> Result<(), VmError> {
        Self::stop_app(id, paths, tools, &SystemProbe, probes, wait)
    }

    /// [`stop_with_probes`](Self::stop_with_probes), with the live processes observed by `process`.
    pub(crate) fn stop_app(
        id: &str,
        paths: &VmPaths,
        tools: &ToolSearch,
        process: &impl ProcessProbe,
        probes: &(impl LegacyStopProbes + ?Sized),
        wait: SettleWait,
    ) -> Result<(), VmError> {
        let meta = Self::meta_to_stop(id, paths)?;
        match decide(&meta, process) {
            Adoption::Legacy => Self::stop_without_signal(id, &meta, paths, tools, probes, wait),
            decided => Self::stop_proven(id, &meta, decided, paths, tools, process),
        }
    }

    fn meta_to_stop(id: &str, paths: &VmPaths) -> Result<VmMeta, VmError> {
        let meta_path = paths.meta_path(id);
        if !meta_path.is_file() {
            return Err(VmError::VmNotFound { id: id.to_string() });
        }
        VmMeta::load(&meta_path)
    }

    /// The stop of a meta with an identity. `adoption` is the decision for `meta`, never `Legacy`.
    /// Each process gets a signal only on a full match, which this reads again now.
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] // Only tart is found by `tools`.
    fn stop_proven(
        id: &str,
        meta: &VmMeta,
        adoption: Adoption,
        paths: &VmPaths,
        tools: &ToolSearch,
        process: &impl ProcessProbe,
    ) -> Result<(), VmError> {
        let spec_path = paths.spec_path(id);
        let meta_path = paths.meta_path(id);
        if let Adoption::Doubt { pid, mismatch } = adoption {
            return Err(VmError::VmStopUnproven {
                id: id.to_string(),
                unproven: Box::new(StopUnproven {
                    pid,
                    mismatch,
                    records: vec![
                        spec_path.display().to_string(),
                        meta_path.display().to_string(),
                    ],
                }),
            });
        }
        let main = verified_pid(process, Some(meta.pid), meta.identity.as_ref());
        let keeper = verified_pid(process, meta.keeper_pid, meta.keeper_identity.as_ref());
        let swtpm = verified_pid(process, meta.swtpm_pid, meta.swtpm_identity.as_ref());
        let stop_error = match meta.tool {
            #[cfg(target_os = "macos")]
            VmTool::Tart => {
                // A pid of 0 skips the signal, so a gone process receives none.
                let stopped = crate::tart::TartRunner::stop_in(tools, id, main.unwrap_or(0));
                crate::keeper::stop_keeper(id, keeper);
                if stopped {
                    None
                } else {
                    Some(VmError::VmStopFailed { id: id.to_string() })
                }
            }
            #[cfg(not(target_os = "macos"))]
            VmTool::Tart => Some(VmError::BackendUnsupported { platform: "macos (tart)".into() }),
            VmTool::Qemu => match meta.clone_dir.as_deref().filter(|d| !d.is_empty()) {
                Some(clone_dir) => {
                    let clone_dir = std::path::Path::new(clone_dir);
                    let session = meta.session_dir.as_deref().filter(|d| !d.is_empty()).map_or_else(
                        || paths.session_dir(&clone_dir_id(clone_dir)),
                        PathBuf::from,
                    );
                    crate::qemu::teardown_verified(main, swtpm, clone_dir, &session);
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

    /// The stop of a meta with no identity, by the pid and the pgrep backstop it always had.
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] // Only tart is found by `tools`.
    fn stop_by_pid(id: &str, meta: &VmMeta, paths: &VmPaths, tools: &ToolSearch) -> Result<(), VmError> {
        let spec_path = paths.spec_path(id);
        let meta_path = paths.meta_path(id);
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
                let stopped = crate::tart::TartRunner::stop_in(tools, id, meta.pid);
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

    /// The app's stop of a meta with no identity. It sends no signal to any process.
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] // Only tart is found by `tools`.
    fn stop_without_signal(
        id: &str,
        meta: &VmMeta,
        paths: &VmPaths,
        tools: &ToolSearch,
        probes: &(impl LegacyStopProbes + ?Sized),
        wait: SettleWait,
    ) -> Result<(), VmError> {
        if meta.tool == VmTool::Qemu {
            return Self::stop_older_qemu(id, meta, paths, probes, wait);
        }
        // A tart guest stops by name. A pid of 0 skips the signal, and the keeper, which has no
        // identity either, ends itself once tart is gone (ADR-0018).
        #[cfg(target_os = "macos")]
        let stop_error = if crate::tart::TartRunner::stop_in(tools, id, 0) {
            None
        } else {
            Some(VmError::VmStopFailed { id: id.to_string() })
        };
        #[cfg(not(target_os = "macos"))]
        let stop_error = Some(VmError::BackendUnsupported { platform: "macos (tart)".into() });
        let _ = std::fs::remove_file(paths.spec_path(id));
        let _ = std::fs::remove_file(paths.meta_path(id));
        match stop_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    /// The stop of an older QEMU guest, through its control sockets (spec: "Lifecycle and
    /// adoption"). The guest's files go only after it has seen that neither socket answers and
    /// no process holds the clone's disk.
    fn stop_older_qemu(
        id: &str,
        meta: &VmMeta,
        paths: &VmPaths,
        probes: &(impl LegacyStopProbes + ?Sized),
        wait: SettleWait,
    ) -> Result<(), VmError> {
        let clone_dir = match meta.clone_dir.as_deref().filter(|d| !d.is_empty()) {
            Some(recorded) => PathBuf::from(recorded),
            None => paths.clone_dir(id),
        };
        let session = find_session_dir(&session_candidates(id, meta, paths));
        stop_through_sockets(probes, wait, session.as_deref(), &clone_dir).map_err(|blockers| {
            VmError::VmStopBlocked { id: id.to_string(), blockers: Box::new(blockers) }
        })?;
        let _ = std::fs::remove_dir_all(&clone_dir);
        if let Some(session) = &session {
            let _ = std::fs::remove_dir_all(session);
        }
        let _ = std::fs::remove_file(paths.spec_path(id));
        let _ = std::fs::remove_file(paths.meta_path(id));
        Ok(())
    }

    /// Delete a QEMU golden image by name. Refuses when running clones
    /// depend on it unless `force`. Ports `VMLifecycle.delete` (QEMU
    /// branch; tart detection is backlog task 12).
    pub fn delete(name: &str, force: bool, paths: &VmPaths) -> Result<(), VmError> {
        Self::delete_with(name, force, paths, &ToolSearch::from_process_path())
    }

    /// [`delete`](Self::delete), with `tart` found by `tools`. The TestAnyware app deletes with
    /// its built-in search, because it has no shell environment.
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] // Only tart is found by `tools`.
    pub fn delete_with(
        name: &str,
        force: bool,
        paths: &VmPaths,
        tools: &ToolSearch,
    ) -> Result<(), VmError> {
        let golden_dir = paths.golden_dir();
        let qcow2 = golden_dir.join(format!("{name}.qcow2"));

        // A tart golden (macOS) takes precedence only when no qcow2 of the
        // same name exists — QEMU stays authoritative for its own images.
        #[cfg(target_os = "macos")]
        if !qcow2.is_file() && crate::tart::golden_exists_in(tools, name) {
            if !force {
                let running = running_tart_clone_ids(paths, tools);
                if !running.is_empty() {
                    // tart cannot cheaply prove which clones back this
                    // image; warn via the in-use error (no pids) unless forced.
                    return Err(VmError::GoldenInUse { name: name.to_string(), clone_pids: vec![] });
                }
            }
            if !crate::tart::delete_golden_in(tools, name) {
                return Err(VmError::TartFailed { detail: format!("tart delete {name} failed") });
            }
            return Ok(());
        }

        if !qcow2.is_file() {
            return Err(VmError::GoldenNotFound { name: name.to_string() });
        }
        if !force {
            let pids = QemuRunner::running_clones_backed_by_in(tools, name, paths);
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
        if wants_tart(opts.platform, &opts.base, paths, &ToolSearch::from_process_path()) {
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
        let tpm = decide_tpm_on_host_in(&ToolSearch::from_process_path(), opts.platform, &host_profile_in(&ToolSearch::from_process_path()))?;
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
        Self::list_with(paths, &ToolSearch::from_process_path())
    }

    /// [`Self::list`], with the tools found by `tools`. The TestAnyware app lists with its
    /// built-in search, because it has no shell environment.
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))] // Only tart is found by `tools`.
    pub fn list_with(paths: &VmPaths, tools: &ToolSearch) -> VmListing {
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
            goldens.extend(crate::tart::list_goldens_in(tools));
            for id in running_tart_clone_ids(paths, tools) {
                let clone = RunningClone { id, platform: "unknown".into(), backend: "tart" };
                running.push(enrich_running(&clone, paths));
            }
            (goldens, running)
        };

        VmListing { goldens, running }
    }
}

/// Which of the running tart VMs `vm list` shows as TestAnyware clones: those
/// named `testanyware-…` (a golden, `testanyware-golden-…`, is not a clone), and
/// those with a sidecar, because `vm start --id` accepts any id — Koine's
/// `koine-verify-<pid>` and a `--keep-on-failure` VM with a custom id carry
/// neither the prefix nor a name `tart list` could tell from a hand-made VM.
/// `has_sidecars` is injected so this stays pure. A sidecar whose VM is not in
/// `running` is deliberately ignored: `vm list` shows what is running, and
/// `vm stop <id>` still removes the leftover sidecar.
#[cfg(any(target_os = "macos", test))]
fn tart_clone_ids(running: Vec<String>, has_sidecars: impl Fn(&str) -> bool) -> Vec<String> {
    running
        .into_iter()
        .filter(|name| {
            let by_prefix = name.starts_with("testanyware-") && !name.starts_with("testanyware-golden-");
            by_prefix || has_sidecars(name)
        })
        .collect()
}

/// The running tart clones, by [`tart_clone_ids`]'s rule. `[]` when tart is absent.
#[cfg(target_os = "macos")]
fn running_tart_clone_ids(paths: &VmPaths, tools: &ToolSearch) -> Vec<String> {
    tart_clone_ids(crate::tart::list_running_names_in(tools), |id| has_sidecars(paths, id))
}

/// Whether `vm start` left a spec or meta sidecar for `id`.
#[cfg(any(target_os = "macos", test))]
fn has_sidecars(paths: &VmPaths, id: &str) -> bool {
    paths.spec_path(id).is_file() || paths.meta_path(id).is_file()
}

/// Whether to route this start to the tart backend (macOS host only).
/// macOS guests always use tart. A Linux guest uses tart only as a
/// fallback: a same-named QEMU qcow2 wins, otherwise a tart golden of
/// that name is used (the kept-built `testanyware-golden-linux-24.04` is
/// a tart VM). Windows always uses QEMU.
#[cfg(target_os = "macos")]
fn wants_tart(platform: Platform, base: &str, paths: &VmPaths, tools: &ToolSearch) -> bool {
    match platform {
        Platform::Macos => true,
        Platform::Windows => false,
        Platform::Linux => {
            let qcow2 = paths.golden_dir().join(format!("{base}.qcow2"));
            !qcow2.is_file() && crate::tart::vm_exists_in(tools, base)
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
/// QEMU's agent wait: 120 probes 5 s apart. That is ten minutes when probes
/// fail fast and about fourteen when each also spends its 2 s request timeout
/// (observed live: 835 s). Deliberately long because Windows boots slowly; a
/// QEMU that *dies* ends the wait at once, so the long window costs nothing on
/// a failed launch.
const QEMU_AGENT_POLLS: (u32, Duration) = (120, Duration::from_secs(5));

/// The two signals that ask a waiting `vm start` to stop. Holding the streams
/// is what keeps tokio's handlers installed; [`restore_defaults`](Self::restore_defaults)
/// puts the process back the way it was once the wait is over.
#[cfg(target_os = "macos")]
struct StopSignals {
    interrupt: tokio::signal::unix::Signal,
    terminate: tokio::signal::unix::Signal,
}

#[cfg(target_os = "macos")]
impl StopSignals {
    /// `None` if the handlers cannot be installed; the wait then runs without
    /// interrupt cleanup, as every start did before `--wait` existed.
    fn install() -> Option<Self> {
        use tokio::signal::unix::{signal, SignalKind};
        Some(Self {
            interrupt: signal(SignalKind::interrupt()).ok()?,
            terminate: signal(SignalKind::terminate()).ok()?,
        })
    }

    /// Resolves with the first stop signal to arrive.
    async fn caught(&mut self) -> nix::sys::signal::Signal {
        use nix::sys::signal::Signal;
        tokio::select! {
            _ = self.interrupt.recv() => Signal::SIGINT,
            _ = self.terminate.recv() => Signal::SIGTERM,
        }
    }

    /// Give SIGINT and SIGTERM back their default action.
    fn restore_defaults() {
        use nix::sys::signal::{signal, SigHandler, Signal};
        for sig in [Signal::SIGINT, Signal::SIGTERM] {
            // SAFETY: SIG_DFL installs no handler code, so there is nothing to
            // be unsound about; it only undoes the handlers tokio installed.
            let _ = unsafe { signal(sig, SigHandler::SigDfl) };
        }
    }

    /// End the process as `caught` would have, so the caller's shell sees a
    /// death by signal (130 / 143) rather than an invented exit code.
    fn end_like(caught: nix::sys::signal::Signal) -> ! {
        Self::restore_defaults();
        let _ = nix::sys::signal::raise(caught);
        // Unreachable unless the signal is blocked; exit with the shell's code.
        std::process::exit(128 + caught as i32)
    }
}

/// The processes of a tart start that exist now. The start future writes them as it makes them,
/// and the cancel arm of [`VmLifecycle::start_tart`] reads them after it dropped the future.
#[cfg(target_os = "macos")]
#[derive(Default)]
struct StartTrack {
    /// The `tart run` that is running, or 0.
    pid: std::sync::atomic::AtomicI32,
    /// The VNC keeper once it started, or 0.
    keeper_pid: std::sync::atomic::AtomicI32,
    /// The identity of the `tart run` that is running, read when it started.
    identity: std::sync::Mutex<Option<ProcessIdentity>>,
    /// The identity of the keeper, read when it started.
    keeper_identity: std::sync::Mutex<Option<ProcessIdentity>>,
}

#[cfg(target_os = "macos")]
impl StartTrack {
    fn pid(&self) -> i32 {
        self.pid.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn keeper_pid(&self) -> Option<i32> {
        Some(self.keeper_pid.load(std::sync::atomic::Ordering::SeqCst)).filter(|pid| *pid > 0)
    }

    fn identity(&self) -> Option<ProcessIdentity> {
        self.identity.lock().ok().and_then(|identity| identity.clone())
    }

    fn keeper_identity(&self) -> Option<ProcessIdentity> {
        self.keeper_identity.lock().ok().and_then(|identity| identity.clone())
    }
}

/// Clean up a tart start that the client left, as after a failed start without
/// `--keep-on-failure`: nobody can reclaim a kept VM, so the VM, its keeper and its sidecars go.
///
/// The cleanup is the teardown of a failed start: when the VM survives it, the meta is written
/// so that `vm stop` can retry, and the sidecars are not dropped. The scratch directory of the
/// display switch goes too: a cancel that arrives during the switch drops the future before it
/// removes its own directory.
#[cfg(target_os = "macos")]
fn abandon_tart(opts: &VmStartOptions, ctx: &StartContext, track: &StartTrack) -> VmError {
    let vm = FailedVm {
        id: opts.id.clone(),
        meta: VmMeta {
            keeper_pid: track.keeper_pid(),
            identity: track.identity(),
            keeper_identity: track.keeper_identity(),
            ..VmMeta::new(opts.id.clone(), VmTool::Tart, track.pid())
        },
        spec: None,
        manual_reclaim: format!("`tart stop {id}` and `tart delete {id}`", id = opts.id),
    };
    settle_failed_start(
        &ctx.paths,
        &ctx.progress,
        &vm,
        Disposition::Teardown,
        || {
            TartRunner::stop_in(&ctx.tools, &opts.id, track.pid());
            crate::keeper::stop_keeper(&opts.id, track.keeper_pid());
        },
        || crate::tart::vm_exists_in(&ctx.tools, &opts.id),
    );
    let _ = std::fs::remove_dir_all(ctx.paths.vms_dir().join(format!("display-{}", opts.id)));
    VmError::StartCancelled { id: opts.id.clone() }
}

/// [`abandon_tart`] for a QEMU start.
fn abandon_qemu(opts: &VmStartOptions, ctx: &StartContext, artifacts: &StartArtifacts) -> VmError {
    settle_failed_start(
        &ctx.paths,
        &ctx.progress,
        &qemu_failed_vm(opts, artifacts),
        Disposition::Teardown,
        || QemuRunner::stop(artifacts.pid, &artifacts.clone_dir, &ctx.paths),
        || artifacts.clone_dir.exists(),
    );
    VmError::StartCancelled { id: opts.id.clone() }
}

/// What a tart boot observed, plus the endpoints it found along the way.
#[cfg(target_os = "macos")]
struct TartBoot {
    observation: BootObservation,
    vnc: Option<crate::tart::VncUrl>,
    ip: Option<String>,
    /// Pid of the VNC keeper (ADR-0018), or `None` if it could not start.
    keeper_pid: Option<i32>,
    /// The identity of `tart run`, read as soon as it started.
    identity: Option<ProcessIdentity>,
    /// The identity of the keeper, read as soon as it started.
    keeper_identity: Option<ProcessIdentity>,
}

/// Run the readiness stages in order — VNC URL, guest IP, agent health —
/// stopping at the first that fails, and record what was seen. Each stage
/// notices a dead `tart run` at once. The process is sampled last, so a death
/// during any stage is what the decision sees.
#[cfg(target_os = "macos")]
async fn observe_tart_boot(
    opts: &VmStartOptions,
    ctx: &StartContext,
    pid: i32,
    log_path: &std::path::Path,
    track: &StartTrack,
    hold: Option<&dyn SlotHold>,
) -> TartBoot {
    use crate::tart::{await_guest_ip, await_vnc_url, Polled};

    let alive = move || child_alive(pid);
    // Read before the first wait: the process is the child that this start made, and nothing
    // has had time to end it and free its pid.
    let identity = read_identity(&SystemProbe, pid);
    *track.identity.lock().unwrap_or_else(|e| e.into_inner()) = identity.clone();
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
    let mut keeper_identity = None;

    let stage_start = Instant::now();
    match await_vnc_url(log_path, VNC_POLLS.0, VNC_POLLS.1, alive).await {
        Polled::Found(url) => {
            // tart prints the URL only after the framework started the VM, so the framework has
            // accepted the start and its slot is proven (spec "admission-queue": "A refusal by
            // the framework").
            if let Some(hold) = hold {
                hold.accepted();
            }
            // Hold one silent VNC connection for the life of tart (ADR-0018),
            // so a client that connects after a pause of 60 s or more cannot
            // crash it on a macOS 27 host. Start it as soon as tart prints the
            // URL, before the first VNC client of any kind. A failure warns;
            // the guest then runs without the protection.
            keeper_pid = crate::keeper::start_keeper(
                &opts.id,
                &url.host,
                url.port,
                &ctx.paths.vms_dir(),
                &ctx.progress,
            );
            track.keeper_pid.store(keeper_pid.unwrap_or(0), std::sync::atomic::Ordering::SeqCst);
            keeper_identity = read_optional_identity(&SystemProbe, keeper_pid);
            *track.keeper_identity.lock().unwrap_or_else(|e| e.into_inner()) =
                keeper_identity.clone();
            vnc = Some(url);
        }
        Polled::ProcessExited => {}
        Polled::TimedOut { attempts } => {
            observation.vnc = VncObservation::Missing { attempts, elapsed: stage_start.elapsed() };
        }
    }

    if vnc.is_some() {
        let stage_start = Instant::now();
        match await_guest_ip(&ctx.tools, &opts.id, IP_POLLS.0, IP_POLLS.1, alive).await {
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
        let port = ctx.agent_port;
        let wait = wait_for_agent_with_oracle(ip, port, AGENT_POLLS.0, AGENT_POLLS.1, alive, || {
            crate::local_network::platform_tool_reaches(ip, port)
        })
        .await;
        observation.agent = AgentObservation::Waited {
            endpoint: format!("http://{ip}:{port}/health"),
            wait,
        };
    }

    if !child_alive(pid) {
        observation.process = ProcessState::Exited { log_tail: read_log_tail(log_path) };
    }
    TartBoot { observation, vnc, ip, keeper_pid, identity, keeper_identity }
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
fn tart_failed_vm(opts: &VmStartOptions, agent_port: u16, pid: i32, boot: &TartBoot) -> FailedVm {
    FailedVm {
        id: opts.id.clone(),
        meta: VmMeta {
            // A kept VM keeps its keeper; `vm stop` signals it later.
            keeper_pid: boot.keeper_pid,
            identity: boot.identity.clone(),
            keeper_identity: boot.keeper_identity.clone(),
            ..VmMeta::new(opts.id.clone(), VmTool::Tart, pid)
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
                .map(|ip| AgentEndpoint { host: ip.clone(), port: agent_port }),
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
        clone_dir: Some(artifacts.clone_dir.display().to_string()),
        // `viewer_window_id` tracked the Swift CLI's *external* AppleScript viewer window;
        // the Rust embedded viewer (ADR-0005) owns its window in-process, so this
        // Swift-compat field is permanently null.
        identity: artifacts.identity.clone(),
        swtpm_pid: artifacts.swtpm_pid,
        swtpm_identity: artifacts.swtpm_identity.clone(),
        session_dir: Some(artifacts.session_dir.display().to_string()),
        ..VmMeta::new(opts.id.clone(), VmTool::Qemu, artifacts.pid)
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
    progress: &Progress,
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
                        progress.line(&format!("warning: could not write the kept VM's spec: {err}"));
                    }
                }
                return true;
            }
            Err(err) => progress.line(&format!(
                "warning: could not write the kept VM's meta, so `vm stop` could not reclaim it; tearing it down instead: {err}"
            )),
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
            progress.line(&format!(
                "warning: could not remove the failed start's VM; reclaim it with `testanyware vm stop {}`",
                vm.id
            ));
            true
        }
        Err(err) => {
            progress.line(&format!(
                "warning: could not remove the failed start's VM, and could not record it for `vm stop` ({err}); reclaim it by hand: {}",
                vm.manual_reclaim
            ));
            false
        }
    }
}

/// The VM id that a clone directory is named after.
fn clone_dir_id(clone_dir: &std::path::Path) -> String {
    clone_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
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

    mod older_qemu_stop {
        use super::*;
        use std::cell::RefCell;
        use std::path::Path;

        /// A guest whose sockets answer, and whose disk is held, until `quit` arrives.
        struct Guest {
            quit_works: bool,
            alive: RefCell<bool>,
            quits: RefCell<u32>,
        }

        impl Guest {
            fn new(quit_works: bool) -> Self {
                Self { quit_works, alive: RefCell::new(true), quits: RefCell::new(0) }
            }
        }

        impl LegacyStopProbes for Guest {
            fn socket_answers(&self, socket: &Path) -> bool {
                *self.alive.borrow() && socket.ends_with("monitor.sock")
            }
            fn send_monitor_quit(&self, _socket: &Path) -> bool {
                *self.quits.borrow_mut() += 1;
                if self.quit_works {
                    *self.alive.borrow_mut() = false;
                }
                true
            }
            fn send_swtpm_shutdown(&self, _socket: &Path) -> bool {
                true
            }
            fn disk_holders(&self, _clone_dir: &Path) -> Result<Vec<i32>, String> {
                Ok(if *self.alive.borrow() { vec![31337] } else { Vec::new() })
            }
            fn pause(&self, _interval: Duration) {}
        }

        const WAIT: SettleWait = SettleWait::looks(2);

        struct Seeded {
            _dir: tempfile::TempDir,
            paths: VmPaths,
            clone: PathBuf,
            session: PathBuf,
        }

        /// An older QEMU meta whose pid is a real process, so a signal would show.
        fn seed(pid: i32) -> Seeded {
            let dir = tempfile::tempdir().unwrap();
            let paths = VmPaths::new(
                dir.path().join("state"),
                dir.path().join("data"),
                dir.path().join("tmp"),
            );
            std::fs::create_dir_all(paths.vms_dir()).unwrap();
            let id = "testanyware-0lde";
            let clone = paths.clone_dir(id);
            std::fs::create_dir_all(&clone).unwrap();
            std::fs::write(clone.join(format!("{id}.qcow2")), b"disk").unwrap();
            let session = paths.session_dir(id);
            std::fs::create_dir_all(&session).unwrap();
            std::fs::write(paths.spec_path(id), b"{}").unwrap();
            let mut meta = VmMeta::new(id, VmTool::Qemu, pid);
            meta.clone_dir = Some(clone.display().to_string());
            meta.write_atomic(&paths.meta_path(id)).unwrap();
            Seeded { _dir: dir, paths, clone, session }
        }

        fn stop(seeded: &Seeded, guest: &Guest) -> Result<(), VmError> {
            VmLifecycle::stop_with_probes(
                "testanyware-0lde",
                &seeded.paths,
                &ToolSearch::from_process_path(),
                guest,
                WAIT,
            )
        }

        fn stranger() -> std::process::Child {
            std::process::Command::new("sleep").arg("60").spawn().unwrap()
        }

        #[test]
        fn a_guest_that_quits_is_removed_and_its_pid_is_not_signalled() {
            let mut stranger = stranger();
            let seeded = seed(stranger.id() as i32);
            let guest = Guest::new(true);
            stop(&seeded, &guest).unwrap();
            assert_eq!(*guest.quits.borrow(), 1);
            assert!(!seeded.clone.exists());
            assert!(!seeded.session.exists());
            assert!(!seeded.paths.spec_path("testanyware-0lde").exists());
            assert!(!seeded.paths.meta_path("testanyware-0lde").exists());
            // The old stop would have sent SIGTERM to this pid.
            assert!(stranger.try_wait().unwrap().is_none(), "the process at the pid was signalled");
            let _ = stranger.kill();
            let _ = stranger.wait();
        }

        #[test]
        fn a_guest_that_stays_keeps_every_file_and_its_pid_is_not_signalled() {
            let mut stranger = stranger();
            let seeded = seed(stranger.id() as i32);
            let guest = Guest::new(false);
            let err = stop(&seeded, &guest).unwrap_err();
            let VmError::VmStopBlocked { blockers, .. } = &err else {
                panic!("{err:?}");
            };
            assert_eq!(blockers.disk_holders, [31337]);
            assert!(blockers.monitor_socket.is_some());
            assert_eq!(err.code(), "VM_STOP_FAILED");
            assert!(err.remediation().unwrap().contains("vm stop testanyware-0lde"));
            assert_eq!(err.details()["disk_holders"], serde_json::json!([31337]));
            assert!(seeded.clone.join("testanyware-0lde.qcow2").exists());
            assert!(seeded.session.exists());
            assert!(seeded.paths.spec_path("testanyware-0lde").exists());
            assert!(seeded.paths.meta_path("testanyware-0lde").exists());
            assert!(stranger.try_wait().unwrap().is_none(), "the process at the pid was signalled");
            let _ = stranger.kill();
            let _ = stranger.wait();
        }

        #[test]
        fn the_session_of_an_older_guest_is_found_by_its_id_in_the_own_temp_dir() {
            let seeded = seed(0);
            let guest = Guest::new(true);
            stop(&seeded, &guest).unwrap();
            // The guest answered on the monitor of the session that was found, so it got `quit`.
            assert_eq!(*guest.quits.borrow(), 1);
        }

        #[test]
        fn a_meta_with_an_identity_keeps_the_stop_it_has() {
            use crate::identity::ProcessIdentity;
            let seeded = seed(0);
            let mut meta = VmMeta::load(&seeded.paths.meta_path("testanyware-0lde")).unwrap();
            meta.identity = Some(ProcessIdentity { start_time: 1, boot_session: "b".into() });
            meta.write_atomic(&seeded.paths.meta_path("testanyware-0lde")).unwrap();
            let guest = Guest::new(false);
            // `pid: 0` skips the signal; the shared teardown removes the clone without a look.
            stop(&seeded, &guest).unwrap();
            assert_eq!(*guest.quits.borrow(), 0, "the identity path never asks the monitor");
            assert!(!seeded.clone.exists());
        }

        #[test]
        fn an_unknown_id_is_not_found() {
            let seeded = seed(0);
            let err = VmLifecycle::stop_with_probes(
                "testanyware-ghost",
                &seeded.paths,
                &ToolSearch::from_process_path(),
                &Guest::new(true),
                WAIT,
            )
            .unwrap_err();
            assert!(matches!(err, VmError::VmNotFound { .. }));
        }
    }

    /// The stop of a QEMU meta with an identity: a signal goes only to a process that matches.
    mod proven_qemu_stop {
        use super::*;
        use crate::meta::VmTool;
        use std::path::Path;

        /// The live processes of a fake host: a start time for each pid it knows.
        struct Host(HashMap<i32, u64>);

        impl ProcessProbe for Host {
            fn start_time(&self, pid: i32) -> Option<u64> {
                self.0.get(&pid).copied()
            }
            fn boot_session(&self) -> Option<String> {
                Some("boot-a".into())
            }
        }

        /// The identity path asks no socket and no disk.
        struct NoSockets;

        impl LegacyStopProbes for NoSockets {
            fn socket_answers(&self, _: &Path) -> bool {
                panic!("the identity path asked a socket");
            }
            fn send_monitor_quit(&self, _: &Path) -> bool {
                panic!("the identity path sent quit");
            }
            fn send_swtpm_shutdown(&self, _: &Path) -> bool {
                panic!("the identity path sent a shutdown");
            }
            fn disk_holders(&self, _: &Path) -> Result<Vec<i32>, String> {
                panic!("the identity path asked for the disk holders");
            }
            fn pause(&self, _: Duration) {}
        }

        const ID: &str = "testanyware-1dd0";

        fn identity(start_time: u64) -> ProcessIdentity {
            ProcessIdentity { start_time, boot_session: "boot-a".into() }
        }

        struct Seeded {
            _dir: tempfile::TempDir,
            paths: VmPaths,
            clone: PathBuf,
            session: PathBuf,
        }

        /// A QEMU meta that records the identity (start time 100) of `qemu`, and of `swtpm` when
        /// there is one.
        fn seed(qemu: i32, swtpm: Option<i32>) -> Seeded {
            let dir = tempfile::tempdir().unwrap();
            let paths = VmPaths::new(
                dir.path().join("state"),
                dir.path().join("data"),
                dir.path().join("tmp"),
            );
            std::fs::create_dir_all(paths.vms_dir()).unwrap();
            let clone = paths.clone_dir(ID);
            std::fs::create_dir_all(&clone).unwrap();
            let session = paths.session_dir(ID);
            std::fs::create_dir_all(&session).unwrap();
            std::fs::write(paths.spec_path(ID), b"{}").unwrap();
            let mut meta = VmMeta::new(ID, VmTool::Qemu, qemu);
            meta.clone_dir = Some(clone.display().to_string());
            meta.session_dir = Some(session.display().to_string());
            meta.identity = Some(identity(100));
            meta.swtpm_pid = swtpm;
            meta.swtpm_identity = swtpm.map(|_| identity(100));
            meta.write_atomic(&paths.meta_path(ID)).unwrap();
            Seeded { _dir: dir, paths, clone, session }
        }

        fn stop(seeded: &Seeded, host: &Host) -> Result<(), VmError> {
            VmLifecycle::stop_app(
                ID,
                &seeded.paths,
                &ToolSearch::from_process_path(),
                host,
                &NoSockets,
                SettleWait::looks(1),
            )
        }

        fn sleeper() -> std::process::Child {
            std::process::Command::new("sleep").arg("60").spawn().unwrap()
        }

        /// `terminate` reaps the child it signals, so ask the system rather than the `Child`.
        fn alive(child: &mut std::process::Child) -> bool {
            process_alive(child.id() as i32)
        }

        #[test]
        fn a_reused_pid_signals_nothing_removes_nothing_and_fails_with_a_remedy() {
            let mut stranger = sleeper();
            let seeded = seed(stranger.id() as i32, None);
            // The process at the pid started at 999, and the meta recorded 100.
            let host = Host(HashMap::from([(stranger.id() as i32, 999)]));
            let err = stop(&seeded, &host).unwrap_err();
            assert!(matches!(err, VmError::VmStopUnproven { .. }), "{err:?}");
            assert_eq!(err.code(), "VM_STOP_FAILED");
            assert_eq!(err.exit_code(), 1);
            assert_eq!(err.details()["reason"], "start_time_differs");
            assert_eq!(err.details()["pid"], stranger.id());
            let remedy = err.remediation().unwrap();
            assert!(remedy.contains(&ID.to_string()) && remedy.contains(".meta.json"), "{remedy}");
            assert!(alive(&mut stranger), "the stranger at the pid was signalled");
            assert!(seeded.clone.exists(), "the clone was removed");
            assert!(seeded.session.exists(), "the session directory was removed");
            assert!(seeded.paths.spec_path(ID).exists());
            assert!(seeded.paths.meta_path(ID).exists());
            let _ = stranger.kill();
            let _ = stranger.wait();
        }

        #[test]
        fn a_matching_process_receives_the_signal_and_the_vm_is_removed() {
            let mut qemu = sleeper();
            let seeded = seed(qemu.id() as i32, None);
            let host = Host(HashMap::from([(qemu.id() as i32, 100)]));
            stop(&seeded, &host).unwrap();
            // `terminate` reaps the child, so a signalled child has an exit status now.
            assert!(!alive(&mut qemu), "the matching process was not signalled");
            assert!(!seeded.clone.exists());
            assert!(!seeded.session.exists());
            assert!(!seeded.paths.meta_path(ID).exists());
            assert!(!seeded.paths.spec_path(ID).exists());
        }

        #[test]
        fn a_process_that_is_gone_is_not_signalled_and_its_records_are_cleaned_up() {
            let mut bystander = sleeper();
            let seeded = seed(bystander.id() as i32, None);
            // The host knows no process at the pid, as for a process that exited.
            stop(&seeded, &Host(HashMap::new())).unwrap();
            assert!(alive(&mut bystander), "a gone process is never signalled");
            assert!(!seeded.clone.exists());
            assert!(!seeded.paths.meta_path(ID).exists());
            let _ = bystander.kill();
            let _ = bystander.wait();
        }

        #[test]
        fn a_swtpm_pid_that_names_another_process_is_not_signalled_beside_a_matching_qemu() {
            let mut qemu = sleeper();
            let mut stranger = sleeper();
            let seeded = seed(qemu.id() as i32, Some(stranger.id() as i32));
            let host = Host(HashMap::from([(qemu.id() as i32, 100), (stranger.id() as i32, 555)]));
            stop(&seeded, &host).unwrap();
            assert!(!alive(&mut qemu), "the matching QEMU was not signalled");
            assert!(alive(&mut stranger), "the swtpm pid that names a stranger was signalled");
            assert!(!seeded.clone.exists());
            let _ = stranger.kill();
            let _ = stranger.wait();
        }

        #[test]
        fn the_direct_stop_gives_a_reused_pid_the_same_refusal() {
            // `stop_with` reads the real host. A start time of 1 µs/tick matches no live process,
            // so the pid of this test process is a stranger to the meta.
            let seeded = seed(std::process::id() as i32, None);
            let err = VmLifecycle::stop_with(ID, &seeded.paths, &ToolSearch::from_process_path())
                .unwrap_err();
            assert!(matches!(err, VmError::VmStopUnproven { .. }), "{err:?}");
            assert!(seeded.clone.exists());
            assert!(seeded.paths.meta_path(ID).exists());
        }
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
            identity: None,
            keeper_identity: None,
            swtpm_pid: None,
            swtpm_identity: None,
            session_dir: None,
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

    // ---- which running tart VMs `vm list` shows ---------------------------

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn tart_clone_ids_lists_a_custom_id_that_has_sidecars() {
        let running = names(&["koine-verify-4242", "hand-made-vm"]);
        let ids = tart_clone_ids(running, |id| id == "koine-verify-4242");
        assert_eq!(ids, vec!["koine-verify-4242"]);
    }

    #[test]
    fn tart_clone_ids_still_lists_prefixed_clones_without_sidecars() {
        let running = names(&["testanyware-a1b2c3d4"]);
        assert_eq!(tart_clone_ids(running, |_| false), vec!["testanyware-a1b2c3d4"]);
    }

    #[test]
    fn tart_clone_ids_skips_a_running_golden_without_sidecars() {
        let running = names(&["testanyware-golden-macos-27"]);
        assert!(tart_clone_ids(running, |_| false).is_empty());
    }

    #[test]
    fn tart_clone_ids_ignores_sidecars_of_vms_that_are_not_running() {
        // `running` is the only source of candidates, so a stale sidecar for a
        // stopped or deleted VM cannot surface.
        assert!(tart_clone_ids(Vec::new(), |_| true).is_empty());
    }

    #[test]
    fn has_sidecars_sees_a_spec_or_a_meta_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        assert!(!has_sidecars(&paths, "koine-verify-1"));
        std::fs::write(paths.meta_path("koine-verify-1"), b"{}").unwrap();
        assert!(has_sidecars(&paths, "koine-verify-1"));
        std::fs::write(paths.spec_path("koine-verify-2"), b"{}").unwrap();
        assert!(has_sidecars(&paths, "koine-verify-2"));
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
                    identity: None,
                    keeper_identity: None,
                    swtpm_pid: None,
                    swtpm_identity: None,
                    session_dir: None,
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
                &Progress::stderr(),
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
            let left = settle_failed_start(&paths, &Progress::stderr(), &vm(), Disposition::Teardown, || {}, || true);
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
            let left = settle_failed_start(&paths, &Progress::stderr(), &vm(), Disposition::Teardown, || {}, || true);
            assert!(!left, "`vm stop` cannot find a VM with no meta");
        }

        #[test]
        fn keep_writes_the_meta_and_the_spec_and_tears_nothing_down() {
            let dir = tempfile::tempdir().unwrap();
            let paths = paths_in(dir.path());
            std::fs::create_dir_all(paths.vms_dir()).unwrap();
            let left = settle_failed_start(
                &paths,
                &Progress::stderr(),
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
            let left = settle_failed_start(&paths, &Progress::stderr(), &early, Disposition::Keep, || {}, || true);
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
                &Progress::stderr(),
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
                wait: None,
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
                identity: None,
                keeper_identity: None,
            };
            let failed = tart_failed_vm(&opts, crate::start_context::AGENT_PORT, 4242, &boot);
            assert_eq!(failed.meta.tool, VmTool::Tart);
            assert_eq!(failed.spec.unwrap().agent.unwrap().host, "192.168.64.9");
            boot.vnc = None;
            boot.ip = None;
            assert!(
                tart_failed_vm(&opts, crate::start_context::AGENT_PORT, 4242, &boot).spec.is_none(),
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
                wait: None,
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
                session_dir: dir.path().join("session"),
                identity: Some(identity(100)),
                swtpm_pid: Some(31338),
                swtpm_identity: Some(identity(101)),
            };
            (dir, artifacts)
        }

        fn identity(start_time: u64) -> ProcessIdentity {
            ProcessIdentity { start_time, boot_session: "boot-a".into() }
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
        fn the_meta_of_a_qemu_guest_records_every_process_and_its_session_directory() {
            let (_dir, art) = artifacts(51234, "");
            let meta = qemu_meta(&opts(false), &art);
            assert_eq!((meta.pid, meta.identity.clone()), (31337, Some(identity(100))));
            assert_eq!((meta.swtpm_pid, meta.swtpm_identity.clone()), (Some(31338), Some(identity(101))));
            assert_eq!(meta.session_dir, Some(art.session_dir.display().to_string()));
            assert_eq!((meta.keeper_pid, meta.keeper_identity), (None, None));
            // A VM that `vm stop` keeps after a failed start carries the same record.
            assert_eq!(qemu_failed_vm(&opts(true), &art).meta, qemu_meta(&opts(true), &art));
        }

        #[test]
        fn a_qemu_guest_without_a_tpm_records_no_swtpm() {
            let (_dir, mut art) = artifacts(51234, "");
            art.swtpm_pid = None;
            art.swtpm_identity = None;
            let json = serde_json::to_string(&qemu_meta(&opts(false), &art)).unwrap();
            assert!(!json.contains("swtpm"), "{json}");
        }

        #[test]
        fn a_successful_qemu_start_publishes_the_same_spec_a_kept_vm_gets() {
            let (_dir, art) = artifacts(51234, "");
            assert_eq!(qemu_spec(&opts(false), &art), qemu_failed_vm(&opts(false), &art).spec.unwrap());
        }
    }
}
