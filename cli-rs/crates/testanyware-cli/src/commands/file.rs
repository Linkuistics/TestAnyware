//! `file {exec|upload|download}` handlers.

use std::io::Write;
use std::path::Path;

use serde_json::json;

use testanyware_protocol::ExecRequest;

use crate::commands::{build_agent_client, exit_agent_error};
use crate::output::{print_error, print_success, OutputMode};
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
    let local_path = Path::new(&local);
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
    let bytes = match std::fs::read(local_path) {
        Ok(b) => b,
        Err(source) => {
            print_error(
                mode,
                "IO_ERROR",
                &format!("failed to read {local}: {source}"),
                Some("check the local path and permissions"),
                json!({ "path": local }),
                crate::output::exit_code_for("IO_ERROR"),
            );
        }
    };
    let client = build_agent_client(&opts, mode);
    match client.upload(&remote, &bytes).await {
        Ok(()) => match mode {
            OutputMode::Json => print_success(json!({
                "local": local,
                "remote": remote,
                "bytes": bytes.len(),
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
    let client = build_agent_client(&opts, mode);
    let bytes = match client.download(&remote).await {
        Ok(b) => b,
        Err(err) => exit_agent_error(err, mode),
    };
    if let Err(source) = std::fs::write(&local, &bytes) {
        print_error(
            mode,
            "IO_ERROR",
            &format!("failed to write {local}: {source}"),
            Some("check the local path and permissions"),
            json!({ "path": local }),
            crate::output::exit_code_for("IO_ERROR"),
        );
    }
    match mode {
        OutputMode::Json => print_success(json!({
            "remote": remote,
            "local": local,
            "bytes": bytes.len(),
        })),
        OutputMode::Text => println!("Downloaded {remote} → {local}"),
    }
}
