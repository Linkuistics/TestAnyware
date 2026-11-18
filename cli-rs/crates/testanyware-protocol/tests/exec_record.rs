//! The exec record reader against recorded replies (spec
//! `docs/specs/guest-command-lifecycle.md`, sections "Exec record" and
//! "Compatibility and version skew").
//!
//! - New Linux replies: `agents/linux/tests/fixtures/exec-records` (leaf k217).
//! - Legacy Linux replies: `agents/linux/tests/fixtures/legacy_linux_replies.json`.
//! - Legacy macOS replies: `cli-rs/tests/fixtures/exec-replies/legacy-macos.json`.
//! - Windows replies: `cli-rs/tests/fixtures/exec-replies/windows.json`. No
//!   recording exists, so they follow the source of the Windows agent.

use std::path::PathBuf;

use base64::Engine as _;
use serde_json::{json, Value};
use testanyware_protocol::exec_record::{
    legacy_projection, read_exec_reply, sha256_hex, ExecReplyError, LegacyProjection, Outcome,
};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn load(path: PathBuf) -> Value {
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

/// Every new Linux fixture as (case, sent id, body).
fn new_linux() -> Vec<(String, Option<String>, Value)> {
    let dir = repo().join("agents/linux/tests/fixtures/exec-records");
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    names.sort();
    assert!(
        names.len() >= 14,
        "the k217 fixtures are missing from {}",
        dir.display()
    );
    names
        .into_iter()
        .map(|p| {
            let f = load(p);
            let sent = f["request"]["id"].as_str().map(str::to_string);
            (
                f["case"].as_str().unwrap().to_string(),
                sent,
                f["body"].clone(),
            )
        })
        .collect()
}

fn fixture(case: &str) -> Value {
    new_linux()
        .into_iter()
        .find(|(c, _, _)| c == case)
        .unwrap_or_else(|| panic!("no fixture {case}"))
        .2
}

fn read(body: &Value) -> Result<testanyware_protocol::ExecResult, ExecReplyError> {
    let id = body["id"].as_str().map(str::to_string);
    read_exec_reply(body.to_string().as_bytes(), id.as_deref())
}

fn problems(body: &Value) -> Vec<String> {
    match read(body) {
        Err(ExecReplyError::Invalid(p)) => p,
        other => panic!("expected an invalid record, got {other:?}"),
    }
}

fn assert_invalid(body: &Value, needle: &str) {
    let p = problems(body);
    assert!(
        p.iter().any(|m| m.contains(needle)),
        "no problem names {needle:?}: {p:?}"
    );
}

fn set_stream(body: &mut Value, name: &str, data: &[u8]) {
    let s = &mut body["evidence"][name];
    s["base64"] = json!(base64::engine::general_purpose::STANDARD.encode(data));
    s["bytes"] = json!(data.len());
    s["fileBytes"] = json!(data.len());
    s["sha256"] = json!(sha256_hex(data));
}

// ---- recorded replies ----

#[test]
fn every_new_linux_reply_is_valid() {
    for (case, sent, body) in new_linux() {
        let sent = sent.or_else(|| body["id"].as_str().map(str::to_string));
        let result = read_exec_reply(body.to_string().as_bytes(), sent.as_deref())
            .unwrap_or_else(|e| panic!("{case}: {e:?}"));
        assert!(result.outcome.is_some(), "{case}");
        assert_eq!(
            result.legacy_projection,
            Some(LegacyProjection::Linux),
            "{case}"
        );
        assert!(result.lifecycle.is_some(), "{case}");
    }
}

#[test]
fn a_new_reply_with_another_id_than_the_sent_one_is_invalid() {
    let body = fixture("requested-id");
    let err = read_exec_reply(body.to_string().as_bytes(), Some("k218.other")).unwrap_err();
    match err {
        ExecReplyError::Invalid(p) => assert!(p[0].contains("is not the sent id"), "{p:?}"),
        other => panic!("{other:?}"),
    }
}

fn assert_legacy(case: &str, body_text: &str) {
    let result = read_exec_reply(body_text.as_bytes(), Some("k218.sent"))
        .unwrap_or_else(|e| panic!("{case}: {e:?}"));
    let body: Value = serde_json::from_str(body_text).unwrap();
    assert_eq!(
        i64::from(result.exit_code),
        body["exitCode"].as_i64().unwrap(),
        "{case}"
    );
    assert_eq!(result.stdout, body["stdout"].as_str().unwrap(), "{case}");
    assert_eq!(result.stderr, body["stderr"].as_str().unwrap(), "{case}");
    assert_eq!(result.timed_out, body["timedOut"].as_bool(), "{case}");
    assert!(result.outcome.is_none() && result.id.is_none(), "{case}");
    assert!(
        result.evidence.is_none() && result.lifecycle.is_none(),
        "{case}"
    );
    assert!(
        result.termination.is_none() && result.remaining_group.is_none(),
        "{case}"
    );
    assert!(result.legacy_projection.is_none(), "{case}");
}

#[test]
fn legacy_linux_replies_read_as_legacy() {
    let f = load(repo().join("agents/linux/tests/fixtures/legacy_linux_replies.json"));
    let cases = f["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for c in cases {
        if c["httpStatus"] == 200 {
            assert_legacy(c["case"].as_str().unwrap(), &c["body"].to_string());
        }
    }
}

#[test]
fn legacy_macos_replies_read_as_legacy() {
    let f = load(repo().join("cli-rs/tests/fixtures/exec-replies/legacy-macos.json"));
    let mut seen = 0;
    for c in f["cases"].as_array().unwrap() {
        if c["httpStatus"] == 200 {
            assert_legacy(c["case"].as_str().unwrap(), c["bodyText"].as_str().unwrap());
            seen += 1;
        }
    }
    assert_eq!(seen, 7);
}

#[test]
fn windows_replies_read_as_legacy() {
    let f = load(repo().join("cli-rs/tests/fixtures/exec-replies/windows.json"));
    for c in f["cases"].as_array().unwrap() {
        assert_legacy(c["case"].as_str().unwrap(), c["bodyText"].as_str().unwrap());
    }
}

#[test]
fn a_null_outcome_is_a_legacy_reply() {
    let body = r#"{"exitCode":0,"stdout":"","stderr":"","outcome":null}"#;
    assert_legacy("null-outcome", body);
}

#[test]
fn a_body_without_legacy_fields_is_a_decode_error() {
    let err = read_exec_reply(br#"{"error":"exec_failed"}"#, None).unwrap_err();
    assert!(matches!(err, ExecReplyError::Decode(_)), "{err:?}");
}

// ---- each contradiction the spec lists ----

#[test]
fn evidence_contradictions_are_invalid() {
    let base = fixture("oracle-text");

    let mut b = base.clone();
    b["evidence"]["stdout"]["sha256"] = json!("00");
    assert_invalid(&b, "evidence.stdout.sha256");

    let mut b = base.clone();
    b["evidence"]["stdout"]["bytes"] = json!(19);
    assert_invalid(&b, "evidence.stdout.bytes");

    let mut b = base.clone();
    b["evidence"]["stderr"]["base64"] = json!("not base64!");
    assert_invalid(&b, "evidence.stderr.base64 does not decode");

    let mut b = base.clone();
    b["evidence"]["stdout"]["fileBytes"] = json!(21);
    assert_invalid(&b, "complete, but bytes is not fileBytes");

    let mut b = base.clone();
    b["evidence"]["stdout"]["capture"] = json!("incomplete");
    assert_invalid(&b, "incomplete, but bytes is not less than fileBytes");
}

/// Review k231, finding R2: an incomplete stream names `deadline` or
/// `read_error`, and a complete stream names no reason.
#[test]
fn capture_reason_contradictions_are_invalid() {
    let incomplete = || {
        let mut b = fixture("deadline-term-ignored");
        let s = &mut b["evidence"]["stdout"];
        s["capture"] = json!("incomplete");
        s["fileBytes"] = json!(s["bytes"].as_i64().unwrap() + 1);
        b
    };

    let b = incomplete();
    assert_invalid(&b, "evidence.stdout.reason is not deadline or read_error");

    let mut b = incomplete();
    b["evidence"]["stdout"]["reason"] = json!("invented");
    assert_invalid(&b, "evidence.stdout.reason is not deadline or read_error");

    for reason in ["deadline", "read_error"] {
        let mut b = incomplete();
        b["evidence"]["stdout"]["reason"] = json!(reason);
        read(&b).unwrap_or_else(|e| panic!("reason {reason} is valid: {e:?}"));
    }

    let mut b = fixture("oracle-text");
    b["evidence"]["stderr"]["reason"] = json!("deadline");
    assert_invalid(&b, "evidence.stderr is complete, but it has a reason");
}

#[test]
fn an_incomplete_capture_is_valid_and_partial() {
    let mut b = fixture("oracle-text");
    b["evidence"]["stdout"]["capture"] = json!("incomplete");
    b["evidence"]["stdout"]["fileBytes"] = json!(4096);
    b["evidence"]["stdout"]["reason"] = json!("deadline");
    let r = read(&b).expect("an incomplete capture is valid");
    assert_eq!(r.evidence.unwrap().stdout.file_bytes, 4096);
}

#[test]
fn legacy_field_contradictions_are_invalid() {
    let mut b = fixture("exit-23-partial");
    b["exitCode"] = json!(0);
    assert_invalid(&b, "legacy fields");

    let mut b = fixture("exit-23-partial");
    b["stdout"] = json!("partial out\n");
    assert_invalid(&b, "legacy fields");

    let mut b = fixture("exit-23-partial");
    b["timedOut"] = json!(false);
    assert_invalid(&b, "legacy fields");

    let mut b = fixture("signal-kill");
    b["exitCode"] = json!(9);
    assert_invalid(&b, "legacy fields");

    let mut b = fixture("deadline");
    b["stderr"] = json!("Process timed out after 2s");
    assert_invalid(&b, "legacy fields");
}

#[test]
fn the_linux_decoder_message_is_not_compared() {
    let mut b = fixture("binary");
    b["stderr"] = json!("another wording of the codec error");
    read(&b).expect("the decoder text is not compared");
    b["exitCode"] = json!(0);
    assert_invalid(&b, "legacy fields");
}

#[test]
fn shape_contradictions_are_invalid() {
    let mut b = fixture("oracle-text");
    b["legacyProjection"] = json!("windows");
    assert_invalid(&b, "does not decode");

    let mut b = fixture("oracle-text");
    b["legacyProjection"] = Value::Null;
    assert_invalid(&b, "legacyProjection");

    let mut b = fixture("oracle-text");
    b["outcome"] = json!({"kind": "vanished"});
    assert_invalid(&b, "does not decode");

    let mut b = fixture("oracle-text");
    b["lifecycle"]["spawnMs"] = json!(1.5);
    assert_invalid(&b, "does not decode");

    let mut b = fixture("oracle-text");
    b["lifecycle"]["spawnMs"] = json!(-1);
    assert_invalid(&b, "lifecycle.spawnMs is negative");

    let mut b = fixture("oracle-text");
    b["id"] = Value::Null;
    let err = read_exec_reply(b.to_string().as_bytes(), Some("x")).unwrap_err();
    assert!(
        matches!(err, ExecReplyError::Invalid(ref p) if p[0].contains("no id")),
        "{err:?}"
    );

    let mut b = fixture("exit-255");
    b["outcome"]["exitCode"] = json!(256);
    b["exitCode"] = json!(256);
    assert_invalid(&b, "is not 0 to 255");
}

#[test]
fn presence_contradictions_are_invalid() {
    let mut b = fixture("oracle-text");
    b["remainingGroup"] = Value::Null;
    assert_invalid(&b, "remainingGroup");

    let mut b = fixture("deadline");
    b["remainingGroup"] = json!("none");
    assert_invalid(&b, "remainingGroup");

    let mut b = fixture("deadline");
    b["termination"] = Value::Null;
    assert_invalid(&b, "termination is not present");

    let mut b = fixture("oracle-text");
    b["termination"] = fixture("deadline")["termination"].clone();
    assert_invalid(&b, "termination is not present");
}

/// Review k231, finding R1: the outcome decides whether a command started.
/// A null identity cannot remove the termination evidence of a timeout.
#[test]
fn a_started_outcome_requires_its_identity() {
    let mut b = fixture("deadline-term-ignored");
    b["lifecycle"]["leaderPid"] = Value::Null;
    b["termination"] = Value::Null;
    let p = problems(&b);
    assert!(
        p.iter().any(|m| m.contains("a started command lacks")),
        "{p:?}"
    );
    assert!(
        p.iter().any(|m| m.contains("termination is not present")),
        "{p:?}"
    );

    for field in ["leaderPid", "groupId", "durationMs"] {
        let mut b = fixture("oracle-text");
        b["lifecycle"][field] = Value::Null;
        assert_invalid(&b, "a started command lacks");
    }

    for field in ["leaderPid", "groupId"] {
        let mut b = fixture("oracle-text");
        b["lifecycle"][field] = json!(-1);
        assert_invalid(&b, &format!("lifecycle.{field} is negative"));
    }
}

/// A cancellation before the start, made from the spawn failure. Today's
/// Linux agent gives it the fixed legacy fields of a cancellation.
fn cancelled_before_start() -> Value {
    let mut b = fixture("spawn-failure");
    b["outcome"] = json!({"kind": "cancelled", "beforeStart": true});
    b["stderr"] = json!("Process cancelled");
    b
}

#[test]
fn a_command_that_never_started_has_no_identity_and_no_termination() {
    read(&fixture("spawn-failure")).expect("a spawn failure has no leader");
    read(&cancelled_before_start()).expect("a cancellation before the start has no leader");

    for base in [fixture("spawn-failure"), cancelled_before_start()] {
        for (field, value) in [
            ("leaderPid", json!(4242)),
            ("groupId", json!(4242)),
            ("durationMs", json!(0)),
            ("leaderStartTime", json!("12345")),
        ] {
            let mut b = base.clone();
            b["lifecycle"][field] = value;
            assert_invalid(&b, "a command that never started has a leader identity");
        }
    }

    let mut b = cancelled_before_start();
    b["termination"] = fixture("deadline")["termination"].clone();
    assert_invalid(&b, "termination is not present");
}

/// A timed-out record with the given termination block.
fn terminated(termination: Value, finalised: i64) -> Value {
    let mut b = fixture("deadline-term-ignored");
    b["lifecycle"]["finalisedAfterMs"] = json!(finalised);
    b["termination"] = termination;
    b
}

fn term(
    term: i64,
    kill: i64,
    status: Value,
    exit: Value,
    empty: &str,
    empty_at: Value,
    within: bool,
) -> Value {
    json!({
        "termSentAfterMs": term,
        "killSentAfterMs": kill,
        "leaderStatus": status,
        "leaderExitAfterMs": exit,
        "groupEmpty": empty,
        "groupEmptyAfterMs": empty_at,
        "withinBounds": within,
    })
}

#[test]
fn termination_contradictions_are_invalid() {
    let sig9 = json!({"kind": "signalled", "signal": 9});
    let unavailable = json!({"kind": "unavailable"});

    let b = terminated(
        term(500, 100, sig9.clone(), json!(500), "yes", json!(500), true),
        600,
    );
    assert_invalid(&b, "termSentAfterMs <= killSentAfterMs");

    let b = terminated(
        term(0, 900, sig9.clone(), json!(800), "yes", json!(900), true),
        800,
    );
    assert_invalid(&b, "termSentAfterMs <= killSentAfterMs <= finalisedAfterMs");

    let b = terminated(
        term(0, 100, sig9.clone(), Value::Null, "yes", json!(100), true),
        200,
    );
    assert_invalid(&b, "leaderExitAfterMs is null");

    let b = terminated(
        term(
            0,
            100,
            unavailable.clone(),
            json!(100),
            "no",
            Value::Null,
            false,
        ),
        200,
    );
    assert_invalid(&b, "leaderExitAfterMs is set");

    let b = terminated(
        term(50, 100, sig9.clone(), json!(10), "yes", json!(100), true),
        200,
    );
    assert_invalid(&b, "leaderExitAfterMs is not between");

    let b = terminated(
        term(0, 100, sig9.clone(), json!(300), "yes", json!(300), true),
        200,
    );
    assert_invalid(&b, "leaderExitAfterMs is not between");

    let b = terminated(
        term(
            0,
            100,
            unavailable.clone(),
            Value::Null,
            "yes",
            json!(150),
            true,
        ),
        200,
    );
    assert_invalid(&b, "unavailable requires groupEmpty no");

    let b = terminated(
        term(0, 100, sig9.clone(), json!(100), "yes", Value::Null, true),
        200,
    );
    assert_invalid(&b, "groupEmptyAfterMs is null");

    let b = terminated(
        term(0, 100, sig9.clone(), json!(50), "yes", json!(90), true),
        200,
    );
    assert_invalid(&b, "groupEmptyAfterMs is not after");

    let b = terminated(
        term(0, 100, sig9.clone(), json!(150), "yes", json!(120), true),
        200,
    );
    assert_invalid(&b, "groupEmptyAfterMs is not after");

    let b = terminated(
        term(0, 100, sig9.clone(), json!(100), "yes", json!(250), true),
        200,
    );
    assert_invalid(&b, "groupEmptyAfterMs is not after");

    let b = terminated(
        term(0, 100, sig9.clone(), json!(100), "no", json!(100), false),
        200,
    );
    assert_invalid(&b, "groupEmptyAfterMs is set");

    let b = terminated(
        term(0, 100, sig9.clone(), json!(100), "yes", json!(100), false),
        200,
    );
    assert_invalid(&b, "withinBounds is false, but the derived value is true");

    let b = terminated(
        term(-1, 100, sig9, json!(100), "yes", json!(100), true),
        200,
    );
    assert_invalid(&b, "termSentAfterMs is negative");
}

#[test]
fn both_leader_orders_are_valid() {
    let sig15 = json!({"kind": "signalled", "signal": 15});
    let sig9 = json!({"kind": "signalled", "signal": 9});
    // The leader ends on SIGTERM, before SIGKILL.
    read(&terminated(
        term(0, 1500, sig15, json!(40), "yes", json!(1500), true),
        1510,
    ))
    .expect("leader exit before SIGKILL");
    // The leader ignores SIGTERM and ends after SIGKILL.
    read(&terminated(
        term(0, 1500, sig9, json!(1520), "yes", json!(1520), true),
        1530,
    ))
    .expect("leader exit after SIGKILL");
    // The recorded Linux reply where the leader ignored SIGTERM.
    read(&fixture("deadline-term-ignored")).expect("recorded term-ignored reply");
}

#[test]
fn an_unavailable_leader_status_is_valid() {
    let b = terminated(
        term(
            0,
            2000,
            json!({"kind": "unavailable"}),
            Value::Null,
            "no",
            Value::Null,
            false,
        ),
        3400,
    );
    let r = read(&b).expect("unavailable status with groupEmpty no");
    assert!(!r.termination.unwrap().within_bounds);
}

#[test]
fn within_bounds_is_derived_from_the_root_bounds() {
    let sig9 = json!({"kind": "signalled", "signal": 9});
    // At both bounds exactly.
    read(&terminated(
        term(0, 2000, sig9.clone(), json!(2000), "yes", json!(2000), true),
        4000,
    ))
    .expect("2000 and 2000 are within bounds");
    // SIGKILL past the grace bound.
    read(&terminated(
        term(
            0,
            2001,
            sig9.clone(),
            json!(2001),
            "yes",
            json!(2001),
            false,
        ),
        2001,
    ))
    .expect("late SIGKILL is valid but out of bounds");
    assert_invalid(
        &terminated(
            term(0, 2001, sig9.clone(), json!(2001), "yes", json!(2001), true),
            2001,
        ),
        "withinBounds",
    );
    // Publication past the kill, drain and reap bound.
    assert_invalid(
        &terminated(
            term(0, 1000, sig9.clone(), json!(1000), "yes", json!(1000), true),
            3001,
        ),
        "withinBounds",
    );
    // A group of unknown state is never within bounds.
    read(&terminated(
        term(0, 100, sig9, json!(100), "unknown", Value::Null, false),
        200,
    ))
    .expect("unknown group is valid but out of bounds");
}

#[test]
fn agent_clock_values_are_read_without_host_time() {
    // The agent clock alone measures each time. The reader checks that each
    // time is not negative, the order and the bounds within the record. It
    // never compares a time with a host time, so large and zero values are
    // valid. These cases do not measure timing bounds from the controller.
    let big = 9_000_000_000_000_i64;
    let sig9 = json!({"kind": "signalled", "signal": 9});
    let mut b = terminated(
        term(big, big, sig9, json!(big), "yes", json!(big), false),
        big + 1,
    );
    b["lifecycle"]["spawnMs"] = json!(big);
    b["lifecycle"]["durationMs"] = json!(big);
    b["lifecycle"]["ownerLatenessMs"] = json!(big);
    let r = read(&b).expect("large agent times are valid");
    assert_eq!(r.lifecycle.unwrap().finalised_after_ms, big + 1);

    let mut b = fixture("oracle-text");
    b["lifecycle"]["spawnMs"] = json!(0);
    b["lifecycle"]["durationMs"] = json!(0);
    b["lifecycle"]["finalisedAfterMs"] = json!(0);
    read(&b).expect("zero times are valid");
}

// ---- the macOS projection, ahead of the macOS agent (leaf k219) ----

fn macos_record(outcome: Value, stdout: &[u8], stderr: &[u8]) -> Value {
    let o: Outcome = serde_json::from_value(outcome.clone()).unwrap();
    let legacy = legacy_projection(LegacyProjection::Macos, &o, stdout, stderr);
    let mut b = fixture(if matches!(o, Outcome::TimedOut { .. }) {
        "deadline"
    } else {
        "oracle-text"
    });
    b["outcome"] = outcome;
    b["legacyProjection"] = json!("macos");
    b["exitCode"] = json!(legacy.exit_code);
    b["stdout"] = json!(legacy.stdout);
    b["stderr"] = json!(legacy.stderr.unwrap());
    b["timedOut"] = json!(legacy.timed_out.unwrap());
    set_stream(&mut b, "stdout", stdout);
    set_stream(&mut b, "stderr", stderr);
    b
}

#[test]
fn macos_projection_matches_todays_macos_values() {
    let b = macos_record(
        json!({"kind": "exited", "exitCode": 0}),
        b"Explain this change.",
        b"separate stderr\n",
    );
    assert_eq!(
        (b["stdout"].as_str(), b["stderr"].as_str()),
        (Some("Explain this change."), Some("separate stderr"))
    );
    read(&b).expect("exited");

    let b = macos_record(
        json!({"kind": "signalled", "signal": 9, "signalName": "SIGKILL"}),
        b"before signal\n",
        b"",
    );
    assert_eq!(b["exitCode"], 9);
    read(&b).expect("signalled");

    let b = macos_record(
        json!({"kind": "timed_out", "timeoutSeconds": 2}),
        b"before deadline\n",
        b"",
    );
    assert_eq!(b["stderr"], "Process timed out after 2s");
    assert_eq!(b["timedOut"], true);
    read(&b).expect("timed out");

    let b = macos_record(
        json!({"kind": "timed_out", "timeoutSeconds": 10}),
        b"partial out\n",
        b"partial err\n",
    );
    assert_eq!(b["stderr"], "partial err\n[Process timed out after 10s]");

    let b = macos_record(
        json!({"kind": "exited", "exitCode": 0}),
        b"\x00\xff\x80A\n",
        b"",
    );
    assert_eq!(b["stdout"], "");
    read(&b).expect("non-UTF-8 stream becomes empty");

    let mut b = macos_record(json!({"kind": "exited", "exitCode": 0}), b"out", b"");
    b["timedOut"] = Value::Null;
    assert_invalid(&b, "legacy fields");
}

/// An exited macOS record with stated legacy text. The text is not derived
/// from the reader, so the case checks the reader against the agent.
fn macos_exited(stdout: &[u8], stderr: &[u8], legacy_out: &str, legacy_err: &str) -> Value {
    let mut b = fixture("oracle-text");
    b["outcome"] = json!({"kind": "exited", "exitCode": 0});
    b["legacyProjection"] = json!("macos");
    b["exitCode"] = json!(0);
    b["stdout"] = json!(legacy_out);
    b["stderr"] = json!(legacy_err);
    b["timedOut"] = json!(false);
    set_stream(&mut b, "stdout", stdout);
    set_stream(&mut b, "stderr", stderr);
    b
}

/// Review k231, finding R3. Each legacy value is the output of the
/// Foundation call `replacingOccurrences(of: "\\s+$", with: "", options:
/// .regularExpression)` from ProcessRunner.swift, run with Swift 6.4.2 on
/// macOS.
#[test]
fn macos_projection_trims_as_foundation_trims() {
    read(&macos_exited(b"a\x0b", b"", "a", "")).expect("Foundation removes a vertical tab");
    assert_invalid(
        &macos_exited(b"a\x0b", b"", "a\u{000B}", ""),
        "legacy fields",
    );

    let next_line = "e\u{0085}".as_bytes();
    read(&macos_exited(b"", next_line, "", "e")).expect("Foundation removes U+0085");
    assert_invalid(
        &macos_exited(b"", next_line, "", "e\u{0085}"),
        "legacy fields",
    );

    read(&macos_exited(b"a\x1c", b"", "a\u{001C}", "")).expect("Foundation keeps U+001C");
    assert_invalid(&macos_exited(b"a\x1c", b"", "a", ""), "legacy fields");

    let zero_width = "a\u{200B}".as_bytes();
    read(&macos_exited(zero_width, b"", "a\u{200B}", "")).expect("Foundation keeps U+200B");
}
