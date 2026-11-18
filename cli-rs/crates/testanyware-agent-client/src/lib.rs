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
use hyper::client::conn::http1::Parts;
use hyper_util::rt::TokioIo;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::Serialize;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_util::io::ReaderStream;

pub use http::StatusCode;
use transport::{AbortOnDrop, Deadline};
pub use transport::{
    AgentConnector, AgentStream, BoxedStream, ConnectFuture, ConnectorFailure, TcpConnector,
};

use testanyware_protocol::exec_record::{
    read_cancel_reply, read_exec_reply, CancelState, ExecReplyError,
};
use testanyware_protocol::{
    ActionResponse, ElementQuery, ErrorResponse, ExecCancelRequest, ExecRequest, ExecResult,
    HealthResponse, InspectResponse, SetValueRequest, SnapshotRequest, SnapshotResponse,
    WaitRequest, WindowMoveRequest, WindowResizeRequest, WindowTarget,
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

    /// The agent sent a typed exec record that breaks a validity rule. Each
    /// entry names one broken rule. The record is contradictory evidence, so
    /// it never reaches the caller as a result.
    #[error("the agent exec record is contradictory: {}", .0.join("; "))]
    ExecRecordInvalid(Vec<String>),

    /// The agent answered HTTP 404 for an endpoint that only a newer agent
    /// has, for example `/exec/cancel` on an old agent. The request did
    /// nothing in the guest.
    #[error("the agent does not have the endpoint {endpoint}; it is older than the Host CLI")]
    EndpointUnsupported { endpoint: String },

    /// The download body had bytes past its declared `Content-Length`. The
    /// destination is unchanged, because the bytes cannot be trusted
    /// (docs/specs/guest-command-lifecycle.md, section "Host CLI download").
    #[error("the download body has bytes past its declared length: {0}")]
    DownloadFramingInvalid(String),

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
            AgentError::ExecRecordInvalid(_) => "EXEC_RECORD_INVALID",
            AgentError::EndpointUnsupported { .. } => "AGENT_ENDPOINT_UNSUPPORTED",
            AgentError::DownloadFramingInvalid(_) => "DOWNLOAD_FRAMING_INVALID",
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
        "exec_id_in_use" => "EXEC_ID_IN_USE",
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

/// The stream under hyper's client connection.
type Io = TokioIo<BoxedStream>;

/// How the task that drives a connection ended. A framed exchange keeps the
/// stream and the bytes that hyper read but did not parse.
type ConnectionEnd = Result<Option<Parts<Io>>, hyper::Error>;

/// A response whose headers have arrived, with the guard that keeps its
/// connection alive until the body is read.
struct Exchange {
    response: Response<Incoming>,
    connection: AbortOnDrop<ConnectionEnd>,
}

/// Whether a request needs the stream back after its response, to check
/// what follows the body.
#[derive(Clone, Copy, PartialEq)]
enum Framing {
    /// Drop the stream with the connection, as every request did before.
    Plain,
    /// Keep the stream, so the client can check for bytes past the body.
    Checked,
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
                Framing::Plain,
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
        let deadline = Deadline::after(timeout);
        let exchange = self.send_json(&deadline, "/exec", request).await?;
        // A new agent answers a duplicate exec id with 409 and the token
        // `exec_id_in_use`. Every other status keeps today's error.
        if exchange.response.status() == StatusCode::CONFLICT {
            return Err(error_from_response(&deadline, exchange).await);
        }
        check_status(&exchange.response)?;
        let bytes = read_body(&deadline, exchange).await?;
        // A typed record must pass the validity rules of the spec, section
        // "Validity rules". A legacy reply returns as today.
        match read_exec_reply(&bytes, request.id.as_deref()) {
            Ok(result) => Ok(result),
            Err(ExecReplyError::Invalid(problems)) => Err(AgentError::ExecRecordInvalid(problems)),
            Err(ExecReplyError::Decode(_)) => decode_body(&bytes),
        }
    }

    /// Cancel the guest command with the exec id of `request` (ADR-0025). The
    /// whole request, from the dial to the last byte, runs within `timeout`.
    ///
    /// An agent without `/exec/cancel` answers HTTP 404, which gives
    /// [`AgentError::EndpointUnsupported`] whatever the body says. A terminal
    /// record must be a valid typed record of the cancelled id.
    pub async fn cancel_exec(
        &self,
        request: &ExecCancelRequest,
        timeout: Duration,
    ) -> Result<CancelState, AgentError> {
        const ENDPOINT: &str = "/exec/cancel";
        let deadline = Deadline::after(timeout);
        let exchange = self.send_json(&deadline, ENDPOINT, request).await?;
        let status = exchange.response.status();
        if status == StatusCode::NOT_FOUND {
            return Err(AgentError::EndpointUnsupported {
                endpoint: ENDPOINT.to_string(),
            });
        }
        if !status.is_success() {
            return Err(error_from_response(&deadline, exchange).await);
        }
        let bytes = read_body(&deadline, exchange).await?;
        match read_cancel_reply(&bytes, &request.id) {
            Ok(state) => Ok(state),
            Err(ExecReplyError::Invalid(problems)) => Err(AgentError::ExecRecordInvalid(problems)),
            Err(ExecReplyError::Decode(message)) => Err(AgentError::Decode(message)),
        }
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
                Framing::Plain,
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
    /// body is consumed chunk-by-chunk into a sibling temp file, so memory
    /// stays bounded. Returns the number of bytes written.
    ///
    /// The steps follow docs/specs/guest-command-lifecycle.md, section "Host
    /// CLI download". A body shorter than its `Content-Length` fails with
    /// `CONNECTION_DROPPED`. A byte after the body, before the agent closes
    /// the connection, fails with `DOWNLOAD_FRAMING_INVALID`. Only then does
    /// the client flush the temp file to disk and rename it to `local`.
    ///
    /// Every failure removes the temp file and leaves `local` unchanged. A
    /// guard owns the temp file, so a caller that drops this future, for
    /// example at a signal, also removes it.
    pub async fn download(&self, remote: &str, local: &Path) -> Result<u64, AgentError> {
        let deadline = Deadline::after(self.config.timeout);
        let exchange = self.open_download(&deadline, remote).await?;
        let declared = declared_length(exchange.response.headers())?;

        let (temp, mut temp_file) = create_temp_sibling(local)?;
        let Exchange {
            response,
            mut connection,
        } = exchange;
        let written = stream_to_writer(&deadline, response, &mut temp_file, temp.path()).await?;
        check_declared_length(declared, written)?;
        check_after_body(&deadline, &mut connection).await?;
        temp_file.sync_all().await.map_err(|e| {
            local_io(
                temp.path(),
                format!("failed to flush {} to disk: {e}", temp.path().display()),
            )
        })?;
        // Drop the handle before the rename, so that on Windows the file is
        // not still open during the rename.
        drop(temp_file);
        tokio::fs::rename(temp.path(), local).await.map_err(|e| {
            local_io(
                local,
                format!("failed to finalize {}: {e}", local.display()),
            )
        })?;
        temp.keep();
        Ok(written)
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
                Framing::Checked,
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
        let bytes = self.post_json_bytes_within(path, body, timeout).await?;
        decode_body(&bytes)
    }

    /// POST `body` as JSON and return the reply body, all within `timeout`.
    async fn post_json_bytes_within<B>(
        &self,
        path: &str,
        body: &B,
        timeout: Duration,
    ) -> Result<Vec<u8>, AgentError>
    where
        B: Serialize + ?Sized,
    {
        let deadline = Deadline::after(timeout);
        let exchange = self.send_json(&deadline, path, body).await?;
        check_status(&exchange.response)?;
        let bytes = read_body(&deadline, exchange).await?;
        Ok(bytes.to_vec())
    }

    /// POST `body` as JSON within `deadline` and return the exchange once the
    /// response headers arrive, whatever the status.
    async fn send_json<B>(
        &self,
        deadline: &Deadline,
        path: &str,
        body: &B,
    ) -> Result<Exchange, AgentError>
    where
        B: Serialize + ?Sized,
    {
        let json = serde_json::to_vec(body)
            .map_err(|e| AgentError::Internal(format!("failed to encode request: {e}")))?;
        self.open(
            deadline,
            Method::POST,
            &self.url(path),
            Some(Payload {
                content_type: "application/json",
                length: None,
            }),
            buffered_body(json),
            Framing::Plain,
        )
        .await
    }

    /// Dial, send one request and wait for the response headers. The request
    /// asks the agent to close the connection, so no connection serves a
    /// second request. A task drives the connection until the returned
    /// `Exchange` is dropped. With [`Framing::Checked`], the task returns the
    /// stream when the connection ends.
    async fn open(
        &self,
        deadline: &Deadline,
        method: Method,
        url: &str,
        payload: Option<Payload>,
        body: RequestBody,
        framing: Framing,
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
                    // A plain request sees a failed connection as its own
                    // error. A checked one reads the end after the body.
                    // `without_shutdown` returns the stream and its unparsed
                    // bytes once the closed exchange is done:
                    // https://docs.rs/hyper/1.9.0/hyper/client/conn/http1/struct.Connection.html#method.without_shutdown
                    match framing {
                        Framing::Plain => connection.await.map(|()| None),
                        Framing::Checked => connection.without_shutdown().await.map(Some),
                    }
                }));
                let response = sender
                    .send_request(request)
                    .await
                    .map_err(|e| dropped(&e))?;
                Ok(Exchange {
                    response,
                    connection: driver,
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
        connection: _connection,
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

/// The temp file of one download. Dropping it removes the file, unless
/// [`TempFile::keep`] ran after the rename.
struct TempFile {
    path: PathBuf,
    keep: bool,
}

impl TempFile {
    fn path(&self) -> &Path {
        &self.path
    }

    /// The rename moved the file to its destination, so leave it.
    fn keep(mut self) {
        self.keep = true;
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.keep {
            // Synchronous, because a dropped future runs no further await.
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Create a uniquely-named temp file in the destination's own directory, so
/// the later rename stays on one filesystem and is atomic. The leading dot
/// keeps it out of casual `ls`; pid + a process-local counter avoid
/// collisions between concurrent downloads.
///
/// The file is created synchronously, so the guard exists before any await.
/// A creation on the blocking pool could finish after a dropped future, and
/// then no guard would remove its file.
///
/// The creation is exclusive. An existing entry at the name, such as a FIFO,
/// a symlink or the file of a killed run, is never opened. Opening a FIFO
/// would block the task that must see a signal (review finding R2 of
/// host-cli-control-k239). The next name is tried instead, up to
/// `TEMP_NAME_ATTEMPTS` names.
fn create_temp_sibling(dest: &Path) -> Result<(TempFile, tokio::fs::File), AgentError> {
    const TEMP_NAME_ATTEMPTS: usize = 16;
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let dir = dest.parent().filter(|p| !p.as_os_str().is_empty());
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".to_string());
    let mut attempt = 0;
    loop {
        attempt += 1;
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let temp_name = format!(".{name}.testanyware-{}.{seq}.tmp", std::process::id());
        let temp_path = match dir {
            Some(d) => d.join(temp_name),
            None => PathBuf::from(temp_name),
        };
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => {
                let temp = TempFile {
                    path: temp_path,
                    keep: false,
                };
                return Ok((temp, tokio::fs::File::from_std(file)));
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::AlreadyExists
                    && attempt < TEMP_NAME_ATTEMPTS => {}
            Err(e) => {
                return Err(local_io(
                    &temp_path,
                    format!("failed to create temp file {}: {e}", temp_path.display()),
                ))
            }
        }
    }
}

/// The `Content-Length` that a download reply declares, or `None` without
/// one. hyper enforces the length only when it frames the body by it. A
/// reply that also declares `Transfer-Encoding: chunked` is framed by its
/// chunks, and hyper ignores the length (review finding R3 of
/// host-cli-control-k239). So the client reads the length itself. An
/// unreadable value, or two values that differ, is contradictory framing.
fn declared_length(headers: &http::HeaderMap) -> Result<Option<u64>, AgentError> {
    let mut declared = None;
    for value in headers.get_all(CONTENT_LENGTH) {
        let length = value
            .to_str()
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .ok_or_else(|| {
                AgentError::DownloadFramingInvalid(format!(
                    "the reply declares an unreadable Content-Length {value:?}"
                ))
            })?;
        if declared.is_some_and(|d| d != length) {
            return Err(AgentError::DownloadFramingInvalid(
                "the reply declares two different Content-Length values".to_string(),
            ));
        }
        declared = Some(length);
    }
    Ok(declared)
}

/// Compare the decoded body with its declared length. More bytes are bytes
/// past the length, as `DOWNLOAD_FRAMING_INVALID`. Fewer bytes are a short
/// body, as `CONNECTION_DROPPED` (spec, section "Host CLI download").
fn check_declared_length(declared: Option<u64>, written: u64) -> Result<(), AgentError> {
    match declared {
        Some(length) if written > length => Err(AgentError::DownloadFramingInvalid(format!(
            "the body has {written} bytes, past its declared length of {length}"
        ))),
        Some(length) if written < length => Err(AgentError::ConnectionDropped(format!(
            "the body ended after {written} of its declared {length} bytes"
        ))),
        _ => Ok(()),
    }
}

/// Check what follows a complete download body. The task that drove the
/// connection returns the bytes that hyper read past the body and the
/// stream. Any byte in either, before the agent closes the stream, means
/// that the agent sent more than its `Content-Length`.
///
/// A direct connection reports a reset after the body as a read error, and
/// the download fails with `CONNECTION_DROPPED`. Through the app, the relay
/// ends a guest reset with a close of its Unix socket. A Unix stream socket
/// has no reset, so this reads an end of stream and accepts the body. The
/// relay forwards every byte that reached the host before the reset, so a
/// byte past the body still fails here. Only bytes that never left the guest
/// stay unseen (review finding R4 of host-cli-control-k239, an accepted
/// trade-off).
async fn check_after_body(
    deadline: &Deadline,
    connection: &mut AbortOnDrop<ConnectionEnd>,
) -> Result<(), AgentError> {
    deadline
        .run("reading the end of the response", async {
            let end = (&mut connection.0)
                .await
                .map_err(|e| AgentError::Internal(format!("the connection task failed: {e}")))?;
            let parts = match end {
                Ok(Some(parts)) => parts,
                Ok(None) => {
                    return Err(AgentError::Internal(
                        "a checked download lost its stream".to_string(),
                    ))
                }
                Err(e) => return Err(dropped(&e)),
            };
            if !parts.read_buf.is_empty() {
                return Err(AgentError::DownloadFramingInvalid(format!(
                    "{} bytes followed the body",
                    parts.read_buf.len()
                )));
            }
            let mut stream = parts.io.into_inner();
            let mut probe = [0u8; 512];
            match stream.read(&mut probe).await {
                Ok(0) => Ok(()),
                Ok(n) => Err(AgentError::DownloadFramingInvalid(format!(
                    "at least {n} bytes followed the body"
                ))),
                Err(e) => Err(dropped(&e)),
            }
        })
        .await
}

/// Drain the response body into `writer`, returning the byte count. Stream
/// errors map to transport `AgentError`s; local write failures map to
/// `LocalIo` against `temp_path`. Each chunk is written before the next one
/// is read, so the body is never held whole.
async fn stream_to_writer<W>(
    deadline: &Deadline,
    response: Response<Incoming>,
    writer: &mut W,
    temp_path: &Path,
) -> Result<u64, AgentError>
where
    W: AsyncWrite + Unpin,
{
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
        assert_eq!(map_wire_error("exec_id_in_use"), "EXEC_ID_IN_USE");
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
