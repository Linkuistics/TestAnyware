//! HTTP client for the TestAnyware in-VM agent (port 8648).
//!
//! Mirrors the Swift `AgentTCPClient` in
//! `cli/Sources/TestAnywareDriver/Agent/AgentTCPClient.swift`. Endpoints
//! transmit JSON over HTTP/1.1 on the loopback or VM-host bridge.
//! Binary file transfers (`/upload`, `/download`) wrap content as base64
//! inside the JSON body — a quirk of the agent's minimal HTTP parser.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use reqwest::StatusCode;
use serde::Serialize;
use thiserror::Error;

use testanyware_protocol::{
    ActionResponse, DownloadRequest, DownloadResponse, ElementQuery, ErrorResponse, ExecRequest,
    ExecResult, HealthResponse, InspectResponse, SnapshotRequest, SnapshotResponse, UploadRequest,
};

/// Connection parameters for the in-VM agent.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub host: String,
    pub port: u16,
    /// Per-request timeout. The CLI exposes `--timeout` for long-running
    /// `exec` calls; the default keeps short-poll calls responsive.
    pub timeout: Duration,
}

impl AgentConfig {
    pub const DEFAULT_PORT: u16 = 8648;

    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            timeout: Duration::from_secs(30),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

/// Errors produced by the agent client.
///
/// The variants align with the §4 catalogue from the CLI design contract;
/// `code()` returns the stable token the CLI surfaces in `--json` output.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("connection refused: {0}")]
    ConnectionRefused(String),

    #[error("connection timed out: {0}")]
    Timeout(String),

    #[error("connection dropped: {0}")]
    ConnectionDropped(String),

    #[error("HTTP {status}: {body}")]
    HttpStatus { status: StatusCode, body: String },

    #[error("decode failure: {0}")]
    Decode(String),

    /// The agent returned a structured error envelope. `wire_error` is the
    /// raw `error` field as emitted by the agent; the §4.5 mapping table
    /// turns common values into stable §4 codes (`ELEMENT_NOT_FOUND`,
    /// `WINDOW_NOT_FOUND`, etc.). Unmapped strings fall through to
    /// `AGENT_ERROR_UNKNOWN`.
    #[error("agent error: {wire_error}{}", details_suffix(.details))]
    Wire {
        wire_error: String,
        details: Option<String>,
    },

    #[error("internal client error: {0}")]
    Internal(String),
}

fn details_suffix(details: &Option<String>) -> String {
    match details {
        Some(d) => format!(" — {d}"),
        None => String::new(),
    }
}

impl AgentError {
    /// Stable §4 code token surfaced in `--json` output.
    pub fn code(&self) -> &'static str {
        match self {
            AgentError::ConnectionRefused(_) => "CONNECTION_REFUSED",
            AgentError::Timeout(_) => "CONNECTION_TIMEOUT",
            AgentError::ConnectionDropped(_) => "CONNECTION_DROPPED",
            AgentError::HttpStatus { status, .. } if *status == StatusCode::UNAUTHORIZED => {
                "AUTH_REQUIRED"
            }
            AgentError::HttpStatus { .. } => "AGENT_ERROR_UNKNOWN",
            AgentError::Decode(_) => "INTERNAL",
            AgentError::Wire { wire_error, .. } => map_wire_error(wire_error),
            AgentError::Internal(_) => "INTERNAL",
        }
    }
}

/// §4.5 wire-error → CLI code. Unknown strings become
/// `AGENT_ERROR_UNKNOWN`; the wire string is preserved on the
/// `AgentError::Wire` variant for `details.wire_error` exposure.
fn map_wire_error(wire: &str) -> &'static str {
    match wire {
        "not_found" => "ELEMENT_NOT_FOUND",
        "ambiguous" => "ELEMENT_AMBIGUOUS",
        "window_not_found" => "WINDOW_NOT_FOUND",
        "action_unsupported" => "ACTION_UNSUPPORTED",
        "accessibility_unavailable" => "AUTH_REQUIRED",
        "exec_failed" => "EXEC_FAILED",
        "upload_failed" => "UPLOAD_FAILED",
        "download_failed" => "DOWNLOAD_FAILED",
        _ => "AGENT_ERROR_UNKNOWN",
    }
}

