//! `file cancel`, `--exec-id`, CLI interruption and the download checks of the Host CLI
//! (spec `docs/specs/guest-command-lifecycle.md`, sections "Cancel endpoint", "CLI
//! interruption", "Host CLI download" and "Errors"; ADR-0025 and ADR-0026).
//!
//! Each test runs the real binary through the test app against a loopback agent: wiremock
//! for JSON replies, and a raw TCP agent where a reply must stall or break its framing.
//!
//! The interruption and cleanup cases are also the corrected-pass side of two old-fail
//! controls: the CLI SIGTERM case of `macos-disconnect-availability-k13` and the interrupt
//! case of `download-consistency-k7`. `task cli:control-old-path` runs them against the Host
//! CLI before this change, where each must fail.

#![cfg(unix)]

mod common;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

use common::{serial, FakeAgent, OutputExt, TestApp};
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use serde_json::{json, Value};
use wiremock::{Request, ResponseTemplate};

const VM: &str = "testanyware-feed0222";

/// The spec's bound from SIGTERM to signal death (section "Bounds").
const SIGNAL_DEATH_BOUND: Duration = Duration::from_secs(2);

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The body of the recorded Linux reply `exit-23-partial`, a valid typed record.
fn linux_record() -> Value {
    let path = repo().join("agents/linux/tests/fixtures/exec-records/exit-23-partial.json");
    let fixture: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    fixture["body"].clone()
}

/// The bodies of the requests to `route`, oldest first.
fn bodies(agent: &FakeAgent, route: &str) -> Vec<Value> {
    agent
        .received()
        .into_iter()
        .filter(|r| r.path == route)
        .map(|r| serde_json::from_slice(&r.body).unwrap())
        .collect()
}

