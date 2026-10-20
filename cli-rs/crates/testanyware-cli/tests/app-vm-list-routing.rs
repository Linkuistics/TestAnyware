//! `vm list` through the TestAnyware app (spec: "The protocol", "Lifecycle and adoption").
//!
//! The real Host CLI binary runs against the real app executable in test mode. The VM records
//! are sidecars in a temporary state directory, and `tart list` is a fake script in the one
//! directory where the app looks for tools. The fixture has the same rows on every host except
//! the tart rows, which only a macOS host lists.
//!
//! Where a host keeps the direct path (Linux, Windows), each test also runs the CLI with no
//! socket and requires the same output. A macOS host has no direct path, so there the expected
//! output is written out below, and the routed run must print it.

#![cfg(unix)]

mod common;

use common::{serial, OutputExt, TestApp, HOST_GOES_DIRECT};
use serde_json::{json, Value};

/// What the fake `tart list --format json` prints: a golden, and a running clone with sidecars.
const TART_LIST: &str = r#"[{"Name":"testanyware-golden-macos-27","State":"stopped"},{"Name":"testanyware-bbbb2222","State":"running"}]"#;

const QEMU_GOLDEN: &str = "testanyware-golden-windows-11";
const QEMU_CLONE: &str = "testanyware-aaaa1111";
const TART_GOLDEN: &str = "testanyware-golden-macos-27";
const TART_CLONE: &str = "testanyware-bbbb2222";

/// A fake tool directory, one QEMU golden, one running QEMU clone and one running tart clone.
fn fixture() -> TestApp {
    TestApp::start_after(|app| {
        app.add_tool("tart", &format!("[ \"$1\" = list ] && echo '{TART_LIST}'"));
        app.add_qemu_golden(QEMU_GOLDEN);
        app.add_running_qemu_clone(QEMU_CLONE, "linux", 5901);
        app.add_sidecars(TART_CLONE, "macos", "tart", 5902);
    })
}

/// One row of the listing.
struct Row {
    kind: &'static str,
    name: &'static str,
    platform: &'static str,
    backend: &'static str,
    vnc_port: Option<u16>,
}

/// The rows that the fixture lists on this host.
fn rows() -> Vec<Row> {
    let golden = |name, platform, backend| Row {
        kind: "golden",
        name,
        platform,
        backend,
        vnc_port: None,
    };
    let running = |name, platform, backend, port| Row {
        kind: "running",
        name,
        platform,
        backend,
        vnc_port: Some(port),
    };
    let mut rows = vec![golden(QEMU_GOLDEN, "windows", "qemu")];
    if cfg!(target_os = "macos") {
        rows.push(golden(TART_GOLDEN, "macos", "tart"));
    }
    rows.push(running(QEMU_CLONE, "linux", "qemu", 5901));
    if cfg!(target_os = "macos") {
        rows.push(running(TART_CLONE, "macos", "tart", 5902));
    }
    rows
}

fn item(row: &Row) -> Value {
    let running = row.vnc_port.is_some();
    json!({
        "kind": row.kind,
        "name": row.name,
        "platform": row.platform,
        "backend": row.backend,
        "pid": running.then(std::process::id),
        "vnc": row.vnc_port.map(|port| format!("127.0.0.1:{port}")),
        "agent": running.then_some("127.0.0.1:8648"),
    })
}

/// The text that `vm list` has always printed for `rows`.
fn text(rows: &[Row]) -> String {
    let mut out = String::from("Golden images:\n");
    let goldens: Vec<&Row> = rows.iter().filter(|r| r.kind == "golden").collect();
    for row in &goldens {
        out += &format!("  {:<8} {:<40} {}\n", row.platform, row.name, row.backend);
    }
    out += "\nRunning clones:\n";
    for row in rows.iter().filter(|r| r.kind == "running") {
        out += &format!(
            "  {:<24} {:<8} vnc=127.0.0.1:{} agent=127.0.0.1:8648 pid={}\n",
            row.name,
            row.platform,
            row.vnc_port.unwrap(),
            std::process::id()
        );
    }
    out
}

/// Run `args` through the app, and where the host keeps the direct path, directly too. Both must
/// print the same, and `check` reads that output.
fn routed_and_direct(app: &TestApp, args: &[&str], check: impl Fn(&std::process::Output)) {
    let routed = app.cli(args);
    assert_eq!(routed.code(), 0, "routed {args:?}: {}", routed.err());
    assert_eq!(routed.err(), "", "stderr of routed {args:?}");
    check(&routed);
    if HOST_GOES_DIRECT {
        let direct = app.cli_default(args);
        assert_eq!(direct.code(), 0, "direct {args:?}: {}", direct.err());
        assert_eq!(direct.out(), routed.out(), "direct and routed {args:?}");
        assert_eq!(direct.err(), "", "stderr of direct {args:?}");
    }
}

#[test]
fn the_text_listing_through_the_app_is_the_listing_that_vm_list_prints() {
    let _lock = serial();
    let app = fixture();
    routed_and_direct(&app, &["vm", "list"], |out| {
        assert_eq!(out.out(), text(&rows()));
    });
}

