//! The agent commands routed through the TestAnyware app (spec: "The protocol").
//!
//! Each test runs the real `testanyware` binary against the real `testanyware-app` executable
//! in its test mode, with a wiremock In-VM agent on loopback as the guest. `TESTANYWARE_APP_SOCKET`
//! turns the routing on. A command through the app must print what the same command prints
//! on the direct path, and it must dial nothing itself.

#![cfg(unix)]

mod common;

use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

use common::{pattern, serial, FakeAgent, OutputExt, TestApp};
use serde_json::json;
use wiremock::ResponseTemplate;

const VM: &str = "testanyware-feed0001";

/// A fake agent that answers the agent commands, and an app that serves its VM.
struct Fixture {
    agent: FakeAgent,
    app: TestApp,
}

impl Fixture {
    /// The fakes start first, and the app spawns once (see the rules in `common`).
    fn new() -> Self {
        let agent = FakeAgent::start();
        agent.mount(
            "GET",
            "/health",
            ResponseTemplate::new(200)
                .set_body_json(json!({ "accessible": true, "platform": "macos" })),
        );
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
        agent.mount(
            "POST",
            "/exec",
            ResponseTemplate::new(200).set_body_json(json!({
                "exitCode": 0,
                "stdout": "hello from the guest\n",
                "stderr": ""
            })),
        );
        agent.mount(
            "POST",
            "/upload",
            ResponseTemplate::new(200).set_body_json(json!({ "success": true })),
        );
        let app = TestApp::start();
        app.add_vm(VM, agent.addr());
        Fixture { agent, app }
    }

    fn endpoint(&self) -> String {
        self.agent.addr().to_string()
    }
}

/// Run `args` on both paths and require the same output. Returns the stdout.
fn same_on_both_paths(fixture: &Fixture, args: &[&str]) -> String {
    let before = fixture.agent.request_count();
    let direct = fixture.app.cli_direct(args);
    let after_direct = fixture.agent.request_count();
    let routed = fixture.app.cli(args);
    let after_routed = fixture.agent.request_count();

    assert_eq!(direct.code(), 0, "direct {args:?}: {}", direct.err());
    assert_eq!(routed.code(), 0, "routed {args:?}: {}", routed.err());
    assert_eq!(routed.out(), direct.out(), "stdout of {args:?}");
    assert_eq!(routed.err(), direct.err(), "stderr of {args:?}");
    assert!(
        !direct.out().is_empty(),
        "{args:?} printed nothing, so the comparison is empty"
    );
    // Both runs reached the guest, so the app relayed the routed one.
    assert!(
        after_direct > before,
        "the direct run did not reach the fake agent"
    );
    assert!(
        after_routed > after_direct,
        "the routed run did not reach the fake agent"
    );
    routed.out()
}

#[test]
fn agent_snapshot_through_the_app_prints_the_direct_output() {
    let _lock = serial();
    let fixture = Fixture::new();
    let text = same_on_both_paths(&fixture, &["agent", "snapshot", "--vm", VM]);
    assert!(text.contains("Fake Window"), "{text}");
    same_on_both_paths(&fixture, &["agent", "snapshot", "--vm", VM, "--json"]);
    same_on_both_paths(&fixture, &["agent", "health", "--vm", VM]);
}

#[test]
fn a_routed_vm_command_never_reads_the_spec_so_it_cannot_dial() {
    let _lock = serial();
    let fixture = Fixture::new();
    let direct = fixture.app.cli_direct(&["agent", "health", "--vm", VM]);
    assert_eq!(direct.code(), 0, "{}", direct.err());
    let routed = fixture.app.cli(&["agent", "health", "--vm", VM]);
    assert_eq!(routed.code(), 0, "{}", routed.err());

    // The agent sees what the client calls it. A direct client reads the spec and names the
    // address it dialed. A routed client knows the id only, so it names the agent by that. A
    // routed run that dialed the guest itself would have to read the spec first, and it would
    // show the address.
    let hosts: Vec<_> = fixture
        .agent
        .received()
        .into_iter()
        .map(|r| r.host.unwrap_or_default())
        .collect();
    assert_eq!(
        hosts,
        [fixture.endpoint(), format!("{VM}:8648")],
        "the Host header of the direct run, then of the routed run"
    );
}

