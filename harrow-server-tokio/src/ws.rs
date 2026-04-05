//! WebSocket support for the Tokio server backend.
//!
//! # Example
//!
//! ```rust,ignore
//! use harrow::{App, Request, Response};
//! use harrow::ws::{Message, WebSocket};
//!
//! async fn ws_handler(req: Request) -> Response {
//!     harrow::ws::upgrade(req, |mut ws: WebSocket| async move {
//!         while let Some(Ok(msg)) = ws.recv().await {
//!             match msg {
//!                 Message::Text(text) => { ws.send(Message::Text(text)).await.ok(); }
//!                 Message::Close(_) => break,
//!                 _ => {}
//!             }
//!         }
//!     })
//!     .unwrap_or_else(|e| e.into_response())
//! }
//! ```

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::{Sink, SinkExt, Stream, StreamExt};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use harrow_core::request::Request;
use harrow_core::response::Response;
use harrow_core::ws::{
    Message, Utf8Bytes, WsError, negotiate_protocol, upgrade_response, validate_upgrade,
};

/// Configuration for a WebSocket connection.
///
/// Wraps tungstenite's `WebSocketConfig` with sensible defaults.
#[derive(Debug, Clone, Default)]
pub struct WsConfig {
    inner: WebSocketConfig,
    protocols: Vec<String>,
}

impl WsConfig {
    /// Maximum size of an incoming message (default: 64 MiB).
    pub fn max_message_size(mut self, size: usize) -> Self {
        self.inner.max_message_size = Some(size);
        self
    }

    /// Maximum size of a single frame (default: 16 MiB).
    pub fn max_frame_size(mut self, size: usize) -> Self {
        self.inner.max_frame_size = Some(size);
        self
    }

    /// Size of the write buffer (default: 128 KiB).
    pub fn write_buffer_size(mut self, size: usize) -> Self {
        self.inner.write_buffer_size = size;
        self
    }

    /// Maximum size of the write buffer. Provides backpressure when the
    /// buffer fills due to slow writes (default: unlimited).
    pub fn max_write_buffer_size(mut self, size: usize) -> Self {
        self.inner.max_write_buffer_size = size;
        self
    }

    /// Accept frames that are not masked by the client (default: false).
    pub fn accept_unmasked_frames(mut self, accept: bool) -> Self {
        self.inner.accept_unmasked_frames = accept;
        self
    }

    /// Set supported subprotocols in priority order.
    ///
    /// During the upgrade, the first protocol in this list that the client
    /// also requested will be selected and included in the 101 response.
    pub fn protocols<I, S>(mut self, protocols: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.protocols = protocols.into_iter().map(Into::into).collect();
        self
    }
}

/// A WebSocket connection handle.
///
/// Implements [`Stream`] and [`Sink`] for composable async patterns.
/// Use `.split()` (via `StreamExt`) for concurrent read/write.
pub struct WebSocket {
    inner: WebSocketStream<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>>,
    /// The negotiated subprotocol, if any.
    protocol: Option<String>,
}

impl WebSocket {
    /// The negotiated subprotocol, if any.
    pub fn protocol(&self) -> Option<&str> {
        self.protocol.as_deref()
    }

    /// Receive the next message from the client.
    ///
    /// Returns `None` when the connection is closed.
    /// Automatically skips raw frame messages and responds to close frames.
    pub async fn recv(&mut self) -> Option<Result<Message, WsError>> {
        use tokio_tungstenite::tungstenite::Message as TMsg;
        loop {
            match self.inner.next().await? {
                Ok(TMsg::Frame(_)) => continue,
                Ok(msg @ TMsg::Close(_)) => {
                    // Auto-respond with a close frame if the client initiated.
                    let _ = self.inner.close(None).await;
                    return Some(Ok(from_tungstenite(msg)));
                }
                Ok(msg) => return Some(Ok(from_tungstenite(msg))),
                Err(e) => return Some(Err(WsError::Transport(e.to_string()))),
            }
        }
    }

    /// Send a message to the client.
    pub async fn send(&mut self, msg: Message) -> Result<(), WsError> {
        self.inner
            .send(to_tungstenite(msg))
            .await
            .map_err(|e| WsError::Transport(e.to_string()))
    }

    /// Close the WebSocket connection gracefully.
    pub async fn close(mut self) -> Result<(), WsError> {
        self.inner
            .close(None)
            .await
            .map_err(|e| WsError::Transport(e.to_string()))
    }
}

