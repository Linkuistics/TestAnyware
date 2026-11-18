//! `file {exec|upload|download}` handlers.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use testanyware_agent_client::AgentClient;
use testanyware_protocol::exec_record::{
    CancelState, Capture, LeaderStatus, Lifecycle, Outcome, StreamEvidence, Termination,
};
use testanyware_protocol::{ExecCancelRequest, ExecRequest, ExecResult};

use crate::commands::interrupt::{raise_again, Interrupts};
use crate::commands::{build_agent_client, exit_agent_error};
use crate::output::{print_success, OutputMode};
use crate::resolve::ConnectionOptions;

pub struct ExecArgs {
    pub command: String,
    pub timeout: i64,
    pub detach: bool,
    /// The exec id that `--exec-id` gave, already checked. `None` sends a
    /// random UUID.
    pub exec_id: Option<String>,
}

/// How long CLI interruption waits for the reply to its cancellation, from
/// the dial to the last byte (spec, section "CLI interruption", step 3).
pub const INTERRUPT_CANCEL_LIMIT: Duration = Duration::from_millis(1500);

/// What ended a command that listens for SIGINT and SIGTERM.
enum Ended<T> {
    Done(T),
    Interrupted(i32),
}

/// Run `work` until it ends or a signal arrives. `work` is dropped before
/// this returns, so its guards have run when a signal ends it.
///
/// When `work` ends first, the default actions come back, so a signal during
/// the output kills the process as before. A signal that the handler caught
/// before that point is raised again here. The command has already ended, so
/// no cancellation is sent (review finding R1 of host-cli-control-k239).
async fn until_interrupted<T>(
    work: impl std::future::Future<Output = T>,
    mut interrupts: Interrupts,
) -> Ended<T> {
    tokio::pin!(work);
    let ended = tokio::select! {
        done = &mut work => Ended::Done(done),
        number = interrupts.recv() => Ended::Interrupted(number),
    };
    match ended {
        Ended::Done(done) => match interrupts.finish() {
            None => Ended::Done(done),
            Some(number) => raise_again(number),
        },
        interrupted => interrupted,
    }
}

/// `testanyware file exec` and its `exec` alias.
///
/// Text mode: stdout/stderr are passthrough (Swift parity); the binary's
/// own exit code is the in-VM process's legacy exit code.
/// JSON mode: emit a single envelope. The top-level `exit_code` carries the
/// legacy in-VM exit, and the binary itself exits 0 when the transport and the
/// record are sound (§5 sub-process exit-code rule). A guest outcome never
/// sets `ok` to false. The added fields carry the typed exec record
/// (`docs/specs/guest-command-lifecycle.md`, section "Host CLI semantics").
///
/// An attached command sends the `--exec-id` or a random exec id. A record
/// that breaks a validity rule fails with `EXEC_RECORD_INVALID` in both modes.
/// A detached command has no record, so it sends no id and its output does not
/// change.
///
/// At SIGINT or SIGTERM, an attached command follows the spec's "CLI
/// interruption" steps. It sends one cancellation for its exec id on a new
/// guest connection, waits for the reply for 1.5 s at most, and raises the
/// same signal again. An old agent answers 404, and the signal is raised
/// again without a cancellation.
pub async fn run_exec(opts: ConnectionOptions, args: ExecArgs, mode: OutputMode, dry_run: bool) {
    let request = ExecRequest {
        command: args.command,
        timeout: args.timeout,
        detach: args.detach,
        id: None,
    };
    if dry_run {
        emit_exec_dry_run(&request, mode);
        return;
    }

    let exec_id = args.exec_id;
    let request = ExecRequest {
        id: (!request.detach).then(|| exec_id.unwrap_or_else(new_exec_id)),
        ..request
    };
    let client = build_agent_client(&opts, mode);
    let outcome = match &request.id {
        // A detached command has no id to cancel, so it keeps the default
        // actions, as today.
        None => client.exec(&request).await,
        Some(id) => match until_interrupted(client.exec(&request), Interrupts::install()).await {
            Ended::Done(outcome) => outcome,
            Ended::Interrupted(number) => {
                cancel_after_interrupt(&client, id).await;
                raise_again(number)
            }
        },
    };
    match outcome {
        Ok(result) => match mode {
            OutputMode::Json => print_success(exec_envelope(&request, &result)),
            OutputMode::Text => {
                if !result.stdout.is_empty() {
                    print!("{}", result.stdout);
                }
                if !result.stderr.is_empty() {
                    let mut stderr = std::io::stderr();
                    let _ = stderr.write_all(result.stderr.as_bytes());
                    if !result.stderr.ends_with('\n') {
                        let _ = stderr.write_all(b"\n");
                    }
                }
                if !result.succeeded() {
                    std::process::exit(result.exit_code);
                }
            }
        },
        Err(err) => exit_agent_error(err, mode),
    }
}

