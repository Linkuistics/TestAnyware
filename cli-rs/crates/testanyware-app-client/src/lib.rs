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
use testanyware_app_protocol::{
    validate_socket_path, AppError, ClientHello, StreamKind, StreamTarget, UnreachableCause,
    STREAM_UPGRADE_TOKEN,
};

/// A guest stream after the `101`: the bytes of the guest protocol, unchanged, in both
/// directions. It is a tokio byte stream.
pub type AppStream = TokioIo<hyper::upgrade::Upgraded>;

/// The most bytes of an error body that the client reads. An error object is far smaller.
const ERROR_BODY_LIMIT: usize = 64 * 1024;

const START_REMEDY: &str = "Start the TestAnyware app. The Host CLI never starts it.";

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
        self.preflight()?;
        let socket = self.dial().await?;
        let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(socket))
            .await
            .map_err(|e| self.connection_error(&e))?;
        // `with_upgrades` lets the connection hand its stream over after the `101`. This
        // task ends when it has done so, or when the request fails.
        // https://docs.rs/hyper/1.9.0/hyper/upgrade/index.html
        tokio::spawn(async move {
            let _ = connection.with_upgrades().await;
        });

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
        let status = response.status();
        let body = Limited::new(response.into_body(), ERROR_BODY_LIMIT)
            .collect()
            .await
            .map(|collected| collected.to_bytes())
            .unwrap_or_default();
        Err(
            serde_json::from_slice::<AppError>(&body).unwrap_or_else(|_| {
                internal(format!(
                    "The TestAnyware app answered HTTP {status} without an error object."
                ))
            }),
        )
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
}
