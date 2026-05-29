//! HTTP client for the TestAnyware in-VM agent (port 8648).
//!
//! Mirrors the Swift `AgentTCPClient` in
//! `cli/Sources/TestAnywareDriver/Agent/AgentTCPClient.swift`. Endpoints
//! transmit JSON over HTTP/1.1 on the loopback or VM-host bridge.
//! Binary file transfers (`/upload`, `/download`) stream raw bytes as
//! `application/octet-stream` with the path in a percent-encoded `?path=`
//! query parameter (ADR-0001) — no base64, no whole-file buffering on
//! either end.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::StreamExt;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::StatusCode;
use serde::Serialize;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

use testanyware_protocol::{
    ActionResponse, ElementQuery, ErrorResponse, ExecRequest, ExecResult, HealthResponse,
    InspectResponse, SnapshotRequest, SnapshotResponse,
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

    /// A local filesystem operation failed: reading the upload source, or
    /// creating/writing/renaming the download destination. Distinct from
    /// agent-side failures — surfaces as the CLI `IO_ERROR` code. `path` is
    /// the local path involved so callers can attach it to `--json` details.
    #[error("{reason}")]
    LocalIo { path: String, reason: String },

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
            AgentError::LocalIo { .. } => "IO_ERROR",
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
        "invalid_json" => "AGENT_INVALID_JSON",
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

    /// Build a file-transfer URL with the guest path in a percent-encoded
    /// `?path=` query parameter. We encode every non-alphanumeric byte as
    /// `%XX` (space → `%20`, never `+`) so the single wire form decodes
    /// identically across all three agent stacks — Hummingbird and ASP.NET
    /// parse the query per RFC 3986, while Python's `parse_qs` would read a
    /// literal `+` as a space. `reqwest`'s own `.query()` (form-encoding)
    /// would emit `+` for spaces and break that parity.
    fn file_url(&self, endpoint: &str, guest_path: &str) -> String {
        let encoded = utf8_percent_encode(guest_path, NON_ALPHANUMERIC);
        format!("{}{}?path={}", self.config.base_url(), endpoint, encoded)
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

    /// Stream `local` to the agent's `remote` path (ADR-0001). The file is
    /// sent as a raw `application/octet-stream` body with the destination in
    /// a percent-encoded `?path=` query parameter; nothing buffers the whole
    /// file. Returns the number of bytes sent (the local file's size).
    pub async fn upload(&self, remote: &str, local: &Path) -> Result<u64, AgentError> {
        let file = tokio::fs::File::open(local)
            .await
            .map_err(|e| local_io(local, format!("failed to read {}: {e}", local.display())))?;
        let len = file
            .metadata()
            .await
            .map_err(|e| local_io(local, format!("failed to stat {}: {e}", local.display())))?
            .len();

        let body = reqwest::Body::wrap_stream(ReaderStream::new(file));
        let response = self
            .http
            .post(self.file_url("/upload", remote))
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            // Advertise the length explicitly. Without it, a streamed body is
            // sent `Transfer-Encoding: chunked`, which Hummingbird and Kestrel
            // decode but Python's `http.server` (the Linux agent) does not — it
            // reads only `Content-Length` and would silently write a 0-byte
            // file. We know the size, so frame the request with it (ADR-0001).
            .header(reqwest::header::CONTENT_LENGTH, len)
            .body(body)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(error_from_response(response).await);
        }
        let bytes = response.bytes().await?;
        let action: ActionResponse = decode_body(&bytes)?;
        if action.success {
            Ok(len)
        } else {
            Err(AgentError::Wire {
                wire_error: "upload_failed".into(),
                details: action.message,
            })
        }
    }

    /// Stream the agent's `remote` path into `local` (ADR-0001). The response
    /// body is consumed chunk-by-chunk into a sibling temp file, which is
    /// atomically renamed into place only on a complete transfer; any error
    /// unlinks the temp file, so `local` is never left truncated. Returns the
    /// number of bytes written.
    pub async fn download(&self, remote: &str, local: &Path) -> Result<u64, AgentError> {
        let response = self
            .http
            .post(self.file_url("/download", remote))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(error_from_response(response).await);
        }

        let (temp_path, mut temp_file) = create_temp_sibling(local).await?;
        let outcome = stream_to_file(response, &mut temp_file, &temp_path).await;
        // Drop the handle before rename/unlink so all bytes are flushed and,
        // on Windows, the file is not still open during the rename.
        drop(temp_file);
        match outcome {
            Ok(written) => {
                tokio::fs::rename(&temp_path, local).await.map_err(|e| {
                    local_io(
                        local,
                        format!("failed to finalize {}: {e}", local.display()),
                    )
                })?;
                Ok(written)
            }
            Err(err) => {
                let _ = tokio::fs::remove_file(&temp_path).await;
                Err(err)
            }
        }
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

/// Consume a non-2xx response into an `AgentError`. A structured
/// `{error, details}` envelope (how the agents report `upload_failed` /
/// `download_failed`) is preferred; otherwise the raw body is attached to an
/// `HttpStatus` for diagnostics.
async fn error_from_response(response: reqwest::Response) -> AgentError {
    let status = response.status();
    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => return AgentError::from(e),
    };
    match serde_json::from_slice::<ErrorResponse>(&bytes) {
        Ok(err) => AgentError::Wire {
            wire_error: err.error,
            details: err.details,
        },
        Err(_) => AgentError::HttpStatus {
            status,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        },
    }
}

