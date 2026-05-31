//! live-vm-gate.rs — the live-VM verification gate (grove `050-live-vm-gate`).
//!
//! The capstone check for the headless automation stack. It clones + starts a
//! real **golden VM** via the tart runner, drives the *built `testanyware`
//! binary as a subprocess* (so it exercises the real command surface, not crate
//! internals), runs four checkable assertions against the running guest, and
//! tears the VM down — even if an assertion fails (see [`VmGuard`]).
//!
//! ## How to run
//!
//! ```text
//! TESTANYWARE_LIVE_VM=1 cargo test -p testanyware-cli --test live-vm-gate -- --ignored live_vm_gate
//! ```
//!
//! Two independent guards keep the normal suite VM-free:
//!   * the test is `#[ignore]`d, so a bare `cargo test` skips it entirely;
//!   * even under `--ignored`, it early-returns unless `TESTANYWARE_LIVE_VM=1`.
//!
//! ## What golden it needs
//!
//! The gate targets the **macOS** golden (`testanyware-golden-macos-tahoe`),
//! driven by the tart backend on an arm64 Mac host. macOS is the only platform
//! whose freshly-booted golden exposes *deterministic* on-screen content with no
//! test-specific tooling baked into the image (`minimal-images`): a clean Finder
//! desktop always shows the system **menu bar** (`Apple │ Finder │ File │ Edit │
//! View │ Go │ Window │ Help`). That one fixture serves three of the four checks,
//! and macOS is also the host where the in-process Apple **Vision** OCR engine
//! runs. Override the platform with `TESTANYWARE_LIVE_VM_PLATFORM`, but the four
//! checks below assert macOS-specific content, so a non-macOS guest is reported
//! as unsupported rather than silently passing.
//!
//! ## The four checks
//!
//! 1. **Input landing** — locate the `File` menu-bar item in `agent snapshot`,
//!    `input click` its centre, and assert the re-snapshot shows the File menu
//!    *open* (its items appear deep in the AX tree). Proves a raw RFB input click
//!    lands at AX-reported coordinates and the agent observes the effect.
//! 2. **`agent show-menu`** — drive `show-menu --menu File` and assert its
//!    rendered tree contains a known File-menu item. Exercises the agent-
//!    orchestrated menu path (RFB click + snapshot), distinct from check 1's raw
//!    `input click`.
//! 3. **ZRLE + Tight capture correctness** — capture the same static menu-bar
//!    region three times, forcing ZRLE / Tight / Raw via the `020`
//!    `TESTANYWARE_RFB_ENCODING` override, and assert the decoded captures are
//!    byte-identical (Raw is ground truth). Proves the live decoders agree.
//! 4. **Live Vision OCR** — `screen find-text File --json` on this macOS host
//!    uses the in-process Vision engine; assert `engine == "vision"` and that
//!    the query is found with a plausible bounding box. **This closes the live
//!    Vision-OCR check deferred by `040-macos-vision-ocr` (ADR-0002 / 0003).**
//!    Best-effort: also exercises the `TESTANYWARE_OCR_FALLBACK=1` daemon path
//!    for parity, skipped (not failed) when the EasyOCR daemon is unavailable.
//!
//! All four run regardless of individual failures; the test asserts once at the
//! end with a per-check summary, so one red check does not mask the others.

use std::process::{Command, Output};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::Value;

/// Path to the binary under test, injected by Cargo for integration tests.
const BIN: &str = env!("CARGO_BIN_EXE_testanyware");

/// The gate only drives a VM when explicitly opted in.
fn gate_enabled() -> bool {
    std::env::var("TESTANYWARE_LIVE_VM").as_deref() == Ok("1")
}

/// Target platform for the golden. The checks assert macOS content, so the
/// default — and only supported value — is `macos`.
fn target_platform() -> String {
    std::env::var("TESTANYWARE_LIVE_VM_PLATFORM").unwrap_or_else(|_| "macos".into())
}

// ---------------------------------------------------------------------------
// Subprocess helpers (mirror cli-contract.rs)
// ---------------------------------------------------------------------------

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to invoke `{BIN} {}`: {e}", args.join(" ")))
}

