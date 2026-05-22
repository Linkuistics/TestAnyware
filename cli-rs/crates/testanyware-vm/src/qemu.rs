//! QEMU / swtpm orchestration. Port of `QEMURunner.swift`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::VmError;
use crate::monitor::QemuMonitorClient;
use crate::paths::VmPaths;
use crate::preflight::{check_kvm, check_swtpm};
use crate::process::{pgrep_first, process_alive, terminate};
use crate::qemu_profile::{host_profile, resolve_uefi_code, which, QemuProfile};

/// A golden image discovered on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenImage {
    pub name: String,
    pub platform: String,
    pub backend: &'static str,
}

/// A running QEMU clone discovered on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningClone {
    pub id: String,
    pub platform: String,
    pub backend: &'static str,
}

/// Inputs to `build_qemu_args` — the per-clone files and sockets.
#[derive(Debug, Clone)]
pub struct QemuLaunchSpec {
    pub uefi_code: PathBuf,
    pub clone_efivars: PathBuf,
    pub clone_qcow2: PathBuf,
    pub tpm_socket: PathBuf,
    pub monitor_socket: PathBuf,
    pub display: Option<String>,
}

/// Options for `QemuRunner::start`.
#[derive(Debug, Clone)]
pub struct QemuStartOptions {
    pub id: String,
    pub base: String,
    pub display: Option<String>,
    /// Whether the guest needs a TPM (Windows). Drives the swtpm preflight.
    pub needs_tpm: bool,
}

/// Result of a successful `QemuRunner::start`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartArtifacts {
    pub pid: i32,
    pub vnc_port: u16,
    pub agent_port: Option<u16>,
    pub clone_dir: PathBuf,
}

/// Classify a golden / clone name into a platform string. Ports
/// `QEMURunner.platformFromName`.
pub fn platform_from_name(name: &str) -> String {
    if name.contains("macos") || name.contains("tahoe") {
        "macos".into()
    } else if name.contains("linux") || name.contains("ubuntu") {
        "linux".into()
    } else if name.contains("windows") {
        "windows".into()
    } else {
        "unknown".into()
    }
}

/// Scan `<golden>/*.qcow2`. Ports `QEMURunner.scanGoldenDir`.
pub fn scan_golden_dir(golden_dir: &Path) -> Vec<GoldenImage> {
    let Ok(entries) = std::fs::read_dir(golden_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let file = entry.file_name();
        let file = file.to_string_lossy();
        if let Some(name) = file.strip_suffix(".qcow2") {
            out.push(GoldenImage {
                name: name.to_string(),
                platform: platform_from_name(name),
                backend: "qemu",
            });
        }
    }
    out
}

/// Scan `<clones>/` for running QEMU VMs by checking each clone's
/// TMPDIR-staged `monitor.sock`. Ports `QEMURunner.scanClonesDir` — the
/// clone subdirectory name is the VM id; the monitor socket lives at
/// `<sessions>/testanyware-<id>/monitor.sock`.
pub fn scan_clones_dir(clones_dir: &Path, sessions_root: &Path) -> Vec<RunningClone> {
    let Ok(entries) = std::fs::read_dir(clones_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        let sock = sessions_root.join(format!("testanyware-{id}")).join("monitor.sock");
        if sock.exists() {
            out.push(RunningClone {
                platform: platform_from_name(&id),
                id,
                backend: "qemu",
            });
        }
    }
    out
}

