//! The agent client over streams that are not TCP sockets (ADR-0004).
//!
//! Each test plugs an in-memory duplex stream into `AgentClient` through the
//! `AgentConnector` seam. A hyper server, or a few raw bytes, plays the agent
//! on the other end. The tests pin the transport rules of the spec's
//! "The protocol" section: one connection for each request, one deadline for
//! the whole request, no redirect, and a stable code for each failure.

use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use serde_json::json;
use testanyware_agent_client::{
    AgentClient, AgentConfig, AgentConnector, AgentError, BoxedStream, ConnectFuture,
    ConnectorFailure,
};
use testanyware_protocol::ExecRequest;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, DuplexStream};

type Serve = dyn Fn(DuplexStream) + Send + Sync;

/// Hands the client one end of a fresh duplex stream for each dial, and gives
/// the other end to `serve`.
struct DuplexConnector {
    serve: Box<Serve>,
    dials: Arc<AtomicUsize>,
}

impl DuplexConnector {
    fn new(serve: impl Fn(DuplexStream) + Send + Sync + 'static) -> (Arc<Self>, Arc<AtomicUsize>) {
        let dials = Arc::new(AtomicUsize::new(0));
        let connector = Arc::new(Self {
            serve: Box::new(serve),
            dials: dials.clone(),
        });
        (connector, dials)
    }
}

impl AgentConnector for DuplexConnector {
    fn connect(&self) -> ConnectFuture {
        self.dials.fetch_add(1, Ordering::SeqCst);
        let (client_end, server_end) = duplex(64 * 1024);
        (self.serve)(server_end);
        Box::pin(async move { Ok(Box::new(client_end) as BoxedStream) })
    }
}

fn client_over(connector: Arc<dyn AgentConnector>, timeout: Duration) -> AgentClient {
    AgentClient::with_connector(
        AgentConfig::new("agent.test", 8648).with_timeout(timeout),
        connector,
    )
}

fn health_json() -> Response<Full<Bytes>> {
    Response::new(Full::new(Bytes::from(
        json!({ "accessible": true, "platform": "linux" }).to_string(),
    )))
}

/// Run a hyper HTTP/1.1 server on `stream` with `handler`. Reports on `closed`
/// when the connection ends, which hyper does only after the peer closes it
/// or the server decides to close it.
fn serve_hyper<F, Fut>(stream: DuplexStream, handler: F, closed: Arc<AtomicUsize>)
where
    F: Fn(Request<Incoming>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Response<Full<Bytes>>, Infallible>> + Send + 'static,
{
    tokio::spawn(async move {
        let service = service_fn(handler);
        let served = hyper::server::conn::http1::Builder::new()
            .serve_connection(TokioIo::new(stream), service)
            .await;
        if served.is_ok() {
            closed.fetch_add(1, Ordering::SeqCst);
        }
    });
}

/// Read from `stream` until the end of the request head.
async fn read_request_head(stream: &mut DuplexStream) {
    let mut seen = Vec::new();
    let mut byte = [0u8; 1];
    while !seen.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).await.unwrap_or(0) == 0 {
            return;
        }
        seen.push(byte[0]);
    }
}

