//! A version mismatch between the Host CLI and the running app is never silent (spec: "Version
//! skew").
//!
//! The real `testanyware` binary runs against the real `testanyware-app` executable in test
//! mode. The app's test mode forges its build, its protocol range and the build of its bundle
//! on disk, so each row of the remedy table of the spec is a run. The Host CLI is the real one:
//! its build is the version of this workspace, and its range is 1 to 1.
//!
//! The vehicle is `agent snapshot --vm`, which needs a fake In-VM agent and nothing else. A run
//! that the app refuses must never reach the agent.

#![cfg(unix)]

mod common;

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use common::{serial, FakeAgent, OutputExt, TestApp};
use serde_json::{json, Value};
use wiremock::ResponseTemplate;

const VM: &str = "testanyware-feed0002";

/// The build of the Host CLI under test.
const CLI_BUILD: &str = env!("CARGO_PKG_VERSION");

/// What `agent snapshot --vm` printed before the app, and prints now.
const SNAPSHOT_TEXT: &str = "\"Fake Window\" (window) 800x600 [focused] app:\"FakeApp\"\n";

fn snapshot_json() -> String {
    let envelope = json!({
        "ok": true,
        "schema_version": "1.0",
        "windows": [{
            "appName": "FakeApp",
            "focused": true,
            "positionX": 10.0,
            "positionY": 20.0,
            "sizeHeight": 600.0,
            "sizeWidth": 800.0,
            "title": "Fake Window",
            "windowType": "window"
        }]
    });
    format!("{}\n", serde_json::to_string_pretty(&envelope).unwrap())
}

/// A fake agent that answers the snapshot, and an app that states what `app_args` forge.
struct Fixture {
    agent: FakeAgent,
    app: TestApp,
}

impl Fixture {
    fn new(app_args: &[&str]) -> Self {
        let agent = FakeAgent::start();
        agent.mount(
            "POST",
            "/snapshot",
            ResponseTemplate::new(200).set_body_json(json!({
                "windows": [{
                    "title": "Fake Window",
                    "windowType": "window",
                    "sizeWidth": 800,
                    "sizeHeight": 600,
                    "positionX": 10,
                    "positionY": 20,
                    "appName": "FakeApp",
                    "focused": true
                }]
            })),
        );
        let args: Vec<String> = app_args.iter().map(|a| a.to_string()).collect();
        let app = TestApp::start_after_with_args(|app| app.add_vm(VM, agent.addr()), &args);
        Fixture { agent, app }
    }

    fn snapshot(&self, extra: &[&str]) -> std::process::Output {
        let mut args = vec!["agent", "snapshot", "--vm", VM];
        args.extend_from_slice(extra);
        self.app.cli(&args)
    }
}

/// The lines of stderr that carry the notice.
fn notice_lines(stderr: &str) -> Vec<&str> {
    stderr.lines().filter(|l| l.contains("notice")).collect()
}

// ---- ranges that do not overlap: the command fails, with one remedy of the table ----

/// Run `agent snapshot` against an app with `app_args`, and require the failure of the table: the
/// envelope names both builds, both ranges and `remedy`, and no request reached the guest.
fn fails_with(app_args: &[&str], app_build: &str, app_range: &str, remedy_word: &str, remedy: &str) {
    let fixture = Fixture::new(app_args);

    let json = fixture.snapshot(&["--json"]);
    assert_eq!(json.code(), 1, "{}", json.err());
    let envelope = json.json();
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_eq!(envelope["code"], "APP_INCOMPATIBLE", "{envelope}");
    assert_eq!(envelope["details"]["cause"], "protocol", "{envelope}");
    assert_eq!(envelope["details"]["remedy"], remedy_word, "{envelope}");
    assert_eq!(envelope["remediation"], remedy, "{envelope}");
    let message = envelope["message"].as_str().unwrap();
    for needle in [app_build, CLI_BUILD, app_range, "1-1"] {
        assert!(message.contains(needle), "{needle} missing from {message:?}");
    }
    assert_eq!(envelope["details"]["app"]["build"], app_build);
    assert_eq!(envelope["details"]["client"]["build"], CLI_BUILD);

    let text = fixture.snapshot(&[]);
    assert_eq!(text.code(), 1);
    assert_eq!(text.out(), "", "a failure prints nothing on stdout");
    assert!(text.err().contains(remedy), "{}", text.err());

    assert_eq!(fixture.agent.request_count(), 0, "a refused run reached the guest");
}

#[test]
fn an_older_app_with_a_newer_bundle_fails_and_says_to_quit_and_open_it() {
    let _lock = serial();
    fails_with(
        &["--test-build", "0.0.0", "--test-protocol-range", "0-0", "--test-bundle-build", "9.9.9"],
        "0.0.0",
        "0-0",
        "quit_and_open_app",
        "Quit TestAnyware from its menu, and open it again.",
    );
}

#[test]
fn an_older_app_with_an_older_bundle_fails_and_says_to_upgrade_the_cask() {
    let _lock = serial();
    fails_with(
        &["--test-build", "0.0.0", "--test-protocol-range", "0-0", "--test-bundle-build", "0.0.0"],
        "0.0.0",
        "0-0",
        "upgrade_cask",
        "Run `brew upgrade --cask testanyware-app`.",
    );
}

#[test]
fn an_older_app_that_runs_from_no_bundle_says_to_upgrade_the_cask() {
    let _lock = serial();
    fails_with(
        &["--test-build", "0.0.0", "--test-protocol-range", "0-0"],
        "0.0.0",
        "0-0",
        "upgrade_cask",
        "Run `brew upgrade --cask testanyware-app`.",
    );
}

