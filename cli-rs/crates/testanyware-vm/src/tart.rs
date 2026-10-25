//! tart VM orchestration (macOS host only). Port of `TartRunner.swift`.
//!
//! tart wraps Apple's Virtualization.framework, so this backend is
//! reachable only on a macOS host — the module is `#[cfg(target_os =
//! "macos")]`-gated at the crate root and never referenced in a Linux or
//! Windows build (consistent with ADR-0003's per-target gating).
//!
//! Pure parsers (`parse_vnc_url`, `parse_goldens`, `parse_running_names`,
//! `poll_vnc_url`) are unit-tested without invoking `tart`. The
//! `tart`-invoking operations (`clone`, `run_detached`, `start`, …) are
//! exercised by the manual `vm start --platform macos` verification the
//! `010-tart-runner` leaf calls for, not by unit tests.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

use crate::error::VmError;
use crate::qemu::{platform_from_name, GoldenImage};
use crate::tools::ToolSearch;

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

/// Names of the VMs `tart list --format json` reports as running, whatever
/// their name. Ports the `.running` arm of `TartRunner.parseList` without its
/// prefix filter: whether a running VM is TestAnyware's is for the caller to
/// decide (by the `testanyware-` prefix or by its sidecars), because `vm start
/// --id` accepts any id. Malformed JSON yields `[]`.
pub fn parse_running_names(tart_json: &str) -> Vec<String> {
    decode_list(tart_json)
        .into_iter()
        .filter(|vm| vm.state.as_deref() == Some("running"))
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
        if let Some(url) = find_vnc_url(log_path) {
            return Some(url);
        }
        if attempt + 1 < attempts {
            std::thread::sleep(interval);
        }
    }
    None
}

/// The first well-formed `vnc://…` URL in the log, if any yet. A missing log
/// is "not yet".
fn find_vnc_url(log_path: &Path) -> Option<VncUrl> {
    let text = std::fs::read_to_string(log_path).ok()?;
    let url = text.split_whitespace().find(|t| t.starts_with("vnc://"))?;
    parse_vnc_url(url).ok()
}

/// How a liveness-aware poll ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Polled<T> {
    Found(T),
    /// The process being waited on exited; the poll stopped at once.
    ProcessExited,
    TimedOut { attempts: u32 },
}

/// Probe up to `attempts` times, `interval` apart, until `probe` yields a
/// value — but stop at once if `alive` reports the process gone, instead of
/// spending the rest of the budget waiting on a corpse. A probe that already
/// has an answer wins over a process that has just died. Sleeps on tokio's
/// timer, so it never parks a runtime thread.
pub async fn poll_until<T>(
    attempts: u32,
    interval: Duration,
    mut alive: impl FnMut() -> bool,
    mut probe: impl FnMut() -> Option<T>,
) -> Polled<T> {
    for attempt in 0..attempts {
        if let Some(found) = probe() {
            return Polled::Found(found);
        }
        if !alive() {
            return Polled::ProcessExited;
        }
        if attempt + 1 < attempts {
            tokio::time::sleep(interval).await;
        }
    }
    Polled::TimedOut { attempts }
}

/// [`poll_vnc_url`] for `vm start`: async and aware of `tart run` dying.
pub async fn await_vnc_url(
    log_path: &Path,
    attempts: u32,
    interval: Duration,
    alive: impl FnMut() -> bool,
) -> Polled<VncUrl> {
    poll_until(attempts, interval, alive, || find_vnc_url(log_path)).await
}

/// [`poll_ip`] for `vm start`: async and aware of `tart run` dying.
pub async fn await_guest_ip(
    tools: &ToolSearch,
    id: &str,
    attempts: u32,
    interval: Duration,
    alive: impl FnMut() -> bool,
) -> Polled<String> {
    poll_until(attempts, interval, alive, || guest_ip_once(tools, id)).await
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
    run_tart_in(&ToolSearch::from_process_path(), args)
}

