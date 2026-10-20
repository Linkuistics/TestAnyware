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
