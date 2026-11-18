//! The typed exec record and its validity rules.
//!
//! Source of truth: `docs/specs/guest-command-lifecycle.md`, section "Exec
//! record" (legacy fields, added fields, termination fields, validity rules),
//! and ADR-0023. A new agent sends the added fields beside the legacy
//! `exitCode`, `stdout`, `stderr` and `timedOut`. An old agent and the Windows
//! agent send the legacy fields only.
//!
//! [`read_exec_reply`] reads an `/exec` reply. It returns a legacy reply
//! unchanged, and it accepts a typed reply only when every validity rule
//! holds. A record that breaks a rule is contradictory evidence, so the
//! caller reports `EXEC_RECORD_INVALID` and never reports it as a success.

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::ExecResult;

/// The root grace bound and the root bound for kill, drain and reap, in
/// milliseconds. Spec, section "Termination fields".
pub const ROOT_BOUND_MS: i64 = 2000;

/// How the guest command ended. The wire tags it by `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Outcome {
    Exited {
        exit_code: i64,
    },
    Signalled {
        signal: i64,
        signal_name: String,
    },
    TimedOut {
        /// The deadline of the request, as the agent states it.
        timeout_seconds: serde_json::Number,
    },
    Cancelled {
        before_start: bool,
    },
    SpawnFailed {
        errno: i64,
        error_name: String,
        message: String,
    },
}

impl Outcome {
    /// The wire name of the kind, for example `timed_out`.
    pub fn kind(&self) -> &'static str {
        match self {
            Outcome::Exited { .. } => "exited",
            Outcome::Signalled { .. } => "signalled",
            Outcome::TimedOut { .. } => "timed_out",
            Outcome::Cancelled { .. } => "cancelled",
            Outcome::SpawnFailed { .. } => "spawn_failed",
        }
    }

    /// Whether a leader started. Only a spawn failure and a cancellation
    /// before the start have no leader. The reader derives this from the
    /// outcome, so a null identity cannot remove the evidence that a started
    /// command requires.
    pub fn started(&self) -> bool {
        !matches!(
            self,
            Outcome::SpawnFailed { .. } | Outcome::Cancelled { before_start: true }
        )
    }
}

/// Whether the agent captured every byte of a stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capture {
    Complete,
    Incomplete,
}

/// The captured bytes of one stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamEvidence {
    pub base64: String,
    pub bytes: i64,
    /// The file size at finalisation.
    pub file_bytes: i64,
    pub sha256: String,
    pub capture: Capture,
    /// `deadline` or `read_error` for an incomplete stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl StreamEvidence {
    /// The decoded bytes, or `None` when `base64` does not decode.
    pub fn decoded(&self) -> Option<Vec<u8>> {
        base64::engine::general_purpose::STANDARD
            .decode(&self.base64)
            .ok()
    }
}

/// The two captured streams.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub stdout: StreamEvidence,
    pub stderr: StreamEvidence,
}

/// The identity and the timings of the owned group. The agent clock measures
/// each time, so the reader never compares one with a host time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lifecycle {
    pub leader_pid: Option<i64>,
    pub group_id: Option<i64>,
    pub leader_start_time: Option<String>,
    pub spawn_ms: i64,
    pub duration_ms: Option<i64>,
    pub owner_lateness_ms: i64,
    pub finalised_after_ms: i64,
}

/// Whether members of the group remained after the leader ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemainingGroup {
    /// The wire value `none`: no member remained.
    #[serde(rename = "none")]
    NoMember,
    Present,
    Unknown,
}

impl RemainingGroup {
    pub fn as_str(self) -> &'static str {
        match self {
            RemainingGroup::NoMember => "none",
            RemainingGroup::Present => "present",
            RemainingGroup::Unknown => "unknown",
        }
    }
}

/// The exit status of the leader during a termination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum LeaderStatus {
    Exited { exit_code: i64 },
    Signalled { signal: i64 },
    Unavailable,
}

/// The answer of the final group probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupEmpty {
    Yes,
    No,
    Unknown,
}

impl GroupEmpty {
    pub fn as_str(self) -> &'static str {
        match self {
            GroupEmpty::Yes => "yes",
            GroupEmpty::No => "no",
            GroupEmpty::Unknown => "unknown",
        }
    }
}

