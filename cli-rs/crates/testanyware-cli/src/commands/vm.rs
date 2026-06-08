//! `vm {start|stop|list|delete}` command handlers.
//!
//! Bridges clap-parsed args to the `testanyware-vm` crate and emits the
//! contract §3 JSON envelope (or text). Ports the surface of
//! `cli/Sources/testanyware/VMCommand.swift`.

use serde_json::{json, Value};

use testanyware_vm::lifecycle::{Platform, VmLifecycle, VmListing, VmStartOptions};
use testanyware_vm::{VmError, VmMeta, VmPaths};

use crate::output::{print_error, print_success, OutputMode};
use crate::resolve::ConnectionOptions;

/// A flattened `vm list` row, ready for JSON or text rendering.
pub struct ListItem {
    pub kind: &'static str,
    pub name: String,
    pub platform: String,
    pub backend: &'static str,
    pub pid: Option<i32>,
    pub vnc: Option<String>,
    pub agent: Option<String>,
}

// ---- handlers -----------------------------------------------------------

/// `testanyware vm start`.
pub async fn run_vm_start(
    platform: String,
    base: Option<String>,
    id: Option<String>,
    display: Option<String>,
    viewer: bool,
    mode: OutputMode,
    dry_run: bool,
) {
    let parsed = match Platform::parse(&platform) {
        Ok(p) => p,
        Err(err) => exit_vm_error(err, mode),
    };
    let opts = VmStartOptions::new(parsed, base, id, display, viewer);
    let paths = VmPaths::from_process_env();

    if dry_run {
        // Validate without side effects: the golden must exist in the
        // backend `start` would route to (tart on macOS, else QEMU), plus
        // the QEMU host preflight. Routing lives in the vm crate.
        if let Err(err) = VmLifecycle::dry_run_validate_start(&opts, &paths) {
            exit_vm_error(err, mode);
        }
        emit_start_plan(&opts, mode);
        return;
    }

    match VmLifecycle::start(&opts, &paths).await {
        Ok(result) => {
            if result.agent_unreachable {
                eprintln!(
                    "warning: agent did not reach health within the boot window — \
                     agent commands will fail until it comes up"
                );
            }
            // Capture the viewer endpoint before the envelope arm moves any
            // `result` fields. The viewer resolves through `--connect` against
            // the spec file `start` just wrote — the same chain `--vm` uses.
            let viewer_connect = viewer.then(|| result.spec_path.display().to_string());
            match mode {
                OutputMode::Text => println!("{}", result.id),
                OutputMode::Json => {
                    let agent = result
                        .spec
                        .agent
                        .as_ref()
                        .map(|a| json!({ "host": a.host, "port": a.port }))
                        .unwrap_or(Value::Null);
                    print_success(json!({
                        "id": result.id,
                        "platform": result.platform.as_str(),
                        "base": opts.base,
                        "vnc": { "host": result.spec.vnc.host, "port": result.spec.vnc.port },
                        "agent": agent,
                        "spec_path": result.spec_path.display().to_string(),
                        "meta_path": result.meta_path.display().to_string(),
                    }));
                }
            }
            // Sugar (ADR-0005 Q2): the start envelope is already emitted, so
            // an `--json` consumer has its data before this window blocks.
            // Opening it inline (blocking-until-close) is acceptable for an
            // explicit window request. Dry-run returns earlier, never here.
            if let Some(connect) = viewer_connect {
                let conn = ConnectionOptions { connect: Some(connect), ..Default::default() };
                crate::commands::viewer::run_viewer(conn);
            }
        }
        Err(err) => exit_vm_error(err, mode),
    }
}

fn emit_start_plan(opts: &VmStartOptions, mode: OutputMode) {
    match mode {
        OutputMode::Text => {
            println!("dry-run: would start {} (platform {}, base {})",
                opts.id, opts.platform.as_str(), opts.base);
        }
        OutputMode::Json => {
            print_success(json!({
                "dry_run": true,
                "id": opts.id,
                "platform": opts.platform.as_str(),
                "base": opts.base,
                "vnc": { "host": "localhost", "port": 0 },
            }));
        }
    }
}

