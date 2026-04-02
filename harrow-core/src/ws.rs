//! WebSocket upgrade handshake and shared types.
//!
//! This module provides the runtime-agnostic parts of WebSocket support:
//! - Handshake validation and accept key computation
//! - Shared message types
//!
//! The actual upgrade and frame handling is implemented by the server backends
//! (`harrow-server-tokio`, `harrow-server-monoio`).

use http::StatusCode;
use http::header::{
    CONNECTION, SEC_WEBSOCKET_ACCEPT, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE,
};

use crate::request::Request;
use crate::response::Response;

/// The WebSocket GUID used in the Sec-WebSocket-Accept computation (RFC 6455).
const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-5AB53F3B86DB";

/// Errors that can occur during WebSocket handshake validation.
#[derive(Debug)]
pub enum WsError {
    /// Missing or incorrect `Upgrade: websocket` header.
    MissingUpgrade,
    /// Missing or incorrect `Connection: Upgrade` header.
    MissingConnection,
    /// Missing `Sec-WebSocket-Key` header.
    MissingKey,
    /// Missing or unsupported `Sec-WebSocket-Version` (must be "13").
    UnsupportedVersion,
}

impl std::fmt::Display for WsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsError::MissingUpgrade => write!(f, "missing Upgrade: websocket header"),
            WsError::MissingConnection => write!(f, "missing Connection: Upgrade header"),
            WsError::MissingKey => write!(f, "missing Sec-WebSocket-Key header"),
            WsError::UnsupportedVersion => {
                write!(f, "unsupported Sec-WebSocket-Version (expected 13)")
            }
        }
    }
}

impl std::error::Error for WsError {}

impl crate::response::IntoResponse for WsError {
    fn into_response(self) -> Response {
        Response::new(StatusCode::BAD_REQUEST, self.to_string())
    }
}

/// Validate that a request is a valid WebSocket upgrade request.
/// Returns the `Sec-WebSocket-Key` value on success.
pub fn validate_upgrade(req: &Request) -> Result<String, WsError> {
    // Check Upgrade: websocket
    let upgrade = req
        .header(UPGRADE.as_str())
        .ok_or(WsError::MissingUpgrade)?;
    if !upgrade.eq_ignore_ascii_case("websocket") {
        return Err(WsError::MissingUpgrade);
    }

    // Check Connection: Upgrade
    let conn = req
        .header(CONNECTION.as_str())
        .ok_or(WsError::MissingConnection)?;
    if !conn.to_ascii_lowercase().contains("upgrade") {
        return Err(WsError::MissingConnection);
    }

    // Check Sec-WebSocket-Version: 13
    let version = req
        .header(SEC_WEBSOCKET_VERSION.as_str())
        .ok_or(WsError::UnsupportedVersion)?;
    if version != "13" {
        return Err(WsError::UnsupportedVersion);
    }

    // Extract Sec-WebSocket-Key
    let key = req
        .header(SEC_WEBSOCKET_KEY.as_str())
        .ok_or(WsError::MissingKey)?;

    Ok(key.to_string())
}

/// Compute the `Sec-WebSocket-Accept` value from the client's key (RFC 6455 Section 4.2.2).
pub fn accept_key(key: &str) -> String {
    use base64::Engine;
    use sha1::{Digest, Sha1};

    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(WS_GUID.as_bytes());
    let hash = hasher.finalize();

    base64::engine::general_purpose::STANDARD.encode(hash)
}

/// Build the HTTP 101 Switching Protocols response for a WebSocket upgrade.
pub fn upgrade_response(key: &str) -> Response {
    let accept = accept_key(key);
    Response::new(StatusCode::SWITCHING_PROTOCOLS, "")
        .header(UPGRADE.as_str(), "websocket")
        .header(CONNECTION.as_str(), "Upgrade")
        .header(SEC_WEBSOCKET_ACCEPT.as_str(), &accept)
}

/// WebSocket message types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// UTF-8 text message.
    Text(String),
    /// Binary message.
    Binary(Vec<u8>),
    /// Ping message.
    Ping(Vec<u8>),
    /// Pong message.
    Pong(Vec<u8>),
    /// Close message with optional code and reason.
    Close(Option<(u16, String)>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_key_is_deterministic() {
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept1 = accept_key(key);
        let accept2 = accept_key(key);
        assert_eq!(accept1, accept2);
        // Verify it's valid base64 and 28 chars (SHA-1 = 20 bytes → 28 base64 chars)
        assert_eq!(accept1.len(), 28);
    }

    #[test]
    fn accept_key_differs_for_different_inputs() {
        let a = accept_key("key1");
        let b = accept_key("key2");
        assert_ne!(a, b);
    }

    #[test]
    fn upgrade_response_has_correct_headers() {
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let resp = upgrade_response(key);
        assert_eq!(resp.status_code(), StatusCode::SWITCHING_PROTOCOLS);
        let inner = resp.into_inner();
        assert_eq!(inner.headers().get(UPGRADE).unwrap(), "websocket");
        assert_eq!(inner.headers().get(CONNECTION).unwrap(), "Upgrade");
        assert!(inner.headers().get(SEC_WEBSOCKET_ACCEPT).is_some());
        assert_eq!(
            inner.headers().get(SEC_WEBSOCKET_ACCEPT).unwrap(),
            &accept_key(key),
        );
    }
}