/// How the owner ended the group of a timed-out or cancelled command. Each
/// time is milliseconds from the outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Termination {
    pub term_sent_after_ms: i64,
    pub kill_sent_after_ms: i64,
    pub leader_status: LeaderStatus,
    pub leader_exit_after_ms: Option<i64>,
    pub group_empty: GroupEmpty,
    pub group_empty_after_ms: Option<i64>,
    pub within_bounds: bool,
}

impl Termination {
    /// The value that `withinBounds` must hold. It is true exactly when the
    /// group is empty, SIGKILL went within the grace bound, and the record was
    /// published within the bound for kill, drain and reap.
    pub fn derived_within_bounds(&self, finalised_after_ms: i64) -> bool {
        self.group_empty == GroupEmpty::Yes
            && self.kill_sent_after_ms <= ROOT_BOUND_MS
            && finalised_after_ms - self.kill_sent_after_ms <= ROOT_BOUND_MS
    }
}

/// The platform whose legacy values the legacy fields carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyProjection {
    Macos,
    Linux,
}

impl LegacyProjection {
    pub fn as_str(self) -> &'static str {
        match self {
            LegacyProjection::Macos => "macos",
            LegacyProjection::Linux => "linux",
        }
    }
}

/// Why an `/exec` reply could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecReplyError {
    /// The body lacks the legacy fields, as a body that today's reader
    /// rejects.
    Decode(String),
    /// The body carries a typed record that breaks a validity rule. Each
    /// entry names one broken rule.
    Invalid(Vec<String>),
}

/// Read an `/exec` reply body.
///
/// A body without `outcome` is a legacy reply, and it returns with every
/// added field `None`. A body with `outcome` returns only when every validity
/// rule holds. `sent_id` is the exec id that the request carried.
pub fn read_exec_reply(body: &[u8], sent_id: Option<&str>) -> Result<ExecResult, ExecReplyError> {
    let value: Value =
        serde_json::from_slice(body).map_err(|e| ExecReplyError::Decode(e.to_string()))?;
    let typed = value.get("outcome").is_some_and(|o| !o.is_null());
    if !typed {
        let legacy: LegacyFields =
            serde_json::from_value(value).map_err(|e| ExecReplyError::Decode(e.to_string()))?;
        return Ok(ExecResult {
            exit_code: legacy.exit_code,
            stdout: legacy.stdout,
            stderr: legacy.stderr,
            timed_out: legacy.timed_out,
            ..ExecResult::default()
        });
    }
    let result: ExecResult = serde_json::from_value(value)
        .map_err(|e| ExecReplyError::Invalid(vec![format!("the record does not decode: {e}")]))?;
    let problems = validity_problems(&result, sent_id);
    if problems.is_empty() {
        Ok(result)
    } else {
        Err(ExecReplyError::Invalid(problems))
    }
}

#[derive(Deserialize)]
struct LegacyFields {
    #[serde(rename = "exitCode")]
    exit_code: i32,
    stdout: String,
    stderr: String,
    #[serde(rename = "timedOut", default)]
    timed_out: Option<bool>,
}

/// Every broken validity rule of a typed record, one message per rule. An
/// empty list means a valid record. Spec, section "Validity rules".
pub fn validity_problems(result: &ExecResult, sent_id: Option<&str>) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(outcome) = &result.outcome else {
        problems.push("the record has no outcome".to_string());
        return problems;
    };

    match (sent_id, result.id.as_deref()) {
        (Some(sent), Some(id)) if sent == id => {}
        (Some(sent), Some(id)) => {
            problems.push(format!("id {id:?} is not the sent id {sent:?}"));
        }
        (None, Some(_)) => {}
        (_, None) => problems.push("the record has no id".to_string()),
    }
    if result.legacy_projection.is_none() {
        problems.push("legacyProjection is not macos or linux".to_string());
    }
    if let Outcome::Exited { exit_code } = outcome {
        if !(0..=255).contains(exit_code) {
            problems.push(format!("outcome exitCode {exit_code} is not 0 to 255"));
        }
    }

    let streams = match &result.evidence {
        Some(evidence) => {
            let out = check_stream("stdout", &evidence.stdout, &mut problems);
            let err = check_stream("stderr", &evidence.stderr, &mut problems);
            out.zip(err)
        }
        None => {
            problems.push("the record has no evidence".to_string());
            None
        }
    };

    let Some(lifecycle) = &result.lifecycle else {
        problems.push("the record has no lifecycle".to_string());
        return problems;
    };
    check_times(
        &[
            ("lifecycle.leaderPid", lifecycle.leader_pid),
            ("lifecycle.groupId", lifecycle.group_id),
            ("lifecycle.spawnMs", Some(lifecycle.spawn_ms)),
            ("lifecycle.durationMs", lifecycle.duration_ms),
            (
                "lifecycle.ownerLatenessMs",
                Some(lifecycle.owner_lateness_ms),
            ),
            (
                "lifecycle.finalisedAfterMs",
                Some(lifecycle.finalised_after_ms),
            ),
        ],
        &mut problems,
    );

    let started = outcome.started();
    check_identity(lifecycle, started, &mut problems);

    let ended_alone = matches!(outcome, Outcome::Exited { .. } | Outcome::Signalled { .. });
    if ended_alone != result.remaining_group.is_some() {
        problems.push("remainingGroup is not present exactly for exited and signalled".to_string());
    }
    let terminated = started
        && matches!(
            outcome,
            Outcome::TimedOut { .. } | Outcome::Cancelled { .. }
        );
    match (&result.termination, terminated) {
        (Some(termination), true) => {
            check_termination(termination, lifecycle.finalised_after_ms, &mut problems);
        }
        (None, false) => {}
        _ => problems.push(
            "termination is not present exactly for a timed-out or cancelled command that started"
                .to_string(),
        ),
    }

    if let (Some(projection), Some((out, err))) = (result.legacy_projection, streams) {
        check_legacy_fields(result, projection, outcome, &out, &err, &mut problems);
    }
    problems
}

