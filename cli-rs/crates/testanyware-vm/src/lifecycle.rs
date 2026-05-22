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
    /// Start a VM end-to-end. QEMU backend only — `macos` (tart) returns
    /// `BackendUnsupported`. Ports `VMLifecycle.start` / `startQEMU`.
    pub async fn start(opts: &VmStartOptions, paths: &VmPaths) -> Result<VmStartResult, VmError> {
        if opts.platform == Platform::Macos {
            return Err(VmError::BackendUnsupported { platform: "macos".into() });
        }
        std::fs::create_dir_all(paths.vms_dir())
            .map_err(|e| VmError::Io(format!("create {}: {e}", paths.vms_dir().display())))?;

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

    /// Stop a VM and remove its sidecars. Ports `VMLifecycle.stop`
    /// (QEMU branch). A `tart` meta returns `BackendUnsupported`.
    pub fn stop(id: &str, paths: &VmPaths) -> Result<(), VmError> {
        let spec_path = paths.spec_path(id);
        let meta_path = paths.meta_path(id);
        if !meta_path.is_file() {
            return Err(VmError::VmNotFound { id: id.to_string() });
        }
        let meta = VmMeta::load(&meta_path)?;
        match meta.tool {
            VmTool::Tart => {
                return Err(VmError::BackendUnsupported { platform: "macos (tart)".into() });
            }
            VmTool::Qemu => {
                let clone_dir = meta.clone_dir.clone().ok_or_else(|| VmError::VmStopFailed {
                    id: id.to_string(),
                })?;
                QemuRunner::stop(meta.pid, std::path::Path::new(&clone_dir), paths);
            }
        }
        let _ = std::fs::remove_file(&spec_path);
        let _ = std::fs::remove_file(&meta_path);
        Ok(())
    }

    /// Delete a QEMU golden image by name. Refuses when running clones
    /// depend on it unless `force`. Ports `VMLifecycle.delete` (QEMU
    /// branch; tart detection is backlog task 12).
    pub fn delete(name: &str, force: bool, paths: &VmPaths) -> Result<(), VmError> {
        let golden_dir = paths.golden_dir();
        let qcow2 = golden_dir.join(format!("{name}.qcow2"));
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

    /// List goldens and running clones. Ports `VMCommand.List` (QEMU
    /// entries only). Running clones are enriched from their sidecars.
    pub fn list(paths: &VmPaths) -> VmListing {
        let goldens = scan_golden_dir(&paths.golden_dir());
        let raw: Vec<RunningClone> =
            scan_clones_dir(&paths.clones_dir(), &session_root(paths));
        let running = raw
            .into_iter()
            .map(|clone| enrich_running(&clone, paths))
            .collect();
        VmListing { goldens, running }
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
    RunningEntry {
        id: clone.id.clone(),
        platform: clone.platform.clone(),
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
    fn list_returns_goldens_and_running_clones() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(paths.golden_dir()).unwrap();
        std::fs::write(paths.golden_dir().join("testanyware-golden-linux-24.04.qcow2"), b"x").unwrap();
        let listing = VmLifecycle::list(&paths);
        assert_eq!(listing.goldens.len(), 1);
        assert_eq!(listing.running.len(), 0);
    }
}
