//! `file {exec|upload|download}` handlers.

use std::io::Write;
use std::path::Path;

use serde_json::{json, Value};

use testanyware_protocol::exec_record::{
    Capture, LeaderStatus, Lifecycle, Outcome, StreamEvidence, Termination,
};
use testanyware_protocol::{ExecRequest, ExecResult};

use crate::commands::{build_agent_client, exit_agent_error};
use crate::output::{print_success, OutputMode};
use crate::resolve::ConnectionOptions;

pub struct ExecArgs {
    pub command: String,
    pub timeout: i64,
    pub detach: bool,
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
/// An attached command sends a random exec id. A record that breaks a validity
/// rule fails with `EXEC_RECORD_INVALID` in both modes. A detached command has
/// no record, so it sends no id and its output does not change.
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

    let request = ExecRequest {
        id: (!request.detach).then(new_exec_id),
        ..request
    };
    let client = build_agent_client(&opts, mode);
    match client.exec(&request).await {
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
        Some(outcome) => json!({
            "exec_id": result.id,
            "record": "agent",
            "legacy_projection": result.legacy_projection.map(|p| p.as_str()),
            "outcome": outcome_json(outcome),
            "evidence": result.evidence.as_ref().map(|e| json!({
                "stdout": stream_json(&e.stdout),
                "stderr": stream_json(&e.stderr),
            })),
            "lifecycle": result.lifecycle.as_ref().map(lifecycle_json),
            "termination": result.termination.as_ref().map(termination_json),
            "remaining_group": result.remaining_group.map(|g| g.as_str()),
        }),
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
    // never left truncated on a failed transfer (ADR-0001).
    let client = build_agent_client(&opts, mode);
    match client.download(&remote, Path::new(&local)).await {
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