/// Steps 1 to 3 of the spec's "CLI interruption": one cancellation with
/// `waitMs` 0 on a new guest connection of the normal route, within 1.5 s.
/// The client opens a new connection for each request. Every outcome,
/// including HTTP 404 from an old agent and no reply, leads to the same next
/// step, so the result is not read.
async fn cancel_after_interrupt(client: &AgentClient, id: &str) {
    let request = ExecCancelRequest {
        id: id.to_string(),
        wait_ms: 0,
    };
    let _ = client.cancel_exec(&request, INTERRUPT_CANCEL_LIMIT).await;
}

/// The arguments of `file cancel`.
pub struct CancelArgs {
    /// The exec id, already checked.
    pub exec_id: String,
    /// Seconds to wait for the final record, 0 to 10.
    pub wait: u64,
}

/// `testanyware file cancel` (ADR-0025). Sends one cancellation for an exec
/// id and reports the state, and the record when the state is `terminal`.
/// The command is idempotent: a repeat reports the current state and never
/// changes a terminal outcome. An old agent answers 404, which gives
/// `AGENT_ENDPOINT_UNSUPPORTED`.
pub async fn run_cancel(
    opts: ConnectionOptions,
    args: CancelArgs,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        match mode {
            OutputMode::Json => print_success(json!({
                "exec_id": args.exec_id,
                "wait": args.wait,
                "dry_run": true,
            })),
            OutputMode::Text => println!("DRY-RUN: would cancel exec id {}", args.exec_id),
        }
        return;
    }
    let request = ExecCancelRequest {
        id: args.exec_id,
        wait_ms: args.wait * 1000,
    };
    // As `file exec`, the transport gets 10 s past the wait of the agent.
    let limit = Duration::from_secs(args.wait + 10);
    let client = build_agent_client(&opts, mode);
    match client.cancel_exec(&request, limit).await {
        Ok(state) => match mode {
            OutputMode::Json => print_success(json!({
                "exec_id": request.id,
                "state": state.as_str(),
                "record": match &state {
                    CancelState::Terminal(result) => record_json(result),
                    _ => Value::Null,
                },
            })),
            OutputMode::Text => {
                println!("{}", state.as_str());
                if let CancelState::Terminal(result) = &state {
                    if let Some(outcome) = &result.outcome {
                        println!("outcome: {}", outcome_text(outcome));
                    }
                }
            }
        },
        Err(err) => exit_agent_error(err, mode),
    }
}

/// One line that names an outcome, for text mode.
fn outcome_text(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Exited { exit_code } => format!("exited with code {exit_code}"),
        Outcome::Signalled { signal_name, .. } => format!("ended by signal {signal_name}"),
        Outcome::TimedOut { timeout_seconds } => {
            format!("timed out after {timeout_seconds} s")
        }
        Outcome::Cancelled { before_start: true } => "cancelled before it started".to_string(),
        Outcome::Cancelled {
            before_start: false,
        } => "cancelled".to_string(),
        Outcome::SpawnFailed { error_name, .. } => format!("spawn failed ({error_name})"),
    }
}