/// A started command names its leader, its group and its duration. A command
/// that never started names none of them and no leader start time.
fn check_identity(lifecycle: &Lifecycle, started: bool, problems: &mut Vec<String>) {
    let identity = [
        lifecycle.leader_pid,
        lifecycle.group_id,
        lifecycle.duration_ms,
    ];
    if started && identity.iter().any(Option::is_none) {
        problems
            .push("a started command lacks lifecycle.leaderPid, groupId or durationMs".to_string());
    }
    if !started && (identity.iter().any(Option::is_some) || lifecycle.leader_start_time.is_some()) {
        problems.push("a command that never started has a leader identity".to_string());
    }
}

fn check_stream(
    name: &str,
    stream: &StreamEvidence,
    problems: &mut Vec<String>,
) -> Option<Vec<u8>> {
    let Some(data) = stream.decoded() else {
        problems.push(format!("evidence.{name}.base64 does not decode"));
        return None;
    };
    if stream.bytes != data.len() as i64 {
        problems.push(format!(
            "evidence.{name}.bytes is {}, but base64 decodes to {} bytes",
            stream.bytes,
            data.len()
        ));
    }
    if stream.sha256 != sha256_hex(&data) {
        problems.push(format!(
            "evidence.{name}.sha256 is not the digest of the bytes"
        ));
    }
    match stream.capture {
        Capture::Complete if stream.bytes != stream.file_bytes => problems.push(format!(
            "evidence.{name} is complete, but bytes is not fileBytes"
        )),
        Capture::Incomplete if stream.bytes >= stream.file_bytes => problems.push(format!(
            "evidence.{name} is incomplete, but bytes is not less than fileBytes"
        )),
        _ => {}
    }
    match (stream.capture, stream.reason.as_deref()) {
        (Capture::Complete, None) | (Capture::Incomplete, Some("deadline" | "read_error")) => {}
        (Capture::Complete, Some(_)) => {
            problems.push(format!("evidence.{name} is complete, but it has a reason"));
        }
        (Capture::Incomplete, _) => problems.push(format!(
            "evidence.{name}.reason is not deadline or read_error"
        )),
    }
    Some(data)
}

/// The lowercase hex SHA-256 digest of `data`.
/// API: https://docs.rs/sha2/0.10.9/sha2/ (`Digest::digest` returns the byte array).
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn check_times(times: &[(&str, Option<i64>)], problems: &mut Vec<String>) {
    for (name, value) in times {
        if let Some(ms) = value {
            if *ms < 0 {
                problems.push(format!("{name} is negative"));
            }
        }
    }
}

