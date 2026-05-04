//! Per-command handler functions invoked by `main.rs`.
//!
//! Each handler:
//!  1. Resolves the connection options to an `AgentClient`,
//!  2. Calls one or more endpoint methods on the client,
//!  3. Emits text or JSON output per `OutputMode`,
//!  4. Exits with the right §5 code on success or failure.
//!
//! Splitting the work across `agent.rs` and `file.rs` keeps each file
//! focused; the dispatcher in `main.rs` calls into these helpers.

pub mod agent;
pub mod file;

use testanyware_agent_client::{AgentClient, AgentConfig, AgentError};

use crate::output::{exit_code_for, print_error, OutputMode};
use crate::resolve::{resolve_agent, ConnectionOptions, ResolveError, ResolvedAgent};

/// Resolve the connection options to an `AgentClient`. On failure, print
/// the §3.4 error envelope (or text-mode equivalent) and exit.
pub fn build_agent_client(opts: &ConnectionOptions, mode: OutputMode) -> AgentClient {
    let resolved = match resolve_agent(opts) {
        Ok(r) => r,
        Err(err) => exit_resolve_error(err, mode),
    };
    let config = AgentConfig::new(resolved.host, resolved.port);
    match AgentClient::new(config) {
        Ok(c) => c,
        Err(err) => exit_agent_error(err, mode),
    }
}

pub fn build_agent_client_with_endpoint(endpoint: ResolvedAgent, mode: OutputMode) -> AgentClient {
    match AgentClient::new(AgentConfig::new(endpoint.host, endpoint.port)) {
        Ok(c) => c,
        Err(err) => exit_agent_error(err, mode),
    }
}

pub fn exit_resolve_error(err: ResolveError, mode: OutputMode) -> ! {
    let code = err.code();
    let message = err.to_string();
    let remediation = remediation_for(code);
    print_error(
        mode,
        code,
        &message,
        remediation,
        err.details(),
        err.exit_code(),
    );
}

pub fn exit_agent_error(err: AgentError, mode: OutputMode) -> ! {
    let code = err.code();
    let message = err.to_string();
    let details = match &err {
        AgentError::Wire { wire_error, details } => serde_json::json!({
            "wire_error": wire_error,
            "wire_details": details,
        }),
        AgentError::HttpStatus { status, body } => serde_json::json!({
            "http_status": status.as_u16(),
            "body": body,
        }),
        _ => serde_json::Value::Null,
    };
    print_error(
        mode,
        code,
        &message,
        remediation_for(code),
        details,
        exit_code_for(code),
    );
}

fn remediation_for(code: &str) -> Option<&'static str> {
    match code {
        "NO_CONNECTION_SPECIFIED" => Some(
            "Provide --connect <path>, --vm <id>, or --agent <host:port>; or set \
             TESTANYWARE_VM_ID / TESTANYWARE_AGENT. See `docs/reference/connection-spec.md`.",
        ),
        "VM_NOT_FOUND" => Some(
            "Run `testanyware vm list` to see available VMs, or start one with \
             `testanyware vm start`.",
        ),
        "AUTH_REQUIRED" => Some(
            "Grant accessibility permission to the in-VM agent (System Settings \
             → Privacy & Security → Accessibility on macOS).",
        ),
        "CONNECTION_REFUSED" => Some(
            "Check that the VM is running and the agent listens on the expected \
             port (default 8648).",
        ),
        "CONNECTION_TIMEOUT" => Some(
            "Try a longer `--timeout`, or verify the network path to the agent.",
        ),
        "ELEMENT_NOT_FOUND" => Some(
            "Run `testanyware agent snapshot` to see available elements, or relax \
             the --role / --label filter.",
        ),
        "ELEMENT_AMBIGUOUS" => Some(
            "Add `--index <n>` or narrow the filter (e.g. add `--window <name>`).",
        ),
        "WINDOW_NOT_FOUND" => Some(
            "Run `testanyware agent windows` to see available windows.",
        ),
        _ => None,
    }
}