/// Poll `done` every 10 ms for `limit` at most.
fn wait_until(limit: Duration, what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < limit, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Wait for `child` to end, for `limit` at most. A child that outlives it is killed, and
/// the test fails.
fn wait_child(child: &mut Child, limit: Duration) -> ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the Host CLI did not end within {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn stderr_of(child: &mut Child) -> String {
    let mut text = String::new();
    if let Some(mut pipe) = child.stderr.take() {
        let _ = pipe.read_to_string(&mut text);
    }
    text
}

/// Send `signal` to `child` and return the status and the time to its end.
fn interrupt(child: &mut Child, signal: Signal) -> (ExitStatus, Duration) {
    let sent = Instant::now();
    kill(Pid::from_raw(child.id() as i32), signal).unwrap();
    let status = wait_child(child, Duration::from_secs(10));
    (status, sent.elapsed())
}

/// The child died of `signal`, as the parent of a Host CLI without a handler sees it.
fn assert_signal_death(status: ExitStatus, signal: Signal, stderr: &str) {
    assert_eq!(
        status.signal(),
        Some(signal as i32),
        "the Host CLI must die of {signal:?}, got {status:?}; stderr: {stderr}"
    );
}

// ---- file cancel ----

/// An agent whose `/exec/cancel` gives `reply`, and an app that serves its VM.
fn cancel_guest(reply: ResponseTemplate) -> (FakeAgent, TestApp) {
    let agent = FakeAgent::start();
    agent.mount("POST", "/exec/cancel", reply);
    let app = TestApp::start();
    app.add_vm(VM, agent.addr());
    (agent, app)
}

#[test]
fn file_cancel_reports_each_plain_state() {
    let _lock = serial();
    for state in ["terminating", "unknown_id"] {
        let (agent, app) =
            cancel_guest(ResponseTemplate::new(200).set_body_json(json!({ "state": state })));

        let run = app.cli(&["file", "cancel", "--vm", VM, "--exec-id", "run-7", "--json"]);
        assert_eq!(run.code(), 0, "stderr: {}", run.err());
        let env = run.json();
        assert_eq!(env["ok"], true);
        assert_eq!(env["exec_id"], "run-7");
        assert_eq!(env["state"], state);
        assert_eq!(env["record"], Value::Null);

        let text = app.cli(&[
            "file",
            "cancel",
            "--vm",
            VM,
            "--exec-id",
            "run-7",
            "--wait",
            "2",
        ]);
        assert_eq!(text.code(), 0, "stderr: {}", text.err());
        assert_eq!(text.out(), format!("{state}\n"));

        assert_eq!(
            bodies(&agent, "/exec/cancel"),
            vec![
                json!({ "id": "run-7", "waitMs": 0 }),
                json!({ "id": "run-7", "waitMs": 2000 })
            ]
        );
    }
}

#[test]
fn file_cancel_reports_the_terminal_record() {
    let _lock = serial();
    let record = linux_record();
    let id = record["id"].as_str().unwrap().to_string();
    let (_agent, app) = cancel_guest(
        ResponseTemplate::new(200).set_body_json(json!({ "state": "terminal", "record": record })),
    );

    let run = app.cli(&["file", "cancel", "--vm", VM, "--exec-id", &id, "--json"]);
    assert_eq!(run.code(), 0, "stderr: {}", run.err());
    let env = run.json();
    assert_eq!(env["state"], "terminal");
    assert_eq!(env["record"]["exec_id"], id.as_str());
    assert_eq!(
        env["record"]["outcome"],
        json!({ "kind": "exited", "exit_code": 23 })
    );
    assert_eq!(env["record"]["exit_code"], 23);
    assert_eq!(env["record"]["legacy_projection"], "linux");

    let text = app.cli(&["file", "cancel", "--vm", VM, "--exec-id", &id]);
    assert_eq!(text.out(), "terminal\noutcome: exited with code 23\n");

    // The record of another id contradicts the cancellation.
    let other = app.cli(&["file", "cancel", "--vm", VM, "--exec-id", "other", "--json"]);
    assert_eq!(other.code(), 1);
    assert_eq!(other.json()["code"], "EXEC_RECORD_INVALID");
}

#[test]
fn file_cancel_on_an_old_agent_reports_that_it_cannot_cancel() {
    let _lock = serial();
    let (_agent, app) =
        cancel_guest(ResponseTemplate::new(404).set_body_json(json!({ "error": "not_found" })));
    let run = app.cli(&["file", "cancel", "--vm", VM, "--exec-id", "run-7", "--json"]);
    assert_eq!(run.code(), 1, "stdout: {}", run.out());
    let env = run.json();
    assert_eq!(env["ok"], false);
    assert_eq!(env["code"], "AGENT_ENDPOINT_UNSUPPORTED");
    assert_eq!(env["details"]["endpoint"], "/exec/cancel");
}

#[test]
fn an_exec_id_that_breaks_the_rule_is_a_usage_error() {
    let _lock = serial();
    let app = TestApp::start();
    for args in [
        vec!["file", "cancel", "--vm", VM, "--exec-id", "has space"],
        vec![
            "file",
            "exec",
            "--vm",
            VM,
            "--exec-id",
            &"x".repeat(65),
            "true",
        ],
        vec![
            "file",
            "exec",
            "--vm",
            VM,
            "--exec-id",
            "run-7",
            "--detach",
            "true",
        ],
        vec![
            "file",
            "cancel",
            "--vm",
            VM,
            "--exec-id",
            "run-7",
            "--wait",
            "11",
        ],
    ] {
        let run = app.cli(&args);
        assert_eq!(run.code(), 2, "{args:?}: stderr {}", run.err());
    }
}

// ---- --exec-id ----

#[test]
fn file_exec_sends_the_given_exec_id() {
    let _lock = serial();
    let agent = FakeAgent::start();
    agent.mount_responder("POST", "/exec", |req: &Request| {
        let sent: Value = serde_json::from_slice(&req.body).unwrap();
        let mut body = linux_record();
        body["id"] = sent["id"].clone();
        ResponseTemplate::new(200).set_body_json(body)
    });
    let app = TestApp::start();
    app.add_vm(VM, agent.addr());

    let run = app.cli(&[
        "exec",
        "--vm",
        VM,
        "--exec-id",
        "run-7.step-2",
        "--json",
        "true",
    ]);
    assert_eq!(run.code(), 0, "stderr: {}", run.err());
    assert_eq!(run.json()["exec_id"], "run-7.step-2");
    assert_eq!(bodies(&agent, "/exec")[0]["id"], "run-7.step-2");
}

#[test]
fn a_duplicate_exec_id_exits_with_exec_id_in_use() {
    let _lock = serial();
    let agent = FakeAgent::start();
    agent.mount(
        "POST",
        "/exec",
        ResponseTemplate::new(409).set_body_json(json!({
            "error": "exec_id_in_use",
            "details": "exec id run-7 names a running or retained command",
        })),
    );
    let app = TestApp::start();
    app.add_vm(VM, agent.addr());

    let run = app.cli(&["exec", "--vm", VM, "--exec-id", "run-7", "--json", "true"]);
    assert_eq!(run.code(), 5, "stdout: {}", run.out());
    let env = run.json();
    assert_eq!(env["code"], "EXEC_ID_IN_USE");
    assert_eq!(env["details"]["wire_error"], "exec_id_in_use");

    let text = app.cli(&["exec", "--vm", VM, "--exec-id", "run-7", "true"]);
    assert_eq!(text.code(), 5);
}

// ---- CLI interruption of file exec ----

/// How the fake agent answers the cancellation of an interrupted exec.
enum CancelReply {
    Terminating,
    OldAgent,
    NoAnswer,
}

/// A guest whose `/exec` does not answer for 20 s, and whose cancellation answers as
/// `reply`.
fn held_exec_guest(reply: CancelReply) -> (FakeAgent, TestApp) {
    let agent = FakeAgent::start();
    agent.mount(
        "POST",
        "/exec",
        ResponseTemplate::new(200)
            .set_body_json(linux_record())
            .set_delay(Duration::from_secs(20)),
    );
    agent.mount(
        "POST",
        "/exec/cancel",
        match reply {
            CancelReply::Terminating => {
                ResponseTemplate::new(200).set_body_json(json!({ "state": "terminating" }))
            }
            CancelReply::OldAgent => {
                ResponseTemplate::new(404).set_body_json(json!({ "error": "not_found" }))
            }
            CancelReply::NoAnswer => ResponseTemplate::new(200)
                .set_body_json(json!({ "state": "terminating" }))
                .set_delay(Duration::from_secs(20)),
        },
    );
    let app = TestApp::start();
    app.add_vm(VM, agent.addr());
    (agent, app)
}

/// Start an attached exec, wait until the agent holds it, and interrupt the Host CLI with
/// `signal`. Returns the status, the time from the signal to the end, the exec id that the
/// Host CLI sent and stderr.
fn interrupt_exec(
    agent: &FakeAgent,
    app: &TestApp,
    signal: Signal,
) -> (ExitStatus, Duration, String, String) {
    let mut child = app.spawn_cli(&["exec", "--vm", VM, "sleep 60"], |_| {});
    wait_until(
        Duration::from_secs(10),
        "the agent to hold the exec",
        || !bodies(agent, "/exec").is_empty(),
    );
    let id = bodies(agent, "/exec")[0]["id"]
        .as_str()
        .expect("the Host CLI sends an exec id")
        .to_string();
    let (status, took) = interrupt(&mut child, signal);
    let stderr = stderr_of(&mut child);
    (status, took, id, stderr)
}

#[test]
fn an_interrupted_exec_sends_one_cancellation_and_dies_of_the_same_signal() {
    let _lock = serial();
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        let (agent, app) = held_exec_guest(CancelReply::Terminating);
        let (status, took, id, stderr) = interrupt_exec(&agent, &app, signal);

        assert_signal_death(status, signal, &stderr);
        assert!(
            took < SIGNAL_DEATH_BOUND,
            "{signal:?}: signal death took {took:?}"
        );
        // Old path: the Host CLI died at once, and the agent got no cancellation (k13).
        assert_eq!(
            bodies(&agent, "/exec/cancel"),
            vec![json!({ "id": id, "waitMs": 0 })],
            "{signal:?}: one cancellation for the exec id, with waitMs 0"
        );
    }
}

#[test]
fn an_interrupted_exec_on_an_old_agent_dies_of_the_signal_without_a_cancellation() {
    let _lock = serial();
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        let (agent, app) = held_exec_guest(CancelReply::OldAgent);
        let (status, took, id, stderr) = interrupt_exec(&agent, &app, signal);

        assert_signal_death(status, signal, &stderr);
        assert!(
            took < SIGNAL_DEATH_BOUND,
            "{signal:?}: signal death took {took:?}"
        );
        // The Host CLI tried once. The old agent answered 404 and cancelled nothing.
        assert_eq!(
            bodies(&agent, "/exec/cancel"),
            vec![json!({ "id": id, "waitMs": 0 })]
        );
    }
}

