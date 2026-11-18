//! `testanyware exec` reads the exec record (spec `docs/specs/guest-command-lifecycle.md`,
//! sections "Host CLI JSON mode", "Host CLI text mode" and "Compatibility and version skew").
//!
//! Each test runs the real binary through the test app against a wiremock agent. The agent
//! replays a recorded reply:
//!
//! - New Linux replies: `agents/linux/tests/fixtures/exec-records` (leaf k217). The agent copies
//!   the exec id of the request into the reply, as the real agent does.
//! - Legacy Linux replies and the output of the old Host CLI:
//!   `agents/linux/tests/fixtures/legacy_linux_replies.json`.
//! - Legacy macOS replies: `cli-rs/tests/fixtures/exec-replies/legacy-macos.json`.
//! - Windows replies, built from the source: `cli-rs/tests/fixtures/exec-replies/windows.json`.

#![cfg(unix)]

mod common;

use std::path::PathBuf;
use std::process::Output;

use common::{serial, FakeAgent, OutputExt, TestApp};
use serde_json::{json, Value};
use wiremock::{Request, ResponseTemplate};

const VM: &str = "testanyware-feed0002";

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn load(rel: &str) -> Value {
    let path = repo().join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

/// The body of the new Linux fixture `case`.
fn linux_record(case: &str) -> Value {
    load(&format!(
        "agents/linux/tests/fixtures/exec-records/{case}.json"
    ))["body"]
        .clone()
}

/// How the fake agent answers `POST /exec`.
#[derive(Clone)]
enum Reply {
    /// A new record. The agent puts the sent id into it.
    Record(Value),
    /// A new record sent exactly as given, whatever id the request carried.
    RecordAsIs(Value),
    /// A legacy body, byte for byte.
    Raw(String),
}

/// A fake agent that answers with `reply`, and an app that serves its VM.
struct Guest {
    agent: FakeAgent,
    app: TestApp,
}

impl Guest {
    fn new(reply: Reply) -> Self {
        let agent = FakeAgent::start();
        agent.mount_responder("POST", "/exec", move |req: &Request| {
            let sent: Value = serde_json::from_slice(&req.body).unwrap();
            match &reply {
                Reply::Record(body) => {
                    let mut body = body.clone();
                    body["id"] = sent["id"].clone();
                    ResponseTemplate::new(200).set_body_json(body)
                }
                Reply::RecordAsIs(body) => ResponseTemplate::new(200).set_body_json(body.clone()),
                Reply::Raw(text) => ResponseTemplate::new(200)
                    .set_body_raw(text.clone().into_bytes(), "application/json"),
            }
        });
        let app = TestApp::start();
        app.add_vm(VM, agent.addr());
        Guest { agent, app }
    }

    fn exec(&self, extra: &[&str]) -> Output {
        let mut args = vec!["exec", "printf guest", "--vm", VM];
        args.extend_from_slice(extra);
        self.app.cli(&args)
    }

    /// The bodies of the `POST /exec` requests, oldest first.
    fn sent(&self) -> Vec<Value> {
        self.agent
            .received()
            .into_iter()
            .filter(|r| r.path == "/exec")
            .map(|r| serde_json::from_slice(&r.body).unwrap())
            .collect()
    }
}

/// The JSON envelope of a run that succeeded.
fn success(run: &Output) -> Value {
    assert_eq!(
        run.code(),
        0,
        "stdout: {}\nstderr: {}",
        run.out(),
        run.err()
    );
    assert_eq!(run.err(), "");
    let env = run.json();
    assert_eq!(env["ok"], true);
    assert_eq!(env["schema_version"], "1.0");
    env
}

// ---- new records ----

#[test]
fn a_new_record_adds_the_record_fields() {
    let _lock = serial();
    let body = linux_record("exit-23-partial");
    let guest = Guest::new(Reply::Record(body.clone()));
    let env = success(&guest.exec(&["--json"]));

    let sent = guest.sent();
    let id = sent[0]["id"]
        .as_str()
        .expect("the Host CLI sends an exec id");
    assert_eq!(id.len(), 36, "a UUID: {id}");
    assert_eq!(env["exec_id"], id);
    assert_eq!(env["record"], "agent");
    assert_eq!(env["legacy_projection"], "linux");
    // The guest outcome is in `outcome`. It never makes `ok` false.
    assert_eq!(env["outcome"], json!({ "kind": "exited", "exit_code": 23 }));
    assert_eq!(env["exit_code"], 23);
    assert_eq!(env["stdout"], "partial out");
    assert_eq!(env["stderr"], "partial err");
    assert_eq!(env["timed_out"], false);
    assert_eq!(env["remaining_group"], body["remainingGroup"]);
    assert_eq!(env["termination"], Value::Null);

    let out = &env["evidence"]["stdout"];
    let wire = &body["evidence"]["stdout"];
    assert_eq!(out["base64"], wire["base64"]);
    assert_eq!(out["bytes"], wire["bytes"]);
    assert_eq!(out["file_bytes"], wire["fileBytes"]);
    assert_eq!(out["sha256"], wire["sha256"]);
    assert_eq!(out["capture"], "complete");
    assert_eq!(out["reason"], Value::Null);

    let life = &env["lifecycle"];
    let wire = &body["lifecycle"];
    for (snake, camel) in [
        ("leader_pid", "leaderPid"),
        ("group_id", "groupId"),
        ("leader_start_time", "leaderStartTime"),
        ("spawn_ms", "spawnMs"),
        ("duration_ms", "durationMs"),
        ("owner_lateness_ms", "ownerLatenessMs"),
        ("finalised_after_ms", "finalisedAfterMs"),
    ] {
        assert_eq!(life[snake], wire[camel], "lifecycle.{snake}");
    }
}

#[test]
fn a_signalled_command_is_a_success_with_its_outcome() {
    let _lock = serial();
    let guest = Guest::new(Reply::Record(linux_record("signal-kill")));
    let env = success(&guest.exec(&["--json"]));
    assert_eq!(
        env["outcome"],
        json!({ "kind": "signalled", "signal": 9, "signal_name": "SIGKILL" })
    );
    assert_eq!(env["exit_code"], -9);
}

#[test]
fn a_timed_out_command_carries_its_termination() {
    let _lock = serial();
    let body = linux_record("deadline-term-ignored");
    let guest = Guest::new(Reply::Record(body.clone()));
    let env = success(&guest.exec(&["--json"]));
    assert_eq!(env["outcome"]["kind"], "timed_out");
    assert_eq!(
        env["outcome"]["timeout_seconds"],
        body["outcome"]["timeoutSeconds"]
    );
    assert_eq!(env["remaining_group"], Value::Null);
    let t = &env["termination"];
    let wire = &body["termination"];
    for (snake, camel) in [
        ("term_sent_after_ms", "termSentAfterMs"),
        ("kill_sent_after_ms", "killSentAfterMs"),
        ("leader_exit_after_ms", "leaderExitAfterMs"),
        ("group_empty", "groupEmpty"),
        ("group_empty_after_ms", "groupEmptyAfterMs"),
        ("within_bounds", "withinBounds"),
    ] {
        assert_eq!(t[snake], wire[camel], "termination.{snake}");
    }
    assert_eq!(t["leader_status"]["kind"], wire["leaderStatus"]["kind"]);
}

#[test]
fn a_spawn_failure_is_a_success_with_its_outcome() {
    let _lock = serial();
    let guest = Guest::new(Reply::Record(linux_record("spawn-failure")));
    let env = success(&guest.exec(&["--json"]));
    assert_eq!(env["outcome"]["kind"], "spawn_failed");
    assert_eq!(env["outcome"]["error_name"], "EAGAIN");
    assert_eq!(env["lifecycle"]["leader_pid"], Value::Null);
}

/// With a new Linux agent, text mode prints what the old Host CLI printed with the old agent.
#[test]
fn text_mode_with_a_new_agent_matches_the_old_host_cli() {
    let _lock = serial();
    let legacy = load("agents/linux/tests/fixtures/legacy_linux_replies.json");
    let dir = repo().join("agents/linux/tests/fixtures/exec-records");
    let mut checked = 0;
    for case in legacy["cases"].as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        if !dir.join(format!("{name}.json")).exists() {
            continue;
        }
        let guest = Guest::new(Reply::Record(linux_record(name)));
        let run = guest.exec(&[]);
        let text = &case["cliText"];
        let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(
            i64::from(run.code()),
            text["returncode"].as_i64().unwrap(),
            "{name} status"
        );
        assert_eq!(
            hex(&run.stdout),
            text["stdoutHex"].as_str().unwrap(),
            "{name} stdout"
        );
        assert_eq!(
            hex(&run.stderr),
            text["stderrHex"].as_str().unwrap(),
            "{name} stderr"
        );

        // The kept JSON fields keep the values of the old Host CLI.
        let env = success(&guest.exec(&["--json"]));
        let old = &case["cliJson"]["stdout"];
        for key in ["exit_code", "stdout", "stderr", "timed_out"] {
            assert_eq!(env[key], old[key], "{name} {key}");
        }
        checked += 1;
    }
    assert!(checked >= 8, "only {checked} cases match a new fixture");
}