async fn wait_for(count: &AtomicUsize, expected: usize) {
    for _ in 0..200 {
        if count.load(Ordering::SeqCst) == expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!(
        "expected the count to reach {expected}, it is {}",
        count.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn the_client_runs_over_an_in_memory_stream() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let closed = Arc::new(AtomicUsize::new(0));
    let (connector, dials) = DuplexConnector::new(move |stream| {
        let log = log.clone();
        serve_hyper(
            stream,
            move |req| {
                let log = log.clone();
                async move {
                    log.lock().unwrap().push((
                        req.method().to_string(),
                        req.uri().to_string(),
                        req.headers()["host"].to_str().unwrap().to_string(),
                    ));
                    Ok(health_json())
                }
            },
            closed.clone(),
        );
    });

    let client = client_over(connector, Duration::from_secs(5));
    let health = client.health().await.expect("health over a duplex stream");

    assert!(health.accessible);
    assert_eq!(dials.load(Ordering::SeqCst), 1);
    assert_eq!(
        *seen.lock().unwrap(),
        vec![(
            "GET".to_string(),
            "/health".to_string(),
            "agent.test:8648".to_string()
        )]
    );
}

#[tokio::test]
async fn each_request_opens_its_own_connection_and_closes_it() {
    let closed = Arc::new(AtomicUsize::new(0));
    let asked_to_close = Arc::new(AtomicUsize::new(0));
    let (connector, dials) = DuplexConnector::new({
        let closed = closed.clone();
        let asked_to_close = asked_to_close.clone();
        move |stream| {
            let asked_to_close = asked_to_close.clone();
            serve_hyper(
                stream,
                move |req| {
                    let asked_to_close = asked_to_close.clone();
                    async move {
                        if req.headers()["connection"] == "close" {
                            asked_to_close.fetch_add(1, Ordering::SeqCst);
                        }
                        Ok(health_json())
                    }
                },
                closed.clone(),
            );
        }
    });

    let client = client_over(connector, Duration::from_secs(5));
    for _ in 0..3 {
        client.health().await.expect("health");
    }

    assert_eq!(dials.load(Ordering::SeqCst), 3, "one dial for each request");
    assert_eq!(asked_to_close.load(Ordering::SeqCst), 3);
    // A server connection that served one request ends only after the client
    // closes it. Three ended connections show that no connection stayed open.
    wait_for(&closed, 3).await;
}

/// A connector whose dial never completes.
struct StalledConnector;

impl AgentConnector for StalledConnector {
    fn connect(&self) -> ConnectFuture {
        Box::pin(std::future::pending::<io::Result<BoxedStream>>())
    }
}

#[tokio::test]
async fn a_stalled_dial_is_a_connection_timeout() {
    let client = client_over(Arc::new(StalledConnector), Duration::from_millis(150));
    let err = client.health().await.expect_err("the dial never completes");
    assert!(matches!(err, AgentError::Timeout(_)), "got {err:?}");
    assert_eq!(err.code(), "CONNECTION_TIMEOUT");
}

/// A connector whose dial fails with an OS error.
struct FailingConnector(i32);

impl AgentConnector for FailingConnector {
    fn connect(&self) -> ConnectFuture {
        let errno = self.0;
        Box::pin(async move { Err(io::Error::from_raw_os_error(errno)) })
    }
}

#[tokio::test]
async fn a_failed_dial_is_connection_refused_with_its_errno() {
    let client = client_over(Arc::new(FailingConnector(111)), Duration::from_secs(5));
    let err = client.health().await.expect_err("the dial fails");
    assert_eq!(err.code(), "CONNECTION_REFUSED");
    let AgentError::ConnectionRefused { message, os_errno } = err else {
        panic!("expected ConnectionRefused");
    };
    assert_eq!(os_errno, Some(111));
    assert!(message.contains("agent.test:8648"), "got: {message}");
    assert!(message.contains("os error 111"), "got: {message}");
}

#[tokio::test]
async fn the_timeout_covers_the_body_as_well_as_the_headers() {
    // The head arrives at once. The body promises 1000 bytes, sends 3, and
    // stalls with the stream open.
    let (connector, _) = DuplexConnector::new(|mut stream| {
        tokio::spawn(async move {
            read_request_head(&mut stream).await;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 1000\r\n\r\nabc")
                .await
                .unwrap();
            std::future::pending::<()>().await;
        });
    });

    let client = client_over(connector, Duration::from_millis(300));
    let err = client.health().await.expect_err("the body stalls");
    assert_eq!(err.code(), "CONNECTION_TIMEOUT", "got {err:?}");
}

#[tokio::test]
async fn a_connection_that_ends_early_is_dropped() {
    let (connector, _) = DuplexConnector::new(|mut stream| {
        tokio::spawn(async move {
            read_request_head(&mut stream).await;
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 1000\r\n\r\nabc")
                .await
                .unwrap();
            // Dropping the stream closes it before the body is complete.
        });
    });

    let client = client_over(connector, Duration::from_secs(5));
    let err = client.health().await.expect_err("the body ends early");
    assert_eq!(err.code(), "CONNECTION_DROPPED", "got {err:?}");
}

#[tokio::test]
async fn the_timeout_of_exec_replaces_the_timeout_of_the_client() {
    let closed = Arc::new(AtomicUsize::new(0));
    let (connector, _) = DuplexConnector::new(move |stream| {
        serve_hyper(
            stream,
            |_req| async {
                tokio::time::sleep(Duration::from_millis(400)).await;
                Ok(Response::new(Full::new(Bytes::from(
                    json!({ "exitCode": 0, "stdout": "", "stderr": "" }).to_string(),
                ))))
            },
            closed.clone(),
        );
    });
    // The client's own limit is shorter than the reply takes.
    let client = client_over(connector, Duration::from_millis(100));

    let health = client.health().await.expect_err("a short timeout applies");
    assert_eq!(health.code(), "CONNECTION_TIMEOUT");

    let request = ExecRequest {
        command: "true".into(),
        timeout: 0,
        detach: false,
        id: None,
    };
    let result = client
        .exec(&request)
        .await
        .expect("exec runs under its own, longer timeout");
    assert_eq!(result.exit_code, 0);
}

#[tokio::test]
async fn a_redirect_is_an_error_and_is_not_followed() {
    let requests = Arc::new(AtomicUsize::new(0));
    let closed = Arc::new(AtomicUsize::new(0));
    let (connector, dials) = DuplexConnector::new({
        let requests = requests.clone();
        move |stream| {
            let requests = requests.clone();
            serve_hyper(
                stream,
                move |_req| {
                    let requests = requests.clone();
                    async move {
                        requests.fetch_add(1, Ordering::SeqCst);
                        let mut response = Response::new(Full::new(Bytes::new()));
                        *response.status_mut() = http::StatusCode::FOUND;
                        response
                            .headers_mut()
                            .insert("location", "/elsewhere".parse().unwrap());
                        Ok(response)
                    }
                },
                closed.clone(),
            );
        }
    });

    let client = client_over(connector, Duration::from_secs(5));
    let err = client.health().await.expect_err("a 302 is not success");

    let AgentError::HttpStatus { status, .. } = &err else {
        panic!("expected HttpStatus, got {err:?}");
    };
    assert_eq!(status.as_u16(), 302);
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    assert_eq!(
        dials.load(Ordering::SeqCst),
        1,
        "no second dial to follow it"
    );
}

#[tokio::test]
async fn an_error_body_maps_as_before_over_a_stream() {
    let closed = Arc::new(AtomicUsize::new(0));
    let (connector, _) = DuplexConnector::new(move |stream| {
        serve_hyper(
            stream,
            |_req| async {
                let mut response = Response::new(Full::new(Bytes::from(
                    json!({ "error": "download_failed", "details": "no such file" }).to_string(),
                )));
                *response.status_mut() = http::StatusCode::BAD_REQUEST;
                Ok(response)
            },
            closed.clone(),
        );
    });

    let dir = tempfile::tempdir().unwrap();
    let client = client_over(connector, Duration::from_secs(5));
    let err = client
        .download("/missing", &dir.path().join("out.bin"))
        .await
        .expect_err("the agent refuses");
    assert_eq!(err.code(), "DOWNLOAD_FAILED");
}

/// A connector that fails with its own structured error.
struct StructuredFailureConnector(ConnectorFailure);

impl AgentConnector for StructuredFailureConnector {
    fn connect(&self) -> ConnectFuture {
        let failure = self.0.clone();
        Box::pin(async move { Err(failure.into()) })
    }
}

#[tokio::test]
async fn a_connector_failure_keeps_its_code_remedy_and_details() {
    let failure = ConnectorFailure {
        code: "VM_NOT_FOUND".into(),
        message: "No spec found for VM id 'x'".into(),
        remediation: Some("Run `testanyware vm list`.".into()),
        details: json!({ "vm_id": "x" }),
    };
    let client = client_over(
        Arc::new(StructuredFailureConnector(failure.clone())),
        Duration::from_secs(5),
    );
    let err = client.health().await.expect_err("the dial fails");
    assert_eq!(err.code(), "VM_NOT_FOUND");
    assert_eq!(err.to_string(), failure.message);
    match err {
        AgentError::Connector(got) => assert_eq!(got, failure),
        other => panic!("expected AgentError::Connector, got {other:?}"),
    }
}

// ---- download framing (spec "Host CLI download") ----

/// One piece of a raw reply: wait `after`, then write `bytes`.
struct Piece {
    after: Duration,
    bytes: Vec<u8>,
}

fn piece(bytes: impl Into<Vec<u8>>) -> Piece {
    Piece {
        after: Duration::ZERO,
        bytes: bytes.into(),
    }
}

/// What the raw agent does after its last piece.
#[derive(Clone, Copy)]
enum End {
    /// Close the stream, as an agent does after a `Connection: close` reply.
    Close,
    /// Keep the stream open until the client goes away.
    Hold,
}

/// An agent that reads the request head and then writes `pieces` byte for
/// byte, so a test can serve a recorded reply to the real client.
fn raw_agent(pieces: Vec<Piece>, end: End) -> Arc<DuplexConnector> {
    let pieces = Arc::new(pieces);
    let (connector, _) = DuplexConnector::new(move |mut stream| {
        let pieces = pieces.clone();
        tokio::spawn(async move {
            read_request_head(&mut stream).await;
            for piece in pieces.iter() {
                tokio::time::sleep(piece.after).await;
                if stream.write_all(&piece.bytes).await.is_err() {
                    return;
                }
            }
            if let End::Hold = end {
                let mut sink = [0u8; 64];
                while stream.read(&mut sink).await.unwrap_or(0) > 0 {}
            }
        });
    });
    connector
}

/// A raw reply of `content_length` with `body`, as HTTP/1.1.
fn reply(content_length: usize, body: &[u8]) -> Vec<u8> {
    let mut bytes = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/octet-stream\r\ncontent-length: {content_length}\r\n\r\n"
    )
    .into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

/// The head and the received body of one case of the retained k14 run. The
/// old Linux agent wrote exactly these bytes
/// (docs/experiments/exec-reliability-linux-download-consistency.md).
fn recorded_linux_reply(case: &str) -> (Vec<u8>, Vec<u8>) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../../experiments/exec-reliability/evidence/linux-download-consistency-k14/local-01/cases/{case}/raw"
    ));
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("record.json")).unwrap()).unwrap();
    let head = record["agent"]["trace"]
        .as_array()
        .unwrap()
        .iter()
        .find(|event| event["kind"] == "headers")
        .expect("the record holds the reply head")["text"]
        .as_str()
        .unwrap()
        .as_bytes()
        .to_vec();
    let body = std::fs::read(dir.join("received.bin")).unwrap();
    (head, body)
}