#[test]
fn an_unanswered_cancellation_waits_1_5_s_at_most() {
    let _lock = serial();
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        let (agent, app) = held_exec_guest(CancelReply::NoAnswer);
        let (status, took, _id, stderr) = interrupt_exec(&agent, &app, signal);

        assert_signal_death(status, signal, &stderr);
        assert!(
            took >= Duration::from_millis(1400),
            "{signal:?}: the Host CLI waited for the reply only {took:?}"
        );
        assert!(
            took < SIGNAL_DEATH_BOUND,
            "{signal:?}: signal death took {took:?}"
        );
        assert_eq!(bodies(&agent, "/exec/cancel").len(), 1);
    }
}

#[test]
fn an_ignored_sigint_stays_ignored() {
    // A background job of a shell inherits an ignored SIGINT. The Host CLI keeps it ignored
    // and sends no cancellation.
    let _lock = serial();
    let agent = FakeAgent::start();
    agent.mount_responder("POST", "/exec", |req: &Request| {
        let sent: Value = serde_json::from_slice(&req.body).unwrap();
        let mut body = linux_record();
        body["id"] = sent["id"].clone();
        ResponseTemplate::new(200)
            .set_body_json(body)
            .set_delay(Duration::from_millis(1500))
    });
    let app = TestApp::start();
    app.add_vm(VM, agent.addr());

    let mut child = app.spawn_cli(&["exec", "--vm", VM, "--json", "true"], |command| {
        // SAFETY: signal(2) is async-signal-safe, and nothing else runs in the child
        // between fork and exec.
        unsafe {
            command.pre_exec(|| {
                nix::sys::signal::signal(Signal::SIGINT, nix::sys::signal::SigHandler::SigIgn)
                    .map(|_| ())
                    .map_err(std::io::Error::from)
            });
        }
    });
    wait_until(
        Duration::from_secs(10),
        "the agent to hold the exec",
        || !bodies(&agent, "/exec").is_empty(),
    );
    kill(Pid::from_raw(child.id() as i32), Signal::SIGINT).unwrap();
    let status = wait_child(&mut child, Duration::from_secs(10));
    assert_eq!(status.code(), Some(0), "stderr: {}", stderr_of(&mut child));
    assert!(bodies(&agent, "/exec/cancel").is_empty());
}