/// Construct a `LocalIo` error for `path` with a human-readable `reason`.
fn local_io(path: &Path, reason: String) -> AgentError {
    AgentError::LocalIo {
        path: path.display().to_string(),
        reason,
    }
}

/// Create a uniquely-named temp file in the destination's own directory, so
/// the later rename stays on one filesystem and is atomic. The leading dot
/// keeps it out of casual `ls`; pid + a process-local counter avoid
/// collisions between concurrent downloads.
async fn create_temp_sibling(dest: &Path) -> Result<(PathBuf, tokio::fs::File), AgentError> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = dest.parent().filter(|p| !p.as_os_str().is_empty());
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_string());
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let temp_name = format!(".{name}.testanyware-{}.{seq}.tmp", std::process::id());
    let temp_path = match dir {
        Some(d) => d.join(temp_name),
        None => PathBuf::from(temp_name),
    };
    let file = tokio::fs::File::create(&temp_path).await.map_err(|e| {
        local_io(
            &temp_path,
            format!("failed to create temp file {}: {e}", temp_path.display()),
        )
    })?;
    Ok((temp_path, file))
}

/// Drain the response body into `file`, returning the byte count. HTTP/stream
/// errors map to transport `AgentError`s; local write failures map to
/// `LocalIo` against `temp_path`.
async fn stream_to_file(
    response: reqwest::Response,
    file: &mut tokio::fs::File,
    temp_path: &Path,
) -> Result<u64, AgentError> {
    let mut written: u64 = 0;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await.map_err(|e| {
            local_io(
                temp_path,
                format!("failed to write {}: {e}", temp_path.display()),
            )
        })?;
        written += chunk.len() as u64;
    }
    file.flush().await.map_err(|e| {
        local_io(
            temp_path,
            format!("failed to flush {}: {e}", temp_path.display()),
        )
    })?;
    Ok(written)
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
        assert_eq!(map_wire_error("invalid_json"), "AGENT_INVALID_JSON");
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
        let result: Result<ActionResponse, AgentError> = decode_body(bytes);
        match result {
            Err(AgentError::Wire { wire_error, details }) => {
                assert_eq!(wire_error, "download_failed");
                assert_eq!(details.as_deref(), Some("file not found"));
            }
            other => panic!("expected Wire error, got {other:?}"),
        }
    }

    #[test]
    fn file_url_percent_encodes_path_with_pct20_not_plus() {
        let cfg = AgentConfig::new("10.0.0.5", 8648);
        let client = AgentClient::new(cfg).expect("client builds");
        // Space → %20 (never +), and a literal + → %2B, so the single wire
        // form decodes identically across Hummingbird / ASP.NET / parse_qs.
        let url = client.file_url("/upload", "/tmp/my docs/a+b.bin");
        assert_eq!(
            url,
            "http://10.0.0.5:8648/upload?path=%2Ftmp%2Fmy%20docs%2Fa%2Bb%2Ebin"
        );
        assert!(!url.contains('+'));
    }
}
