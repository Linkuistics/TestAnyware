//! The byte-stream seam under the agent client (ADR-0004).
//!
//! The agent client speaks HTTP/1.1 through hyper's client connection, which
//! runs on any async byte stream. An [`AgentConnector`] supplies that stream:
//! [`TcpConnector`] dials the guest directly, and a caller can plug in any
//! other stream, such as a guest stream from the TestAnyware app, without
//! touching the protocol code.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::AgentError;

/// Any bidirectional async byte stream the agent client can run HTTP on.
pub trait AgentStream: AsyncRead + AsyncWrite + Unpin + Send + 'static {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> AgentStream for T {}

/// A connected stream with its concrete type erased.
pub type BoxedStream = Box<dyn AgentStream>;

/// The future of one dial.
pub type ConnectFuture = Pin<Box<dyn Future<Output = io::Result<BoxedStream>> + Send>>;

/// Opens one stream to the agent. The client calls it once for each request
/// and never reuses the stream, so an implementation holds no pool.
pub trait AgentConnector: Send + Sync + 'static {
    fn connect(&self) -> ConnectFuture;
}

/// The default connector: a direct TCP connection, as every host used before
/// the TestAnyware app.
#[derive(Debug, Clone)]
pub struct TcpConnector {
    host: String,
    port: u16,
}

impl TcpConnector {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }
}

impl AgentConnector for TcpConnector {
    fn connect(&self) -> ConnectFuture {
        let host = self.host.clone();
        let port = self.port;
        Box::pin(async move {
            let stream = TcpStream::connect((host.as_str(), port)).await?;
            // Best effort: small JSON requests should not wait for Nagle.
            let _ = stream.set_nodelay(true);
            Ok(Box::new(stream) as BoxedStream)
        })
    }
}

/// One absolute time limit for a whole request, from the dial to the last
/// byte of the body. Every await of a request runs under the same instant.
pub(crate) struct Deadline {
    at: Instant,
    limit: Duration,
}

impl Deadline {
    pub(crate) fn after(limit: Duration) -> Self {
        Self {
            at: Instant::now() + limit,
            limit,
        }
    }

    pub(crate) async fn run<T>(
        &self,
        step: &str,
        work: impl Future<Output = Result<T, AgentError>>,
    ) -> Result<T, AgentError> {
        match tokio::time::timeout_at(self.at, work).await {
            Ok(outcome) => outcome,
            Err(_) => Err(AgentError::Timeout(format!(
                "{step} did not finish within {:?}",
                self.limit
            ))),
        }
    }
}

/// Aborts the task that drives a connection when the request that owns it
/// ends, whether it finished, failed or timed out.
pub(crate) struct AbortOnDrop(pub(crate) JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