impl From<reqwest::Error> for AgentError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            AgentError::Timeout(err.to_string())
        } else if err.is_connect() {
            AgentError::ConnectionRefused(err.to_string())
        } else if err.is_request() || err.is_body() {
            AgentError::ConnectionDropped(err.to_string())
        } else {
            AgentError::Internal(err.to_string())
        }
    }
}

/// Async HTTP client for the in-VM agent.
pub struct AgentClient {
    config: AgentConfig,
    http: reqwest::Client,
}

impl AgentClient {
    pub fn new(config: AgentConfig) -> Result<Self, AgentError> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(AgentError::from)?;
        Ok(Self { config, http })
    }

    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url(), path)
    }

    // -----------------------------------------------------------------
    // Health
    // -----------------------------------------------------------------

    /// Probe `/health`. Returns the agent's structured health body.
    /// A non-2xx response surfaces as `HttpStatus`.
    pub async fn health(&self) -> Result<HealthResponse, AgentError> {
        let response = self.http.get(self.url("/health")).send().await?;
        check_status(&response)?;
        let bytes = response.bytes().await?;
        decode_body(&bytes)
    }

    // -----------------------------------------------------------------
    // Accessibility queries
    // -----------------------------------------------------------------

    pub async fn windows(&self) -> Result<SnapshotResponse, AgentError> {
        self.post_json("/windows", &serde_json::json!({})).await
    }

    pub async fn snapshot(
        &self,
        request: &SnapshotRequest,
    ) -> Result<SnapshotResponse, AgentError> {
        self.post_json("/snapshot", request).await
    }

    pub async fn inspect(&self, query: &ElementQuery) -> Result<InspectResponse, AgentError> {
        self.post_json("/inspect", query).await
    }

    pub async fn press(&self, query: &ElementQuery) -> Result<ActionResponse, AgentError> {
        self.post_json("/press", query).await
    }

    // -----------------------------------------------------------------
    // System: exec / upload / download
    // -----------------------------------------------------------------

    pub async fn exec(&self, request: &ExecRequest) -> Result<ExecResult, AgentError> {
        // Per Swift parity: give the URLSession layer a few seconds of
        // headroom past the agent's own deadline so a long-but-progressing
        // exec is not aborted client-side.
        let http_timeout = Duration::from_secs(request.timeout.max(0) as u64 + 10);
        let response = self
            .http
            .post(self.url("/exec"))
            .timeout(http_timeout)
            .json(request)
            .send()
            .await?;
        check_status(&response)?;
        let bytes = response.bytes().await?;
        decode_body(&bytes)
    }

    pub async fn upload(&self, path: &str, content: &[u8]) -> Result<(), AgentError> {
        let body = UploadRequest {
            path: path.to_string(),
            content: BASE64_STANDARD.encode(content),
        };
        let action: ActionResponse = self.post_json("/upload", &body).await?;
        if action.success {
            Ok(())
        } else {
            Err(AgentError::Wire {
                wire_error: "upload_failed".into(),
                details: action.message,
            })
        }
    }

    pub async fn download(&self, path: &str) -> Result<Vec<u8>, AgentError> {
        let body = DownloadRequest {
            path: path.to_string(),
        };
        let response: DownloadResponse = self.post_json("/download", &body).await?;
        BASE64_STANDARD
            .decode(response.content.as_bytes())
            .map_err(|e| AgentError::Decode(format!("invalid base64 from /download: {e}")))
    }

    // -----------------------------------------------------------------
    // Transport
    // -----------------------------------------------------------------

    async fn post_json<B, R>(&self, path: &str, body: &B) -> Result<R, AgentError>
    where
        B: Serialize + ?Sized,
        R: serde::de::DeserializeOwned,
    {
        let response = self
            .http
            .post(self.url(path))
            .json(body)
            .send()
            .await?;
        check_status(&response)?;
        let bytes = response.bytes().await?;
        decode_body(&bytes)
    }
}

