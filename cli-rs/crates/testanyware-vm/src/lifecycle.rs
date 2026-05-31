//! End-to-end VM lifecycle orchestrator. Port of `VMLifecycle.swift`
//! (QEMU paths only; tart is a separate backlog task).

use std::path::PathBuf;
use std::time::Duration;

use crate::error::VmError;
use crate::health::wait_for_agent;
use crate::id::generate_id;
use crate::meta::{VmMeta, VmTool};
use crate::paths::VmPaths;
use crate::process::process_alive;
use crate::qemu::{
    scan_clones_dir, scan_golden_dir, GoldenImage, QemuRunner, QemuStartOptions, RunningClone,
};
use crate::spec::{AgentEndpoint, VmSpec, VncEndpoint};

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
            Platform::Macos => "testanyware-golden-macos-tahoe",
            Platform::Linux => "testanyware-golden-linux-24.04",
            Platform::Windows => "testanyware-golden-windows-11",
        }
    }

    /// Windows guests need a TPM 2.0 socket.
    fn needs_tpm(self) -> bool {
        matches!(self, Platform::Windows)
    }
}

/// Inputs for `VmLifecycle::start`. Ports `VMStartOptions`.
#[derive(Debug, Clone)]
pub struct VmStartOptions {
    pub platform: Platform,
    pub base: String,
    pub id: String,
    pub display: Option<String>,
    pub open_viewer: bool,
}

