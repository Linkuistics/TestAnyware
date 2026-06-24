//! tart VM orchestration (macOS host only). Port of `TartRunner.swift`.
//!
//! tart wraps Apple's Virtualization.framework, so this backend is
//! reachable only on a macOS host — the module is `#[cfg(target_os =
//! "macos")]`-gated at the crate root and never referenced in a Linux or
//! Windows build (consistent with ADR-0003's per-target gating).
//!
//! Pure parsers (`parse_vnc_url`, `parse_goldens`, `parse_running_ids`,
//! `poll_vnc_url`) are unit-tested without invoking `tart`. The
//! `tart`-invoking operations (`clone`, `run_detached`, `start`, …) are
//! exercised by the manual `vm start --platform macos` verification the
//! `010-tart-runner` leaf calls for, not by unit tests.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::error::VmError;
use crate::qemu::{platform_from_name, GoldenImage};
use crate::qemu_profile::which;

/// One `tart list --format json` row. tart capitalizes its keys.
#[derive(Debug, Deserialize)]
struct TartVm {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "State")]
    state: Option<String>,
}

/// Decode `tart list --format json`, returning `[]` on malformed input —
/// the boundary to an externally-owned tool absorbs schema drift so a
/// tart upgrade cannot break `vm list`. Ports the leniency shared by
/// `parseList` / `parseAllVMNames` / `parseAllVMs`.
fn decode_list(tart_json: &str) -> Vec<TartVm> {
    if tart_json.is_empty() {
        return Vec::new();
    }
    serde_json::from_str(tart_json).unwrap_or_default()
}

/// A parsed `vnc://[:password@]host:port` URL emitted by `tart run
/// --vnc-experimental`. Ports `TartRunner.VNCURL`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VncUrl {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
}

/// Parse a `vnc://[:password@]host:port[...]` URL.
///
/// tart appends a trailing `...` progress marker; strip it first. The
/// password-only credential form (`:pw@host`) is hand-parsed because URL
/// libraries do not reliably handle it. Ports `TartRunner.parseVNCURL`.
pub fn parse_vnc_url(raw: &str) -> Result<VncUrl, VmError> {
    let malformed = || VmError::TartFailed { detail: format!("malformed VNC URL from tart: {raw}") };
    let mut s = raw.trim();
    s = s.trim_end_matches('.');
    let rest = s.strip_prefix("vnc://").ok_or_else(malformed)?;
    // Optional `[:]password@` credential, hand-split: URL parsers mishandle
    // the password-only (`:pw@host`) form tart emits.
    let (password, host_port) = match rest.split_once('@') {
        Some((cred, after)) => {
            let cred = cred.strip_prefix(':').unwrap_or(cred);
            let pw = if cred.is_empty() { None } else { Some(cred.to_string()) };
            (pw, after)
        }
        None => (None, rest),
    };
    let (host, port) = host_port.rsplit_once(':').ok_or_else(malformed)?;
    let port: u16 = port.parse().map_err(|_| malformed())?;
    if host.is_empty() {
        return Err(malformed());
    }
    Ok(VncUrl { host: host.to_string(), port, password })
}

/// Golden images from `tart list --format json`: every entry whose name
/// starts with `testanyware-golden-`, regardless of run state. Ports the
/// `.golden` arm of `TartRunner.parseList`. Malformed JSON yields `[]` —
/// a tart upgrade must not break `vm list` (boundary leniency).
pub fn parse_goldens(tart_json: &str) -> Vec<GoldenImage> {
    decode_list(tart_json)
        .into_iter()
        .filter(|vm| vm.name.starts_with("testanyware-golden-"))
        .map(|vm| GoldenImage {
            platform: platform_from_name(&vm.name),
            name: vm.name,
            backend: "tart",
        })
        .collect()
}

