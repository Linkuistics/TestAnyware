//! `vm {start|stop|list|delete}` command handlers.
//!
//! Bridges clap-parsed args to the `testanyware-vm` crate and emits the
//! contract §3 JSON envelope (or text). Ports the surface of
//! `cli/Sources/testanyware/VMCommand.swift`.

use serde_json::{json, Value};

use testanyware_vm::lifecycle::{Platform, VmLifecycle, VmListing, VmStartOptions};
use testanyware_vm::{VmError, VmMeta, VmPaths};

use crate::output::{print_error, print_success, OutputMode};

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
    if viewer {
        eprintln!(
            "note: --viewer is not yet ported to the Rust CLI (backlog task 8); \
             starting the VM without a viewer window."
        );
    }
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