#[test]
fn agent_exec_through_the_app_prints_the_direct_output() {
    let _lock = serial();
    let fixture = Fixture::new();
    let text = same_on_both_paths(&fixture, &["exec", "echo hello", "--vm", VM]);
    assert_eq!(text, "hello from the guest\n");
    same_on_both_paths(&fixture, &["exec", "echo hello", "--vm", VM, "--json"]);
}

#[test]
fn file_upload_and_download_through_the_app_print_the_direct_output() {
    let _lock = serial();
    let fixture = Fixture::new();
    let local = fixture.app.root().join("up.bin");
    std::fs::write(&local, pattern(1, 100_000)).unwrap();
    let local = local.to_str().unwrap();
    let text = same_on_both_paths(
        &fixture,
        &["file", "upload", local, "/tmp/up.bin", "--vm", VM],
    );
    assert!(text.starts_with("Uploaded"), "{text}");
    same_on_both_paths(
        &fixture,
        &["file", "upload", local, "/tmp/up.bin", "--vm", VM, "--json"],
    );

    fixture.agent.mount(
        "POST",
        "/download",
        ResponseTemplate::new(200).set_body_bytes(pattern(2, 70_000)),
    );
    let saved = fixture.app.root().join("down.bin");
    let saved = saved.to_str().unwrap();
    let text = same_on_both_paths(
        &fixture,
        &["file", "download", "/tmp/down.bin", saved, "--vm", VM],
    );
    assert!(text.starts_with("Downloaded"), "{text}");
    assert_eq!(std::fs::read(saved).unwrap(), pattern(2, 70_000));
}

#[test]
fn a_large_transfer_passes_through_the_app_unchanged() {
    const UP: usize = 33 * 1024 * 1024;
    const DOWN: usize = 40 * 1024 * 1024;
    let _lock = serial();
    let fixture = Fixture::new();

    let local = fixture.app.root().join("big-up.bin");
    std::fs::write(&local, pattern(3, UP)).unwrap();
    let up = fixture.app.cli(&[
        "file",
        "upload",
        local.to_str().unwrap(),
        "/tmp/big.bin",
        "--vm",
        VM,
        "--json",
    ]);
    assert_eq!(up.code(), 0, "{}", up.err());
    assert_eq!(up.json()["bytes"], UP);
    let received = fixture.agent.received();
    let upload = received
        .iter()
        .find(|r| r.path == "/upload")
        .expect("an upload arrived");
    assert_eq!(upload.body.len(), UP);
    assert!(
        upload.body == pattern(3, UP),
        "the uploaded bytes differ from the source"
    );

    fixture.agent.mount(
        "POST",
        "/download",
        ResponseTemplate::new(200).set_body_bytes(pattern(4, DOWN)),
    );
    let saved = fixture.app.root().join("big-down.bin");
    let down = fixture.app.cli(&[
        "file",
        "download",
        "/tmp/big.bin",
        saved.to_str().unwrap(),
        "--vm",
        VM,
        "--json",
    ]);
    assert_eq!(down.code(), 0, "{}", down.err());
    assert_eq!(down.json()["bytes"], DOWN);
    assert!(
        std::fs::read(&saved).unwrap() == pattern(4, DOWN),
        "the downloaded bytes differ from the guest's"
    );
}

#[test]
fn an_explicit_agent_endpoint_goes_through_the_app() {
    let _lock = serial();
    let fixture = Fixture::new();
    // No VM spec is involved: the app dials the endpoint that the request names.
    let endpoint = fixture.endpoint();
    let text = same_on_both_paths(&fixture, &["agent", "snapshot", "--agent", &endpoint]);
    assert!(text.contains("Fake Window"), "{text}");
    // The default port is resolved by the CLI, because the app requires `host:port`. The fake
    // is not on 8648, so the request must fail at the guest, not at the app's grammar.
    let default_port = fixture
        .app
        .cli(&["agent", "health", "--agent", "127.0.0.1", "--json"]);
    assert_eq!(
        default_port.json()["code"],
        "CONNECTION_REFUSED",
        "{}",
        default_port.out()
    );
}

