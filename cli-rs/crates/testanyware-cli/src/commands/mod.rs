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
pub mod doctor;
pub mod file;
pub mod input;
pub mod menu_bar;
pub mod record;
pub mod screen;
pub mod viewer;
pub mod vm;
pub mod window;

use testanyware_agent_client::{AgentClient, AgentConfig, AgentError};
use testanyware_rfb::{RfbConnection, RfbError};
use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio::net::TcpStream;

use crate::output::{exit_code_for, print_error, OutputMode};
use crate::resolve::{resolve_agent, ConnectionOptions, ResolveError, ResolvedAgent, ResolvedVnc};

/// Open an RFB connection to a resolved VNC endpoint and apply the HiDPI
/// logical target the spec carried (ADR-0016 D2), if any.
///
/// Every VNC consumer (`screen *`, `input *`, `record`) funnels its connect
/// through here so the scale-aware surface is wired uniformly: a `@2x` VM's
/// spec carries a [`logical`](ResolvedVnc::logical) target, which k5's
/// connection downsamples reads to and scales pointer writes from. On the
/// default 1× path `logical` is `None` and this is a plain connect.
pub(crate) async fn connect_vnc(
    endpoint: &ResolvedVnc,
) -> Result<RfbConnection<BufReader<TcpStream>>, RfbError> {
    let mut conn = RfbConnection::connect(
        &endpoint.host,
        endpoint.port,
        endpoint.password.as_deref().map(str::as_bytes),
    )
    .await?;
    apply_logical_target(&mut conn, endpoint);
    Ok(conn)
}