/// `testanyware vm stop`.
pub async fn run_vm_stop(id: Option<String>, mode: OutputMode, dry_run: bool) {
    let Some(id) = id.filter(|s| !s.is_empty()) else {
        print_error(
            mode,
            "USAGE_ERROR",
            "VM id required: pass it as an argument or set TESTANYWARE_VM_ID",
            Some("Run `testanyware vm list` to see running VM ids."),
            json!({}),
            2,
        );
    };
    let paths = VmPaths::from_process_env();
    let meta_path = paths.meta_path(&id);
    if !meta_path.is_file() {
        exit_vm_error(VmError::VmNotFound { id }, mode);
    }

    if dry_run {
        let pid = VmMeta::load(&meta_path).ok().map(|m| m.pid);
        let pid_str = pid.map_or_else(|| "unknown".to_string(), |p| p.to_string());
        match mode {
            OutputMode::Text => println!("dry-run: would stop {id} (pid {pid_str})"),
            OutputMode::Json => {
                print_success(json!({ "dry_run": true, "id": id, "stopped": false }));
            }
        }
        return;
    }

    match VmLifecycle::stop(&id, &paths) {
        Ok(()) => match mode {
            OutputMode::Text => println!("stopped {id}"),
            OutputMode::Json => print_success(json!({ "id": id, "stopped": true })),
        },
        Err(err) => exit_vm_error(err, mode),
    }
}

/// `testanyware vm list`.
pub async fn run_vm_list(
    mode: OutputMode,
    limit: usize,
    all: bool,
    filter: Option<String>,
) {
    let filters = match filter.as_deref().map(parse_filter) {
        Some(Ok(f)) => f,
        Some(Err(msg)) => print_error(
            mode,
            "USAGE_ERROR",
            &format!("invalid --filter: {msg}"),
            Some("Expected comma-separated field=value pairs, e.g. --filter platform=windows."),
            json!({ "value": filter.unwrap_or_default() }),
            2,
        ),
        None => Vec::new(),
    };
    let paths = VmPaths::from_process_env();
    let listing = VmLifecycle::list(&paths);
    let items: Vec<ListItem> = listing_items(&listing)
        .into_iter()
        .filter(|item| filter_matches(item, &filters))
        .collect();
    let (shown, returned, total, truncated) = apply_limit(items, limit, all);

    match mode {
        OutputMode::Text => render_list_text(&shown, returned, total, truncated),
        OutputMode::Json => {
            let json_items: Vec<Value> = shown.iter().map(item_to_json).collect();
            print_success(json!({
                "items": json_items,
                "returned": returned,
                "total": total,
                "truncated": truncated,
            }));
        }
    }
}

/// `testanyware vm delete`.
pub async fn run_vm_delete(name: String, force: bool, mode: OutputMode, dry_run: bool) {
    let paths = VmPaths::from_process_env();

    if dry_run {
        if let Err(err) = VmLifecycle::dry_run_validate_delete(&name, &paths) {
            exit_vm_error(err, mode);
        }
        match mode {
            OutputMode::Text => println!("dry-run: would delete golden {name}"),
            OutputMode::Json => {
                print_success(json!({ "dry_run": true, "name": name, "deleted": false }));
            }
        }
        return;
    }

    match VmLifecycle::delete(&name, force, &paths) {
        Ok(()) => match mode {
            OutputMode::Text => println!("deleted {name}"),
            OutputMode::Json => print_success(json!({ "name": name, "deleted": true })),
        },
        Err(err) => exit_vm_error(err, mode),
    }
}

/// `testanyware vm create-golden` (ADR-0007/0008; grove node
/// `110-vm-create-golden-macos`).
///
/// `--dry-run` validates and prints the 5-boot plan, mutating nothing (both
/// text and `--json`). A real invocation drives the full pipeline end-to-end
/// ([`testanyware_vm::finalize::create_golden`]): boot-1 normal-mode
/// provisioning (pubkey, defaults, wallpaper, CLT, Homebrew, agent + plist) →
/// the SIP/TCC cycle (disable SIP via Recovery, grant TCC, re-enable SIP) →
/// the agent-health gate, clean shutdown, and `tart clone` to the golden. On
/// success it reports the created golden's name; the throwaway setup VM is
/// consumed. Mirrors the `run_vm_start` dry-run-validate → emit-plan shape.
pub async fn run_vm_create_golden(
    platform: String,
    version: Option<String>,
    name: Option<String>,
    iso: Option<String>,
    mode: OutputMode,
    dry_run: bool,
) {
    // macOS, Windows, and Linux goldens are all supported — all three are
    // macOS-host work (macOS/Linux via tart, Windows via QEMU+swtpm). An unknown
    // platform string is a usage error (INVALID_PLATFORM, exit 2).
    let parsed = match Platform::parse(&platform) {
        Ok(p) => p,
        Err(err) => exit_vm_error(err, mode),
    };
    // Per-platform default version (macOS: tahoe; Windows: 11; Linux: 24.04).
    let version = version.unwrap_or_else(|| match parsed {
        Platform::Windows => "11".into(),
        Platform::Linux => "24.04".into(),
        Platform::Macos => "tahoe".into(),
    });
    let name = name.unwrap_or_else(|| format!("testanyware-golden-{}-{version}", parsed.as_str()));

    match parsed {
        Platform::Windows => run_vm_create_golden_windows(version, name, iso, mode, dry_run).await,
        Platform::Linux => run_vm_create_golden_linux(version, name, mode, dry_run).await,
        Platform::Macos => run_vm_create_golden_macos(version, name, mode, dry_run).await,
    }
}