/// Running-clone ids from `tart list --format json`: entries with
/// `state == "running"` whose name starts with `testanyware-` but not
/// `testanyware-golden-`. Ports the `.running` arm of
/// `TartRunner.parseList`. Malformed JSON yields `[]`.
pub fn parse_running_ids(tart_json: &str) -> Vec<String> {
    decode_list(tart_json)
        .into_iter()
        .filter(|vm| {
            vm.state.as_deref() == Some("running")
                && vm.name.starts_with("testanyware-")
                && !vm.name.starts_with("testanyware-golden-")
        })
        .map(|vm| vm.name)
        .collect()
}

/// Every VM name in the catalog, regardless of state or prefix. Used by
/// lifecycle paths (collision detection, existence checks) that address
/// user-supplied `--id`s not following the `testanyware-` convention.
/// Ports `TartRunner.parseAllVMNames`. Malformed JSON yields `[]`.
pub fn parse_all_names(tart_json: &str) -> Vec<String> {
    decode_list(tart_json).into_iter().map(|vm| vm.name).collect()
}

/// Poll `log_path` for the first `vnc://…` URL, up to `attempts` times
/// spaced by `interval`. A missing log file is "not yet"; the loop keeps
/// polling until the deadline. Returns `None` on timeout. Ports
/// `TartRunner.pollVNCURL`.
pub fn poll_vnc_url(log_path: &Path, attempts: u32, interval: Duration) -> Option<VncUrl> {
    for attempt in 0..attempts {
        if let Ok(text) = std::fs::read_to_string(log_path) {
            if let Some(url) = text.split_whitespace().find(|t| t.starts_with("vnc://")) {
                if let Ok(parsed) = parse_vnc_url(url) {
                    return Some(parsed);
                }
            }
        }
        if attempt + 1 < attempts {
            std::thread::sleep(interval);
        }
    }
    None
}

// ---- live `tart`-invoking operations -----------------------------------
//
// These shell out to the `tart` binary, so they are not unit-tested; the
// `010-tart-runner` leaf verifies them with a manual `vm start
// --platform macos` against the real golden (clone+start is cheap —
// `vm-costs`).