#[test]
fn a_connect_spec_goes_through_the_app_as_an_endpoint() {
    let _lock = serial();
    let fixture = Fixture::new();
    let spec = fixture.app.root().join("spec.json");
    let addr = fixture.agent.addr();
    std::fs::write(
        &spec,
        json!({ "agent": { "host": addr.ip().to_string(), "port": addr.port() } }).to_string(),
    )
    .unwrap();
    same_on_both_paths(
        &fixture,
        &["agent", "health", "--connect", spec.to_str().unwrap()],
    );
}

#[test]
fn the_vm_id_from_the_environment_goes_through_the_app() {
    let _lock = serial();
    let fixture = Fixture::new();
    let out = fixture.app.cli_with(
        &["agent", "health"],
        &[
            (
                "TESTANYWARE_APP_SOCKET",
                fixture.app.socket().to_str().unwrap(),
            ),
            ("TESTANYWARE_VM_ID", VM),
        ],
    );
    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(fixture.agent.request_count(), 1);
}

#[test]
fn without_the_variable_the_cli_dials_the_guest_itself() {
    let _lock = serial();
    // The app does not run. An empty variable counts as unset, so the command still works.
    let agent = FakeAgent::start();
    agent.mount(
        "GET",
        "/health",
        ResponseTemplate::new(200)
            .set_body_json(json!({ "accessible": true, "platform": "macos" })),
    );
    let app = TestApp::not_running();
    let endpoint = agent.addr().to_string();
    for extra in [vec![], vec![("TESTANYWARE_APP_SOCKET", "")]] {
        let out = app.cli_with(&["agent", "health", "--agent", &endpoint], &extra);
        assert_eq!(out.code(), 0, "{}", out.err());
        assert_eq!(out.out(), "OK\n");
    }
    assert_eq!(agent.request_count(), 2);
}

#[test]
fn a_missing_app_fails_with_not_running_and_dials_nothing() {
    let _lock = serial();
    let agent = FakeAgent::start();
    agent.mount(
        "GET",
        "/health",
        ResponseTemplate::new(200)
            .set_body_json(json!({ "accessible": true, "platform": "macos" })),
    );
    let app = TestApp::not_running();
    let endpoint = agent.addr().to_string();
    let args = ["agent", "health", "--agent", &endpoint, "--json"];

    // The guest answers a direct dial, so a CLI that fell back to it would succeed.
    let direct = app.cli_direct(&args);
    assert_eq!(direct.code(), 0, "positive control: {}", direct.err());
    assert_eq!(agent.request_count(), 1);

    let routed = app.cli(&args);
    assert_eq!(routed.code(), 1, "{}", routed.out());
    let envelope = routed.json();
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["code"], "APP_UNREACHABLE");
    assert_eq!(envelope["details"]["cause"], "not_running");
    assert!(envelope["remediation"]
        .as_str()
        .unwrap()
        .contains("never starts"));
    assert_eq!(
        agent.request_count(),
        1,
        "the CLI dialed the guest after it lost the app"
    );

    // The text mode names the cause and the remedy, and the exit code is the same.
    let text = app.cli(&["agent", "health", "--agent", &endpoint]);
    assert_eq!(text.code(), 1);
    assert!(text.err().contains("TestAnyware app"), "{}", text.err());
    assert!(text.err().contains("hint:"), "{}", text.err());
}

#[test]
fn a_stale_socket_file_is_not_running_too() {
    let _lock = serial();
    let app = TestApp::not_running();
    drop(UnixListener::bind(app.socket()).unwrap());
    let out = app.cli(&["agent", "health", "--vm", VM, "--json"]);
    assert_eq!(out.code(), 1);
    assert_eq!(out.json()["details"]["cause"], "not_running");
}