/// macOS golden: the 5-boot SIP/TCC pipeline (node `110`, ADR-0007/0008).
async fn run_vm_create_golden_macos(
    version: String,
    name: String,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_golden_plan(&version, &name, mode);
        return;
    }

    // tart is macOS-only, so a non-macOS host cannot serve the command.
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&version, &name);
        exit_vm_error(
            VmError::BackendUnsupported {
                platform: "macos (vm create-golden requires a macOS host)".into(),
            },
            mode,
        );
    }

    #[cfg(target_os = "macos")]
    {
        use testanyware_vm::finalize::create_golden;
        use testanyware_vm::golden::GoldenOptions;
        use testanyware_vm::VmPaths;

        let opts = GoldenOptions { version: version.clone(), name: name.clone() };
        let paths = VmPaths::from_process_env();
        match create_golden(&opts, &paths).await {
            Ok(golden) => match mode {
                OutputMode::Text => {
                    println!("Golden image '{golden}' created successfully.");
                    println!("Use it with: testanyware vm start --platform macos --base {golden}");
                }
                OutputMode::Json => print_success(json!({
                    "platform": "macos",
                    "version": version,
                    "name": golden,
                    "created": true,
                })),
            },
            Err(err) => exit_vm_error(err, mode),
        }
    }
}

/// Windows golden: unattended ISO install + agent provisioning (leaf `220/020`,
/// ADR-0009). macOS-host work (FAT32 media built with `hdiutil`).
async fn run_vm_create_golden_windows(
    version: String,
    name: String,
    iso: Option<String>,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_windows_golden_plan(&version, &name, iso.as_deref(), mode);
        return;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&version, &name, &iso);
        exit_vm_error(
            VmError::BackendUnsupported {
                platform: "macos (vm create-golden --platform windows requires a macOS host)".into(),
            },
            mode,
        );
    }

    #[cfg(target_os = "macos")]
    {
        use std::path::Path;
        use testanyware_vm::golden::GoldenOptions;
        use testanyware_vm::golden_windows::create_golden_windows;
        use testanyware_vm::VmPaths;

        let opts = GoldenOptions { version: version.clone(), name: name.clone() };
        let paths = VmPaths::from_process_env();
        let iso_path = iso.as_deref().map(Path::new);
        match create_golden_windows(&opts, iso_path, &paths).await {
            Ok(golden) => match mode {
                OutputMode::Text => {
                    println!("Golden image '{golden}' created successfully.");
                    println!("Use it with: testanyware vm start --platform windows --base {golden}");
                }
                OutputMode::Json => print_success(json!({
                    "platform": "windows",
                    "version": version,
                    "name": golden,
                    "created": true,
                })),
            },
            Err(err) => exit_vm_error(err, mode),
        }
    }
}

/// Linux golden: a tart-based SSH provisioning pass + one apply-settings reboot
/// (leaf `230`, ADR-0007). macOS-host work (built on this Mac via tart), no
/// SIP/TCC/recovery cycle.
async fn run_vm_create_golden_linux(
    version: String,
    name: String,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_linux_golden_plan(&version, &name, mode);
        return;
    }

    // tart is macOS-only, so a non-macOS host cannot serve the command.
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&version, &name);
        exit_vm_error(
            VmError::BackendUnsupported {
                platform: "macos (vm create-golden --platform linux requires a macOS host)".into(),
            },
            mode,
        );
    }

    #[cfg(target_os = "macos")]
    {
        use testanyware_vm::golden::GoldenOptions;
        use testanyware_vm::golden_linux::create_golden_linux;
        use testanyware_vm::VmPaths;

        let opts = GoldenOptions { version: version.clone(), name: name.clone() };
        let paths = VmPaths::from_process_env();
        match create_golden_linux(&opts, &paths).await {
            Ok(golden) => match mode {
                OutputMode::Text => {
                    println!("Golden image '{golden}' created successfully.");
                    println!("Use it with: testanyware vm start --platform linux --base {golden}");
                }
                OutputMode::Json => print_success(json!({
                    "platform": "linux",
                    "version": version,
                    "name": golden,
                    "created": true,
                })),
            },
            Err(err) => exit_vm_error(err, mode),
        }
    }
}