// ---- the download checks ----

/// What the raw agent does after it has written its reply.
#[derive(Clone, Copy)]
enum End {
    Close,
    Hold,
}

/// An agent on loopback that reads each request head, writes `reply` byte for byte, and
/// then closes or holds the connection.
fn raw_agent(reply: Vec<u8>, end: End) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let reply = reply.clone();
            std::thread::spawn(move || {
                let mut seen = Vec::new();
                let mut byte = [0u8; 1];
                while !seen.ends_with(b"\r\n\r\n") {
                    match stream.read(&mut byte) {
                        Ok(1) => seen.push(byte[0]),
                        _ => return,
                    }
                }
                let _ = stream.write_all(&reply);
                if let End::Hold = end {
                    let mut sink = [0u8; 64];
                    while matches!(stream.read(&mut sink), Ok(n) if n > 0) {}
                }
            });
        }
    });
    addr
}

/// The head and the received body of one case of the retained k14 Linux run, joined.
fn recorded_linux_reply(case: &str) -> Vec<u8> {
    let dir = repo().join(format!(
        "experiments/exec-reliability/evidence/linux-download-consistency-k14/local-01/cases/{case}/raw"
    ));
    let record: Value =
        serde_json::from_slice(&std::fs::read(dir.join("record.json")).unwrap()).unwrap();
    let head = record["agent"]["trace"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["kind"] == "headers")
        .unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string();
    let mut bytes = head.into_bytes();
    bytes.extend(std::fs::read(dir.join("received.bin")).unwrap());
    bytes
}

fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A download through the app from an agent that serves `reply`, into an empty directory.
fn download(reply: Vec<u8>) -> (std::process::Output, tempfile::TempDir) {
    let agent = raw_agent(reply, End::Close);
    let app = TestApp::start();
    app.add_vm(VM, agent);
    let dest_dir = tempfile::tempdir().unwrap();
    let dest = dest_dir.path().join("evidence.json");
    let run = app.cli(&[
        "file",
        "download",
        "--vm",
        VM,
        "/tmp/evidence.json",
        dest.to_str().unwrap(),
        "--json",
    ]);
    (run, dest_dir)
}

#[test]
fn a_download_with_bytes_past_its_length_fails_and_leaves_no_file() {
    let _lock = serial();
    // Old path: Host CLI 2.8.0 kept 25 of these 148 bytes and exited 0 (k14).
    let (run, dir) = download(recorded_linux_reply("append-after-fstat"));
    assert_eq!(run.code(), 1, "stdout: {}", run.out());
    assert_eq!(run.json()["code"], "DOWNLOAD_FRAMING_INVALID");
    assert_eq!(names_in(dir.path()), Vec::<String>::new());
}

#[test]
fn a_short_download_body_fails_and_leaves_no_file() {
    let _lock = serial();
    let (run, dir) = download(recorded_linux_reply("truncate-after-fstat"));
    assert_eq!(run.code(), 1, "stdout: {}", run.out());
    assert_eq!(run.json()["code"], "CONNECTION_DROPPED");
    assert_eq!(names_in(dir.path()), Vec::<String>::new());
}

#[test]
fn an_exact_download_body_through_the_raw_agent_succeeds() {
    let _lock = serial();
    let oracle = [0x00u8, 0xff, 0x80, 0x41, 0x0a];
    let mut reply =
        b"HTTP/1.1 200 OK\r\ncontent-type: application/octet-stream\r\ncontent-length: 5\r\n\r\n"
            .to_vec();
    reply.extend_from_slice(&oracle);
    let (run, dir) = download(reply);
    assert_eq!(run.code(), 0, "stderr: {}", run.err());
    assert_eq!(run.json()["bytes"], 5);
    assert_eq!(
        std::fs::read(dir.path().join("evidence.json")).unwrap(),
        oracle
    );
    assert_eq!(names_in(dir.path()), vec!["evidence.json".to_string()]);
}

#[test]
fn an_interrupted_download_removes_its_temp_file_and_dies_of_the_signal() {
    let _lock = serial();
    for signal in [Signal::SIGINT, Signal::SIGTERM] {
        let reply = b"HTTP/1.1 200 OK\r\ncontent-type: application/octet-stream\r\ncontent-length: 1000\r\n\r\npartial".to_vec();
        let agent = raw_agent(reply, End::Hold);
        let app = TestApp::start();
        app.add_vm(VM, agent);
        let dest_dir = tempfile::tempdir().unwrap();
        let dest = dest_dir.path().join("evidence.json");

        let mut child = app.spawn_cli(
            &[
                "file",
                "download",
                "--vm",
                VM,
                "/tmp/evidence.json",
                dest.to_str().unwrap(),
            ],
            |_| {},
        );
        wait_until(Duration::from_secs(10), "the temp file", || {
            !names_in(dest_dir.path()).is_empty()
        });
        let temp = names_in(dest_dir.path());
        assert!(
            temp[0].starts_with(".evidence.json.testanyware-"),
            "{temp:?}"
        );

        let (status, took) = interrupt(&mut child, signal);
        let stderr = stderr_of(&mut child);
        assert_signal_death(status, signal, &stderr);
        assert!(
            took < SIGNAL_DEATH_BOUND,
            "{signal:?}: signal death took {took:?}"
        );
        // Old path: the temp file stayed after SIGTERM during the body (k7).
        assert_eq!(
            names_in(dest_dir.path()),
            Vec::<String>::new(),
            "{signal:?}: the temp file must be removed"
        );
    }
}