#[test]
fn the_json_listing_through_the_app_is_the_listing_that_vm_list_prints() {
    let _lock = serial();
    let app = fixture();
    let rows = rows();
    routed_and_direct(&app, &["vm", "list", "--json"], |out| {
        let envelope = out.json();
        assert_eq!(envelope["ok"], true, "{envelope}");
        assert_eq!(envelope["schema_version"], "1.0");
        let expected: Vec<Value> = rows.iter().map(item).collect();
        assert_eq!(envelope["items"], Value::Array(expected));
        assert_eq!(envelope["returned"], rows.len());
        assert_eq!(envelope["total"], rows.len());
        assert_eq!(envelope["truncated"], false);
    });
}

#[test]
fn a_filter_reaches_the_app_and_selects_the_rows() {
    let _lock = serial();
    let app = fixture();
    routed_and_direct(&app, &["vm", "list", "--json", "--filter", "platform=linux"], |out| {
        let envelope = out.json();
        assert_eq!(envelope["total"], 1, "{envelope}");
        assert_eq!(envelope["items"][0]["name"], QEMU_CLONE);
    });
    routed_and_direct(
        &app,
        &["vm", "list", "--json", "--filter", "kind=golden,backend=qemu"],
        |out| {
            let envelope = out.json();
            assert_eq!(envelope["total"], 1, "{envelope}");
            assert_eq!(envelope["items"][0]["name"], QEMU_GOLDEN);
        },
    );
    // An unknown field never matches, so a typo shows as an empty listing.
    routed_and_direct(&app, &["vm", "list", "--json", "--filter", "colour=blue"], |out| {
        let envelope = out.json();
        assert_eq!(envelope["total"], 0, "{envelope}");
        assert_eq!(envelope["items"], json!([]));
    });
}

#[test]
fn a_limit_reaches_the_app_and_the_listing_says_it_was_cut() {
    let _lock = serial();
    let app = fixture();
    let total = rows().len();
    routed_and_direct(&app, &["vm", "list", "--json", "--limit", "1"], |out| {
        let envelope = out.json();
        assert_eq!(envelope["returned"], 1, "{envelope}");
        assert_eq!(envelope["total"], total);
        assert_eq!(envelope["truncated"], true);
        assert_eq!(envelope["items"][0]["name"], QEMU_GOLDEN);
    });
    routed_and_direct(&app, &["vm", "list", "--limit", "1"], |out| {
        let printed = out.out();
        assert!(
            printed.ends_with(&format!(
                "\nShowing 1 of {total}. Use --limit N or --all to see more.\n"
            )),
            "{printed}"
        );
    });
    routed_and_direct(&app, &["vm", "list", "--json", "--all"], |out| {
        let envelope = out.json();
        assert_eq!(envelope["returned"], total, "{envelope}");
        assert_eq!(envelope["truncated"], false);
    });
}

#[test]
fn the_app_rebuilds_the_listing_at_each_request() {
    let _lock = serial();
    let app = fixture();
    let before = app.cli(&["vm", "list", "--json", "--filter", "kind=running"]);
    let count = before.json()["total"].as_u64().unwrap();

    // A VM that another process started has the same sidecars. The app adopts it by reading them.
    app.add_running_qemu_clone("testanyware-cccc3333", "windows", 5903);
    let after = app.cli(&["vm", "list", "--json", "--filter", "kind=running"]);
    let envelope = after.json();
    assert_eq!(envelope["total"].as_u64().unwrap(), count + 1, "{envelope}");
    let names: Vec<&str> = envelope["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"testanyware-cccc3333"), "{names:?}");
}

#[test]
fn an_invalid_filter_fails_in_the_cli_before_it_asks_the_app() {
    let _lock = serial();
    // No app runs, so a command that asked it would fail as unreachable.
    let app = TestApp::not_running();
    let out = app.cli(&["vm", "list", "--json", "--filter", "platform"]);
    assert_eq!(out.code(), 2, "{}", out.err());
    let envelope = out.json();
    assert_eq!(envelope["code"], "USAGE_ERROR", "{envelope}");
    assert!(envelope["message"].as_str().unwrap().contains("invalid --filter"));
}

#[test]
fn an_app_that_does_not_run_fails_the_listing_and_names_why() {
    let _lock = serial();
    let app = TestApp::not_running();
    let out = app.cli(&["vm", "list", "--json"]);
    assert_eq!(out.code(), 1, "{}", out.err());
    let envelope = out.json();
    assert_eq!(envelope["code"], "APP_UNREACHABLE", "{envelope}");
    assert_eq!(envelope["details"]["cause"], "not_running");
    // The listing falls back to nothing: a direct listing would print rows and exit 0.
    assert_eq!(envelope["ok"], false);

    let text = app.cli(&["vm", "list"]);
    assert_eq!(text.code(), 1);
    assert_eq!(text.out(), "");
    assert!(text.err().contains("TestAnyware app"), "{}", text.err());
}

#[test]
fn directories_that_differ_fail_the_listing() {
    let _lock = serial();
    let app = fixture();
    let other = app.root().join("other-state");
    let out = app.cli_with(
        &["vm", "list", "--json"],
        &[
            ("TESTANYWARE_APP_SOCKET", app.socket().to_str().unwrap()),
            ("XDG_STATE_HOME", other.to_str().unwrap()),
        ],
    );
    assert_eq!(out.code(), 1, "{}", out.err());
    let envelope = out.json();
    assert_eq!(envelope["code"], "APP_INCOMPATIBLE", "{envelope}");
    assert_eq!(envelope["details"]["cause"], "directories");
}
