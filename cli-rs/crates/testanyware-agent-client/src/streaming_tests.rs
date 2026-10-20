//! Proof that an upload and a download stream their bodies (ADR-0001).
//!
//! A file of `TOTAL` bytes crosses a duplex stream that holds 64 KiB. One side
//! generates the bytes on demand and the other side discards them, so no
//! buffer in the test holds the file. Each test measures the largest gap
//! between bytes produced and bytes consumed. A client that held the whole
//! file would show a gap of `TOTAL`. A streaming client shows a gap of a few
//! buffers.

use std::convert::Infallible;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::Frame;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::io::{duplex, AsyncRead, AsyncWrite, ReadBuf};

use super::*;

const TOTAL: u64 = 32 * 1024 * 1024;
const CHUNK: usize = 64 * 1024;
/// Larger than any buffer on the path: the duplex stream, hyper's own, and the
/// chunk in flight. Far smaller than `TOTAL`.
const MAX_GAP: u64 = 2 * 1024 * 1024;

struct OneShotConnector(std::sync::Mutex<Option<tokio::io::DuplexStream>>);

impl AgentConnector for OneShotConnector {
    fn connect(&self) -> ConnectFuture {
        let stream = self.0.lock().unwrap().take();
        Box::pin(async move {
            match stream {
                Some(s) => Ok(Box::new(s) as BoxedStream),
                None => Err(io::Error::other("the test connector dials once")),
            }
        })
    }
}

fn client_over(stream: tokio::io::DuplexStream) -> AgentClient {
    AgentClient::with_connector(
        AgentConfig::new("agent.test", 8648).with_timeout(Duration::from_secs(60)),
        Arc::new(OneShotConnector(std::sync::Mutex::new(Some(stream)))),
    )
}

/// Largest value seen of `produced - consumed`.
#[derive(Default)]
struct Gauge {
    produced: AtomicU64,
    consumed: AtomicU64,
    max_gap: AtomicU64,
}

impl Gauge {
    fn produce(&self, n: u64) {
        self.produced.fetch_add(n, Ordering::SeqCst);
    }

    /// Record `n` bytes consumed. The gap is taken before this write counts,
    /// because it is the lag that the consumer showed when the bytes arrived.
    fn consume(&self, n: u64) {
        let before = self.consumed.fetch_add(n, Ordering::SeqCst);
        let gap = self.produced.load(Ordering::SeqCst).saturating_sub(before);
        self.max_gap.fetch_max(gap, Ordering::SeqCst);
    }
}

/// Zero bytes, generated as the reader asks for them.
struct GeneratedReader {
    left: u64,
    gauge: Arc<Gauge>,
}

impl AsyncRead for GeneratedReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let n = (buf.remaining() as u64).min(self.left) as usize;
        buf.put_slice(&vec![0u8; n]);
        self.left -= n as u64;
        self.gauge.produce(n as u64);
        Poll::Ready(Ok(()))
    }
}

/// Counts what it is given and keeps none of it.
struct CountingSink {
    gauge: Arc<Gauge>,
}

impl AsyncWrite for CountingSink {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.gauge.consume(buf.len() as u64);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[tokio::test]
async fn an_upload_streams_and_nothing_holds_the_whole_file() {
    let gauge = Arc::new(Gauge::default());
    let (client_end, server_end) = duplex(CHUNK);

    // The agent discards the body and reports what it counted.
    let server_gauge = gauge.clone();
    let server = tokio::spawn(async move {
        let service = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
            let gauge = server_gauge.clone();
            async move {
                let advertised = req.headers()["content-length"]
                    .to_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap();
                let mut body = req.into_body();
                while let Some(frame) = body.frame().await {
                    if let Ok(data) = frame.unwrap().into_data() {
                        gauge.consume(data.len() as u64);
                    }
                }
                assert_eq!(advertised, TOTAL);
                Ok::<_, Infallible>(hyper::Response::new(Full::new(Bytes::from_static(
                    br#"{"success":true}"#,
                ))))
            }
        });
        hyper::server::conn::http1::Builder::new()
            .serve_connection(TokioIo::new(server_end), service)
            .await
    });

    let reader = GeneratedReader {
        left: TOTAL,
        gauge: gauge.clone(),
    };
    let sent = client_over(client_end)
        .upload_reader("/tmp/big.bin", reader, TOTAL)
        .await
        .expect("upload succeeds");
    server
        .await
        .unwrap()
        .expect("server connection ends cleanly");

    assert_eq!(sent, TOTAL);
    assert_eq!(gauge.consumed.load(Ordering::SeqCst), TOTAL);
    let gap = gauge.max_gap.load(Ordering::SeqCst);
    assert!(
        gap <= MAX_GAP,
        "up to {gap} bytes were held; the limit is {MAX_GAP}"
    );
}

#[tokio::test]
async fn a_download_streams_and_nothing_holds_the_whole_file() {
    let gauge = Arc::new(Gauge::default());
    let (client_end, server_end) = duplex(CHUNK);

    // The agent generates the body as hyper asks for each chunk.
    let server_gauge = gauge.clone();
    let server = tokio::spawn(async move {
        let service = service_fn(move |_req: hyper::Request<hyper::body::Incoming>| {
            let gauge = server_gauge.clone();
            async move {
                let chunks = futures_util::stream::unfold(0u64, move |sent| {
                    let gauge = gauge.clone();
                    async move {
                        if sent >= TOTAL {
                            return None;
                        }
                        gauge.produce(CHUNK as u64);
                        let frame = Frame::data(Bytes::from(vec![0u8; CHUNK]));
                        Some((Ok::<_, Infallible>(frame), sent + CHUNK as u64))
                    }
                });
                let mut response = hyper::Response::new(StreamBody::new(chunks));
                response
                    .headers_mut()
                    .insert("content-length", TOTAL.to_string().parse().unwrap());
                Ok::<_, Infallible>(response)
            }
        });
        hyper::server::conn::http1::Builder::new()
            .serve_connection(TokioIo::new(server_end), service)
            .await
    });

    let client = client_over(client_end);
    let deadline = Deadline::after(Duration::from_secs(60));
    let exchange = client
        .open_download(&deadline, "/tmp/big.bin")
        .await
        .expect("download opens");
    let mut sink = CountingSink {
        gauge: gauge.clone(),
    };
    let written = stream_to_writer(&deadline, exchange, &mut sink, Path::new("big.bin.tmp"))
        .await
        .expect("download streams");
    server
        .await
        .unwrap()
        .expect("server connection ends cleanly");

    assert_eq!(written, TOTAL);
    assert_eq!(gauge.consumed.load(Ordering::SeqCst), TOTAL);
    let gap = gauge.max_gap.load(Ordering::SeqCst);
    assert!(
        gap <= MAX_GAP,
        "up to {gap} bytes were held; the limit is {MAX_GAP}"
    );
}