/// The 5-boot provisioning sequence (3 normal + 2 recovery) that golden
/// creation drives, as `(step, mode, actions)`. Parity with the boot
/// sequence in `provisioner/scripts/vm-create-golden-macos.sh`. Pure, so
/// the plan is unit-tested and shared by the text and JSON renderers.
fn golden_boot_plan() -> [(u32, &'static str, &'static str); 5] {
    [
        (1, "normal",
         "SSH pubkey install, macOS defaults, solid wallpaper, hide widgets, \
          Xcode CLT, Homebrew, agent binary + LaunchAgent plist"),
        (2, "recovery", "disable SIP (csrutil disable)"),
        (3, "normal",
         "grant TCC: kTCCServiceAccessibility + kTCCServiceSystemPolicyAllFiles"),
        (4, "recovery", "re-enable SIP (csrutil enable)"),
        (5, "normal",
         "verify agent health on port 8648, disable Remote Login, clean shutdown, \
          clone to golden"),
    ]
}

fn emit_golden_plan(version: &str, name: &str, mode: OutputMode) {
    let plan = golden_boot_plan();
    match mode {
        OutputMode::Text => {
            println!(
                "dry-run: would create golden {name} (platform macos, version {version})"
            );
            println!("boot plan (3 normal + 2 recovery = 5 boots):");
            for (step, boot_mode, actions) in plan {
                println!("  {step}. [{boot_mode}] {actions}");
            }
        }
        OutputMode::Json => {
            let steps: Vec<Value> = plan
                .iter()
                .map(|(step, boot_mode, actions)| {
                    json!({ "step": step, "mode": boot_mode, "actions": actions })
                })
                .collect();
            print_success(json!({
                "dry_run": true,
                "platform": "macos",
                "version": version,
                "name": name,
                "boot_plan": steps,
            }));
        }
    }
}

/// The Linux golden boot sequence (2 normal boots — no SIP/TCC/recovery
/// cycle), as `(step, mode, actions)`. Parity with the boot sequence in
/// `provisioner/scripts/vm-create-golden-linux.sh`. Pure, so the plan is
/// unit-tested and shared by the text and JSON renderers. Reuses the macOS
/// `boot_plan` schema shape (both Linux boots are `normal`).
fn linux_boot_plan() -> [(u32, &'static str, &'static str); 2] {
    [
        (1, "normal",
         "SSH pubkey install, Ubuntu Desktop (minimal), NetworkManager via netplan, \
          Firefox, GDM autologin forced to X11, solid-gray locked-down desktop, \
          system updates, silent boot, testanyware-agent (systemd user service + AT-SPI2)"),
        (2, "normal",
         "reboot to apply (GDM autologin starts the agent), verify agent health on \
          port 8648, disable + mask sshd, clean shutdown, clone to golden"),
    ]
}

fn emit_linux_golden_plan(version: &str, name: &str, mode: OutputMode) {
    let plan = linux_boot_plan();
    match mode {
        OutputMode::Text => {
            println!("dry-run: would create golden {name} (platform linux, version {version})");
            println!("boot plan (2 normal boots — no SIP/TCC/recovery cycle):");
            for (step, boot_mode, actions) in plan {
                println!("  {step}. [{boot_mode}] {actions}");
            }
        }
        OutputMode::Json => {
            let steps: Vec<Value> = plan
                .iter()
                .map(|(step, boot_mode, actions)| {
                    json!({ "step": step, "mode": boot_mode, "actions": actions })
                })
                .collect();
            print_success(json!({
                "dry_run": true,
                "platform": "linux",
                "version": version,
                "name": name,
                "boot_plan": steps,
            }));
        }
    }
}

