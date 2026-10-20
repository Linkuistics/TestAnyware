//! The agent commands routed through the TestAnyware app (spec: "The protocol").
//!
//! Each test runs the real `testanyware` binary against the real `testanyware-app` executable
//! in its test mode, with a wiremock In-VM agent on loopback as the guest. A macOS host routes
//! each command through the app, by `TESTANYWARE_APP_SOCKET` or by the default socket. A
//! command through the app must print what the same command printed on the direct path, and it
//! must dial nothing itself.
//!
//! A macOS host has no direct path any more, so the output that it printed is frozen in this
//! file as the oracle. A Linux or Windows host keeps the direct path, and there each routed run
//! is compared with a direct run too.

#![cfg(unix)]

mod common;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

use common::{pattern, serial, FakeAgent, OutputExt, TestApp, HOST_GOES_DIRECT};
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

/// `agent snapshot --vm` as the direct path printed it.
const SNAPSHOT_TEXT: &str = "\"Fake Window\" (window) 800x600 [focused] app:\"FakeApp\"\n";

/// A `--json` output: sorted keys, two-space indent and a final newline.
fn pretty(value: serde_json::Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
}

fn snapshot_json() -> String {
    pretty(json!({
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
    }))
}

/// Run `args` through the app and require `expected` on stdout and nothing on stderr. The
/// guest must receive a request, so the app relayed it.
///
/// `expected` is the output that the direct path printed. Where a host still has the direct
/// path, a direct run must print it too.
fn routed_prints(fixture: &Fixture, args: &[&str], expected: &str) {
    let before = fixture.agent.request_count();
    let routed = fixture.app.cli(args);
    let after_routed = fixture.agent.request_count();
    assert_eq!(routed.code(), 0, "routed {args:?}: {}", routed.err());
    assert_eq!(routed.out(), expected, "stdout of {args:?}");
    assert_eq!(routed.err(), "", "stderr of {args:?}");
    assert!(
        after_routed > before,
        "the routed run did not reach the fake agent"
    );

    if HOST_GOES_DIRECT {
        let direct = fixture.app.cli_default(args);
        assert_eq!(direct.code(), 0, "direct {args:?}: {}", direct.err());
        assert_eq!(direct.out(), expected, "direct stdout of {args:?}");
        assert_eq!(direct.err(), "", "direct stderr of {args:?}");
        assert!(
            fixture.agent.request_count() > after_routed,
            "the direct run did not reach the fake agent"
        );
    }
}

/// One request that the test itself sends to a guest, as a command that dials it would.
fn dial_guest(addr: SocketAddr) -> String {
    let mut stream = TcpStream::connect(addr).expect("the guest accepts a dial");
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    reply
}

#[test]
fn agent_snapshot_through_the_app_prints_the_direct_output() {
    let _lock = serial();
    let fixture = Fixture::new();
    routed_prints(&fixture, &["agent", "snapshot", "--vm", VM], SNAPSHOT_TEXT);
    routed_prints(
        &fixture,
        &["agent", "snapshot", "--vm", VM, "--json"],
        &snapshot_json(),
    );
    routed_prints(&fixture, &["agent", "health", "--vm", VM], "OK\n");
}

#[test]
fn a_routed_vm_command_never_reads_the_spec_so_it_cannot_dial() {
    let _lock = serial();
    let fixture = Fixture::new();
    if HOST_GOES_DIRECT {
        let direct = fixture.app.cli_default(&["agent", "health", "--vm", VM]);
        assert_eq!(direct.code(), 0, "{}", direct.err());
    }
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
    let mut expected = vec![];
    if HOST_GOES_DIRECT {
        expected.push(fixture.endpoint());
    }
    expected.push(format!("{VM}:8648"));
    assert_eq!(hosts, expected, "the Host header of each run");
}

#[test]
fn agent_exec_through_the_app_prints_the_direct_output() {
    let _lock = serial();
    let fixture = Fixture::new();
    routed_prints(
        &fixture,
        &["exec", "echo hello", "--vm", VM],
        "hello from the guest\n",
    );
    routed_prints(
        &fixture,
        &["exec", "echo hello", "--vm", VM, "--json"],
        &pretty(json!({
            "command": "echo hello",
            "exit_code": 0,
            "ok": true,
            "schema_version": "1.0",
            "stderr": "",
            "stdout": "hello from the guest\n",
            "timed_out": false
        })),
    );
}