/// [`run_tart`], with `tart` found by `tools`.
fn run_tart_in(tools: &ToolSearch, args: &[&str]) -> TartResult {
    let Some(tart) = tools.find("tart") else {
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
/// invocation fails, with `tart` found by `tools`. Ports `TartRunner.tartListJSON`.
fn tart_list_json_in(tools: &ToolSearch) -> Option<String> {
    let r = run_tart_in(tools, &["list", "--format", "json"]);
    (r.exit_code == 0).then_some(r.stdout)
}

/// tart goldens currently in the catalog. `[]` when tart is absent.
pub fn list_goldens() -> Vec<GoldenImage> {
    list_goldens_in(&ToolSearch::from_process_path())
}

/// [`list_goldens`], with `tart` found by `tools`.
pub fn list_goldens_in(tools: &ToolSearch) -> Vec<GoldenImage> {
    tart_list_json_in(tools).map(|j| parse_goldens(&j)).unwrap_or_default()
}

/// Names of every running tart VM, goldens included, with `tart` found by `tools`. `[]` when
/// tart is absent.
pub fn list_running_names_in(tools: &ToolSearch) -> Vec<String> {
    tart_list_json_in(tools).map(|j| parse_running_names(&j)).unwrap_or_default()
}

/// Names of the VMs in a `tart list --format json` text that run, or why the text is no list.
///
/// [`parse_running_names`] reads malformed text as "no VM runs", which suits `vm list`. A reader
/// that frees a slot on "no VM runs" needs the failure instead (spec "admission-queue": "The
/// count").
pub fn parse_running_names_strict(tart_json: &str) -> Result<Vec<String>, String> {
    let rows: Vec<TartVm> = serde_json::from_str(tart_json)
        .map_err(|e| format!("tart list printed output that is not a list of VMs: {e}"))?;
    Ok(rows
        .into_iter()
        .filter(|vm| vm.state.as_deref() == Some("running"))
        .map(|vm| vm.name)
        .collect())
}

/// Names of the VMs that `tart list` reports running, or why tart did not answer. A failed call
/// and output that is no list are both errors, so "no VM runs" is always an answer of tart.
pub fn running_names_strict_in(tools: &ToolSearch) -> Result<Vec<String>, String> {
    let r = run_tart_in(tools, &["list", "--format", "json"]);
    if r.exit_code != 0 {
        return Err(format!("tart list failed: {}", r.stderr.trim()));
    }
    parse_running_names_strict(&r.stdout)
}

/// The OS in a `tart get <vm> --format json` text (`darwin` or `linux`), or `None` when the text
/// is not that JSON. tart prints the key `OS`; `tart list` does not print it.
pub fn parse_os(tart_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(tart_json)
        .ok()?
        .get("OS")?
        .as_str()
        .map(str::to_string)
}

/// The OS of the VM `name` from `tart get`, or why tart did not answer.
pub fn os_of_in(tools: &ToolSearch, name: &str) -> Result<String, String> {
    let r = run_tart_in(tools, &["get", name, "--format", "json"]);
    if r.exit_code != 0 {
        return Err(format!("tart get {name} failed: {}", r.stderr.trim()));
    }
    parse_os(&r.stdout).ok_or_else(|| format!("tart get {name} printed no OS"))
}

/// Whether a tart VM with `name` exists in any state. `false` when tart
/// is absent. Ports `TartRunner.vmExists`.
pub fn vm_exists(name: &str) -> bool {
    vm_exists_in(&ToolSearch::from_process_path(), name)
}

/// [`vm_exists`], with `tart` found by `tools`.
pub fn vm_exists_in(tools: &ToolSearch, name: &str) -> bool {
    tart_list_json_in(tools).map(|j| parse_all_names(&j).iter().any(|n| n == name)).unwrap_or(false)
}

/// Whether `name` is a tart golden image. Ports the tart arm of
/// `VMLifecycle.delete`'s backend detection.
pub fn golden_exists(name: &str) -> bool {
    golden_exists_in(&ToolSearch::from_process_path(), name)
}

/// [`golden_exists`], with `tart` found by `tools`.
pub fn golden_exists_in(tools: &ToolSearch, name: &str) -> bool {
    list_goldens_in(tools).iter().any(|g| g.name == name)
}

/// `tart clone <base> <id>`. Ports `TartRunner.clone`.
pub fn clone(base: &str, id: &str) -> Result<(), VmError> {
    clone_in(&ToolSearch::from_process_path(), base, id)
}

/// [`clone`], with `tart` found by `tools`.
pub fn clone_in(tools: &ToolSearch, base: &str, id: &str) -> Result<(), VmError> {
    let r = run_tart_in(tools, &["clone", base, id]);
    if r.exit_code == 0 {
        Ok(())
    } else {
        Err(VmError::TartFailed { detail: format!("tart clone {base} {id}: {}", r.stderr.trim()) })
    }
}

/// `tart create <name> --from-ipsw <ipsw> --disk-size <gb>`: install macOS from
/// a restore image into a new, stopped VM. Takes a while (the restore runs
/// inside Virtualization.framework), so callers should expect minutes. The VM
/// boots to Setup Assistant — no account exists yet.
pub fn create_from_ipsw(name: &str, ipsw: &Path, disk_gb: u32) -> Result<(), VmError> {
    let disk = disk_gb.to_string();
    let ipsw_arg = ipsw.display().to_string();
    let r = run_tart(&["create", name, "--from-ipsw", &ipsw_arg, "--disk-size", &disk]);
    if r.exit_code == 0 {
        Ok(())
    } else {
        Err(VmError::TartFailed {
            detail: format!("tart create {name} --from-ipsw {ipsw_arg}: {}", r.stderr.trim()),
        })
    }
}

/// `tart rename <from> <to>`. Used to publish a finished base VM under its
/// real name only once it is complete.
pub fn rename(from: &str, to: &str) -> Result<(), VmError> {
    let r = run_tart(&["rename", from, to]);
    if r.exit_code == 0 {
        Ok(())
    } else {
        Err(VmError::TartFailed { detail: format!("tart rename {from} {to}: {}", r.stderr.trim()) })
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
    set_display_in(&ToolSearch::from_process_path(), id, display)
}

/// [`set_display`], with `tart` found by `tools`.
pub fn set_display_in(tools: &ToolSearch, id: &str, display: &str) -> Result<(), VmError> {
    let r = run_tart_in(tools, &["set", id, "--display", display]);
    if r.exit_code == 0 {
        Ok(())
    } else {
        Err(VmError::TartFailed { detail: format!("tart set {id} --display {display}: {}", r.stderr.trim()) })
    }
}

/// The disk size in GB from `tart get <vm> --format json` (key `Disk`, an
/// integer), or `None` when the text is not that JSON.
pub fn parse_disk_gb(tart_json: &str) -> Option<u32> {
    let disk = serde_json::from_str::<serde_json::Value>(tart_json)
        .ok()?
        .get("Disk")?
        .as_u64()?;
    u32::try_from(disk).ok()
}

/// The disk size of `id` in GB, or `None` when tart is absent, fails, or prints
/// something else.
pub fn disk_gb(id: &str) -> Option<u32> {
    disk_gb_in(&ToolSearch::from_process_path(), id)
}

/// [`disk_gb`], with `tart` found by `tools`.
pub fn disk_gb_in(tools: &ToolSearch, id: &str) -> Option<u32> {
    let r = run_tart_in(tools, &["get", id, "--format", "json"]);
    if r.exit_code == 0 {
        parse_disk_gb(&r.stdout)
    } else {
        None
    }
}

/// `tart set <id> --disk-size <gb>`: grow the disk of a stopped VM. tart only
/// grows a disk, so a size below the current one is an error, not a shrink.
pub fn set_disk_size(id: &str, gb: u32) -> Result<(), VmError> {
    set_disk_size_in(&ToolSearch::from_process_path(), id, gb)
}

/// [`set_disk_size`], with `tart` found by `tools`.
pub fn set_disk_size_in(tools: &ToolSearch, id: &str, gb: u32) -> Result<(), VmError> {
    let size = gb.to_string();
    let r = run_tart_in(tools, &["set", id, "--disk-size", &size]);
    if r.exit_code == 0 {
        Ok(())
    } else {
        Err(VmError::TartFailed {
            detail: format!("tart set {id} --disk-size {size}: {}", r.stderr.trim()),
        })
    }
}

/// Best-effort `tart stop` then `tart delete` — both no-ops on an absent
/// VM, so non-zero exits are ignored. Ports `TartRunner.removeExisting`.
pub fn remove_existing(id: &str) {
    remove_existing_in(&ToolSearch::from_process_path(), id);
}

/// [`remove_existing`], with `tart` found by `tools`.
pub fn remove_existing_in(tools: &ToolSearch, id: &str) {
    let _ = run_tart_in(tools, &["stop", id]);
    let _ = run_tart_in(tools, &["delete", id]);
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
    delete_golden_in(&ToolSearch::from_process_path(), name)
}

/// [`delete_golden`], with `tart` found by `tools`.
pub fn delete_golden_in(tools: &ToolSearch, name: &str) -> bool {
    run_tart_in(tools, &["delete", name]).exit_code == 0
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
    run_detached_with(&ToolSearch::from_process_path(), id, log_dir, &[])
}

/// [`run_detached`], with `tart` found by `tools`.
pub fn run_detached_in(tools: &ToolSearch, id: &str, log_dir: &Path) -> Result<(i32, PathBuf), VmError> {
    run_detached_with(tools, id, log_dir, &[])
}

/// Like [`run_detached`] but boots into **macOS Recovery**
/// (`tart run … --recovery --no-graphics --vnc-experimental`). The recovery
/// driver ([`crate::recovery`], ADR-0008) drives the resulting VNC framebuffer
/// to toggle SIP; recovery has no SSH, so this is the one boot that cannot go
/// over the provisioning layer. Ports the `--recovery` `tart run` in the
/// script's `_recovery_boot_csrutil` (line 408).
pub fn run_detached_recovery(id: &str, log_dir: &Path) -> Result<(i32, PathBuf), VmError> {
    run_detached_with(&ToolSearch::from_process_path(), id, log_dir, &["--recovery"])
}

/// Shared body for the detached `tart run` variants. `extra` flags are
/// inserted before the always-on `--no-graphics --vnc-experimental` pair.
fn run_detached_with(
    tools: &ToolSearch,
    id: &str,
    log_dir: &Path,
    extra: &[&str],
) -> Result<(i32, PathBuf), VmError> {
    std::fs::create_dir_all(log_dir)
        .map_err(|e| VmError::Io(format!("create {}: {e}", log_dir.display())))?;
    let tart = tools.find("tart")
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
        if let Some(ip) = guest_ip_once(&ToolSearch::from_process_path(), id) {
            return Some(ip);
        }
        if attempt + 1 < attempts {
            std::thread::sleep(interval);
        }
    }
    None
}

/// One IP probe, gated on the `tart list` state column (see [`poll_ip`]).
fn guest_ip_once(tools: &ToolSearch, id: &str) -> Option<String> {
    let running = tart_list_json_in(tools)
        .map(|j| decode_list(&j).iter().any(|vm| vm.name == id && vm.state.as_deref() == Some("running")))
        .unwrap_or(false);
    if !running {
        return None;
    }
    let r = run_tart_in(tools, &["ip", id]);
    let ip = r.stdout.trim();
    (r.exit_code == 0 && !ip.is_empty()).then(|| ip.to_string())
}

/// Inputs for `TartRunner::launch`.
#[derive(Debug, Clone)]
pub struct TartStartOptions {
    pub id: String,
    pub base: String,
    pub display: Option<String>,
}

/// A launched `tart run`: its detached pid and the log the VNC URL (and any
/// failure text) lands in.
#[derive(Debug, Clone)]
pub struct TartLaunch {
    pub pid: i32,
    pub log_path: PathBuf,
}

/// tart lifecycle entry points, mirroring `QemuRunner`.
pub struct TartRunner;

impl TartRunner {
    /// Clone the golden, set its display and start `tart run` detached. It does
    /// not wait for anything: the caller owns the readiness stages (VNC URL,
    /// guest IP, agent), so it can observe and decide on them. If it fails
    /// after the clone exists, the clone is removed — a failed launch leaves no
    /// VM behind. Ports the pre-poll half of `VMLifecycle.startTart`.
    pub async fn launch(opts: &TartStartOptions, paths: &crate::paths::VmPaths) -> Result<TartLaunch, VmError> {
        let tools = ToolSearch::from_process_path();
        Self::prepare_in(&tools, opts, paths).await?;
        Self::run_in(&tools, opts, paths)
    }

    /// The clone half of [`launch`](Self::launch): reclaim a same-id leftover,
    /// clone the golden and set its display. A failed clone leaves no VM. The
    /// caller owns the clone from here and removes it if it never runs — which
    /// lets a `--wait` start run the one clone again after each refusal.
    pub async fn prepare(opts: &TartStartOptions, paths: &crate::paths::VmPaths) -> Result<(), VmError> {
        Self::prepare_in(&ToolSearch::from_process_path(), opts, paths).await
    }

    /// [`prepare`](Self::prepare), with `tart` found by `tools`.
    pub async fn prepare_in(
        tools: &ToolSearch,
        opts: &TartStartOptions,
        paths: &crate::paths::VmPaths,
    ) -> Result<(), VmError> {
        // A start in progress invalidates whatever the id's sidecars described
        // (a reclaimed prior run, or leftovers of a crash). Clear them now so
        // a launch that fails before the start settles cannot leave sidecars
        // pointing at a deleted VM.
        crate::lifecycle::clear_sidecars(paths, &opts.id);

        // Reclaim a same-id VM left over from a prior run.
        if vm_exists_in(tools, &opts.id) {
            remove_existing_in(tools, &opts.id);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        clone_in(tools, &opts.base, &opts.id)?;
        // Always set a display: the user's value, or the TestAnyware default
        // (ADR-0013). Without `--display` tart would fall back to the unknown
        // Virtualization.framework default — off the vision distribution.
        set_display_in(tools, &opts.id, resolve_display(opts.display.as_deref())).inspect_err(|_| {
            remove_existing_in(tools, &opts.id);
        })
    }

    /// The run half of [`launch`](Self::launch): start `tart run` detached on a
    /// prepared clone. A failed spawn removes the clone.
    pub fn run(opts: &TartStartOptions, paths: &crate::paths::VmPaths) -> Result<TartLaunch, VmError> {
        Self::run_in(&ToolSearch::from_process_path(), opts, paths)
    }

    /// [`run`](Self::run), with `tart` found by `tools`.
    pub fn run_in(
        tools: &ToolSearch,
        opts: &TartStartOptions,
        paths: &crate::paths::VmPaths,
    ) -> Result<TartLaunch, VmError> {
        match run_detached_in(tools, &opts.id, &paths.vms_dir()) {
            Ok((pid, log_path)) => Ok(TartLaunch { pid, log_path }),
            Err(err) => {
                remove_existing_in(tools, &opts.id);
                Err(err)
            }
        }
    }

    /// Tear down a tart clone: `tart stop` + `tart delete`, then SIGTERM
    /// the detached `tart run` pid. Returns `true` when the VM existed and
    /// is now gone. Ports the tart arm of `VMLifecycle.stop`.
    pub fn stop(id: &str, pid: i32) -> bool {
        Self::stop_in(&ToolSearch::from_process_path(), id, pid)
    }

    /// [`stop`](Self::stop), with `tart` found by `tools`.
    pub fn stop_in(tools: &ToolSearch, id: &str, pid: i32) -> bool {
        let existed = vm_exists_in(tools, id);
        if existed {
            remove_existing_in(tools, id);
        }
        if pid > 0 {
            crate::process::terminate(pid, Duration::from_millis(200), 10);
        }
        existed && !vm_exists_in(tools, id)
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
    fn parse_running_names_keeps_every_running_vm_and_skips_stopped() {
        let json = r#"[
          {"Name": "testanyware-golden-macos-tahoe", "State": "running"},
          {"Name": "testanyware-a1b2c3d4", "State": "running"},
          {"Name": "koine-verify-4242", "State": "running"},
          {"Name": "testanyware-b5c6d7e8", "State": "stopped"}
        ]"#;
        assert_eq!(
            parse_running_names(json),
            vec!["testanyware-golden-macos-tahoe", "testanyware-a1b2c3d4", "koine-verify-4242"],
        );
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
            assert!(parse_running_names(bad).is_empty(), "running lenient on {bad:?}");
            assert!(parse_all_names(bad).is_empty(), "names lenient on {bad:?}");
        }
    }

    #[test]
    fn parse_disk_gb_reads_the_disk_key_of_tart_get() {
        // `tart get testanyware-golden-linux-24.04 --format json` on tart 2.40.1.
        let json = r#"{
          "DiskFormat" : "raw", "OS" : "linux", "CPU" : 4, "Disk" : 20,
          "Display" : "1024x768", "Size" : "9.002", "Running" : false,
          "State" : "stopped", "Memory" : 4096
        }"#;
        assert_eq!(parse_disk_gb(json), Some(20));
    }

    #[test]
    fn parse_disk_gb_is_none_for_anything_else() {
        for bad in [
            "",
            "not json",
            "{}",
            r#"{"Disk": "20"}"#,
            r#"{"Disk": -1}"#,
            "[]",
        ] {
            assert_eq!(parse_disk_gb(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_strict_list_reader_fails_where_the_lenient_reader_reports_no_vm() {
        let json = r#"[{"Name":"a","State":"running"},{"Name":"b","State":"stopped"}]"#;
        assert_eq!(parse_running_names_strict(json), Ok(vec!["a".to_string()]));
        assert_eq!(parse_running_names_strict("[]"), Ok(Vec::new()));
        for bad in ["", "not json", "{}", r#"[{"State":"running"}]"#] {
            assert!(parse_running_names_strict(bad).is_err(), "{bad:?}");
            assert!(parse_running_names(bad).is_empty(), "lenient on {bad:?}");
        }
    }

    #[test]
    fn parse_os_reads_the_os_key_of_tart_get() {
        assert_eq!(parse_os(r#"{"OS" : "darwin", "Disk" : 90}"#).as_deref(), Some("darwin"));
        assert_eq!(parse_os(r#"{"OS" : "linux"}"#).as_deref(), Some("linux"));
        for bad in ["", "not json", "{}", r#"{"OS": 1}"#, "[]"] {
            assert_eq!(parse_os(bad), None, "{bad:?}");
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

    #[tokio::test]
    async fn poll_until_returns_the_first_answer() {
        let mut calls = 0;
        let polled = poll_until(5, Duration::from_millis(1), || true, || {
            calls += 1;
            (calls == 3).then_some("ip")
        })
        .await;
        assert_eq!(polled, Polled::Found("ip"));
        assert_eq!(calls, 3);
    }

    #[tokio::test]
    async fn poll_until_times_out_after_exactly_its_attempts() {
        let mut calls = 0;
        let polled = poll_until(4, Duration::from_millis(1), || true, || {
            calls += 1;
            None::<()>
        })
        .await;
        assert_eq!(polled, Polled::TimedOut { attempts: 4 });
        assert_eq!(calls, 4);
    }

    #[tokio::test]
    async fn poll_until_stops_at_once_when_the_process_is_gone() {
        // A budget that would take ~10 minutes if spent.
        let started = std::time::Instant::now();
        let polled = poll_until(600, Duration::from_secs(1), || false, || None::<()>).await;
        assert_eq!(polled, Polled::ProcessExited);
        assert!(started.elapsed() < Duration::from_secs(1), "must not sleep out the budget");
    }

    #[tokio::test]
    async fn poll_until_prefers_an_answer_over_a_process_that_just_died() {
        let polled = poll_until(3, Duration::from_millis(1), || false, || Some(7)).await;
        assert_eq!(polled, Polled::Found(7));
    }

    #[tokio::test]
    async fn await_vnc_url_finds_the_url_in_the_log() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tart.log");
        std::fs::write(&log, "VNC server is running at vnc://:abc@127.0.0.1:54321\n").unwrap();
        let polled = await_vnc_url(&log, 3, Duration::from_millis(5), || true).await;
        let Polled::Found(url) = polled else { panic!("expected the URL, got {polled:?}") };
        assert_eq!(url.port, 54321);
    }

    #[tokio::test]
    async fn await_vnc_url_returns_at_once_when_tart_died_before_printing_a_url() {
        // The k2 live finding: a refused start exits within a second leaving
        // only an error line; the old poll waited 60 s for a URL.
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("tart.log");
        std::fs::write(&log, "The number of VMs exceeds the system limit (other running VMs: a, b)\n").unwrap();
        let started = std::time::Instant::now();
        let polled = await_vnc_url(&log, 60, Duration::from_secs(1), || false).await;
        assert_eq!(polled, Polled::ProcessExited);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn await_vnc_url_times_out_with_a_live_process_and_no_url() {
        let dir = tempfile::tempdir().unwrap();
        let polled =
            await_vnc_url(&dir.path().join("missing.log"), 2, Duration::from_millis(5), || true).await;
        assert_eq!(polled, Polled::TimedOut { attempts: 2 });
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