/// The Windows golden phases (single-boot unattended install + agent
/// provisioning), as `(step, phase, actions)`. Parity with the boot sequence
/// in `provisioner/scripts/vm-create-golden-windows.sh`. Pure, so the plan is
/// unit-tested and shared by the text and JSON renderers.
fn windows_install_plan() -> [(u32, &'static str, &'static str); 4] {
    [
        (1, "media",
         "build autounattend USB (answer file + agent + VirtIO ARM64 drivers), \
          blank 64GB NVMe disk, blank UEFI vars, swtpm TPM 2.0"),
        (2, "install",
         "boot from ISO + USB; unattended Setup partitions, installs Windows, \
          creates admin/autologin, installs agent logon task + Chocolatey (20–40 min)"),
        (3, "provision",
         "wait for the in-VM agent on :8648, run desktop-setup, reboot to finalize, \
          settle, clean the desktop"),
        (4, "finalize",
         "agent shutdown, then move the setup disk/efivars/tpm into the golden"),
    ]
}

fn emit_windows_golden_plan(version: &str, name: &str, iso: Option<&str>, mode: OutputMode) {
    let plan = windows_install_plan();
    match mode {
        OutputMode::Text => {
            println!("dry-run: would create golden {name} (platform windows, version {version})");
            match iso {
                Some(p) => println!("  install ISO: {p}"),
                None => println!("  install ISO: cached (pass --iso <path> on first run)"),
            }
            println!("plan (single-boot unattended install + agent provisioning):");
            for (step, phase, actions) in plan {
                println!("  {step}. [{phase}] {actions}");
            }
        }
        OutputMode::Json => {
            let steps: Vec<Value> = plan
                .iter()
                .map(|(step, phase, actions)| {
                    json!({ "step": step, "phase": phase, "actions": actions })
                })
                .collect();
            print_success(json!({
                "dry_run": true,
                "platform": "windows",
                "version": version,
                "name": name,
                "iso": iso,
                "plan": steps,
            }));
        }
    }
}

// ---- pure helpers (unit-tested) -----------------------------------------

/// Flatten a `VmListing` into the unified row form.
pub fn listing_items(listing: &VmListing) -> Vec<ListItem> {
    let mut out = Vec::new();
    for g in &listing.goldens {
        out.push(ListItem {
            kind: "golden",
            name: g.name.clone(),
            platform: g.platform.clone(),
            backend: g.backend,
            pid: None,
            vnc: None,
            agent: None,
        });
    }
    for r in &listing.running {
        out.push(ListItem {
            kind: "running",
            name: r.id.clone(),
            platform: r.platform.clone(),
            backend: r.backend,
            pid: r.pid,
            vnc: r.vnc.clone(),
            agent: r.agent.clone(),
        });
    }
    out
}

/// Parse `--filter` into `(field, value)` pairs. Errors on a pair lacking `=`.
pub fn parse_filter(raw: &str) -> Result<Vec<(String, String)>, String> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for pair in raw.split(',') {
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| format!("'{pair}' is not a field=value pair"))?;
        out.push((k.trim().to_string(), v.trim().to_string()));
    }
    Ok(out)
}

/// True if `item` satisfies every filter. Unknown fields never match.
pub fn filter_matches(item: &ListItem, filters: &[(String, String)]) -> bool {
    filters.iter().all(|(field, value)| match field.as_str() {
        "kind" => item.kind == value,
        "platform" => item.platform == *value,
        "backend" => item.backend == value,
        "name" => item.name == *value,
        _ => false,
    })
}

/// Apply the §9.4 list limit. Returns `(shown, returned, total, truncated)`.
pub fn apply_limit<T>(items: Vec<T>, limit: usize, all: bool) -> (Vec<T>, usize, usize, bool) {
    let total = items.len();
    if all || total <= limit {
        return (items, total, total, false);
    }
    let shown: Vec<T> = items.into_iter().take(limit).collect();
    let returned = shown.len();
    (shown, returned, total, true)
}

fn item_to_json(item: &ListItem) -> Value {
    json!({
        "kind": item.kind,
        "name": item.name,
        "platform": item.platform,
        "backend": item.backend,
        "pid": item.pid,
        "vnc": item.vnc,
        "agent": item.agent,
    })
}