fn check_termination(t: &Termination, finalised: i64, problems: &mut Vec<String>) {
    check_times(
        &[
            ("termination.termSentAfterMs", Some(t.term_sent_after_ms)),
            ("termination.killSentAfterMs", Some(t.kill_sent_after_ms)),
            ("termination.leaderExitAfterMs", t.leader_exit_after_ms),
            ("termination.groupEmptyAfterMs", t.group_empty_after_ms),
        ],
        problems,
    );
    if !(t.term_sent_after_ms <= t.kill_sent_after_ms && t.kill_sent_after_ms <= finalised) {
        problems.push(
            "termination breaks termSentAfterMs <= killSentAfterMs <= finalisedAfterMs".to_string(),
        );
    }
    // The leader can exit before SIGKILL or after it, so no order with
    // killSentAfterMs is required.
    let unavailable = t.leader_status == LeaderStatus::Unavailable;
    match t.leader_exit_after_ms {
        None if !unavailable => problems.push(
            "termination.leaderExitAfterMs is null, but leaderStatus is not unavailable"
                .to_string(),
        ),
        Some(_) if unavailable => problems.push(
            "termination.leaderExitAfterMs is set, but leaderStatus is unavailable".to_string(),
        ),
        Some(exit) if exit < t.term_sent_after_ms || exit > finalised => problems.push(
            "termination.leaderExitAfterMs is not between termSentAfterMs and finalisedAfterMs"
                .to_string(),
        ),
        _ => {}
    }
    if unavailable && t.group_empty != GroupEmpty::No {
        problems.push("termination.leaderStatus unavailable requires groupEmpty no".to_string());
    }
    match (t.group_empty, t.group_empty_after_ms) {
        (GroupEmpty::Yes, None) => problems
            .push("termination.groupEmptyAfterMs is null, but groupEmpty is yes".to_string()),
        (GroupEmpty::Yes, Some(empty)) => {
            let after_exit = t.leader_exit_after_ms.map_or(true, |exit| empty >= exit);
            if empty < t.kill_sent_after_ms || !after_exit || empty > finalised {
                problems.push(
                    "termination.groupEmptyAfterMs is not after killSentAfterMs and \
                     leaderExitAfterMs and at most finalisedAfterMs"
                        .to_string(),
                );
            }
        }
        (_, Some(_)) => problems
            .push("termination.groupEmptyAfterMs is set, but groupEmpty is not yes".to_string()),
        (_, None) => {}
    }
    if t.within_bounds != t.derived_within_bounds(finalised) {
        problems.push(format!(
            "termination.withinBounds is {}, but the derived value is {}",
            t.within_bounds, !t.within_bounds
        ));
    }
}

/// The legacy fields that today's agent of a platform gives for an outcome
/// and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyValues {
    pub exit_code: i64,
    pub stdout: String,
    /// `None` when the text is a Linux decoder message. The Host CLI does not
    /// compare that text, because it is the wording of the Python codec.
    pub stderr: Option<String>,
    /// `None` when the platform omits `timedOut`.
    pub timed_out: Option<bool>,
}

fn check_legacy_fields(
    result: &ExecResult,
    projection: LegacyProjection,
    outcome: &Outcome,
    stdout: &[u8],
    stderr: &[u8],
    problems: &mut Vec<String>,
) {
    let expected = legacy_projection(projection, outcome, stdout, stderr);
    let stderr_matches = expected
        .stderr
        .as_deref()
        .map_or(true, |e| e == result.stderr);
    if i64::from(result.exit_code) != expected.exit_code
        || result.stdout != expected.stdout
        || !stderr_matches
        || result.timed_out != expected.timed_out
    {
        problems.push(format!(
            "the legacy fields (exitCode {}, stdout {:?}, stderr {:?}, timedOut {:?}) are not \
             the {} projection {expected:?}",
            result.exit_code,
            result.stdout,
            result.stderr,
            result.timed_out,
            projection.as_str()
        ));
    }
}

/// The legacy projection of `outcome` and the evidence bytes. Spec, section
/// "Legacy fields".
pub fn legacy_projection(
    projection: LegacyProjection,
    outcome: &Outcome,
    stdout: &[u8],
    stderr: &[u8],
) -> LegacyValues {
    match projection {
        LegacyProjection::Macos => macos_projection(outcome, stdout, stderr),
        LegacyProjection::Linux => linux_projection(outcome, stdout, stderr),
    }
}