#[test]
fn file_upload_and_download_through_the_app_print_the_direct_output() {
    let _lock = serial();
    let fixture = Fixture::new();
    let local = fixture.app.root().join("up.bin");
    std::fs::write(&local, pattern(1, 100_000)).unwrap();
    let local = local.to_str().unwrap();
    routed_prints(
        &fixture,
        &["file", "upload", local, "/tmp/up.bin", "--vm", VM],
        &format!("Uploaded {local} \u{2192} /tmp/up.bin\n"),
    );
    routed_prints(
        &fixture,
        &["file", "upload", local, "/tmp/up.bin", "--vm", VM, "--json"],
        &pretty(json!({
            "bytes": 100_000,
            "local": local,
            "ok": true,
            "remote": "/tmp/up.bin",
            "schema_version": "1.0"
        })),
    );

    fixture.agent.mount(
        "POST",
        "/download",
        ResponseTemplate::new(200).set_body_bytes(pattern(2, 70_000)),
    );
    let saved = fixture.app.root().join("down.bin");
    let saved = saved.to_str().unwrap();
    routed_prints(
        &fixture,
        &["file", "download", "/tmp/down.bin", saved, "--vm", VM],
        &format!("Downloaded /tmp/down.bin \u{2192} {saved}\n"),
    );
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
    routed_prints(
        &fixture,
        &["agent", "snapshot", "--agent", &endpoint],
        SNAPSHOT_TEXT,
    );
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
    routed_prints(
        &fixture,
        &["agent", "health", "--connect", spec.to_str().unwrap()],
        "OK\n",
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

/// A fake agent that answers `/health`, as the guest of the tests of the default.
fn healthy_agent() -> FakeAgent {
    let agent = FakeAgent::start();
    agent.mount(
        "GET",
        "/health",
        ResponseTemplate::new(200)
            .set_body_json(json!({ "accessible": true, "platform": "macos" })),
    );
    agent
}

/// On a Linux or Windows host, a command without the variable dials the guest itself, as it
/// did before the app (spec: "Linux and Windows hosts keep the direct path"). No app runs.
#[cfg(not(target_os = "macos"))]
#[test]
fn a_linux_or_windows_host_without_the_variable_dials_the_guest_itself() {
    let _lock = serial();
    let agent = healthy_agent();
    let app = TestApp::not_running();
    let endpoint = agent.addr().to_string();
    // An empty variable counts as unset.
    for extra in [vec![], vec![("TESTANYWARE_APP_SOCKET", "")]] {
        let out = app.cli_with(&["agent", "health", "--agent", &endpoint], &extra);
        assert_eq!(out.code(), 0, "{}", out.err());
        assert_eq!(out.out(), "OK\n");
    }
    assert_eq!(agent.request_count(), 2);
}

/// On a macOS host, a command without the variable goes to the default socket of the app, in
/// Application Support under `HOME`, for each way to name a guest.
#[cfg(target_os = "macos")]
#[test]
fn a_macos_host_without_the_variable_uses_the_default_socket_of_the_app() {
    let _lock = serial();
    let agent = healthy_agent();
    let app = TestApp::start_on_the_default_socket();
    app.add_vm(VM, agent.addr());
    assert!(app
        .socket()
        .ends_with("Library/Application Support/TestAnyware/testanyware.sock"));
    let endpoint = agent.addr().to_string();

    // An empty variable counts as unset.
    for extra in [vec![], vec![("TESTANYWARE_APP_SOCKET", "")]] {
        let out = app.cli_with(&["agent", "health", "--vm", VM], &extra);
        assert_eq!(out.code(), 0, "{}", out.err());
        assert_eq!(out.out(), "OK\n");
    }
    let out = app.cli_default(&["agent", "health", "--agent", &endpoint]);
    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(agent.request_count(), 3);

    // The proof of routing: the Host CLI named the VM by its id, so it read no spec, and
    // the app dialed the guest. A CLI that dialed the guest would have named its address.
    let hosts: Vec<_> = agent
        .received()
        .into_iter()
        .map(|r| r.host.unwrap_or_default())
        .collect();
    assert_eq!(hosts[..2], [format!("{VM}:8648"), format!("{VM}:8648")]);
    assert_eq!(hosts[2], endpoint);
}

#[test]
fn the_variable_overrides_the_default_socket() {
    let _lock = serial();
    let agent = healthy_agent();
    // An app serves on the named socket. Nothing serves on the default one.
    let app = TestApp::start();
    app.add_vm(VM, agent.addr());
    let out = app.cli(&["agent", "health", "--vm", VM]);
    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(out.out(), "OK\n");
    if cfg!(target_os = "macos") {
        let default = app.cli_default(&["agent", "health", "--vm", VM, "--json"]);
        assert_eq!(default.code(), 1, "{}", default.out());
        assert_eq!(
            agent.request_count(),
            1,
            "the default socket reached the guest"
        );
    }
}

/// The error that a macOS host gives when no app answers on the default socket, and the app
/// bundle is where the test puts it.
#[cfg(target_os = "macos")]
fn default_socket_error(app: &TestApp, endpoint: &str) -> serde_json::Value {
    let out = app.cli_default(&["agent", "health", "--agent", endpoint, "--json"]);
    assert_eq!(out.code(), 1, "{}", out.out());
    out.json()
}

/// A macOS host never connects to the guest itself, with or without the app. No command
/// starts the app either (spec: "No client starts the app").
#[cfg(target_os = "macos")]
#[test]
fn a_macos_host_without_an_app_fails_and_dials_nothing() {
    let _lock = serial();
    let agent = healthy_agent();
    let endpoint = agent.addr().to_string();

    // Positive control: this guest answers a dial, so a CLI that fell back to one would
    // succeed.
    assert!(dial_guest(agent.addr()).starts_with("HTTP/1.1 200"));
    assert_eq!(agent.request_count(), 1);

    // No bundle in the user's Applications. The test cannot remove `/Applications`, so the
    // expected code follows what that directory holds on this machine.
    let app = TestApp::not_running_on_the_default_socket();
    let envelope = default_socket_error(&app, &endpoint);
    let installed = std::path::Path::new("/Applications/TestAnyware.app").exists();
    assert_eq!(
        envelope["code"],
        if installed {
            "APP_UNREACHABLE"
        } else {
            "APP_NOT_INSTALLED"
        },
        "{envelope}"
    );
    assert_eq!(envelope["ok"], false);
    assert!(
        !app.socket().exists(),
        "the CLI started or created something on the socket"
    );

    // The app is installed in the user's Applications and does not run: unreachable, with the
    // cause and the remedy of the setup.
    std::fs::create_dir_all(app.root().join("Applications/TestAnyware.app")).unwrap();
    let envelope = default_socket_error(&app, &endpoint);
    assert_eq!(envelope["code"], "APP_UNREACHABLE", "{envelope}");
    assert_eq!(envelope["details"]["cause"], "not_running");
    let remedy = envelope["remediation"].as_str().unwrap();
    assert!(remedy.contains("Start at Login"), "{remedy}");
    assert!(remedy.contains("never starts"), "{remedy}");

    assert_eq!(
        agent.request_count(),
        1,
        "the CLI dialed the guest after it found no app"
    );
}

/// The remedy of a missing app is the setup, whole. The cask does not exist yet, so the text
/// says that the release provides it.
#[cfg(target_os = "macos")]
#[test]
fn a_missing_app_names_the_five_steps_of_the_setup() {
    if std::path::Path::new("/Applications/TestAnyware.app").exists() {
        eprintln!("skipped: this machine has the app in /Applications");
        return;
    }
    let _lock = serial();
    let app = TestApp::not_running_on_the_default_socket();
    let envelope = default_socket_error(&app, "127.0.0.1:1");
    assert_eq!(envelope["code"], "APP_NOT_INSTALLED", "{envelope}");
    assert_eq!(
        envelope["details"]["socket"],
        app.socket().to_str().unwrap()
    );
    let searched = envelope["details"]["searched"].as_array().unwrap();
    assert_eq!(searched[0], "/Applications/TestAnyware.app");
    let remedy = envelope["remediation"].as_str().unwrap();
    for step in [
        "1. Install the cask with `brew install --cask linkuistics/taps/testanyware-app`",
        "The release provides the cask",
        "2. Open TestAnyware from /Applications",
        "3. Answer the \"downloaded from the Internet\" dialog with Open",
        "4. Turn on \"Start at Login\"",
        "5. Allow the Local Network alert",
    ] {
        assert!(remedy.contains(step), "{step} missing from {remedy}");
    }
    // The text mode names the problem and gives the hint, and the exit code is the same.
    let text = app.cli_default(&["agent", "health", "--agent", "127.0.0.1:1"]);
    assert_eq!(text.code(), 1);
    assert!(text.err().contains("is not installed"), "{}", text.err());
    assert!(text.err().contains("hint:"), "{}", text.err());
}

#[test]
fn a_missing_app_fails_with_not_running_and_dials_nothing() {
    let _lock = serial();
    let agent = healthy_agent();
    let app = TestApp::not_running();
    let endpoint = agent.addr().to_string();
    let args = ["agent", "health", "--agent", &endpoint, "--json"];

    // Positive control: the guest answers a dial, so a CLI that fell back to one would succeed.
    assert!(dial_guest(agent.addr()).starts_with("HTTP/1.1 200"));
    assert_eq!(agent.request_count(), 1);

    // The socket is named, so the Host CLI does not judge the install places: the app that
    // it expected there is not running.
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
    let routed = fixture.app.cli(&args);
    if HOST_GOES_DIRECT {
        assert_eq!(fixture.app.cli_default(&args).code(), 3);
    }
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
    let routed = fixture.app.cli(&args);
    if HOST_GOES_DIRECT {
        let direct = fixture.app.cli_default(&args);
        assert_eq!(direct.code(), 1);
        assert_eq!(direct.json()["code"], "CONNECTION_REFUSED");
    }
    assert_eq!(routed.code(), 1, "{}", routed.out());
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
    if HOST_GOES_DIRECT {
        assert_eq!(fixture.app.cli_default(&args).code(), 2);
    }
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