fn render_list_text(items: &[ListItem], returned: usize, total: usize, truncated: bool) {
    let goldens: Vec<&ListItem> = items.iter().filter(|i| i.kind == "golden").collect();
    let running: Vec<&ListItem> = items.iter().filter(|i| i.kind == "running").collect();
    println!("Golden images:");
    if goldens.is_empty() {
        println!("  (none)");
    } else {
        for g in goldens {
            println!("  {:<8} {:<40} {}", g.platform, g.name, g.backend);
        }
    }
    println!();
    println!("Running clones:");
    if running.is_empty() {
        println!("  (none)");
    } else {
        for r in running {
            println!(
                "  {:<24} {:<8} vnc={} agent={} pid={}",
                r.name,
                r.platform,
                r.vnc.as_deref().unwrap_or("-"),
                r.agent.as_deref().unwrap_or("-"),
                r.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
            );
        }
    }
    if truncated {
        println!("\nShowing {returned} of {total}. Use --limit N or --all to see more.");
    }
}

fn exit_vm_error(err: VmError, mode: OutputMode) -> ! {
    let code = err.code();
    let exit = err.exit_code();
    let remediation = err.remediation();
    print_error(mode, code, &err.to_string(), remediation.as_deref(), err.details(), exit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_boot_plan_is_three_normal_two_recovery() {
        let plan = golden_boot_plan();
        assert_eq!(plan.len(), 5);
        let normal = plan.iter().filter(|(_, m, _)| *m == "normal").count();
        let recovery = plan.iter().filter(|(_, m, _)| *m == "recovery").count();
        assert_eq!(normal, 3, "script parity: 3 normal boots");
        assert_eq!(recovery, 2, "script parity: 2 recovery boots");
        // Steps are 1-based and contiguous.
        for (i, (step, _, _)) in plan.iter().enumerate() {
            assert_eq!(*step as usize, i + 1);
        }
        // The SIP cycle brackets the TCC grant: disable (2) → grant (3) →
        // enable (4), matching the script's ordering.
        assert!(plan[1].2.contains("disable SIP"));
        assert!(plan[2].2.contains("TCC"));
        assert!(plan[3].2.contains("re-enable SIP"));
    }

    #[test]
    fn linux_boot_plan_is_two_normal_boots() {
        let plan = linux_boot_plan();
        assert_eq!(plan.len(), 2);
        // No recovery cycle on Linux — both boots are normal.
        assert!(plan.iter().all(|(_, m, _)| *m == "normal"));
        for (i, (step, _, _)) in plan.iter().enumerate() {
            assert_eq!(*step as usize, i + 1);
        }
        // Boot 1 provisions; boot 2 applies + clones.
        assert!(plan[0].2.contains("Ubuntu Desktop"));
        assert!(plan[0].2.contains("testanyware-agent"));
        assert!(plan[1].2.contains("verify agent health"));
        assert!(plan[1].2.contains("clone to golden"));
    }

    #[test]
    fn parse_filter_reads_comma_separated_pairs() {
        let f = parse_filter("platform=windows,backend=qemu").unwrap();
        assert_eq!(f, vec![
            ("platform".to_string(), "windows".to_string()),
            ("backend".to_string(), "qemu".to_string()),
        ]);
    }

    #[test]
    fn parse_filter_rejects_a_pair_without_equals() {
        assert!(parse_filter("platform").is_err());
        assert!(parse_filter("platform=windows,oops").is_err());
    }

    #[test]
    fn parse_filter_empty_string_is_no_filters() {
        assert_eq!(parse_filter("").unwrap(), Vec::<(String, String)>::new());
    }

    #[test]
    fn filter_matches_compares_known_fields() {
        let item = ListItem {
            kind: "running",
            name: "testanyware-aa".into(),
            platform: "windows".into(),
            backend: "qemu",
            pid: Some(7),
            vnc: None,
            agent: None,
        };
        assert!(filter_matches(&item, &[("platform".into(), "windows".into())]));
        assert!(!filter_matches(&item, &[("platform".into(), "linux".into())]));
        assert!(filter_matches(&item, &[("kind".into(), "running".into())]));
        // Unknown fields never match — surfaces a typo as an empty result.
        assert!(!filter_matches(&item, &[("colour".into(), "blue".into())]));
    }

    #[test]
    fn apply_limit_truncates_and_flags() {
        let items: Vec<u8> = (0..150).collect();
        let (shown, returned, total, truncated) = apply_limit(items.clone(), 100, false);
        assert_eq!(returned, 100);
        assert_eq!(total, 150);
        assert!(truncated);
        assert_eq!(shown.len(), 100);

        let (shown_all, returned_all, total_all, truncated_all) =
            apply_limit(items, 100, true);
        assert_eq!(returned_all, 150);
        assert_eq!(total_all, 150);
        assert!(!truncated_all);
        assert_eq!(shown_all.len(), 150);
    }
}