fn run_env(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output()
        .unwrap_or_else(|e| panic!("failed to invoke `{BIN} {}`: {e}", args.join(" ")))
}

/// Run a `--json` command, asserting success, and parse stdout as JSON.
fn run_json(args: &[&str]) -> Value {
    let out = run(args);
    assert!(
        out.status.success(),
        "`testanyware {}` exited {:?}\nstdout: {}\nstderr: {}",
        args.join(" "),
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "`testanyware {}` stdout did not parse as JSON ({e}):\n{}",
            args.join(" "),
            String::from_utf8_lossy(&out.stdout),
        )
    })
}

// ---------------------------------------------------------------------------
// Teardown guard — stops the VM even if a check panics
// ---------------------------------------------------------------------------

/// Holds a started VM id and runs `vm stop <id>` on drop, so a panicking
/// assertion (or `?`-style early Err) never leaks a running clone.
struct VmGuard(String);

impl Drop for VmGuard {
    fn drop(&mut self) {
        let out = run(&["vm", "stop", &self.0, "--json"]);
        if out.status.success() {
            eprintln!("[teardown] stopped {}", self.0);
        } else {
            eprintln!(
                "[teardown] WARNING: `vm stop {}` failed ({:?}): {}",
                self.0,
                out.status.code(),
                String::from_utf8_lossy(&out.stderr),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot-walking helpers
// ---------------------------------------------------------------------------

/// Centre (rounded) of a top-level menu-bar item, e.g. `File`. Searches the
/// `menuBar` window's direct `elements`. Returns `None` if absent.
fn menu_bar_item_center(snap: &Value, label: &str) -> Option<(i64, i64)> {
    let windows = snap.get("windows")?.as_array()?;
    for w in windows {
        if w.get("windowType").and_then(Value::as_str) != Some("menuBar") {
            continue;
        }
        let Some(elems) = w.get("elements").and_then(Value::as_array) else {
            continue;
        };
        for e in elems {
            if e.get("label").and_then(Value::as_str) != Some(label) {
                continue;
            }
            let (Some(x), Some(y), Some(wd), Some(ht)) = (
                e.get("positionX").and_then(Value::as_f64),
                e.get("positionY").and_then(Value::as_f64),
                e.get("sizeWidth").and_then(Value::as_f64),
                e.get("sizeHeight").and_then(Value::as_f64),
            ) else {
                continue;
            };
            return Some(((x + wd / 2.0).round() as i64, (y + ht / 2.0).round() as i64));
        }
    }
    None
}

/// Collect every `menu-item` label anywhere in a snapshot, walking the nested
/// `children` arrays (an open menu's items live deep in the AX tree).
fn collect_menu_item_labels(snap: &Value) -> Vec<String> {
    fn walk(v: &Value, out: &mut Vec<String>) {
        if v.get("role").and_then(Value::as_str) == Some("menu-item") {
            if let Some(l) = v.get("label").and_then(Value::as_str) {
                out.push(l.to_string());
            }
        }
        if let Some(children) = v.get("children").and_then(Value::as_array) {
            for c in children {
                walk(c, out);
            }
        }
    }
    let mut out = Vec::new();
    if let Some(windows) = snap.get("windows").and_then(Value::as_array) {
        for w in windows {
            if let Some(elems) = w.get("elements").and_then(Value::as_array) {
                for e in elems {
                    walk(e, &mut out);
                }
            }
        }
    }
    out
}

/// Known items in Finder's `File` menu — any one present means the menu opened.
const FILE_MENU_ITEMS: &[&str] = &[
    "New Finder Window",
    "New Folder",
    "New Smart Folder",
    "Get Info",
    "Find",
];

fn file_menu_is_open(snap: &Value) -> bool {
    let labels = collect_menu_item_labels(snap);
    FILE_MENU_ITEMS
        .iter()
        .any(|item| labels.iter().any(|l| l == item))
}

/// Dismiss any open menu and let the UI settle.
fn dismiss_menus(vm: &str) {
    let _ = run(&["input", "key", "Escape", "--vm", vm, "--json"]);
    sleep(Duration::from_millis(300));
}

// ---------------------------------------------------------------------------
// Readiness
// ---------------------------------------------------------------------------

/// Poll `agent snapshot` until the Finder menu bar with a `File` item appears,
/// proving the guest desktop has rendered. Returns the File item centre.
fn wait_for_menu_bar(vm: &str, timeout: Duration) -> Result<(i64, i64), String> {
    let deadline = Instant::now() + timeout;
    // Every loop iteration assigns `last` before the deadline check reads it.
    let mut last: String;
    loop {
        let out = run(&["agent", "snapshot", "--vm", vm, "--json"]);
        if out.status.success() {
            if let Ok(snap) = serde_json::from_slice::<Value>(&out.stdout) {
                if let Some(c) = menu_bar_item_center(&snap, "File") {
                    return Ok(c);
                }
                last = "snapshot had no Finder menu bar `File` item".into();
            } else {
                last = "snapshot stdout did not parse as JSON".into();
            }
        } else {
            last = format!(
                "agent snapshot exited {:?}: {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr),
            );
        }
        if Instant::now() >= deadline {
            return Err(format!("menu bar never became ready: {last}"));
        }
        sleep(Duration::from_millis(750));
    }
}

// ---------------------------------------------------------------------------
// The four checks. Each returns Ok(note) or Err(reason).
// ---------------------------------------------------------------------------

/// Check 1 — input landing: raw `input click` on the File menu-bar item opens
/// the menu, and the agent's snapshot reflects it.
fn check_input_landing(vm: &str, file_center: (i64, i64)) -> Result<String, String> {
    dismiss_menus(vm);
    let (cx, cy) = file_center;
    let click = run(&[
        "input",
        "click",
        &cx.to_string(),
        &cy.to_string(),
        "--vm",
        vm,
        "--json",
    ]);
    if !click.status.success() {
        return Err(format!(
            "input click ({cx},{cy}) failed: {}",
            String::from_utf8_lossy(&click.stderr)
        ));
    }

    // Re-snapshot, allowing a brief window for the menu to render.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snap = run_json(&["agent", "snapshot", "--vm", vm, "--json"]);
        if file_menu_is_open(&snap) {
            dismiss_menus(vm);
            return Ok(format!(
                "click ({cx},{cy}) opened the File menu (items observed in AX tree)"
            ));
        }
        if Instant::now() >= deadline {
            dismiss_menus(vm);
            return Err(format!(
                "after input click ({cx},{cy}), the File menu did not open; \
                 menu items seen: {:?}",
                collect_menu_item_labels(&snap)
            ));
        }
        sleep(Duration::from_millis(400));
    }
}

/// Check 2 — `agent show-menu`: the agent-orchestrated menu path opens File and
/// its rendered tree contains a known item.
fn check_show_menu(vm: &str) -> Result<String, String> {
    dismiss_menus(vm);
    let out = run(&["agent", "show-menu", "--menu", "File", "--vm", vm]);
    if !out.status.success() {
        return Err(format!(
            "show-menu --menu File exited {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr),
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let found = FILE_MENU_ITEMS.iter().find(|item| text.contains(*item));
    dismiss_menus(vm);
    match found {
        Some(item) => Ok(format!("show-menu File rendered {item:?}")),
        None => Err(format!(
            "show-menu File output contained none of {FILE_MENU_ITEMS:?}; got:\n{text}"
        )),
    }
}

/// Check 3 — encoding correctness: ZRLE / Tight / Raw captures of the same
/// static region are byte-identical.
fn check_encoding(vm: &str) -> Result<String, String> {
    dismiss_menus(vm);
    // A static slice of the left menu bar (Apple..Go): no clock (right side)
    // and no notifications (right side), so the pixels do not change between
    // the three sequential captures.
    let region = "0,0,250,28";
    let dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;

    let mut bytes: Vec<(&str, Vec<u8>)> = Vec::new();
    for enc in ["zrle", "tight", "raw"] {
        let path = dir.path().join(format!("enc-{enc}.png"));
        let path_str = path.to_str().unwrap();
        let out = run_env(
            &[
                "screen", "capture", "--vm", vm, "--region", region, "-o", path_str, "--json",
            ],
            &[("TESTANYWARE_RFB_ENCODING", enc)],
        );
        if !out.status.success() {
            return Err(format!(
                "capture forced {enc} exited {:?}: {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr),
            ));
        }
        let body: Value = serde_json::from_slice(&out.stdout)
            .map_err(|e| format!("capture {enc} stdout not JSON: {e}"))?;
        if body.get("width").and_then(Value::as_u64) != Some(250)
            || body.get("height").and_then(Value::as_u64) != Some(28)
        {
            return Err(format!("capture {enc} reported unexpected size: {body}"));
        }
        let data = std::fs::read(&path).map_err(|e| format!("read {enc} png: {e}"))?;
        bytes.push((enc, data));
    }

    let (_, raw) = &bytes[2];
    for (enc, data) in &bytes[..2] {
        if data != raw {
            return Err(format!(
                "forced {enc} capture differs from Raw ({} vs {} bytes) — \
                 the {enc} decoder disagrees with Raw ground truth",
                data.len(),
                raw.len(),
            ));
        }
    }
    Ok(format!(
        "ZRLE, Tight, Raw captures of region {region} are byte-identical ({} bytes)",
        raw.len()
    ))
}

/// Check 4 — live Vision OCR. Closes the deferred `040-macos-vision-ocr` live
/// check (ADR-0002 / 0003).
fn check_vision_ocr(vm: &str) -> Result<String, String> {
    dismiss_menus(vm);
    let body = {
        let out = run(&[
            "screen",
            "find-text",
            "File",
            "--vm",
            vm,
            "--json",
            "--timeout",
            "10",
        ]);
        if !out.status.success() {
            return Err(format!(
                "find-text File exited {:?}: {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr),
            ));
        }
        serde_json::from_slice::<Value>(&out.stdout)
            .map_err(|e| format!("find-text stdout not JSON: {e}"))?
    };

    let engine = body.get("engine").and_then(Value::as_str).unwrap_or("");
    if engine != "vision" {
        return Err(format!(
            "expected Vision engine on macOS host, got engine={engine:?}"
        ));
    }
    let detections = body
        .get("detections")
        .and_then(Value::as_array)
        .ok_or("find-text response had no detections array")?;
    let hit = detections.iter().find(|d| {
        d.get("text")
            .and_then(Value::as_str)
            .is_some_and(|t| t.to_lowercase().contains("file"))
    });
    let Some(hit) = hit else {
        return Err(format!(
            "Vision OCR did not find the query text 'File'; detections: {detections:?}"
        ));
    };
    let w = hit.get("width").and_then(Value::as_f64).unwrap_or(0.0);
    let h = hit.get("height").and_then(Value::as_f64).unwrap_or(0.0);
    if w <= 0.0 || h <= 0.0 {
        return Err(format!("Vision OCR hit has an implausible bounding box: {hit}"));
    }

    // Best-effort daemon parity (§ optional). The EasyOCR daemon may be absent
    // on the host (no Python env); skip, do not fail, when unavailable.
    let parity = {
        let out = run_env(
            &[
                "screen",
                "find-text",
                "File",
                "--vm",
                vm,
                "--json",
                "--timeout",
                "5",
            ],
            &[("TESTANYWARE_OCR_FALLBACK", "1")],
        );
        let fb: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
        match fb.get("engine").and_then(Value::as_str) {
            Some("easyocr_daemon") => "daemon parity ✓ (engine=easyocr_daemon also found 'File')",
            _ => {
                let code = fb.get("code").and_then(Value::as_str).unwrap_or("unavailable");
                eprintln!("  [vision-ocr] daemon parity skipped (fallback {code})");
                "daemon parity skipped (unavailable on this host)"
            }
        }
    };

    Ok(format!(
        "engine=vision found 'File' at {w:.0}x{h:.0} px; {parity}"
    ))
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

#[test]
#[ignore = "live VM: TESTANYWARE_LIVE_VM=1 cargo test --test live-vm-gate -- --ignored live_vm_gate"]
fn live_vm_gate() {
    if !gate_enabled() {
        eprintln!(
            "live_vm_gate: skipped — set TESTANYWARE_LIVE_VM=1 to drive a real golden VM."
        );
        return;
    }

    let platform = target_platform();
    if platform != "macos" {
        eprintln!(
            "live_vm_gate: only the macOS golden is supported (the checks assert \
             macOS menu-bar / Vision content); got TESTANYWARE_LIVE_VM_PLATFORM={platform:?}."
        );
        return;
    }

    eprintln!("live_vm_gate: starting {platform} golden (clone + boot)…");
    let start = run_json(&["vm", "start", "--platform", &platform, "--json"]);
    let id = start
        .get("id")
        .and_then(Value::as_str)
        .expect("vm start --json must return an id")
        .to_string();
    eprintln!("live_vm_gate: started {id}; running checks.");
    let _guard = VmGuard(id.clone());

    // Wait for the desktop to render before asserting on its content.
    let file_center = wait_for_menu_bar(&id, Duration::from_secs(60))
        .unwrap_or_else(|e| panic!("guest never became ready: {e}"));

    // Run all four; collect outcomes so one failure does not mask the rest.
    // Order: read-only/static-screen checks first, interactive ones last.
    let results: Vec<(&str, Result<String, String>)> = vec![
        ("vision-ocr", check_vision_ocr(&id)),
        ("encoding", check_encoding(&id)),
        ("input-landing", check_input_landing(&id, file_center)),
        ("show-menu", check_show_menu(&id)),
    ];

    eprintln!("\nlive_vm_gate results:");
    let mut failures = Vec::new();
    for (name, res) in &results {
        match res {
            Ok(note) => eprintln!("  ✓ {name}: {note}"),
            Err(reason) => {
                eprintln!("  ✗ {name}: {reason}");
                failures.push(format!("{name}: {reason}"));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} live-VM checks failed:\n  - {}",
        failures.len(),
        results.len(),
        failures.join("\n  - "),
    );
    eprintln!("\nlive_vm_gate: all {} checks passed.", results.len());
}

// ---------------------------------------------------------------------------
// Offline unit tests for the pure snapshot-walking helpers (run in the normal
// suite; no VM). These pin the JSON shapes the gate depends on.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod helper_tests {
    use super::*;
    use serde_json::json;

    fn sample_menu_bar() -> Value {
        json!({
            "windows": [
                { "title": "Notification Center", "windowType": "systemDialog",
                  "elements": [ { "role": "group", "label": null } ] },
                { "title": "Menu Bar", "windowType": "menuBar", "elements": [
                    { "role": "menu-item", "label": "Apple",
                      "positionX": 10.0, "positionY": 0.0, "sizeWidth": 34.0, "sizeHeight": 30.0 },
                    { "role": "menu-item", "label": "File",
                      "positionX": 106.0, "positionY": 0.0, "sizeWidth": 42.0, "sizeHeight": 30.0 }
                ] }
            ]
        })
    }

    #[test]
    fn finds_menu_bar_item_center() {
        let snap = sample_menu_bar();
        assert_eq!(menu_bar_item_center(&snap, "File"), Some((127, 15)));
        assert_eq!(menu_bar_item_center(&snap, "Apple"), Some((27, 15)));
        assert_eq!(menu_bar_item_center(&snap, "Nonexistent"), None);
    }

    #[test]
    fn detects_open_file_menu_via_nested_children() {
        // A menu-item whose submenu (nested children) holds the File items.
        let snap = json!({
            "windows": [ { "windowType": "menuBar", "elements": [
                { "role": "menu-item", "label": "File", "children": [
                    { "role": "menu", "children": [
                        { "role": "menu-item", "label": "New Finder Window" },
                        { "role": "menu-item", "label": "New Folder" }
                    ] }
                ] }
            ] } ]
        });
        assert!(file_menu_is_open(&snap));

        // Closed: only the bar items, no submenu.
        assert!(!file_menu_is_open(&sample_menu_bar()));
    }

    #[test]
    fn missing_windows_array_is_handled() {
        assert_eq!(menu_bar_item_center(&json!({}), "File"), None);
        assert!(!file_menu_is_open(&json!({})));
        assert!(collect_menu_item_labels(&json!({})).is_empty());
    }
}
