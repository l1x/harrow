//! # Harrow
//!
//! A thin, macro-free HTTP framework over Hyper with built-in observability.

pub use harrow_core::handler;
pub use harrow_core::middleware::{Middleware, Next};
pub use harrow_core::path::PathPattern;
pub use harrow_core::request::{BodyError, Request};
pub use harrow_core::response::{IntoResponse, Response};
pub use harrow_core::route::{App, Group, Route, RouteMetadata, RouteTable};
pub use harrow_core::state::TypeMap;

pub use harrow_server::{serve, serve_with_shutdown};

#[cfg(feature = "o11y")]
pub mod o11y {
    pub use harrow_o11y::o11y_middleware::o11y_middleware;
    pub use harrow_o11y::request_id;
    pub use harrow_o11y::{record_request, O11yConfig};
}