// ---- invalid records ----

fn assert_record_invalid(guest: &Guest) {
    let run = guest.exec(&["--json"]);
    assert_eq!(
        run.code(),
        1,
        "stdout: {}\nstderr: {}",
        run.out(),
        run.err()
    );
    let env = run.json();
    assert_eq!(env["ok"], false);
    assert_eq!(env["code"], "EXEC_RECORD_INVALID");
    assert!(
        env["details"]["problems"]
            .as_array()
            .is_some_and(|p| !p.is_empty()),
        "{env}"
    );

    let run = guest.exec(&[]);
    assert_eq!(run.code(), 1, "text mode");
    assert_eq!(run.out(), "", "text mode prints no guest output");
    assert!(!run.err().is_empty(), "text mode names the error on stderr");
}

#[test]
fn a_contradictory_record_is_an_error() {
    let _lock = serial();
    let mut body = linux_record("oracle-text");
    body["evidence"]["stdout"]["sha256"] = json!("0".repeat(64));
    assert_record_invalid(&Guest::new(Reply::Record(body)));
}

/// Review k231, findings R1 and R2: the reader rejects these records, so the
/// Host CLI reports the error and never reports a success.
#[test]
fn a_record_that_hides_its_evidence_is_an_error() {
    let _lock = serial();
    let mut body = linux_record("deadline-term-ignored");
    body["lifecycle"]["leaderPid"] = Value::Null;
    body["termination"] = Value::Null;
    assert_record_invalid(&Guest::new(Reply::Record(body)));

    let mut body = linux_record("oracle-text");
    body["evidence"]["stderr"]["reason"] = json!("deadline");
    assert_record_invalid(&Guest::new(Reply::Record(body)));
}

