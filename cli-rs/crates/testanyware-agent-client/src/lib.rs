//! HTTP client for the TestAnyware in-VM agent (port 8648).
//!
//! Mirrors the Swift `AgentTCPClient` in
//! `cli/Sources/TestAnywareDriver/Agent/AgentTCPClient.swift`. Endpoints
//! transmit JSON over HTTP/1.1 on the loopback or VM-host bridge.
//! Binary file transfers (`/upload`, `/download`) stream raw bytes as
//! `application/octet-stream` with the path in a percent-encoded `?path=`
//! query parameter (ADR-0001) — no base64, no whole-file buffering on
//! either end.
//!
//! The client speaks HTTP/1.1 through hyper's client connection over any
//! byte stream (ADR-0004). The [`AgentConnector`] seam supplies the stream:
//! a direct TCP connection by default, or a guest stream from the
//! TestAnyware app.

mod transport;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures_util::TryStreamExt;
use http::header::{ACCEPT, CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, HOST};
use http::{Method, Request, Response};
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper_util::rt::TokioIo;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Serialize;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio_util::io::ReaderStream;

pub use http::StatusCode;
use transport::{AbortOnDrop, Deadline};
pub use transport::{
    AgentConnector, AgentStream, BoxedStream, ConnectFuture, ConnectorFailure, TcpConnector,
};

