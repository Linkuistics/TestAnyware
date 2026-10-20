//! The Host CLI's client of the TestAnyware app (spec: "The protocol").
//!
//! [`AppClient::open_stream`] asks the app for one guest stream on its Unix socket. It checks
//! the socket path and its directory first, states the Host CLI's protocol range, build and
//! directories in the request headers, and takes the connection over after the `101`. The
//! client dials no guest, because only the app does.
//!
//! Every failure is an [`AppError`] in the Host CLI's own vocabulary. The app sends most of
//! them. The client makes `APP_UNREACHABLE` itself, because no app answered.

use std::path::{Path, PathBuf};

use bytes::Bytes;
use http::header::{CONNECTION, HOST, UPGRADE};
use http::{Method, Request, StatusCode};
use http_body_util::{BodyExt, Empty, Limited};
use hyper_util::rt::TokioIo;
use serde::de::DeserializeOwned;
use testanyware_app_protocol::{
    validate_socket_path, AppError, ClientHello, StreamKind, StreamTarget, UnreachableCause,
    STREAM_UPGRADE_TOKEN,
};

/// A guest stream after the `101`: the bytes of the guest protocol, unchanged, in both
/// directions. It is a tokio byte stream.
pub type AppStream = TokioIo<hyper::upgrade::Upgraded>;

/// The most bytes of an error body that the client reads. An error object is far smaller.
const ERROR_BODY_LIMIT: usize = 64 * 1024;

/// The most bytes of a document that the client reads. A listing of thousands of VMs is smaller.
const DOCUMENT_BODY_LIMIT: usize = 16 * 1024 * 1024;

const START_REMEDY: &str = "Open TestAnyware from /Applications, and turn on \"Start at Login\" \
    in its menu, so that macOS starts it at each login. The Host CLI never starts it.";

/// Opens guest streams through the app on one socket.
#[derive(Debug, Clone)]
pub struct AppClient {
    socket: PathBuf,
    hello: ClientHello,
}

impl AppClient {
    /// A client for the app on `socket`. `hello` is what each request states about the Host
    /// CLI: its protocol range, its build and the directories it resolved.
    pub fn new(socket: PathBuf, hello: ClientHello) -> Self {
        Self { socket, hello }
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// Ask the app for a stream of `kind` to `target`. The result is the connection after the
    /// `101 Switching Protocols`. An error before the upgrade is the app's error object.
    pub async fn open_stream(
        &self,
        target: &StreamTarget,
        kind: StreamKind,
    ) -> Result<AppStream, AppError> {
        let mut sender = self.connect(true).await?;
        let mut request = Request::builder()
            .method(Method::GET)
            .uri(target.path(kind))
            .header(HOST, "testanyware")
            .header(CONNECTION, "Upgrade")
            .header(UPGRADE, STREAM_UPGRADE_TOKEN);
        for (name, value) in self.hello.to_headers() {
            request = request.header(name, value);
        }
        let request = request
            .body(Empty::<Bytes>::new())
            .map_err(|e| internal(format!("the request for a guest stream is invalid: {e}")))?;

        let mut response = sender
            .send_request(request)
            .await
            .map_err(|e| self.connection_error(&e))?;
        if response.status() == StatusCode::SWITCHING_PROTOCOLS {
            let upgraded = hyper::upgrade::on(&mut response)
                .await
                .map_err(|e| self.connection_error(&e))?;
            return Ok(TokioIo::new(upgraded));
        }
        Err(error_of(response).await)
    }

    /// Ask the app for a JSON document with `GET path_and_query`, and read it as a `T`. An error
    /// object that the app sends in place of the document is the error.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        path_and_query: &str,
    ) -> Result<T, AppError> {
        let mut sender = self.connect(false).await?;
        let mut request = Request::builder()
            .method(Method::GET)
            .uri(path_and_query)
            .header(HOST, "testanyware");
        for (name, value) in self.hello.to_headers() {
            request = request.header(name, value);
        }
        let request = request
            .body(Empty::<Bytes>::new())
            .map_err(|e| internal(format!("the request for {path_and_query} is invalid: {e}")))?;
        let response = sender
            .send_request(request)
            .await
            .map_err(|e| self.connection_error(&e))?;
        if response.status() != StatusCode::OK {
            return Err(error_of(response).await);
        }
        let body = Limited::new(response.into_body(), DOCUMENT_BODY_LIMIT)
            .collect()
            .await
            .map_err(|e| self.connection_ended(UnreachableCause::SocketError, e))?
            .to_bytes();
        serde_json::from_slice(&body).map_err(|e| {
            internal(format!(
                "The TestAnyware app answered {path_and_query} with a document that does not read: {e}."
            ))
        })
    }

