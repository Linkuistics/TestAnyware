//! `file {exec|upload|download}` handlers.

use std::io::Write;
use std::path::Path;

use serde_json::json;

use testanyware_protocol::ExecRequest;

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
/// own exit code is the in-VM process's exit code.
/// JSON mode: emit a single envelope; `details.exit_code` carries the
/// in-VM exit, the binary itself exits 0 on a clean spawn (per §10.1
/// gap-report and §5 sub-process exit-code rule).
pub async fn run_exec(opts: ConnectionOptions, args: ExecArgs, mode: OutputMode, dry_run: bool) {
    let request = ExecRequest {
        command: args.command,
        timeout: args.timeout,
        detach: args.detach,
    };
    if dry_run {
        emit_exec_dry_run(&request, mode);
        return;
    }

    let client = build_agent_client(&opts, mode);
    match client.exec(&request).await {
        Ok(result) => match mode {
            OutputMode::Json => {
                print_success(json!({
                    "command": request.command,
                    "exit_code": result.exit_code,
                    "stdout": result.stdout,
                    "stderr": result.stderr,
                    "timed_out": result.timed_out.unwrap_or(false),
                }));
            }
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