#[test]
fn an_unsafe_socket_directory_is_refused_before_the_cli_connects() {
    let _lock = serial();
    let app = TestApp::not_running();
    std::fs::set_permissions(app.socket_dir(), std::fs::Permissions::from_mode(0o755)).unwrap();
    // A live listener shows that the refusal comes from the directory check.
    let listener = UnixListener::bind(app.socket()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let out = app.cli(&["agent", "health", "--vm", VM, "--json"]);
    assert_eq!(out.code(), 1, "{}", out.out());
    let envelope = out.json();
    assert_eq!(envelope["code"], "APP_UNREACHABLE");
    assert_eq!(envelope["details"]["cause"], "unsafe_socket_directory");
    assert!(
        listener.accept().is_err(),
        "the CLI connected to a socket in an unsafe directory"
    );
}

#[test]
fn a_socket_path_of_more_than_103_bytes_is_refused() {
    let _lock = serial();
    let app = TestApp::not_running();
    let long = format!("/tmp/{}/testanyware.sock", "x".repeat(110));
    let out = app.cli_with(
        &["agent", "health", "--vm", VM, "--json"],
        &[("TESTANYWARE_APP_SOCKET", &long)],
    );
    assert_eq!(out.code(), 1);
    let envelope = out.json();
    assert_eq!(envelope["code"], "APP_UNREACHABLE");
    assert_eq!(envelope["details"]["cause"], "socket_path_too_long");
}

#[test]
fn errors_of_the_app_reach_the_envelope_with_the_exit_code_of_the_code() {
    let _lock = serial();
    let fixture = Fixture::new();

    // VM_NOT_FOUND exits 3 on both paths.
    let args = ["agent", "health", "--vm", "testanyware-absent", "--json"];
    let direct = fixture.app.cli_direct(&args);
    let routed = fixture.app.cli(&args);
    assert_eq!(direct.code(), 3);
    assert_eq!(routed.code(), 3, "{}", routed.out());
    assert_eq!(routed.json()["code"], "VM_NOT_FOUND");
    assert_eq!(routed.json()["ok"], false);
    assert!(routed.json()["remediation"].is_string());

    // An id that is not a plain id gets the same answer without a request.
    let odd = fixture
        .app
        .cli(&["agent", "health", "--vm", "../etc", "--json"]);
    assert_eq!(odd.code(), 3);
    assert_eq!(odd.json()["code"], "VM_NOT_FOUND");

    // A guest that refuses the dial: CONNECTION_REFUSED, with the errno from the app's dial.
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let closed_addr = closed.local_addr().unwrap();
    drop(closed);
    fixture.app.add_vm("testanyware-closed", closed_addr);
    let args = ["agent", "health", "--vm", "testanyware-closed", "--json"];
    let direct = fixture.app.cli_direct(&args);
    let routed = fixture.app.cli(&args);
    assert_eq!(direct.code(), 1);
    assert_eq!(routed.code(), 1, "{}", routed.out());
    assert_eq!(direct.json()["code"], "CONNECTION_REFUSED");
    assert_eq!(routed.json()["code"], "CONNECTION_REFUSED");
    assert!(
        routed.json()["details"]["errno"].is_number(),
        "{}",
        routed.out()
    );

    // A spec with no agent section: NO_CONNECTION_SPECIFIED, exit 2.
    std::fs::write(
        fixture
            .app
            .state_dir()
            .join("vms")
            .join("testanyware-noagent.json"),
        json!({ "vnc": { "host": "127.0.0.1", "port": 1 }, "platform": "macos" }).to_string(),
    )
    .unwrap();
    let args = ["agent", "health", "--vm", "testanyware-noagent", "--json"];
    assert_eq!(fixture.app.cli_direct(&args).code(), 2);
    let routed = fixture.app.cli(&args);
    assert_eq!(routed.code(), 2, "{}", routed.out());
    assert_eq!(routed.json()["code"], "NO_CONNECTION_SPECIFIED");
}

#[test]
fn other_directories_fail_with_app_incompatible_and_dial_nothing() {
    let _lock = serial();
    let fixture = Fixture::new();
    let other = fixture.app.root().join("other-state");
    let out = fixture.app.cli_with(
        &["agent", "health", "--vm", VM, "--json"],
        &[
            (
                "TESTANYWARE_APP_SOCKET",
                fixture.app.socket().to_str().unwrap(),
            ),
            ("XDG_STATE_HOME", other.to_str().unwrap()),
        ],
    );
    assert_eq!(out.code(), 1, "{}", out.out());
    let envelope = out.json();
    assert_eq!(envelope["code"], "APP_INCOMPATIBLE");
    assert_eq!(envelope["details"]["cause"], "directories");
    assert!(envelope["remediation"]
        .as_str()
        .unwrap()
        .contains("XDG_STATE_HOME"));
    assert_eq!(fixture.agent.request_count(), 0);
}
