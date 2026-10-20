//! The body of a response: a document that is ready, or events that arrive while a start runs.

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::{Body, Frame, SizeHint};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

/// What a response sends.
pub enum AppBody {
    /// A document that is complete when the response starts.
    Full(Full<Bytes>),
    /// The events of a start. The body ends when the sender of the channel is gone.
    Events(EventBody),
}

impl AppBody {
    pub fn full(bytes: impl Into<Bytes>) -> Self {
        AppBody::Full(Full::new(bytes.into()))
    }

    pub fn events(receiver: UnboundedReceiver<Bytes>, on_drop: CancellationToken) -> Self {
        AppBody::Events(EventBody { receiver, on_drop })
    }
}

/// The lines of a start, as they come. Dropping the body cancels `on_drop`: hyper drops it when
/// the response ends, and when the connection of the client goes away first. A start that has
/// ended ignores the cancel, and a start that runs stops and cleans up (spec: "The protocol").
pub struct EventBody {
    receiver: UnboundedReceiver<Bytes>,
    on_drop: CancellationToken,
}

impl Drop for EventBody {
    fn drop(&mut self) {
        self.on_drop.cancel();
    }
}

impl Body for AppBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        match self.get_mut() {
            AppBody::Full(full) => Pin::new(full).poll_frame(cx),
            AppBody::Events(events) => events
                .receiver
                .poll_recv(cx)
                .map(|chunk| chunk.map(|bytes| Ok(Frame::data(bytes)))),
        }
    }

    fn is_end_stream(&self) -> bool {
        match self {
            AppBody::Full(full) => full.is_end_stream(),
            AppBody::Events(_) => false,
        }
    }

    fn size_hint(&self) -> SizeHint {
        match self {
            AppBody::Full(full) => full.size_hint(),
            AppBody::Events(_) => SizeHint::default(),
        }
    }
}