/// Promote a non-2xx response to an `AgentError`. The body is consumed for
/// diagnostic context; if it parses as the structured error envelope
/// (`{error, details}`), that variant is preferred.
fn check_status(response: &reqwest::Response) -> Result<(), AgentError> {
    if response.status().is_success() {
        Ok(())
    } else {
        // The `bytes()` call would consume the response, so we stash the
        // status now and let the caller consume the body to attach detail.
        Err(AgentError::HttpStatus {
            status: response.status(),
            body: String::new(),
        })
    }
}

fn decode_body<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, AgentError> {
    // Fast path: the response is the expected shape.
    match serde_json::from_slice::<T>(bytes) {
        Ok(v) => Ok(v),
        Err(primary) => {
            // Slow path: maybe the agent emitted the structured error
            // envelope despite returning HTTP 200. Some agent versions
            // wrap upload/download failures this way (see Swift's parse
            // of upload `ActionResponse{success:false}`).
            if let Ok(err) = serde_json::from_slice::<ErrorResponse>(bytes) {
                Err(AgentError::Wire {
                    wire_error: err.error,
                    details: err.details,
                })
            } else {
                Err(AgentError::Decode(primary.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let cfg = AgentConfig::new("192.168.64.2", AgentConfig::DEFAULT_PORT);
        assert_eq!(cfg.base_url(), "http://192.168.64.2:8648");
        assert_eq!(cfg.timeout, Duration::from_secs(30));
    }

    #[test]
    fn client_builds() {
        let cfg = AgentConfig::new("localhost", 8648);
        let client = AgentClient::new(cfg).expect("reqwest client should build");
        assert_eq!(client.config().host, "localhost");
    }

    #[test]
    fn wire_error_mapping_table() {
        assert_eq!(map_wire_error("not_found"), "ELEMENT_NOT_FOUND");
        assert_eq!(map_wire_error("ambiguous"), "ELEMENT_AMBIGUOUS");
        assert_eq!(map_wire_error("window_not_found"), "WINDOW_NOT_FOUND");
        assert_eq!(map_wire_error("action_unsupported"), "ACTION_UNSUPPORTED");
        assert_eq!(map_wire_error("accessibility_unavailable"), "AUTH_REQUIRED");
        assert_eq!(map_wire_error("exec_failed"), "EXEC_FAILED");
        assert_eq!(map_wire_error("upload_failed"), "UPLOAD_FAILED");
        assert_eq!(map_wire_error("download_failed"), "DOWNLOAD_FAILED");
        assert_eq!(map_wire_error("anything-else"), "AGENT_ERROR_UNKNOWN");
    }

    #[test]
    fn error_code_for_wire_variant() {
        let err = AgentError::Wire {
            wire_error: "not_found".into(),
            details: Some("no Save button".into()),
        };
        assert_eq!(err.code(), "ELEMENT_NOT_FOUND");
    }

    #[test]
    fn error_code_for_unauthorized() {
        let err = AgentError::HttpStatus {
            status: StatusCode::UNAUTHORIZED,
            body: String::new(),
        };
        assert_eq!(err.code(), "AUTH_REQUIRED");
    }

    #[test]
    fn decode_body_falls_back_to_error_envelope() {
        // If the payload is shaped like `{error, details}`, we prefer the
        // `Wire` variant over a generic `Decode` failure.
        let bytes = br#"{"error":"download_failed","details":"file not found"}"#;
        let result: Result<DownloadResponse, AgentError> = decode_body(bytes);
        match result {
            Err(AgentError::Wire { wire_error, details }) => {
                assert_eq!(wire_error, "download_failed");
                assert_eq!(details.as_deref(), Some("file not found"));
            }
            other => panic!("expected Wire error, got {other:?}"),
        }
    }
}