/// The JSON of a typed record that a cancellation returned. The legacy fields
/// keep their names in `file exec`, and the typed fields are in snake_case.
fn record_json(result: &ExecResult) -> Value {
    let mut fields = json!({
        "exit_code": result.exit_code,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "timed_out": result.timed_out.unwrap_or(false),
    });
    if let (Value::Object(base), Value::Object(extra)) = (&mut fields, typed_json(result)) {
        base.extend(extra);
    }
    fields
}

/// The typed fields of a record that has an outcome.
fn typed_json(result: &ExecResult) -> Value {
    json!({
        "exec_id": result.id,
        "legacy_projection": result.legacy_projection.map(|p| p.as_str()),
        "outcome": result.outcome.as_ref().map(outcome_json),
        "evidence": result.evidence.as_ref().map(|e| json!({
            "stdout": stream_json(&e.stdout),
            "stderr": stream_json(&e.stderr),
        })),
        "lifecycle": result.lifecycle.as_ref().map(lifecycle_json),
        "termination": result.termination.as_ref().map(termination_json),
        "remaining_group": result.remaining_group.map(|g| g.as_str()),
    })
}

/// The `file exec` JSON fields. The legacy fields copy the agent record
/// unchanged, as today. An attached command adds the record fields. A legacy
/// reply gives the outcome `unknown`, which a caller never counts as a
/// success.
pub(crate) fn exec_envelope(request: &ExecRequest, result: &ExecResult) -> Value {
    let mut fields = json!({
        "command": request.command,
        "exit_code": result.exit_code,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "timed_out": result.timed_out.unwrap_or(false),
    });
    if request.detach {
        return fields;
    }
    let added = match &result.outcome {
        Some(_) => {
            let mut typed = typed_json(result);
            typed["record"] = json!("agent");
            typed
        }
        None => json!({
            "exec_id": request.id,
            "record": "legacy",
            "legacy_projection": null,
            "outcome": {"kind": "unknown", "reason": "legacy_agent"},
            "evidence": null,
            "lifecycle": null,
            "termination": null,
            "remaining_group": null,
        }),
    };
    if let (Value::Object(base), Value::Object(extra)) = (&mut fields, added) {
        base.extend(extra);
    }
    fields
}

fn outcome_json(outcome: &Outcome) -> Value {
    match outcome {
        Outcome::Exited { exit_code } => json!({"kind": "exited", "exit_code": exit_code}),
        Outcome::Signalled {
            signal,
            signal_name,
        } => {
            json!({"kind": "signalled", "signal": signal, "signal_name": signal_name})
        }
        Outcome::TimedOut { timeout_seconds } => {
            json!({"kind": "timed_out", "timeout_seconds": timeout_seconds})
        }
        Outcome::Cancelled { before_start } => {
            json!({"kind": "cancelled", "before_start": before_start})
        }
        Outcome::SpawnFailed {
            errno,
            error_name,
            message,
        } => json!({
            "kind": "spawn_failed",
            "errno": errno,
            "error_name": error_name,
            "message": message,
        }),
    }
}

fn stream_json(stream: &StreamEvidence) -> Value {
    json!({
        "base64": stream.base64,
        "bytes": stream.bytes,
        "file_bytes": stream.file_bytes,
        "sha256": stream.sha256,
        "capture": match stream.capture {
            Capture::Complete => "complete",
            Capture::Incomplete => "incomplete",
        },
        "reason": stream.reason,
    })
}

fn lifecycle_json(l: &Lifecycle) -> Value {
    json!({
        "leader_pid": l.leader_pid,
        "group_id": l.group_id,
        "leader_start_time": l.leader_start_time,
        "spawn_ms": l.spawn_ms,
        "duration_ms": l.duration_ms,
        "owner_lateness_ms": l.owner_lateness_ms,
        "finalised_after_ms": l.finalised_after_ms,
    })
}