/// Build the QEMU argument vector. Pure — depends only on the host
/// profile and the per-clone spec. Ports the `qemuArgs` array in
/// `QEMURunner.start`, with the accelerator / machine / cpu taken from
/// the host profile rather than hard-coded to HVF/virt.
pub fn build_qemu_args(profile: &QemuProfile, spec: &QemuLaunchSpec) -> Vec<String> {
    let gpu = match &spec.display {
        Some(d) => {
            let parts: Vec<&str> = d.split('x').collect();
            if parts.len() == 2 {
                format!("virtio-gpu-pci,xres={},yres={}", parts[0], parts[1])
            } else {
                "virtio-gpu-pci".to_string()
            }
        }
        None => "virtio-gpu-pci".to_string(),
    };
    let s = |p: &Path| p.display().to_string();
    vec![
        "-machine".into(), profile.machine.into(),
        "-accel".into(), profile.accelerator.into(),
        "-cpu".into(), profile.cpu.into(),
        "-smp".into(), "4".into(),
        "-m".into(), "4096".into(),
        "-drive".into(), format!("if=pflash,format=raw,file={},readonly=on", s(&spec.uefi_code)),
        "-drive".into(), format!("if=pflash,format=raw,file={}", s(&spec.clone_efivars)),
        "-chardev".into(), format!("socket,id=chrtpm,path={}", s(&spec.tpm_socket)),
        "-tpmdev".into(), "emulator,id=tpm0,chardev=chrtpm".into(),
        "-device".into(), "tpm-tis-device,tpmdev=tpm0".into(),
        "-drive".into(), format!("file={},if=none,id=hd0,format=qcow2", s(&spec.clone_qcow2)),
        "-device".into(), "nvme,drive=hd0,serial=boot,bootindex=0".into(),
        "-device".into(), "ramfb".into(),
        "-device".into(), gpu,
        "-device".into(), "qemu-xhci".into(),
        "-device".into(), "usb-kbd".into(),
        "-device".into(), "usb-tablet".into(),
        "-device".into(), "virtio-net-pci,netdev=net0".into(),
        "-netdev".into(), "user,id=net0,hostfwd=tcp::0-:8648".into(),
        "-vnc".into(), "localhost:0,to=99,password=on".into(),
        "-monitor".into(), format!("unix:{},server,nowait", s(&spec.monitor_socket)),
        "-display".into(), "none".into(),
    ]
}

/// Run `program` with `args` synchronously, inheriting the parent's
/// stderr. Errors on a non-zero exit. Ports `QEMURunner.runAndCheck`.
async fn run_and_check(program: &str, args: &[String]) -> Result<(), VmError> {
    let status = tokio::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .await
        .map_err(|e| VmError::QemuFailed { detail: format!("{program}: {e}") })?;
    if status.success() {
        Ok(())
    } else {
        Err(VmError::QemuFailed {
            detail: format!("{program} {} exited {status}", args.join(" ")),
        })
    }
}