impl VmStartOptions {
    pub fn new(
        platform: Platform,
        base: Option<String>,
        id: Option<String>,
        display: Option<String>,
        open_viewer: bool,
    ) -> Self {
        Self {
            platform,
            base: base.unwrap_or_else(|| platform.default_base().to_string()),
            id: id.unwrap_or_else(generate_id),
            display,
            open_viewer,
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
    /// `true` when the agent did not reach health within the boot window
    /// (the VM still started; agent commands will fail until it comes up).
    pub agent_unreachable: bool,
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

        let qopts = QemuStartOptions {
            id: opts.id.clone(),
            base: opts.base.clone(),
            display: opts.display.clone(),
            needs_tpm: opts.platform.needs_tpm(),
        };
        let artifacts = QemuRunner::start(&qopts, paths).await?;

        // Wait for the agent. Unreachable is a warning, not a failure —
        // the VM started; the spec is written with `agent: null`. Matches
        // Swift `startQEMU` (120 attempts x 5 s).
        let mut agent_endpoint = None;
        let mut agent_unreachable = true;
        if let Some(port) = artifacts.agent_port {
            if wait_for_agent("localhost", port, 120, Duration::from_secs(5)).await {
                agent_endpoint = Some(AgentEndpoint { host: "localhost".into(), port });
                agent_unreachable = false;
            }
        }

        let spec = VmSpec {
            vnc: VncEndpoint {
                host: "localhost".into(),
                port: artifacts.vnc_port,
                password: Some("testanyware".into()),
            },
            agent: agent_endpoint,
            platform: opts.platform.as_str().to_string(),
        };
        let spec_path = paths.spec_path(&opts.id);
        let meta_path = paths.meta_path(&opts.id);
        // QEMU is now running; a sidecar-write failure must tear it down so it
        // cannot survive as an unkillable orphan.
        spec.write_atomic(&spec_path).inspect_err(|_| {
            QemuRunner::stop(artifacts.pid, &artifacts.clone_dir, paths);
        })?;

        // Viewer wiring is backlog task 8; `viewer_window_id` stays null.
        let meta = VmMeta {
            id: opts.id.clone(),
            tool: VmTool::Qemu,
            pid: artifacts.pid,
            clone_dir: Some(artifacts.clone_dir.display().to_string()),
            viewer_window_id: None,
        };
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
            agent_unreachable,
        })
    }

    /// Start a tart-backed VM end-to-end. macOS host only. Mirrors the
    /// QEMU arm above: the runner clones+starts and hands back the VNC
    /// endpoint + guest IP; this method waits for the agent (over the
    /// guest IP, not a localhost forward) and writes the sidecars. Ports
    /// `VMLifecycle.startTart`.
    #[cfg(target_os = "macos")]
    async fn start_tart(opts: &VmStartOptions, paths: &VmPaths) -> Result<VmStartResult, VmError> {
        use crate::tart::{TartRunner, TartStartOptions};

        let topts = TartStartOptions {
            id: opts.id.clone(),
            base: opts.base.clone(),
            display: opts.display.clone(),
        };
        let artifacts = TartRunner::start(&topts, paths).await?;

        // The agent listens on the guest IP (port 8648), not a localhost
        // forward as with QEMU. No IP => agent unreachable, spec `agent: null`.
        let mut agent_endpoint = None;
        let mut agent_unreachable = true;
        if let Some(ip) = &artifacts.ip {
            if wait_for_agent(ip, 8648, 60, Duration::from_secs(2)).await {
                agent_endpoint = Some(AgentEndpoint { host: ip.clone(), port: 8648 });
                agent_unreachable = false;
            }
        }

        let spec = VmSpec {
            vnc: VncEndpoint {
                host: artifacts.vnc.host.clone(),
                port: artifacts.vnc.port,
                password: artifacts.vnc.password.clone(),
            },
            agent: agent_endpoint,
            platform: opts.platform.as_str().to_string(),
        };
        let spec_path = paths.spec_path(&opts.id);
        let meta_path = paths.meta_path(&opts.id);
        // tart is now running; a sidecar-write failure must tear it down
        // so it cannot survive untracked.
        spec.write_atomic(&spec_path).inspect_err(|_| {
            TartRunner::stop(&opts.id, artifacts.pid);
        })?;

        // tart manages its own storage, so the meta carries no clone_dir;
        // `pid` is the detached `tart run` process tracked for `stop`.
        let meta = VmMeta {
            id: opts.id.clone(),
            tool: VmTool::Tart,
            pid: artifacts.pid,
            clone_dir: None,
            viewer_window_id: None,
        };
        meta.write_atomic(&meta_path).inspect_err(|_| {
            TartRunner::stop(&opts.id, artifacts.pid);
            let _ = std::fs::remove_file(&spec_path);
        })?;

        Ok(VmStartResult {
            id: opts.id.clone(),
            platform: opts.platform,
            spec,
            spec_path,
            meta_path,
            agent_unreachable,
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
                if crate::tart::TartRunner::stop(id, meta.pid) {
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
    /// the host preflight for the QEMU path. Returns the backend name so
    /// the caller can report it. Keeps the per-platform routing (and the
    /// macOS-only tart lookups) inside the crate, off the command layer.
    pub fn dry_run_validate_start(opts: &VmStartOptions, paths: &VmPaths) -> Result<&'static str, VmError> {
        #[cfg(target_os = "macos")]
        if wants_tart(opts.platform, &opts.base, paths) {
            if !crate::tart::golden_exists(&opts.base) {
                return Err(VmError::GoldenNotFound { name: opts.base.clone() });
            }
            return Ok("tart");
        }

        if opts.platform == Platform::Macos {
            return Err(VmError::BackendUnsupported { platform: "macos".into() });
        }
        let qcow2 = paths.golden_dir().join(format!("{}.qcow2", opts.base));
        if !qcow2.is_file() {
            return Err(VmError::GoldenNotFound { name: opts.base.clone() });
        }
        crate::preflight::check_kvm()?;
        if opts.platform == Platform::Windows {
            crate::preflight::check_swtpm()?;
        }
        Ok("qemu")
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
        assert_eq!(Platform::Macos.default_base(), "testanyware-golden-macos-tahoe");
    }

    #[test]
    fn start_options_fill_in_base_and_id_defaults() {
        let opts = VmStartOptions::new(Platform::Windows, None, None, None, false);
        assert_eq!(opts.base, "testanyware-golden-windows-11");
        assert!(opts.id.starts_with("testanyware-"));
        let explicit = VmStartOptions::new(
            Platform::Linux,
            Some("custom-base".into()),
            Some("testanyware-fixedid".into()),
            Some("800x600".into()),
            false,
        );
        assert_eq!(explicit.base, "custom-base");
        assert_eq!(explicit.id, "testanyware-fixedid");
        assert_eq!(explicit.display.as_deref(), Some("800x600"));
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
        let opts = VmStartOptions::new(Platform::Macos, None, None, None, false);
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
}