impl Stream for WebSocket {
    type Item = Result<Message, WsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use tokio_tungstenite::tungstenite::Message as TMsg;
        loop {
            match futures_util::ready!(Pin::new(&mut self.inner).poll_next(cx)) {
                Some(Ok(TMsg::Frame(_))) => continue,
                Some(Ok(msg)) => return Poll::Ready(Some(Ok(from_tungstenite(msg)))),
                Some(Err(e)) => return Poll::Ready(Some(Err(WsError::Transport(e.to_string())))),
                None => return Poll::Ready(None),
            }
        }
    }
}

impl Sink<Message> for WebSocket {
    type Error = WsError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner)
            .poll_ready(cx)
            .map_err(|e| WsError::Transport(e.to_string()))
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        Pin::new(&mut self.inner)
            .start_send(to_tungstenite(item))
            .map_err(|e| WsError::Transport(e.to_string()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner)
            .poll_flush(cx)
            .map_err(|e| WsError::Transport(e.to_string()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Pin::new(&mut self.inner)
            .poll_close(cx)
            .map_err(|e| WsError::Transport(e.to_string()))
    }
}

/// Upgrade an HTTP request to a WebSocket connection with default configuration.
///
/// Validates the upgrade headers, returns a 101 response, and spawns a
/// task that calls `handler` with the established WebSocket connection.
///
/// Returns `Err(WsError)` if the request is not a valid WebSocket upgrade.
pub fn upgrade<F, Fut>(req: Request, handler: F) -> Result<Response, WsError>
where
    F: FnOnce(WebSocket) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    upgrade_with_config(req, WsConfig::default(), handler)
}

/// Upgrade an HTTP request to a WebSocket connection with custom configuration.
pub fn upgrade_with_config<F, Fut>(
    mut req: Request,
    config: WsConfig,
    handler: F,
) -> Result<Response, WsError>
where
    F: FnOnce(WebSocket) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let key = validate_upgrade(&req)?;

    // Negotiate subprotocol.
    let protocol_refs: Vec<&str> = config.protocols.iter().map(|s| s.as_str()).collect();
    let selected = if protocol_refs.is_empty() {
        None
    } else {
        negotiate_protocol(&req, &protocol_refs)
    };

    let resp = upgrade_response(&key, selected);

    let on_upgrade: hyper::upgrade::OnUpgrade = req
        .inner_mut()
        .extensions_mut()
        .remove()
        .ok_or(WsError::NotUpgradable)?;

    let ws_config = config.inner;
    let protocol = selected.map(String::from);

    tokio::spawn(async move {
        match on_upgrade.await {
            Ok(upgraded) => {
                let io = hyper_util::rt::TokioIo::new(upgraded);
                let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
                    io,
                    tokio_tungstenite::tungstenite::protocol::Role::Server,
                    Some(ws_config),
                )
                .await;
                let ws = WebSocket {
                    inner: ws_stream,
                    protocol,
                };
                handler(ws).await;
            }
            Err(e) => {
                tracing::error!("websocket upgrade failed: {e}");
            }
        }
    });

    Ok(resp)
}

fn from_tungstenite(msg: tokio_tungstenite::tungstenite::Message) -> Message {
    use tokio_tungstenite::tungstenite::Message as TMsg;
    match msg {
        TMsg::Text(s) => {
            // Zero-copy: tungstenite Utf8Bytes -> bytes::Bytes -> harrow Utf8Bytes.
            // Tungstenite already validated UTF-8.
            let bytes: bytes::Bytes = s.into();
            Message::Text(unsafe { Utf8Bytes::from_bytes_unchecked(bytes) })
        }
        TMsg::Binary(b) => Message::Binary(bytes::Bytes::from(b.to_vec())),
        TMsg::Ping(b) => Message::Ping(bytes::Bytes::from(b.to_vec())),
        TMsg::Pong(b) => Message::Pong(bytes::Bytes::from(b.to_vec())),
        TMsg::Close(frame) => Message::Close(frame.map(|f| (f.code.into(), f.reason.to_string()))),
        TMsg::Frame(_) => unreachable!("Frame messages are filtered in recv()"),
    }
}

fn to_tungstenite(msg: Message) -> tokio_tungstenite::tungstenite::Message {
    use tokio_tungstenite::tungstenite::Message as TMsg;
    match msg {
        Message::Text(s) => TMsg::Text(s.to_string().into()),
        Message::Binary(b) => TMsg::Binary(b.to_vec().into()),
        Message::Ping(b) => TMsg::Ping(b.to_vec().into()),
        Message::Pong(b) => TMsg::Pong(b.to_vec().into()),
        Message::Close(frame) => TMsg::Close(frame.map(|(code, reason)| {
            tokio_tungstenite::tungstenite::protocol::CloseFrame {
                code: code.into(),
                reason: reason.into(),
            }
        })),
    }
}