    /// Check the socket, connect to it and start the HTTP/1.1 connection. With `upgrades`, the
    /// connection can hand its stream over after a `101`.
    async fn connect(
        &self,
        upgrades: bool,
    ) -> Result<hyper::client::conn::http1::SendRequest<Empty<Bytes>>, AppError> {
        self.preflight()?;
        let socket = self.dial().await?;
        let (sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(socket))
            .await
            .map_err(|e| self.connection_error(&e))?;
        // The task ends when the connection does. `with_upgrades` lets a guest stream take the
        // connection over after the `101`.
        // https://docs.rs/hyper/1.9.0/hyper/upgrade/index.html
        tokio::spawn(async move {
            if upgrades {
                let _ = connection.with_upgrades().await;
            } else {
                let _ = connection.await;
            }
        });
        Ok(sender)
    }

    /// The checks that need no connection: the length of the socket path, and the owner and
    /// the mode of its directory (spec: "The socket and access").
    fn preflight(&self) -> Result<(), AppError> {
        if let Err(e) = validate_socket_path(&self.socket) {
            return Err(AppError::unreachable(
                UnreachableCause::SocketPathTooLong,
                &self.socket,
                e.to_string(),
                "Use a shorter path in TESTANYWARE_APP_SOCKET.",
            ));
        }
        self.check_directory()
    }

    #[cfg(unix)]
    fn check_directory(&self) -> Result<(), AppError> {
        let dir = match self.socket.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => dir,
            _ => Path::new("."),
        };
        let own_uid = nix::unistd::geteuid().as_raw();
        match testanyware_app_protocol::socket::check_socket_dir(dir, own_uid) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(unsafe_dir)) => Err(AppError::unreachable(
                UnreachableCause::UnsafeSocketDirectory,
                &self.socket,
                format!(
                    "The Host CLI refuses to connect: in {}, {unsafe_dir}.",
                    dir.display()
                ),
                format!(
                    "Make {} belong to you with mode 0700, or remove it and start the \
                     TestAnyware app again.",
                    dir.display()
                ),
            )),
            Err(e) => Err(self.connect_error(&e)),
        }
    }

    #[cfg(not(unix))]
    fn check_directory(&self) -> Result<(), AppError> {
        Err(AppError::unreachable(
            UnreachableCause::NotRunning,
            &self.socket,
            "The TestAnyware app runs on macOS only.",
            "Unset TESTANYWARE_APP_SOCKET on this host.",
        ))
    }

    #[cfg(unix)]
    async fn dial(&self) -> Result<tokio::net::UnixStream, AppError> {
        tokio::net::UnixStream::connect(&self.socket)
            .await
            .map_err(|e| self.connect_error(&e))
    }

    #[cfg(not(unix))]
    async fn dial(&self) -> Result<tokio::io::DuplexStream, AppError> {
        self.check_directory()
            .map(|()| unreachable!("check_directory fails off Unix"))
    }

    /// A failure to reach the socket. A missing socket and a refused connection mean that
    /// no app runs. Any other failure keeps its errno and its text.
    #[cfg_attr(not(unix), allow(dead_code))] // Only the Unix dial and directory check call it.
    fn connect_error(&self, err: &std::io::Error) -> AppError {
        match err.kind() {
            std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
                AppError::unreachable(
                    UnreachableCause::NotRunning,
                    &self.socket,
                    format!(
                        "Nothing answers on the TestAnyware app's socket {}: {err}.",
                        self.socket.display()
                    ),
                    START_REMEDY,
                )
            }
            _ => AppError::unreachable(
                UnreachableCause::SocketError,
                &self.socket,
                format!(
                    "The Host CLI cannot use the TestAnyware app's socket {}: {err}.",
                    self.socket.display()
                ),
                "Check the socket and its directory, or restart the TestAnyware app.",
            )
            .with_detail("errno", err.raw_os_error()),
        }
    }

    /// A failure after the connection opened. An app that closes the connection before it
    /// answers is on its way out.
    fn connection_error(&self, err: &hyper::Error) -> AppError {
        let cause = if ended_by_peer(err) {
            UnreachableCause::Quitting
        } else {
            UnreachableCause::SocketError
        };
        self.connection_ended(cause, err)
    }

    fn connection_ended(&self, cause: UnreachableCause, err: impl std::fmt::Display) -> AppError {
        AppError::unreachable(
            cause,
            &self.socket,
            format!(
                "The TestAnyware app on {} ended the connection: {err}.",
                self.socket.display()
            ),
            START_REMEDY,
        )
    }
}