/// The names in `dir`, sorted.
fn names_in(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Download through `connector` into a directory that already holds the
/// destination with the bytes `old`. Returns the result, the directory and
/// the destination path.
async fn download_over(
    connector: Arc<DuplexConnector>,
) -> (
    Result<u64, AgentError>,
    tempfile::TempDir,
    std::path::PathBuf,
) {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("evidence.json");
    std::fs::write(&dest, b"old").unwrap();
    let client = client_over(connector, Duration::from_secs(5));
    let result = client.download("/tmp/evidence.json", &dest).await;
    (result, dir, dest)
}

/// The destination still holds `old`, and no temp file remains.
fn assert_unchanged(dir: &tempfile::TempDir, dest: &std::path::Path) {
    assert_eq!(
        std::fs::read(dest).unwrap(),
        b"old",
        "the destination changed"
    );
    assert_eq!(names_in(dir.path()), vec!["evidence.json".to_string()]);
}

#[tokio::test]
async fn the_recorded_old_linux_reply_with_extra_bytes_is_a_framing_failure() {
    // Old path: Host CLI 2.8.0 kept the first 25 bytes of this reply and
    // exited 0 (k14, case append-after-fstat).
    let (head, body) = recorded_linux_reply("append-after-fstat");
    assert_eq!(body.len(), 148, "the retained body");
    let mut bytes = head;
    bytes.extend_from_slice(&body);
    let (result, dir, dest) = download_over(raw_agent(vec![piece(bytes)], End::Close)).await;
    let err = result.expect_err("bytes past Content-Length must fail");
    assert_eq!(err.code(), "DOWNLOAD_FRAMING_INVALID", "got {err:?}");
    assert_unchanged(&dir, &dest);
}

#[tokio::test]
async fn extra_bytes_in_a_later_write_are_a_framing_failure() {
    let pieces = vec![
        piece(reply(5, &[0x00, 0xff, 0x80, 0x41, 0x0a])),
        Piece {
            after: Duration::from_millis(100),
            bytes: b"late".to_vec(),
        },
    ];
    let (result, dir, dest) = download_over(raw_agent(pieces, End::Close)).await;
    let err = result.expect_err("a late byte past Content-Length must fail");
    assert_eq!(err.code(), "DOWNLOAD_FRAMING_INVALID", "got {err:?}");
    assert_unchanged(&dir, &dest);
}

#[tokio::test]
async fn the_recorded_old_linux_reply_with_a_short_body_is_dropped() {
    // k14, case truncate-after-fstat: Content-Length 123 and 25 bytes.
    let (head, body) = recorded_linux_reply("truncate-after-fstat");
    assert_eq!(body.len(), 25, "the retained body");
    let mut bytes = head;
    bytes.extend_from_slice(&body);
    let (result, dir, dest) = download_over(raw_agent(vec![piece(bytes)], End::Close)).await;
    let err = result.expect_err("a short body must fail");
    assert_eq!(err.code(), "CONNECTION_DROPPED", "got {err:?}");
    assert_unchanged(&dir, &dest);
}

#[tokio::test]
async fn an_exact_body_replaces_the_destination() {
    // The control: the same raw agent with an exact body passes.
    let oracle = [0x00, 0xff, 0x80, 0x41, 0x0a];
    let (result, dir, dest) =
        download_over(raw_agent(vec![piece(reply(5, &oracle))], End::Close)).await;
    assert_eq!(result.expect("an exact body succeeds"), 5);
    assert_eq!(std::fs::read(&dest).unwrap(), oracle);
    assert_eq!(names_in(dir.path()), vec!["evidence.json".to_string()]);
}

#[tokio::test]
async fn a_dropped_download_removes_its_temp_file() {
    // A signal makes the Host CLI drop the download future. The guard of the
    // temp file must remove it with no further await.
    let connector = raw_agent(vec![piece(reply(1000, b"partial"))], End::Hold);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("evidence.json");
    let client = client_over(connector, Duration::from_secs(30));
    let task = {
        let dest = dest.clone();
        tokio::spawn(async move { client.download("/tmp/evidence.json", &dest).await })
    };
    let mut seen = Vec::new();
    for _ in 0..200 {
        seen = names_in(dir.path());
        if !seen.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        seen.len(),
        1,
        "one temp file while the body is open: {seen:?}"
    );
    assert!(
        seen[0].starts_with(".evidence.json.testanyware-"),
        "{seen:?}"
    );
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(names_in(dir.path()), Vec::<String>::new());
}