fn termination_json(t: &Termination) -> Value {
    let leader_status = match &t.leader_status {
        LeaderStatus::Exited { exit_code } => json!({"kind": "exited", "exit_code": exit_code}),
        LeaderStatus::Signalled { signal } => json!({"kind": "signalled", "signal": signal}),
        LeaderStatus::Unavailable => json!({"kind": "unavailable"}),
    };
    json!({
        "term_sent_after_ms": t.term_sent_after_ms,
        "kill_sent_after_ms": t.kill_sent_after_ms,
        "leader_status": leader_status,
        "leader_exit_after_ms": t.leader_exit_after_ms,
        "group_empty": t.group_empty.as_str(),
        "group_empty_after_ms": t.group_empty_after_ms,
        "within_bounds": t.within_bounds,
    })
}

/// A random version 4 UUID, the exec id that the Host CLI sends when the
/// caller names none (spec, section "Exec id").
pub(crate) fn new_exec_id() -> String {
    let mut b = [0u8; 16];
    // getrandom 0.2 API: https://docs.rs/getrandom/0.2/getrandom/fn.getrandom.html
    getrandom::getrandom(&mut b).expect("the system random source");
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn emit_exec_dry_run(request: &ExecRequest, mode: OutputMode) {
    match mode {
        OutputMode::Json => {
            print_success(json!({
                "command": request.command,
                "dry_run": true,
                "timeout": request.timeout,
                "detach": request.detach,
            }));
        }
        OutputMode::Text => {
            println!("DRY-RUN: would exec {:?}", request.command);
        }
    }
}

pub async fn run_upload(
    opts: ConnectionOptions,
    local: String,
    remote: String,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        match mode {
            OutputMode::Json => print_success(json!({
                "local": local,
                "remote": remote,
                "dry_run": true,
            })),
            OutputMode::Text => println!("DRY-RUN: would upload {local} → {remote}"),
        }
        return;
    }
    // The client streams the file straight from disk to the agent's raw
    // `application/octet-stream` body — no whole-file buffer here (ADR-0001).
    // `bytes` for the receipt is the streamed size the client reports.
    let client = build_agent_client(&opts, mode);
    match client.upload(&remote, Path::new(&local)).await {
        Ok(bytes) => match mode {
            OutputMode::Json => print_success(json!({
                "local": local,
                "remote": remote,
                "bytes": bytes,
            })),
            OutputMode::Text => println!("Uploaded {local} → {remote}"),
        },
        Err(err) => exit_agent_error(err, mode),
    }
}

pub async fn run_download(
    opts: ConnectionOptions,
    remote: String,
    local: String,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        match mode {
            OutputMode::Json => print_success(json!({
                "remote": remote,
                "local": local,
                "dry_run": true,
            })),
            OutputMode::Text => println!("DRY-RUN: would download {remote} → {local}"),
        }
        return;
    }
    // The client streams the agent's response body into a sibling temp file
    // and atomically renames it into place — bounded memory, and `local` is
    // never left truncated on a failed transfer (ADR-0001). It rejects a
    // short body and bytes past the body (ADR-0026).
    //
    // At SIGINT or SIGTERM the download future is dropped. Its guard removes
    // the temp file, and then the same signal is raised again (spec, section
    // "Host CLI download").
    let client = build_agent_client(&opts, mode);
    let download = client.download(&remote, Path::new(&local));
    let outcome = match until_interrupted(download, Interrupts::install()).await {
        Ended::Done(outcome) => outcome,
        Ended::Interrupted(number) => raise_again(number),
    };
    match outcome {
        Ok(bytes) => match mode {
            OutputMode::Json => print_success(json!({
                "remote": remote,
                "local": local,
                "bytes": bytes,
            })),
            OutputMode::Text => println!("Downloaded {remote} → {local}"),
        },
        Err(err) => exit_agent_error(err, mode),
    }
}