/// Outcome of a synchronous `tart` invocation.
struct TartResult {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

/// Run `tart <args>` synchronously, capturing stdout/stderr. A missing
/// binary yields `exit_code: -1`. Ports `TartRunner.runTart`.
fn run_tart(args: &[&str]) -> TartResult {
    let Some(tart) = which("tart") else {
        return TartResult { exit_code: -1, stdout: String::new(), stderr: "tart not found on PATH".into() };
    };
    match std::process::Command::new(tart).args(args).output() {
        Ok(out) => TartResult {
            exit_code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        },
        Err(e) => TartResult { exit_code: -1, stdout: String::new(), stderr: e.to_string() },
    }
}

/// `tart list --format json` stdout, or `None` when tart is absent or the
/// invocation fails. Ports `TartRunner.tartListJSON`.
fn tart_list_json() -> Option<String> {
    let r = run_tart(&["list", "--format", "json"]);
    (r.exit_code == 0).then_some(r.stdout)
}

/// tart goldens currently in the catalog. `[]` when tart is absent.
pub fn list_goldens() -> Vec<GoldenImage> {
    tart_list_json().map(|j| parse_goldens(&j)).unwrap_or_default()
}

/// Ids of running tart clones. `[]` when tart is absent.
pub fn list_running_ids() -> Vec<String> {
    tart_list_json().map(|j| parse_running_ids(&j)).unwrap_or_default()
}

/// Whether a tart VM with `name` exists in any state. `false` when tart
/// is absent. Ports `TartRunner.vmExists`.
pub fn vm_exists(name: &str) -> bool {
    tart_list_json().map(|j| parse_all_names(&j).iter().any(|n| n == name)).unwrap_or(false)
}

/// Whether `name` is a tart golden image. Ports the tart arm of
/// `VMLifecycle.delete`'s backend detection.
pub fn golden_exists(name: &str) -> bool {
    list_goldens().iter().any(|g| g.name == name)
}

/// `tart clone <base> <id>`. Ports `TartRunner.clone`.
pub fn clone(base: &str, id: &str) -> Result<(), VmError> {
    let r = run_tart(&["clone", base, id]);
    if r.exit_code == 0 {
        Ok(())
    } else {
        Err(VmError::TartFailed { detail: format!("tart clone {base} {id}: {}", r.stderr.trim()) })
    }
}

/// Default guest display when `--display` is omitted (ADR-0013): 1920×1080
/// **px**. The explicit `px` is load-bearing — `tart set --display`'s unit
/// hint defaults to *points* for macOS VMs (`tart set --help`, tart 2.32.1),
/// so a bare `1920x1080` would yield a 3840×2160-px framebuffer at 2× backing
/// scale; `px` pins it to a 1920×1080-px (LoDPI) framebuffer — the contract
/// the vision pipeline consumes.
const DEFAULT_DISPLAY: &str = "1920x1080px";

/// The `tart set --display` value to apply: the user's `--display` verbatim,
/// or [`DEFAULT_DISPLAY`] when omitted. We set a default; we never rewrite an
/// explicit value (ADR-0013).
pub(crate) fn resolve_display(requested: Option<&str>) -> &str {
    requested.unwrap_or(DEFAULT_DISPLAY)
}

/// `tart set <id> --display <WxH>`. Ports `TartRunner.setDisplay`.
pub fn set_display(id: &str, display: &str) -> Result<(), VmError> {
    let r = run_tart(&["set", id, "--display", display]);
    if r.exit_code == 0 {
        Ok(())
    } else {
        Err(VmError::TartFailed { detail: format!("tart set {id} --display {display}: {}", r.stderr.trim()) })
    }
}

/// Best-effort `tart stop` then `tart delete` — both no-ops on an absent
/// VM, so non-zero exits are ignored. Ports `TartRunner.removeExisting`.
pub fn remove_existing(id: &str) {
    let _ = run_tart(&["stop", id]);
    let _ = run_tart(&["delete", id]);
}

/// Best-effort `tart stop <id>` that **stops without deleting** (unlike
/// [`remove_existing`]). Used by the recovery cycle ([`crate::recovery`]),
/// which must stop the setup VM, boot it into recovery, then boot it back to
/// normal — all on the same clone, so a force-stop must not destroy it. Ports
/// the script's `_stop_vm_graceful` force path (`tart stop`, line 345).
pub fn stop(id: &str) {
    let _ = run_tart(&["stop", id]);
}

/// `tart delete <name>` for a golden. `true` on success. Ports
/// `TartRunner.deleteGolden`.
pub fn delete_golden(name: &str) -> bool {
    run_tart(&["delete", name]).exit_code == 0
}

/// The per-run log path for `id`, with any prior run's log of the **same
/// id** cleared first.
///
/// `spawn_detached` opens this log in **append** mode (it is shared with
/// the QEMU path, where append is correct), every same-id `tart run` points
/// at the *same* `<id>.tart.log`, and `vm stop` removes the spec/meta
/// sidecars but **not** this log. So without clearing it, a same-id
/// `stop`→`start` bounce appends the new run's `vnc://` line *after* the
/// prior run's — and `poll_vnc_url` returns the *first* match, resolving the
/// prior run's now-dead port + password. Clearing the log here (rather than
/// truncating in shared `spawn_detached`, which would alter QEMU, or
/// removing on `stop`, which a crash/kill skips) makes `start` idempotent
/// regardless of how the prior run ended. The current run's log is fully
/// preserved — only the prior, now-deleted clone's log is discarded.
fn fresh_log_path(log_dir: &Path, id: &str) -> PathBuf {
    let log_path = log_dir.join(format!("{id}.tart.log"));
    // Best-effort: a missing log is the normal fresh-id case.
    let _ = std::fs::remove_file(&log_path);
    log_path
}

/// Spawn `tart run <id> --no-graphics --vnc-experimental` detached, with
/// stdout+stderr appended to a freshly-cleared `<log_dir>/<id>.tart.log`
/// (see `fresh_log_path`). Returns the detached pid and the log path so the
/// caller can poll for the VNC URL. Ports `TartRunner.runDetached`.
pub fn run_detached(id: &str, log_dir: &Path) -> Result<(i32, PathBuf), VmError> {
    run_detached_with(id, log_dir, &[])
}

/// Like [`run_detached`] but boots into **macOS Recovery**
/// (`tart run … --recovery --no-graphics --vnc-experimental`). The recovery
/// driver ([`crate::recovery`], ADR-0008) drives the resulting VNC framebuffer
/// to toggle SIP; recovery has no SSH, so this is the one boot that cannot go
/// over the provisioning layer. Ports the `--recovery` `tart run` in the
/// script's `_recovery_boot_csrutil` (line 408).
pub fn run_detached_recovery(id: &str, log_dir: &Path) -> Result<(i32, PathBuf), VmError> {
    run_detached_with(id, log_dir, &["--recovery"])
}

/// Shared body for the detached `tart run` variants. `extra` flags are
/// inserted before the always-on `--no-graphics --vnc-experimental` pair.
fn run_detached_with(id: &str, log_dir: &Path, extra: &[&str]) -> Result<(i32, PathBuf), VmError> {
    std::fs::create_dir_all(log_dir)
        .map_err(|e| VmError::Io(format!("create {}: {e}", log_dir.display())))?;
    let tart = which("tart")
        .ok_or_else(|| VmError::TartFailed { detail: "tart not found on PATH".into() })?;
    let log_path = fresh_log_path(log_dir, id);
    let mut args = vec!["run".to_string(), id.to_string()];
    args.extend(extra.iter().map(|s| s.to_string()));
    args.push("--no-graphics".to_string());
    args.push("--vnc-experimental".to_string());
    let pid = crate::detached::spawn_detached(&tart.display().to_string(), &args, &log_path)?;
    Ok((pid, log_path))
}

/// Poll for the guest IP, gating on the `tart list` **state** column
/// rather than on `tart ip` directly: `tart ip` returns a cached/stale
/// address (`tart-ip-lies` memory), so the IP is trusted only once the
/// guest reports `running`. Returns `None` on timeout — the caller treats
/// IP-unavailable as a benign degradation (the agent endpoint is left
/// unset). Ports `TartRunner.pollIP`, hardened with the state gate.
pub fn poll_ip(id: &str, attempts: u32, interval: Duration) -> Option<String> {
    for attempt in 0..attempts {
        let running = tart_list_json()
            .map(|j| decode_list(&j).iter().any(|vm| vm.name == id && vm.state.as_deref() == Some("running")))
            .unwrap_or(false);
        if running {
            let r = run_tart(&["ip", id]);
            if r.exit_code == 0 {
                let ip = r.stdout.trim();
                if !ip.is_empty() {
                    return Some(ip.to_string());
                }
            }
        }
        if attempt + 1 < attempts {
            std::thread::sleep(interval);
        }
    }
    None
}

/// Inputs for `TartRunner::start`.
#[derive(Debug, Clone)]
pub struct TartStartOptions {
    pub id: String,
    pub base: String,
    pub display: Option<String>,
}

/// Result of a successful `TartRunner::start`: the detached `tart run`
/// pid, the VNC endpoint tart handed back, and the (best-effort) guest IP.
#[derive(Debug, Clone)]
pub struct TartStartArtifacts {
    pub pid: i32,
    pub vnc: VncUrl,
    pub ip: Option<String>,
}

/// tart lifecycle entry points, mirroring `QemuRunner`.
pub struct TartRunner;

impl TartRunner {
    /// Clone the golden, start `tart run` detached, and discover the VNC
    /// endpoint (from the run log) and guest IP (state-gated). Ports the
    /// pre-sidecar half of `VMLifecycle.startTart`.
    pub async fn start(opts: &TartStartOptions, paths: &crate::paths::VmPaths) -> Result<TartStartArtifacts, VmError> {
        // Reclaim a same-id VM left over from a prior run.
        if vm_exists(&opts.id) {
            remove_existing(&opts.id);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        clone(&opts.base, &opts.id)?;
        // Always set a display: the user's value, or the TestAnyware default
        // (ADR-0013). Without `--display` tart would fall back to the unknown
        // Virtualization.framework default — off the vision distribution.
        set_display(&opts.id, resolve_display(opts.display.as_deref()))?;

        let (pid, log_path) = run_detached(&opts.id, &paths.vms_dir())?;

        // VNC URL is the readiness signal: tart prints it once the guest's
        // framebuffer is up. No URL within the window => boot timeout.
        let Some(vnc) = poll_vnc_url(&log_path, 60, Duration::from_secs(1)) else {
            crate::process::terminate(pid, Duration::from_millis(200), 10);
            remove_existing(&opts.id);
            return Err(VmError::VmBootTimeout { id: opts.id.clone() });
        };

        let ip = poll_ip(&opts.id, 30, Duration::from_secs(2));
        Ok(TartStartArtifacts { pid, vnc, ip })
    }

    /// Tear down a tart clone: `tart stop` + `tart delete`, then SIGTERM
    /// the detached `tart run` pid. Returns `true` when the VM existed and
    /// is now gone. Ports the tart arm of `VMLifecycle.stop`.
    pub fn stop(id: &str, pid: i32) -> bool {
        let existed = vm_exists(id);
        if existed {
            remove_existing(id);
        }
        if pid > 0 {
            crate::process::terminate(pid, Duration::from_millis(200), 10);
        }
        existed && !vm_exists(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_display_defaults_to_1920x1080px_when_absent() {
        // ADR-0013: the macOS default carries an explicit `px` so VF yields a
        // 1920×1080-px (LoDPI) framebuffer rather than 2× under the macOS
        // points hint. A user-supplied value is passed through untouched.
        assert_eq!(resolve_display(None), "1920x1080px");
        assert_eq!(resolve_display(Some("800x600")), "800x600");
        assert_eq!(resolve_display(Some("1920x1080px")), "1920x1080px");
    }

    #[test]
    fn parse_vnc_url_extracts_host_port_password() {
        let parsed = parse_vnc_url("vnc://:syrup-rotate@127.0.0.1:63530").unwrap();
        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 63530);
        assert_eq!(parsed.password.as_deref(), Some("syrup-rotate"));
    }

    #[test]
    fn parse_vnc_url_strips_trailing_ellipsis() {
        let parsed = parse_vnc_url("vnc://:abc@127.0.0.1:5900...").unwrap();
        assert_eq!(parsed.port, 5900);
        assert_eq!(parsed.password.as_deref(), Some("abc"));
    }

    #[test]
    fn parse_vnc_url_without_password_is_none() {
        let parsed = parse_vnc_url("vnc://127.0.0.1:5900").unwrap();
        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 5900);
        assert_eq!(parsed.password, None);
    }

    #[test]
    fn parse_vnc_url_rejects_non_vnc_scheme_and_missing_port() {
        assert!(parse_vnc_url("http://example.com").is_err());
        assert!(parse_vnc_url("vnc://no-port").is_err());
    }

    #[test]
    fn parse_goldens_keeps_only_golden_prefixed_names() {
        let json = r#"[
          {"Name": "testanyware-golden-macos-tahoe", "State": "stopped", "Disk": 50},
          {"Name": "testanyware-golden-linux-24.04", "State": "stopped", "Disk": 20},
          {"Name": "some-other-vm", "State": "stopped", "Disk": 10},
          {"Name": "testanyware-a1b2c3d4", "State": "running", "Disk": 50}
        ]"#;
        let mut names: Vec<String> = parse_goldens(json).into_iter().map(|g| g.name).collect();
        names.sort();
        assert_eq!(names, vec![
            "testanyware-golden-linux-24.04",
            "testanyware-golden-macos-tahoe",
        ]);
        let macos = parse_goldens(json).into_iter().find(|g| g.name.contains("macos")).unwrap();
        assert_eq!(macos.platform, "macos");
        assert_eq!(macos.backend, "tart");
    }