use testanyware_protocol::{
    ActionResponse, ElementQuery, ErrorResponse, ExecRequest, ExecResult, HealthResponse,
    InspectResponse, SetValueRequest, SnapshotRequest, SnapshotResponse, WaitRequest,
    WindowMoveRequest, WindowResizeRequest, WindowTarget,
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
    /// Any failure to establish the connection (refused, unreachable,
    /// ...) — the variant name predates the split. `message` is the whole
    /// `source()` chain, so it carries the OS cause; `os_errno` is the raw
    /// errno found in that chain, so a caller can tell "refused" from
    /// "no route to host" without parsing text.
    #[error("connection refused: {message}")]
    ConnectionRefused {
        message: String,
        os_errno: Option<i32>,
    },

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

    /// The connector failed with its own structured error, and the client keeps its code,
    /// remedy and details. The TestAnyware app reports a failed dial this way.
    #[error("{}", .0.message)]
    Connector(ConnectorFailure),

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
    pub fn code(&self) -> &str {
        match self {
            AgentError::Connector(failure) => &failure.code,
            AgentError::ConnectionRefused { .. } => "CONNECTION_REFUSED",
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

/// A failed dial. The message names the endpoint and carries the OS cause. A connector's own
/// structured failure passes through unchanged.
fn dial_error(host: &str, port: u16, err: &std::io::Error) -> AgentError {
    if let Some(failure) = err
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<ConnectorFailure>())
    {
        return AgentError::Connector(failure.clone());
    }
    AgentError::ConnectionRefused {
        message: format!("failed to connect to {host}:{port}: {}", error_chain(err)),
        os_errno: os_errno_in_chain(err),
    }
}

/// A connection that failed after it opened: a hyper error, or a body that
/// ended early.
fn dropped(err: &(dyn std::error::Error + 'static)) -> AgentError {
    AgentError::ConnectionDropped(error_chain(err))
}

/// `err` and every `source()` beneath it, joined with `: `. A wrapper's own
/// `Display` often names only its layer; the cause (for a connect failure,
/// `... (os error N)`) lives below it.
fn error_chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = vec![err.to_string()];
    let mut source = err.source();
    while let Some(cause) = source {
        parts.push(cause.to_string());
        source = cause.source();
    }
    parts.join(": ")
}

/// The raw OS errno of the first `io::Error` in `err`'s `source()` chain.
fn os_errno_in_chain(err: &(dyn std::error::Error + 'static)) -> Option<i32> {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(e) = current {
        if let Some(code) = e
            .downcast_ref::<std::io::Error>()
            .and_then(|io| io.raw_os_error())
        {
            return Some(code);
        }
        current = e.source();
    }
    None
}

/// The request body type: a buffered JSON payload or a streamed file.
type RequestBody = UnsyncBoxBody<Bytes, std::io::Error>;

/// Async HTTP client for the in-VM agent.
///
/// Each request opens its own connection through the [`AgentConnector`] and
/// closes it after the response. The client never keeps a connection for a
/// second request.
pub struct AgentClient {
    config: AgentConfig,
    connector: Arc<dyn AgentConnector>,
}

/// A response whose headers have arrived, with the guard that keeps its
/// connection alive until the body is read.
struct Exchange {
    response: Response<Incoming>,
    _connection: AbortOnDrop,
}

impl AgentClient {
    /// A client that dials the configured host and port over TCP.
    pub fn new(config: AgentConfig) -> Result<Self, AgentError> {
        let connector = Arc::new(TcpConnector::new(config.host.clone(), config.port));
        Ok(Self::with_connector(config, connector))
    }

    /// A client that gets each connection from `connector`. `config.host` and
    /// `config.port` then only name the agent in the `Host` header and in
    /// messages.
    pub fn with_connector(config: AgentConfig, connector: Arc<dyn AgentConnector>) -> Self {
        Self { config, connector }
    }

    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url(), path)
    }

    /// Build a file-transfer URL with the guest path in a percent-encoded
    /// `?path=` query parameter. We encode every non-alphanumeric byte as
    /// `%XX` (space → `%20`, never `+`) so the single wire form decodes
    /// identically across all three agent stacks — Hummingbird and ASP.NET
    /// parse the query per RFC 3986, while Python's `parse_qs` would read a
    /// literal `+` as a space. Form-encoding would emit `+` for spaces and
    /// break that parity.
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
        let deadline = Deadline::after(self.config.timeout);
        let exchange = self
            .open(
                &deadline,
                Method::GET,
                &self.url("/health"),
                None,
                empty_body(),
            )
            .await?;
        check_status(&exchange.response)?;
        let bytes = read_body(&deadline, exchange).await?;
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
    // Accessibility actions (pure HTTP — no VNC). Parity with the Swift
    // `AgentTCPClient` action methods; each posts a small JSON body and
    // decodes the shared `ActionResponse`.
    // -----------------------------------------------------------------

    pub async fn set_value(&self, request: &SetValueRequest) -> Result<ActionResponse, AgentError> {
        self.post_json("/set-value", request).await
    }

    pub async fn focus(&self, query: &ElementQuery) -> Result<ActionResponse, AgentError> {
        self.post_json("/focus", query).await
    }

    /// Poll the agent until accessibility is ready (optionally scoped to a
    /// window), bounded by the request's `timeout`. Mirrors Swift's
    /// `agent wait`.
    pub async fn wait(&self, request: &WaitRequest) -> Result<ActionResponse, AgentError> {
        self.post_json("/wait", request).await
    }

    // -----------------------------------------------------------------
    // Window management actions (pure HTTP — no VNC).
    // -----------------------------------------------------------------

    pub async fn window_focus(&self, request: &WindowTarget) -> Result<ActionResponse, AgentError> {
        self.post_json("/window-focus", request).await
    }

    pub async fn window_resize(
        &self,
        request: &WindowResizeRequest,
    ) -> Result<ActionResponse, AgentError> {
        self.post_json("/window-resize", request).await
    }

    pub async fn window_move(
        &self,
        request: &WindowMoveRequest,
    ) -> Result<ActionResponse, AgentError> {
        self.post_json("/window-move", request).await
    }

    pub async fn window_close(&self, request: &WindowTarget) -> Result<ActionResponse, AgentError> {
        self.post_json("/window-close", request).await
    }

    pub async fn window_minimize(
        &self,
        request: &WindowTarget,
    ) -> Result<ActionResponse, AgentError> {
        self.post_json("/window-minimize", request).await
    }

    // -----------------------------------------------------------------
    // System: exec / upload / download
    // -----------------------------------------------------------------

    pub async fn exec(&self, request: &ExecRequest) -> Result<ExecResult, AgentError> {
        // Per Swift parity: give the transport a few seconds of headroom past
        // the agent's own deadline so a long-but-progressing exec is not
        // aborted client-side. This replaces the client's timeout for the
        // one request.
        let timeout = Duration::from_secs(request.timeout.max(0) as u64 + 10);
        self.post_json_within("/exec", request, timeout).await
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
        self.upload_reader(remote, file, len).await
    }

    /// Stream `len` bytes from `reader` to the agent's `remote` path. The
    /// body is read in `UPLOAD_CHUNK` pieces as the connection takes them.
    async fn upload_reader<R>(&self, remote: &str, reader: R, len: u64) -> Result<u64, AgentError>
    where
        R: AsyncRead + Send + Unpin + 'static,
    {
        let chunks = ReaderStream::with_capacity(reader, UPLOAD_CHUNK).map_ok(Frame::data);
        let body = StreamBody::new(chunks).boxed_unsync();

        let deadline = Deadline::after(self.config.timeout);
        // Advertise the length explicitly. Without it, a streamed body is
        // sent `Transfer-Encoding: chunked`, which Hummingbird and Kestrel
        // decode but Python's `http.server` (the Linux agent) does not — it
        // reads only `Content-Length` and would silently write a 0-byte
        // file. We know the size, so frame the request with it (ADR-0001).
        // hyper keeps an explicit `Content-Length` on a streamed body.
        let exchange = self
            .open(
                &deadline,
                Method::POST,
                &self.file_url("/upload", remote),
                Some(Payload {
                    content_type: "application/octet-stream",
                    length: Some(len),
                }),
                body,
            )
            .await?;
        if !exchange.response.status().is_success() {
            return Err(error_from_response(&deadline, exchange).await);
        }
        let bytes = read_body(&deadline, exchange).await?;
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
        let deadline = Deadline::after(self.config.timeout);
        let exchange = self.open_download(&deadline, remote).await?;

        let (temp_path, mut temp_file) = create_temp_sibling(local).await?;
        let outcome = stream_to_writer(&deadline, exchange, &mut temp_file, &temp_path).await;
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

    /// Send the download request and return the exchange once the agent
    /// answers with a success status.
    async fn open_download(
        &self,
        deadline: &Deadline,
        remote: &str,
    ) -> Result<Exchange, AgentError> {
        let exchange = self
            .open(
                deadline,
                Method::POST,
                &self.file_url("/download", remote),
                None,
                empty_body(),
            )
            .await?;
        if !exchange.response.status().is_success() {
            return Err(error_from_response(deadline, exchange).await);
        }
        Ok(exchange)
    }

    // -----------------------------------------------------------------
    // Transport
    // -----------------------------------------------------------------

    async fn post_json<B, R>(&self, path: &str, body: &B) -> Result<R, AgentError>
    where
        B: Serialize + ?Sized,
        R: serde::de::DeserializeOwned,
    {
        self.post_json_within(path, body, self.config.timeout).await
    }

    /// POST `body` as JSON and decode the reply, all within `timeout`.
    async fn post_json_within<B, R>(
        &self,
        path: &str,
        body: &B,
        timeout: Duration,
    ) -> Result<R, AgentError>
    where
        B: Serialize + ?Sized,
        R: serde::de::DeserializeOwned,
    {
        let json = serde_json::to_vec(body)
            .map_err(|e| AgentError::Internal(format!("failed to encode request: {e}")))?;
        let deadline = Deadline::after(timeout);
        let exchange = self
            .open(
                &deadline,
                Method::POST,
                &self.url(path),
                Some(Payload {
                    content_type: "application/json",
                    length: None,
                }),
                buffered_body(json),
            )
            .await?;
        check_status(&exchange.response)?;
        let bytes = read_body(&deadline, exchange).await?;
        decode_body(&bytes)
    }

    /// Dial, send one request and wait for the response headers. The request
    /// asks the agent to close the connection, so no connection serves a
    /// second request. A task drives the connection until the returned
    /// `Exchange` is dropped.
    async fn open(
        &self,
        deadline: &Deadline,
        method: Method,
        url: &str,
        payload: Option<Payload>,
        body: RequestBody,
    ) -> Result<Exchange, AgentError> {
        let request = self.request(method, url, payload, body)?;
        deadline
            .run("the request", async {
                let stream = self
                    .connector
                    .connect()
                    .await
                    .map_err(|e| dial_error(&self.config.host, self.config.port, &e))?;
                let (mut sender, connection) =
                    hyper::client::conn::http1::handshake(TokioIo::new(stream))
                        .await
                        .map_err(|e| dropped(&e))?;
                let driver = AbortOnDrop(tokio::spawn(async move {
                    // The request sees a failed connection as its own error.
                    let _ = connection.await;
                }));
                let response = sender
                    .send_request(request)
                    .await
                    .map_err(|e| dropped(&e))?;
                Ok(Exchange {
                    response,
                    _connection: driver,
                })
            })
            .await
    }

    /// The HTTP/1.1 request for `url`: origin-form target, with the headers
    /// that every request carries.
    fn request(
        &self,
        method: Method,
        url: &str,
        payload: Option<Payload>,
        body: RequestBody,
    ) -> Result<Request<RequestBody>, AgentError> {
        let uri: http::Uri = url
            .parse()
            .map_err(|e| AgentError::Internal(format!("invalid request URL {url}: {e}")))?;
        let target = uri.path_and_query().map(|t| t.as_str()).unwrap_or("/");
        let mut builder = Request::builder()
            .method(method)
            .uri(target)
            .header(HOST, format!("{}:{}", self.config.host, self.config.port))
            .header(ACCEPT, "*/*")
            .header(CONNECTION, "close");
        if let Some(payload) = payload {
            builder = builder.header(CONTENT_TYPE, payload.content_type);
            if let Some(length) = payload.length {
                builder = builder.header(CONTENT_LENGTH, length);
            }
        }
        builder
            .body(body)
            .map_err(|e| AgentError::Internal(format!("invalid request: {e}")))
    }
}

/// Size of the pieces in which an upload reads its source.
const UPLOAD_CHUNK: usize = 64 * 1024;

/// Content headers of a request that has a body.
struct Payload {
    content_type: &'static str,
    /// Set for a streamed body whose size is known. A buffered body gets its
    /// length from hyper.
    length: Option<u64>,
}

fn empty_body() -> RequestBody {
    buffered_body(Vec::new())
}

fn buffered_body(bytes: Vec<u8>) -> RequestBody {
    Full::new(Bytes::from(bytes))
        .map_err(|never| match never {})
        .boxed_unsync()
}

/// Read the rest of the body into memory, inside the request's deadline.
async fn read_body(deadline: &Deadline, exchange: Exchange) -> Result<Bytes, AgentError> {
    let Exchange {
        response,
        _connection,
    } = exchange;
    deadline
        .run("reading the response", async {
            response
                .into_body()
                .collect()
                .await
                .map(|collected| collected.to_bytes())
                .map_err(|e| dropped(&e))
        })
        .await
}

/// Promote a non-2xx response to an `AgentError` from its status alone. The
/// caller that wants the body uses `error_from_response` instead.
fn check_status(response: &Response<Incoming>) -> Result<(), AgentError> {
    if response.status().is_success() {
        Ok(())
    } else {
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
async fn error_from_response(deadline: &Deadline, exchange: Exchange) -> AgentError {
    let status = exchange.response.status();
    let bytes = match read_body(deadline, exchange).await {
        Ok(b) => b,
        Err(e) => return e,
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

/// Drain the response body into `writer`, returning the byte count. Stream
/// errors map to transport `AgentError`s; local write failures map to
/// `LocalIo` against `temp_path`. Each chunk is written before the next one
/// is read, so the body is never held whole.
async fn stream_to_writer<W>(
    deadline: &Deadline,
    exchange: Exchange,
    writer: &mut W,
    temp_path: &Path,
) -> Result<u64, AgentError>
where
    W: AsyncWrite + Unpin,
{
    let Exchange {
        response,
        _connection,
    } = exchange;
    let mut body = response.into_body();
    let mut written: u64 = 0;
    loop {
        let frame = deadline
            .run("reading the response", async {
                body.frame().await.transpose().map_err(|e| dropped(&e))
            })
            .await?;
        let Some(frame) = frame else { break };
        let Ok(chunk) = frame.into_data() else {
            continue;
        };
        writer.write_all(&chunk).await.map_err(|e| {
            local_io(
                temp_path,
                format!("failed to write {}: {e}", temp_path.display()),
            )
        })?;
        written += chunk.len() as u64;
    }
    writer.flush().await.map_err(|e| {
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
mod streaming_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let cfg = AgentConfig::new("192.168.64.2", AgentConfig::DEFAULT_PORT);
        assert_eq!(cfg.base_url(), "http://192.168.64.2:8648");
        assert_eq!(cfg.timeout, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn refused_connect_keeps_the_os_errno_and_the_whole_chain() {
        // Bind then drop to get a loopback port with nothing listening.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let client = AgentClient::new(
            AgentConfig::new("127.0.0.1", port).with_timeout(Duration::from_secs(2)),
        )
        .unwrap();
        let err = client.health().await.unwrap_err();
        let AgentError::ConnectionRefused { message, os_errno } = err else {
            panic!("a refused connect must be ConnectionRefused, got {err:?}");
        };
        let errno = os_errno.expect("the errno must be recovered from the source chain");
        assert_eq!(
            std::io::Error::from_raw_os_error(errno).kind(),
            std::io::ErrorKind::ConnectionRefused,
        );
        assert!(
            message.contains(&format!("os error {errno}")),
            "message must carry the chain's OS cause, got: {message}",
        );
    }

    #[test]
    fn client_builds() {
        let cfg = AgentConfig::new("localhost", 8648);
        let client = AgentClient::new(cfg).expect("client should build");
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
            Err(AgentError::Wire {
                wire_error,
                details,
            }) => {
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