#[test]
fn an_older_host_cli_fails_and_says_to_upgrade_it() {
    let _lock = serial();
    fails_with(
        &["--test-build", "9.0.0", "--test-protocol-range", "2-3", "--test-bundle-build", "9.0.0"],
        "9.0.0",
        "2-3",
        "upgrade_host_cli",
        "Run `brew upgrade testanyware`.",
    );
}

// ---- builds that differ, with ranges that overlap: the command proceeds, with one notice ----

/// Run the command in both output modes. Require the output of today on stdout, and exactly one
/// notice line on stderr that names both builds and `remedy`.
fn proceeds_with_notice(app_args: &[&str], app_build: &str, remedy: &str) {
    let fixture = Fixture::new(app_args);

    let text = fixture.snapshot(&[]);
    assert_eq!(text.code(), 0, "{}", text.err());
    assert_eq!(text.out(), SNAPSHOT_TEXT, "the notice changes no stdout");
    let err = text.err();
    assert_eq!(err.lines().count(), 1, "one line on stderr, got {err:?}");
    for needle in [app_build, CLI_BUILD, remedy] {
        assert!(err.contains(needle), "{needle} missing from {err:?}");
    }

    let json = fixture.snapshot(&["--json"]);
    assert_eq!(json.code(), 0, "{}", json.err());
    assert_eq!(json.out(), snapshot_json(), "the notice changes no JSON envelope");
    assert_eq!(notice_lines(&json.err()).len(), 1, "{}", json.err());
    assert!(json.err().contains(remedy), "{}", json.err());
}

#[test]
fn builds_that_differ_with_a_newer_bundle_proceed_and_say_to_quit_and_open_the_app() {
    let _lock = serial();
    proceeds_with_notice(
        &["--test-build", "0.0.0", "--test-bundle-build", "9.9.9"],
        "0.0.0",
        "Quit TestAnyware from its menu, and open it again.",
    );
}

#[test]
fn builds_that_differ_with_an_older_bundle_proceed_and_say_to_upgrade_the_cask() {
    let _lock = serial();
    proceeds_with_notice(
        &["--test-build", "0.0.0", "--test-bundle-build", "0.0.0"],
        "0.0.0",
        "Run `brew upgrade --cask testanyware-app`.",
    );
}

#[test]
fn a_newer_app_proceeds_and_says_to_upgrade_the_host_cli() {
    let _lock = serial();
    proceeds_with_notice(
        &["--test-build", "9.0.0", "--test-bundle-build", "9.0.0"],
        "9.0.0",
        "Run `brew upgrade testanyware`.",
    );
}

#[test]
fn a_notice_names_both_builds_in_a_run_that_fails_for_another_reason() {
    let _lock = serial();
    let fixture = Fixture::new(&["--test-build", "0.0.0"]);
    let out = fixture.app.cli(&["agent", "snapshot", "--vm", "testanyware-nosuchvm", "--json"]);
    // The exit code of `VM_NOT_FOUND`, which the skew does not change.
    assert_eq!(out.code(), 3, "{}", out.err());
    assert_eq!(out.json()["code"], "VM_NOT_FOUND");
    assert_eq!(notice_lines(&out.err()).len(), 1, "{}", out.err());
}

// ---- the control: builds that agree are silent ----

#[test]
fn equal_builds_print_nothing_on_stderr() {
    let _lock = serial();
    // A bundle that is newer changes nothing while the running app is the Host CLI's build.
    let fixture = Fixture::new(&["--test-bundle-build", "9.9.9"]);
    let out = fixture.snapshot(&[]);
    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(out.out(), SNAPSHOT_TEXT);
    assert_eq!(out.err(), "");
}

// ---- the status response ----

/// `GET /v1/status` on the app's socket: the response headers, lower-cased, and the JSON body.
fn status(app: &TestApp) -> (String, Value) {
    let mut stream = UnixStream::connect(app.socket()).expect("the app answers");
    stream
        .write_all(b"GET /v1/status HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    let (head, body) = reply.split_once("\r\n\r\n").expect("a response");
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    (head.to_lowercase(), serde_json::from_str(body).expect("a JSON body"))
}

#[test]
fn the_status_says_that_the_running_app_is_older_than_its_bundle() {
    let _lock = serial();
    let fixture = Fixture::new(&["--test-build", "1.0.0", "--test-bundle-build", "1.1.0"]);
    let (head, body) = status(&fixture.app);
    assert_eq!(body["build"], "1.0.0");
    assert_eq!(body["bundle_build"], "1.1.0");
    assert_eq!(body["older_than_bundle"], true);
    assert!(head.contains("x-testanyware-bundle-build: 1.1.0"), "{head}");
}

#[test]
fn the_status_says_that_the_app_is_not_older_when_the_bundle_is_not_newer() {
    let _lock = serial();
    for bundle in ["1.0.0", "0.9.0"] {
        let fixture = Fixture::new(&["--test-build", "1.0.0", "--test-bundle-build", bundle]);
        let (_, body) = status(&fixture.app);
        assert_eq!(body["bundle_build"], bundle);
        assert_eq!(body["older_than_bundle"], false, "bundle {bundle}");
    }
}

#[test]
fn an_app_that_runs_from_no_bundle_states_no_bundle_build() {
    let _lock = serial();
    let fixture = Fixture::new(&[]);
    let (head, body) = status(&fixture.app);
    assert_eq!(body["bundle_build"], Value::Null);
    assert_eq!(body["older_than_bundle"], false);
    assert!(!head.contains("x-testanyware-bundle-build"), "{head}");
}