/// Apply the resolved HiDPI logical target to an already-open connection
/// (ADR-0016 D2), and warn if `@2x` was requested but the framebuffer came back
/// 1× — HiDPI did not take (a 1× host, or it didn't engage). The auto-detected
/// `scale == 1` keeps that case *correct* (everything operates in 1×), never
/// silently wrong; the warning makes it visible. Generic over the transport so
/// the viewer (whose connect is cancellable, not via [`connect_vnc`]) shares it.
pub(crate) fn apply_logical_target<T: AsyncRead + AsyncWrite + Unpin>(
    conn: &mut RfbConnection<T>,
    endpoint: &ResolvedVnc,
) {
    let Some((lw, lh)) = endpoint.logical else { return };
    conn.set_logical_target(lw, lh);
    if conn.scale() == 1 {
        let (pw, ph) = conn.physical_framebuffer_size();
        eprintln!(
            "warning: --display {lw}x{lh}@2x requested but the framebuffer came back \
             {pw}x{ph} (1× scale) — HiDPI did not take; operating in 1× {pw}x{ph}. \
             A 1× host needs the deferred deterministic mechanism (ADR-0016)."
        );
    }
}

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
    let raw = err.to_string();
    let message = friendly_message_for(code, &err).unwrap_or(raw);
    let details = match &err {
        AgentError::Wire { wire_error, details } => serde_json::json!({
            "wire_error": wire_error,
            "wire_details": details,
        }),
        AgentError::HttpStatus { status, body } => serde_json::json!({
            "http_status": status.as_u16(),
            "body": body,
        }),
        AgentError::LocalIo { path, .. } => serde_json::json!({ "path": path }),
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

/// §4.5 friendly-message generator. Translates the §4 CLI code into a
/// human-readable message so the raw snake_case wire token (e.g.
/// `not_found`) does not leak into text-mode output. Returns `None` for
/// codes without a curated message — the caller falls back to the
/// `AgentError` Display impl, which carries diagnostic context (HTTP
/// status, decode-error specifics) the curated messages would lose.
///
/// The wire token is preserved in the JSON envelope's `details.wire_error`
/// regardless, per contract §4.5 fallback rule.
pub fn friendly_message_for(code: &str, err: &AgentError) -> Option<String> {
    let detail = match err {
        AgentError::Wire { details, .. } => details.as_deref(),
        _ => None,
    };
    let with_detail = |base: &str| match detail {
        Some(d) if !d.is_empty() => format!("{base}: {d}"),
        _ => base.to_string(),
    };
    match code {
        "ELEMENT_NOT_FOUND" => Some(with_detail(
            "Element not found",
        )),
        "ELEMENT_AMBIGUOUS" => Some(with_detail(
            "Multiple elements matched the query",
        )),
        "WINDOW_NOT_FOUND" => Some(with_detail(
            "No window matched the --window filter",
        )),
        "ACTION_UNSUPPORTED" => Some(with_detail(
            "Element does not support the requested action",
        )),
        "AUTH_REQUIRED" => Some(with_detail(
            "Accessibility permission is not granted on the target VM",
        )),
        "EXEC_FAILED" => Some(with_detail(
            "Process failed to spawn on the target VM",
        )),
        "UPLOAD_FAILED" => Some(with_detail(
            "Upload failed on the target VM",
        )),
        "DOWNLOAD_FAILED" => Some(with_detail(
            "Download failed on the target VM",
        )),
        "AGENT_INVALID_JSON" => Some(with_detail(
            "Agent rejected the request body as malformed JSON \
             (this indicates a CLI bug — please report it)",
        )),
        "AGENT_ERROR_UNKNOWN" => match err {
            AgentError::Wire { wire_error, .. } => Some(with_detail(&format!(
                "Agent reported an unrecognised error ({wire_error})"
            ))),
            _ => None,
        },
        _ => None,
    }
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
        "IO_ERROR" => Some("Check the local path and permissions."),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(code: &str, details: Option<&str>) -> AgentError {
        AgentError::Wire {
            wire_error: code.to_string(),
            details: details.map(|s| s.to_string()),
        }
    }

    #[test]
    fn friendly_message_translates_every_canonical_4_5_token() {
        // §4.5 of the contract: the wire-token → CLI-code mapping is
        // 1:1, and user-facing text never carries the raw snake_case
        // token. This test guards both halves.
        let cases = [
            ("not_found", "ELEMENT_NOT_FOUND", "Element not found"),
            ("ambiguous", "ELEMENT_AMBIGUOUS", "Multiple elements matched the query"),
            ("window_not_found", "WINDOW_NOT_FOUND", "No window matched the --window filter"),
            (
                "action_unsupported",
                "ACTION_UNSUPPORTED",
                "Element does not support the requested action",
            ),
            (
                "accessibility_unavailable",
                "AUTH_REQUIRED",
                "Accessibility permission is not granted on the target VM",
            ),
            ("exec_failed", "EXEC_FAILED", "Process failed to spawn on the target VM"),
            ("upload_failed", "UPLOAD_FAILED", "Upload failed on the target VM"),
            ("download_failed", "DOWNLOAD_FAILED", "Download failed on the target VM"),
            (
                "invalid_json",
                "AGENT_INVALID_JSON",
                "Agent rejected the request body as malformed JSON \
                 (this indicates a CLI bug — please report it)",
            ),
        ];
        for (token, expected_code, expected_msg) in cases {
            let err = wire(token, None);
            assert_eq!(err.code(), expected_code, "wire {token} → code");
            let friendly = friendly_message_for(err.code(), &err)
                .unwrap_or_else(|| panic!("expected friendly message for {token}"));
            assert_eq!(friendly, expected_msg, "wire {token} → friendly message");
            // Critical: the user-visible message never echoes the raw
            // wire token.
            assert!(
                !friendly.contains(token),
                "friendly message for {token} leaked the raw wire token: {friendly:?}"
            );
        }
    }

    #[test]
    fn friendly_message_appends_details_when_present() {
        let err = wire("not_found", Some("no Save button"));
        let friendly = friendly_message_for(err.code(), &err).unwrap();
        assert_eq!(friendly, "Element not found: no Save button");
    }

    #[test]
    fn friendly_message_omits_empty_details_separator() {
        let err = wire("not_found", Some(""));
        let friendly = friendly_message_for(err.code(), &err).unwrap();
        assert_eq!(friendly, "Element not found");
    }

    #[test]
    fn friendly_message_for_unknown_wire_token_includes_raw_token() {
        // Per §4.5 fallback: unrecognised wire strings surface as
        // AGENT_ERROR_UNKNOWN with the wire string preserved in
        // details.wire_error. The friendly message also names the raw
        // token so the user can search for it; this is the *only*
        // canonical user-visible surface where the raw token appears.
        // Use a token that is deliberately not in the §4.5 mapping
        // table — adding a new canonical token would silently break
        // this test if it picked an existing or soon-to-exist mapping.
        let unknown = "totally_made_up_token";
        let err = wire(unknown, None);
        assert_eq!(err.code(), "AGENT_ERROR_UNKNOWN");
        let friendly = friendly_message_for(err.code(), &err).unwrap();
        assert!(
            friendly.contains(unknown),
            "AGENT_ERROR_UNKNOWN friendly message must surface the raw \
             token for diagnostics: got {friendly:?}"
        );
    }

    #[test]
    fn friendly_message_returns_none_for_non_curated_codes() {
        // Codes outside §4.5 (transport-layer, INTERNAL, USAGE_ERROR,
        // etc.) keep the AgentError Display impl, which carries
        // context the curated messages would discard.
        let err = AgentError::ConnectionRefused("dial tcp: connection refused".into());
        assert_eq!(err.code(), "CONNECTION_REFUSED");
        assert!(friendly_message_for(err.code(), &err).is_none());

        // Decode failures aren't curated either: the parse error
        // (column, expected token, etc.) is the diagnostic context the
        // user needs.
        let err = AgentError::Decode("expected `,`".into());
        assert_eq!(err.code(), "INTERNAL");
        assert!(friendly_message_for(err.code(), &err).is_none());
    }
}