    #[test]
    fn parse_running_ids_skips_goldens_and_stopped() {
        let json = r#"[
          {"Name": "testanyware-golden-macos-tahoe", "State": "running"},
          {"Name": "testanyware-a1b2c3d4", "State": "running"},
          {"Name": "testanyware-b5c6d7e8", "State": "stopped"}
        ]"#;
        assert_eq!(parse_running_ids(json), vec!["testanyware-a1b2c3d4"]);
    }

    #[test]
    fn parse_all_names_returns_every_name() {
        let json = r#"[
          {"Name": "testanyware-golden-macos-tahoe", "State": "stopped"},
          {"Name": "testanyware-a1b2c3d4", "State": "running"},
          {"Name": "my-custom-vm", "State": "running"}
        ]"#;
        let mut names = parse_all_names(json);
        names.sort();
        assert_eq!(names, vec![
            "my-custom-vm",
            "testanyware-a1b2c3d4",
            "testanyware-golden-macos-tahoe",
        ]);
    }

    #[test]
    fn parsers_are_lenient_on_malformed_or_empty_json() {
        for bad in ["", "not json", "{}"] {
            assert!(parse_goldens(bad).is_empty(), "goldens lenient on {bad:?}");
            assert!(parse_running_ids(bad).is_empty(), "running lenient on {bad:?}");
            assert!(parse_all_names(bad).is_empty(), "names lenient on {bad:?}");
        }
    }

    #[test]
    fn poll_vnc_url_finds_url_in_a_log_file() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tart.log");
        std::fs::write(&log, "tart starting...\nVNC: vnc://:abc@127.0.0.1:54321\nready.\n").unwrap();
        let parsed = poll_vnc_url(&log, 3, Duration::from_millis(5)).unwrap();
        assert_eq!(parsed.host, "127.0.0.1");
        assert_eq!(parsed.port, 54321);
        assert_eq!(parsed.password.as_deref(), Some("abc"));
    }

    #[test]
    fn fresh_log_path_clears_a_prior_runs_stale_log() {
        // Regression for 105-tart-restart-stale-vnc: a same-id `stop`→`start`
        // bounce must resolve the *current* run's endpoint, not the prior
        // run's dead port left in the append-only log.
        let dir = tempfile::tempdir().unwrap();
        let id = "viewer-verify";
        let stale = dir.path().join(format!("{id}.tart.log"));
        std::fs::write(&stale, "run 1\nvnc://:old-pw@127.0.0.1:58372\n").unwrap();

        let log_path = fresh_log_path(dir.path(), id);
        assert_eq!(log_path, stale);
        assert!(!stale.exists(), "prior run's log must be cleared before the new run");

        // The new run appends its own (different) endpoint to the fresh log;
        // `poll_vnc_url` now resolves it, not the prior run's dead 58372.
        std::fs::write(&log_path, "run 2\nvnc://:new-pw@127.0.0.1:58373\n").unwrap();
        let parsed = poll_vnc_url(&log_path, 2, Duration::from_millis(5)).unwrap();
        assert_eq!(parsed.port, 58373);
        assert_eq!(parsed.password.as_deref(), Some("new-pw"));
    }

    #[test]
    fn fresh_log_path_is_a_noop_for_a_fresh_id() {
        let dir = tempfile::tempdir().unwrap();
        let log_path = fresh_log_path(dir.path(), "testanyware-fresh");
        assert!(!log_path.exists(), "a never-run id has no log to clear");
    }

    #[test]
    fn poll_vnc_url_returns_none_when_log_has_no_url_or_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tart.log");
        std::fs::write(&log, "no url here\n").unwrap();
        assert!(poll_vnc_url(&log, 2, Duration::from_millis(5)).is_none());
        assert!(poll_vnc_url(&dir.path().join("missing.log"), 2, Duration::from_millis(5)).is_none());
    }
}