#[test]
fn a_record_for_another_id_is_an_error() {
    let _lock = serial();
    assert_record_invalid(&Guest::new(Reply::RecordAsIs(linux_record("requested-id"))));
}

// ---- legacy agents ----

fn assert_legacy(guest: &Guest, body: &Value) {
    let env = success(&guest.exec(&["--json"]));
    let sent = guest.sent();
    assert_eq!(env["exec_id"], sent.last().unwrap()["id"]);
    assert_eq!(env["record"], "legacy");
    assert_eq!(
        env["outcome"],
        json!({ "kind": "unknown", "reason": "legacy_agent" })
    );
    for key in [
        "legacy_projection",
        "evidence",
        "lifecycle",
        "termination",
        "remaining_group",
    ] {
        assert_eq!(env[key], Value::Null, "{key}");
    }
    assert_eq!(env["exit_code"], body["exitCode"]);
    assert_eq!(env["stdout"], body["stdout"]);
    assert_eq!(env["stderr"], body["stderr"]);
    assert_eq!(
        env["timed_out"],
        body.get("timedOut").cloned().unwrap_or(json!(false))
    );
}

#[test]
fn legacy_linux_replies_give_an_unknown_outcome() {
    let _lock = serial();
    let legacy = load("agents/linux/tests/fixtures/legacy_linux_replies.json");
    for case in legacy["cases"].as_array().unwrap() {
        let name = case["case"].as_str().unwrap();
        if !["oracle-text", "signal-kill", "deadline"].contains(&name) {
            continue;
        }
        let body = &case["body"];
        let guest = Guest::new(Reply::Raw(body.to_string()));
        assert_legacy(&guest, body);

        // Text mode prints what the old Host CLI printed.
        let run = guest.exec(&[]);
        let text = &case["cliText"];
        assert_eq!(
            i64::from(run.code()),
            text["returncode"].as_i64().unwrap(),
            "{name}"
        );
    }
}

#[test]
fn legacy_macos_and_windows_replies_give_an_unknown_outcome() {
    let _lock = serial();
    for file in ["legacy-macos.json", "windows.json"] {
        let f = load(&format!("cli-rs/tests/fixtures/exec-replies/{file}"));
        for case in f["cases"].as_array().unwrap() {
            if case.get("httpStatus").is_some_and(|s| s != 200) {
                continue;
            }
            let text = case["bodyText"].as_str().unwrap();
            let body: Value = serde_json::from_str(text).unwrap();
            let guest = Guest::new(Reply::Raw(text.to_string()));
            assert_legacy(&guest, &body);
        }
    }
}

// ---- a detached command ----

#[test]
fn a_detached_command_has_no_record() {
    let _lock = serial();
    let guest = Guest::new(Reply::Raw(
        r#"{"exitCode":0,"stdout":"","stderr":""}"#.into(),
    ));
    let env = success(&guest.exec(&["--detach", "--json"]));
    assert!(
        guest.sent()[0].get("id").is_none(),
        "a detached request carries no id"
    );
    assert_eq!(
        env,
        json!({
            "schema_version": "1.0",
            "ok": true,
            "command": "printf guest",
            "exit_code": 0,
            "stdout": "",
            "stderr": "",
            "timed_out": false
        })
    );
}