/// First PID holding any `.qcow2` in `dir`, via `lsof -t`. Ports
/// `QEMURunner.processHoldingQcow2`.
fn process_holding_qcow2(dir: &Path) -> Option<i32> {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg("lsof -t \"$1\"/*.qcow2 2>/dev/null | head -1")
        .arg("testanyware-qcow2-holder")
        .arg(dir.display().to_string())
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

/// QEMU / swtpm orchestration entry points.
pub struct QemuRunner;

impl QemuRunner {
    /// Clone the golden, start swtpm + QEMU detached, and discover the
    /// dynamic VNC/agent ports. Ports `QEMURunner.start`.
    pub async fn start(opts: &QemuStartOptions, paths: &VmPaths) -> Result<StartArtifacts, VmError> {
        // --- Preflight (constraint: KVM + swtpm) -------------------------
        check_kvm()?;
        if opts.needs_tpm {
            check_swtpm()?;
        }

        let clone_dir = paths.clone_dir(&opts.id);
        let golden_dir = paths.golden_dir();
        let session = paths.session_dir(&opts.id);

        // --- Reclaim a stale clone / session -----------------------------
        if clone_dir.exists() {
            if let Some(pid) = process_holding_qcow2(&clone_dir) {
                terminate(pid, Duration::from_millis(200), 10);
            }
            let _ = std::fs::remove_dir_all(&clone_dir);
        }
        if session.exists() {
            let _ = std::fs::remove_dir_all(&session);
        }
        std::fs::create_dir_all(&clone_dir)
            .map_err(|e| VmError::Io(format!("create {}: {e}", clone_dir.display())))?;
        std::fs::create_dir_all(&session)
            .map_err(|e| VmError::Io(format!("create {}: {e}", session.display())))?;

        // --- Clone the golden artefacts ----------------------------------
        let golden_qcow2 = golden_dir.join(format!("{}.qcow2", opts.base));
        let clone_qcow2 = clone_dir.join(format!("{}.qcow2", opts.id));
        let qemu_img = which("qemu-img")
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "qemu-img".to_string());
        run_and_check(&qemu_img, &[
            "create".into(), "-f".into(), "qcow2".into(),
            "-b".into(), golden_qcow2.display().to_string(),
            "-F".into(), "qcow2".into(),
            clone_qcow2.display().to_string(),
        ])
        .await
        .inspect_err(|_| teardown(0, &clone_dir, &session))?;

        let golden_efivars = golden_dir.join(format!("{}-efivars.fd", opts.base));
        let clone_efivars = clone_dir.join(format!("{}-efivars.fd", opts.id));
        std::fs::copy(&golden_efivars, &clone_efivars)
            .map_err(|e| VmError::Io(format!("copy efivars: {e}")))
            .inspect_err(|_| teardown(0, &clone_dir, &session))?;

        let golden_tpm = golden_dir.join(format!("{}-tpm", opts.base));
        let clone_tpm_dir = clone_dir.join(format!("{}-tpm", opts.id));
        run_and_check("cp", &[
            "-r".into(),
            golden_tpm.display().to_string(),
            clone_tpm_dir.display().to_string(),
        ])
        .await
        .inspect_err(|_| teardown(0, &clone_dir, &session))?;

        // --- Resolve UEFI firmware ---------------------------------------
        let profile = host_profile();
        let uefi_code = resolve_uefi_code(&profile.uefi_code_candidates).ok_or_else(|| {
            teardown(0, &clone_dir, &session);
            VmError::UefiNotFound {
                path: profile
                    .uefi_code_candidates
                    .first()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "<no candidate paths>".into()),
            }
        })?;

        // --- Start swtpm (sockets staged under $TMPDIR) ------------------
        let tpm_socket = session.join("swtpm-sock");
        let monitor_socket = session.join("monitor.sock");
        let swtpm = which("swtpm")
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "swtpm".to_string());
        run_and_check(&swtpm, &[
            "socket".into(),
            "--tpmstate".into(), format!("dir={}", clone_tpm_dir.display()),
            "--ctrl".into(), format!("type=unixio,path={}", tpm_socket.display()),
            "--tpm2".into(),
            "--log".into(), "level=0".into(),
            "--daemon".into(),
        ])
        .await
        .inspect_err(|_| teardown(0, &clone_dir, &session))?;
        tokio::time::sleep(Duration::from_secs(1)).await;

        // --- Launch QEMU detached ----------------------------------------
        let launch = QemuLaunchSpec {
            uefi_code,
            clone_efivars,
            clone_qcow2,
            tpm_socket,
            monitor_socket: monitor_socket.clone(),
            display: opts.display.clone(),
        };
        let args = build_qemu_args(&profile, &launch);
        let qemu_bin = which(profile.qemu_binary)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| profile.qemu_binary.to_string());
        let log_path = clone_dir.join("qemu.log");
        let pid = crate::detached::spawn_detached(&qemu_bin, &args, &log_path)
            .inspect_err(|_| teardown(0, &clone_dir, &session))?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        if !process_alive(pid) {
            // Pass `pid`, not 0: once spawn_detached has handed back a
            // pid, every failure routes the pid through teardown for
            // consistency. `terminate` is a safe no-op on a dead pid.
            teardown(pid, &clone_dir, &session);
            return Err(VmError::QemuFailed {
                detail: "QEMU did not remain running after launch".into(),
            });
        }

        // --- Monitor: set VNC password, discover ports -------------------
        let monitor = QemuMonitorClient::new(&monitor_socket);
        monitor.set_vnc_password("testanyware", 3).await;

        let agent_port = monitor.agent_port(5, Duration::from_secs(1)).await;
        if agent_port.is_none() {
            teardown(pid, &clone_dir, &session);
            return Err(VmError::MonitorDiscoveryFailed);
        }
        let vnc_port = monitor
            .vnc_port(3, Duration::from_millis(500))
            .await
            .unwrap_or(5900);

        Ok(StartArtifacts { pid, vnc_port, agent_port, clone_dir })
    }

    /// Tear down a running QEMU VM. Public wrapper deriving the session
    /// dir from the clone-dir basename (the VM id). Ports `QEMURunner.stop`.
    pub fn stop(pid: i32, clone_dir: &Path, paths: &VmPaths) {
        let id = clone_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        teardown(pid, clone_dir, &paths.session_dir(&id));
    }

    /// Remove a golden image's three artefacts. Idempotent. Ports
    /// `QEMURunner.deleteGolden`.
    pub fn delete_golden(name: &str, golden_dir: &Path) {
        let _ = std::fs::remove_file(golden_dir.join(format!("{name}.qcow2")));
        let _ = std::fs::remove_file(golden_dir.join(format!("{name}-efivars.fd")));
        let _ = std::fs::remove_dir_all(golden_dir.join(format!("{name}-tpm")));
    }

    /// PIDs of running clones whose backing qcow2 is `golden_name`. Ports
    /// `QEMURunner.runningClonesBacked`.
    pub fn running_clones_backed_by(golden_name: &str, paths: &VmPaths) -> Vec<i32> {
        let golden_qcow2 = paths.golden_dir().join(format!("{golden_name}.qcow2"));
        let Ok(entries) = std::fs::read_dir(paths.clones_dir()) else {
            return Vec::new();
        };
        let mut pids = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let Ok(files) = std::fs::read_dir(&dir) else { continue };
            for f in files.flatten() {
                let p = f.path();
                if p.extension().and_then(|e| e.to_str()) != Some("qcow2") {
                    continue;
                }
                if backing_file(&p).as_deref() == Some(golden_qcow2.as_path()) {
                    if let Some(pid) = process_holding_qcow2(&dir) {
                        pids.push(pid);
                    }
                }
            }
        }
        pids
    }
}

