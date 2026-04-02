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

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::WebSocketStream;

use harrow_core::request::Request;
use harrow_core::response::Response;
use harrow_core::ws::{Message, WsError, upgrade_response, validate_upgrade};

/// A WebSocket connection handle.
///
/// Provides `send` and `recv` for bidirectional messaging.
pub struct WebSocket {
    inner: WebSocketStream<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>>,
}

impl WebSocket {
    /// Receive the next message from the client.
    ///
    /// Returns `None` when the connection is closed.
    pub async fn recv(&mut self) -> Option<Result<Message, WsError>> {
        loop {
            match self.inner.next().await? {
                Ok(msg) => return Some(Ok(from_tungstenite(msg))),
                Err(_) => return Some(Err(WsError::MissingConnection)),
            }
        }
    }

    /// Send a message to the client.
    pub async fn send(&mut self, msg: Message) -> Result<(), Box<dyn std::error::Error>> {
        self.inner
            .send(to_tungstenite(msg))
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    /// Close the WebSocket connection gracefully.
    pub async fn close(mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.inner
            .close(None)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

/// Upgrade an HTTP request to a WebSocket connection.
///
/// Validates the upgrade headers, returns a 101 response, and spawns a
/// task that calls `handler` with the established WebSocket connection.
///
/// Returns `Err(WsError)` if the request is not a valid WebSocket upgrade.
pub fn upgrade<F, Fut>(mut req: Request, handler: F) -> Result<Response, WsError>
where
    F: FnOnce(WebSocket) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let key = validate_upgrade(&req)?;
    let resp = upgrade_response(&key);

    // Extract the OnUpgrade handle from hyper's request extensions.
    let on_upgrade: Option<hyper::upgrade::OnUpgrade> = req.inner_mut().extensions_mut().remove();

    if let Some(on_upgrade) = on_upgrade {
        tokio::spawn(async move {
            match on_upgrade.await {
                Ok(upgraded) => {
                    let io = hyper_util::rt::TokioIo::new(upgraded);
                    let ws_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(
                        io,
                        tokio_tungstenite::tungstenite::protocol::Role::Server,
                        None,
                    )
                    .await;
                    let ws = WebSocket { inner: ws_stream };
                    handler(ws).await;
                }
                Err(e) => {
                    tracing::error!("websocket upgrade failed: {e}");
                }
            }
        });
    }

    Ok(resp)
}

fn from_tungstenite(msg: tokio_tungstenite::tungstenite::Message) -> Message {
    use tokio_tungstenite::tungstenite::Message as TMsg;
    match msg {
        TMsg::Text(s) => Message::Text(s.to_string()),
        TMsg::Binary(b) => Message::Binary(b.to_vec()),
        TMsg::Ping(b) => Message::Ping(b.to_vec()),
        TMsg::Pong(b) => Message::Pong(b.to_vec()),
        TMsg::Close(frame) => Message::Close(frame.map(|f| (f.code.into(), f.reason.to_string()))),
        TMsg::Frame(_) => Message::Binary(Vec::new()),
    }
}

fn to_tungstenite(msg: Message) -> tokio_tungstenite::tungstenite::Message {
    use tokio_tungstenite::tungstenite::Message as TMsg;
    match msg {
        Message::Text(s) => TMsg::Text(s.into()),
        Message::Binary(b) => TMsg::Binary(b.into()),
        Message::Ping(b) => TMsg::Ping(b.into()),
        Message::Pong(b) => TMsg::Pong(b.into()),
        Message::Close(frame) => TMsg::Close(frame.map(|(code, reason)| {
            tokio_tungstenite::tungstenite::protocol::CloseFrame {
                code: code.into(),
                reason: reason.into(),
            }
        })),
    }
}