/// The error object in a response that is no success. A body that is no error object becomes
/// `INTERNAL`.
async fn error_of(response: hyper::Response<hyper::body::Incoming>) -> AppError {
    let status = response.status();
    let body = Limited::new(response.into_body(), ERROR_BODY_LIMIT)
        .collect()
        .await
        .map(|collected| collected.to_bytes())
        .unwrap_or_default();
    serde_json::from_slice::<AppError>(&body).unwrap_or_else(|_| {
        internal(format!(
            "The TestAnyware app answered HTTP {status} without an error object."
        ))
    })
}

/// Whether the app closed the connection: hyper reports an incomplete message or a closed
/// channel, or an `io::Error` for a write or a read on the closed socket.
fn ended_by_peer(err: &hyper::Error) -> bool {
    use std::error::Error as _;
    use std::io::ErrorKind::{BrokenPipe, ConnectionAborted, ConnectionReset, UnexpectedEof};
    if err.is_incomplete_message() || err.is_canceled() || err.is_closed() {
        return true;
    }
    let mut source = err.source();
    while let Some(cause) = source {
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return matches!(
                io.kind(),
                BrokenPipe | ConnectionAborted | ConnectionReset | UnexpectedEof
            );
        }
        source = cause.source();
    }
    false
}

fn internal(message: String) -> AppError {
    AppError::new("INTERNAL", message, None, serde_json::Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use testanyware_app_protocol::{Directories, ProtocolRange};

    fn client(socket: &Path) -> AppClient {
        AppClient::new(
            socket.to_path_buf(),
            ClientHello::new(
                ProtocolRange::new(1, 1),
                "test",
                Directories {
                    state_dir: "/s".into(),
                    data_dir: "/d".into(),
                },
            ),
        )
    }

    fn target() -> StreamTarget {
        StreamTarget::Vm { id: "vm1".into() }
    }

    #[cfg(unix)]
    fn private_dir() -> tempfile::TempDir {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }

    #[tokio::test]
    async fn a_path_of_more_than_103_bytes_is_refused_before_any_connection() {
        let long = PathBuf::from(format!("/{}/s.sock", "a".repeat(120)));
        let err = client(&long)
            .open_stream(&target(), StreamKind::Agent)
            .await
            .unwrap_err();
        assert_eq!(err.code, "APP_UNREACHABLE");
        assert_eq!(err.details["cause"], "socket_path_too_long");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_missing_socket_means_the_app_is_not_running() {
        let dir = private_dir();
        let err = client(&dir.path().join("testanyware.sock"))
            .open_stream(&target(), StreamKind::Agent)
            .await
            .unwrap_err();
        assert_eq!(err.code, "APP_UNREACHABLE");
        assert_eq!(err.details["cause"], "not_running");
        assert!(err.remediation.unwrap().contains("never starts"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_missing_directory_means_the_app_is_not_running() {
        let dir = private_dir();
        let err = client(&dir.path().join("absent").join("testanyware.sock"))
            .open_stream(&target(), StreamKind::Agent)
            .await
            .unwrap_err();
        assert_eq!(err.details["cause"], "not_running");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_stale_socket_file_means_the_app_is_not_running() {
        let dir = private_dir();
        let path = dir.path().join("testanyware.sock");
        drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
        let err = client(&path)
            .open_stream(&target(), StreamKind::Agent)
            .await
            .unwrap_err();
        assert_eq!(err.details["cause"], "not_running");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_directory_that_admits_others_is_refused_before_it_connects() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = dir.path().join("testanyware.sock");
        // A live listener proves that the refusal comes from the directory check: a client
        // that skipped it would connect.
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let err = client(&path)
            .open_stream(&target(), StreamKind::Agent)
            .await
            .unwrap_err();
        assert_eq!(err.code, "APP_UNREACHABLE");
        assert_eq!(err.details["cause"], "unsafe_socket_directory");
        assert!(
            listener.accept().is_err(),
            "the client connected to a socket in an unsafe directory"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_app_that_closes_without_an_answer_is_quitting() {
        let dir = private_dir();
        let path = dir.path().join("testanyware.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            drop(stream);
        });
        let err = client(&path)
            .open_stream(&target(), StreamKind::Agent)
            .await
            .unwrap_err();
        assert_eq!(err.details["cause"], "quitting", "{}", err.message);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_error_object_before_the_upgrade_comes_back_whole() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let dir = private_dir();
        let path = dir.path().join("testanyware.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let object = serde_json::json!({
            "code": "VM_NOT_FOUND",
            "message": "No spec found for VM id 'vm1'",
            "remediation": "Run `testanyware vm list`.",
            "details": { "vm_id": "vm1" }
        })
        .to_string();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).await.unwrap();
                head.push(byte[0]);
            }
            let head = String::from_utf8(head).unwrap().to_ascii_lowercase();
            // The request states the upgrade and the whole hello.
            assert!(head.starts_with("get /v1/vms/vm1/agent http/1.1"), "{head}");
            for needle in [
                "connection: upgrade",
                "upgrade: testanyware-stream",
                "x-testanyware-protocol: 1-1",
                "x-testanyware-build: test",
                "x-testanyware-state-dir: /s",
                "x-testanyware-data-dir: /d",
            ] {
                assert!(head.contains(needle), "{needle} missing from {head}");
            }
            let reply = format!(
                "HTTP/1.1 404 Not Found\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{object}",
                object.len()
            );
            stream.write_all(reply.as_bytes()).await.unwrap();
        });
        let err = client(&path)
            .open_stream(&target(), StreamKind::Agent)
            .await
            .unwrap_err();
        assert_eq!(err.code, "VM_NOT_FOUND");
        assert_eq!(err.details["vm_id"], "vm1");
        assert_eq!(
            err.remediation.as_deref(),
            Some("Run `testanyware vm list`.")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_body_that_is_no_error_object_becomes_internal() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let dir = private_dir();
        let path = dir.path().join("testanyware.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            stream
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 2\r\n\r\nno")
                .await
                .unwrap();
        });
        let err = client(&path)
            .open_stream(&target(), StreamKind::Agent)
            .await
            .unwrap_err();
        assert_eq!(err.code, "INTERNAL");
        assert!(err.message.contains("500"), "{}", err.message);
    }

    #[cfg(unix)]
    async fn answer_once(path: &Path, reply: String) -> tokio::task::JoinHandle<String> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::UnixListener::bind(path).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut head = Vec::new();
            let mut byte = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).await.unwrap();
                head.push(byte[0]);
            }
            stream.write_all(reply.as_bytes()).await.unwrap();
            String::from_utf8(head).unwrap().to_ascii_lowercase()
        })
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_document_is_read_after_a_request_that_states_the_hello() {
        let dir = private_dir();
        let path = dir.path().join("testanyware.sock");
        let body = r#"{"n":3}"#;
        let server = answer_once(
            &path,
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                body.len()
            ),
        )
        .await;
        let document: serde_json::Value =
            client(&path).get_json("/v1/vms?limit=1").await.unwrap();
        assert_eq!(document["n"], 3);
        let head = server.await.unwrap();
        assert!(head.starts_with("get /v1/vms?limit=1 http/1.1"), "{head}");
        assert!(head.contains("x-testanyware-protocol: 1-1"), "{head}");
        assert!(head.contains("x-testanyware-state-dir: /s"), "{head}");
        assert!(!head.contains("upgrade"), "a document asks for no upgrade: {head}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_error_object_replaces_the_document() {
        let dir = private_dir();
        let path = dir.path().join("testanyware.sock");
        let object = serde_json::json!({
            "code": "APP_INCOMPATIBLE",
            "message": "m",
            "remediation": "r",
            "details": { "cause": "directories" }
        })
        .to_string();
        answer_once(
            &path,
            format!(
                "HTTP/1.1 409 Conflict\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{object}",
                object.len()
            ),
        )
        .await;
        let err = client(&path)
            .get_json::<serde_json::Value>("/v1/vms?limit=1")
            .await
            .unwrap_err();
        assert_eq!(err.code, "APP_INCOMPATIBLE");
        assert_eq!(err.details["cause"], "directories");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_document_that_does_not_read_becomes_internal() {
        let dir = private_dir();
        let path = dir.path().join("testanyware.sock");
        answer_once(
            &path,
            "HTTP/1.1 200 OK\r\ncontent-length: 3\r\n\r\nabc".to_string(),
        )
        .await;
        let err = client(&path)
            .get_json::<serde_json::Value>("/v1/vms?limit=1")
            .await
            .unwrap_err();
        assert_eq!(err.code, "INTERNAL");
        assert!(err.message.contains("does not read"), "{}", err.message);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_missing_socket_fails_a_document_request_as_not_running() {
        let dir = private_dir();
        let err = client(&dir.path().join("testanyware.sock"))
            .get_json::<serde_json::Value>("/v1/vms?limit=1")
            .await
            .unwrap_err();
        assert_eq!(err.details["cause"], "not_running");
    }
}