/// Shared teardown: SIGTERM→SIGKILL the qemu pid, pgrep-kill the swtpm
/// daemon by its TPM state-dir path, then remove the clone + session
/// dirs. Idempotent — `pid: 0` skips the qemu kill. Ports
/// `QEMURunner.teardown`.
pub fn teardown(pid: i32, clone_dir: &Path, session_dir: &Path) {
    if pid > 0 {
        terminate(pid, Duration::from_millis(100), 20);
    }
    // swtpm has no registry: locate it by its --tpmstate dir and kill it.
    let clone_name = clone_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tpm_dir = clone_dir.join(format!("{clone_name}-tpm"));
    if let Some(swtpm_pid) = pgrep_first(&format!("swtpm.*{}", tpm_dir.display())) {
        terminate(swtpm_pid, Duration::from_millis(200), 5);
    }
    if clone_dir.exists() {
        let _ = std::fs::remove_dir_all(clone_dir);
    }
    if session_dir.exists() {
        let _ = std::fs::remove_dir_all(session_dir);
    }
}

/// `full-backing-filename` from `qemu-img info --output=json`. Ports
/// `QEMURunner.backingFile`.
fn backing_file(qcow2: &Path) -> Option<PathBuf> {
    let qemu_img = which("qemu-img")?;
    let output = std::process::Command::new(qemu_img)
        .args(["info", "--output=json"])
        .arg(qcow2)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("full-backing-filename")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn platform_from_name_classifies_image_names() {
        assert_eq!(platform_from_name("testanyware-golden-macos-tahoe"), "macos");
        assert_eq!(platform_from_name("testanyware-golden-linux-24.04"), "linux");
        assert_eq!(platform_from_name("ubuntu-server"), "linux");
        assert_eq!(platform_from_name("testanyware-golden-windows-11"), "windows");
        assert_eq!(platform_from_name("mystery-image"), "unknown");
    }

    #[test]
    fn scan_golden_dir_lists_qcow2_images() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("testanyware-golden-windows-11.qcow2"), b"x").unwrap();
        fs::write(dir.path().join("testanyware-golden-linux-24.04.qcow2"), b"x").unwrap();
        fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        let mut names: Vec<String> =
            scan_golden_dir(dir.path()).into_iter().map(|g| g.name).collect();
        names.sort();
        assert_eq!(names, vec![
            "testanyware-golden-linux-24.04",
            "testanyware-golden-windows-11",
        ]);
    }

    #[test]
    fn scan_golden_dir_is_empty_for_a_missing_directory() {
        assert!(scan_golden_dir(std::path::Path::new("/no/such/dir/xyzzy")).is_empty());
    }

    #[test]
    fn scan_clones_dir_reports_a_clone_with_a_live_monitor_socket() {
        let clones = tempfile::tempdir().unwrap();
        let sessions = tempfile::tempdir().unwrap();
        // Clone "testanyware-aa" has a monitor.sock in its session dir => running.
        let id = "testanyware-aa";
        fs::create_dir_all(clones.path().join(id)).unwrap();
        let sess = sessions.path().join(format!("testanyware-{id}"));
        fs::create_dir_all(&sess).unwrap();
        fs::write(sess.join("monitor.sock"), b"").unwrap();
        // Clone "testanyware-bb" has no session dir => not running.
        fs::create_dir_all(clones.path().join("testanyware-bb")).unwrap();

        let running = scan_clones_dir(clones.path(), sessions.path());
        let names: Vec<&str> = running.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(names, vec![id]);
    }

    #[test]
    fn build_qemu_args_wires_display_and_sockets() {
        let spec = QemuLaunchSpec {
            uefi_code: std::path::PathBuf::from("/fw/code.fd"),
            clone_efivars: std::path::PathBuf::from("/c/efivars.fd"),
            clone_qcow2: std::path::PathBuf::from("/c/disk.qcow2"),
            tpm_socket: std::path::PathBuf::from("/s/swtpm-sock"),
            monitor_socket: std::path::PathBuf::from("/s/monitor.sock"),
            display: Some("1920x1080".into()),
        };
        let args = build_qemu_args(&host_profile(), &spec);
        let joined = args.join(" ");
        assert!(joined.contains("xres=1920,yres=1080"), "display wired: {joined}");
        assert!(joined.contains("hostfwd=tcp::0-:8648"), "agent forward wired: {joined}");
        assert!(joined.contains("unix:/s/monitor.sock,server,nowait"), "monitor wired: {joined}");
        assert!(joined.contains("path=/s/swtpm-sock"), "tpm chardev wired: {joined}");
        assert!(joined.contains("password=on"), "vnc password gating wired: {joined}");
        assert!(args.contains(&"-accel".to_string()));
    }

    #[test]
    fn build_qemu_args_omits_display_geometry_when_absent() {
        let spec = QemuLaunchSpec {
            uefi_code: "/fw/code.fd".into(),
            clone_efivars: "/c/efivars.fd".into(),
            clone_qcow2: "/c/disk.qcow2".into(),
            tpm_socket: "/s/swtpm-sock".into(),
            monitor_socket: "/s/monitor.sock".into(),
            display: None,
        };
        let joined = build_qemu_args(&host_profile(), &spec).join(" ");
        assert!(!joined.contains("xres="), "no geometry without --display: {joined}");
        assert!(joined.contains("virtio-gpu-pci"), "still wires a GPU: {joined}");
    }
}