fn macos_projection(outcome: &Outcome, stdout: &[u8], stderr: &[u8]) -> LegacyValues {
    let out = macos_decode(stdout);
    let err = macos_decode(stderr);
    let with_note = |note: String| {
        if err.is_empty() {
            note
        } else {
            format!("{err}\n[{note}]")
        }
    };
    let (exit_code, stdout, stderr, timed_out) = match outcome {
        Outcome::Exited { exit_code } => (*exit_code, out, err, false),
        // Foundation `terminationStatus` gives the signal number itself.
        Outcome::Signalled { signal, .. } => (*signal, out, err, false),
        Outcome::TimedOut { timeout_seconds } => (
            -1,
            out,
            with_note(format!("Process timed out after {timeout_seconds}s")),
            true,
        ),
        Outcome::Cancelled { .. } => (-1, out, with_note("Process cancelled".to_string()), false),
        Outcome::SpawnFailed { message, .. } => (-1, String::new(), message.clone(), false),
    };
    LegacyValues {
        exit_code,
        stdout,
        stderr: Some(stderr),
        timed_out: Some(timed_out),
    }
}

/// Strict UTF-8, where a stream that is not valid becomes empty, then the
/// removal of trailing whitespace as the Foundation regex `\s+$` removes it.
/// ICU matches `\s` with the Unicode property White_Space, and Rust
/// `char::is_whitespace` tests the same property. So vertical tab and U+0085
/// go, and U+001C to U+001F, U+180E and U+200B stay.
/// Source: https://github.com/unicode-org/icu/blob/release-76-1/icu4c/source/i18n/regexst.cpp#L51-L53
fn macos_decode(data: &[u8]) -> String {
    let text = std::str::from_utf8(data).unwrap_or("");
    text.trim_end_matches(char::is_whitespace).to_string()
}

fn linux_projection(outcome: &Outcome, stdout: &[u8], stderr: &[u8]) -> LegacyValues {
    let fixed = |stderr: &str| LegacyValues {
        exit_code: -1,
        stdout: String::new(),
        stderr: Some(stderr.to_string()),
        timed_out: None,
    };
    let exit_code = match outcome {
        Outcome::Exited { exit_code } => *exit_code,
        Outcome::Signalled { signal, .. } => -signal,
        Outcome::TimedOut { .. } => return fixed("Process timed out"),
        Outcome::Cancelled { .. } => return fixed("Process cancelled"),
        Outcome::SpawnFailed { message, .. } => return fixed(message),
    };
    match (linux_decode(stdout), linux_decode(stderr)) {
        (Some(out), Some(err)) => LegacyValues {
            exit_code,
            stdout: out,
            stderr: Some(err),
            timed_out: None,
        },
        _ => LegacyValues {
            exit_code: -1,
            stdout: String::new(),
            stderr: None,
            timed_out: None,
        },
    }
}

/// Strict UTF-8 with universal newlines, as `subprocess.run(text=True)` gave,
/// then `str.rstrip()`. Python counts U+001C to U+001F as whitespace, and
/// Rust `char::is_whitespace` does not, so they are added here.
/// Source: https://docs.python.org/3/library/stdtypes.html#str.isspace
fn linux_decode(data: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(data).ok()?;
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    Some(
        text.trim_end_matches(|c: char| {
            c.is_whitespace() || ('\u{001C}'..='\u{001F}').contains(&c)
        })
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_trim_matches_foundation_white_space() {
        // Each expected value is the output of the Foundation call in
        // ProcessRunner.swift, run with Swift 6.4.2 on macOS.
        assert_eq!(macos_decode(b"a \t\r\n"), "a");
        assert_eq!(macos_decode(b"a\x0b"), "a");
        assert_eq!(macos_decode("a\u{0085}".as_bytes()), "a");
        assert_eq!(macos_decode("a\u{00A0}\u{2028}\u{202F}".as_bytes()), "a");
        assert_eq!(macos_decode("a\u{3000}".as_bytes()), "a");
        assert_eq!(macos_decode(b"a\x1c"), "a\x1c");
        assert_eq!(macos_decode(b"a\x1f"), "a\x1f");
        assert_eq!(macos_decode("a\u{180E}".as_bytes()), "a\u{180E}");
        assert_eq!(macos_decode("a\u{200B}".as_bytes()), "a\u{200B}");
        assert_eq!(macos_decode("a\u{FEFF}".as_bytes()), "a\u{FEFF}");
        assert_eq!(macos_decode(b"\xff\n"), "");
    }

    #[test]
    fn linux_decode_translates_newlines_and_strips_python_whitespace() {
        assert_eq!(linux_decode(b"a\r\nb\rc\n").as_deref(), Some("a\nb\nc"));
        assert_eq!(linux_decode(b"a\x1c\x1f\x0b ").as_deref(), Some("a"));
        assert_eq!(linux_decode(b"\xff"), None);
    }

    #[test]
    fn sha256_hex_matches_the_published_vector() {
        // The empty-input digest from FIPS 180-4 test vectors.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
