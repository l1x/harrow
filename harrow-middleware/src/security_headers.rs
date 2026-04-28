use std::sync::Arc;

use harrow_core::handler::HandlerFuture;
use harrow_core::middleware::{Middleware, Next};
use harrow_core::request::Request;
use harrow_core::response::Response;

/// Default value for `X-Content-Type-Options`.
pub const DEFAULT_CONTENT_TYPE_OPTIONS: &str = "nosniff";
/// Default value for `X-Frame-Options`.
pub const DEFAULT_FRAME_OPTIONS: &str = "DENY";
/// Default value for `Referrer-Policy`.
pub const DEFAULT_REFERRER_POLICY: &str = "no-referrer";
/// Conservative default `Permissions-Policy` that disables common browser APIs.
pub const DEFAULT_PERMISSIONS_POLICY: &str = "camera=(), microphone=(), geolocation=()";

/// Configuration for [`SecurityHeadersMiddleware`].
///
/// The default policy is intentionally conservative and does not include
/// `Strict-Transport-Security` or `Content-Security-Policy`, because those are
/// deployment/application-specific. Enable them explicitly once your TLS and
/// asset policy are known.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecurityHeadersConfig {
    pub content_type_options: Option<String>,
    pub frame_options: Option<String>,
    pub referrer_policy: Option<String>,
    pub permissions_policy: Option<String>,
    pub content_security_policy: Option<String>,
    pub strict_transport_security: Option<String>,
    /// When false, existing response headers are preserved. Default: false.
    pub override_existing: bool,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            content_type_options: Some(DEFAULT_CONTENT_TYPE_OPTIONS.into()),
            frame_options: Some(DEFAULT_FRAME_OPTIONS.into()),
            referrer_policy: Some(DEFAULT_REFERRER_POLICY.into()),
            permissions_policy: Some(DEFAULT_PERMISSIONS_POLICY.into()),
            content_security_policy: None,
            strict_transport_security: None,
            override_existing: false,
        }
    }
}

impl SecurityHeadersConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn content_type_options(mut self, value: impl Into<String>) -> Self {
        self.content_type_options = Some(value.into());
        self
    }

    pub fn without_content_type_options(mut self) -> Self {
        self.content_type_options = None;
        self
    }

    pub fn frame_options(mut self, value: impl Into<String>) -> Self {
        self.frame_options = Some(value.into());
        self
    }

    pub fn without_frame_options(mut self) -> Self {
        self.frame_options = None;
        self
    }

    pub fn referrer_policy(mut self, value: impl Into<String>) -> Self {
        self.referrer_policy = Some(value.into());
        self
    }

    pub fn without_referrer_policy(mut self) -> Self {
        self.referrer_policy = None;
        self
    }

    pub fn permissions_policy(mut self, value: impl Into<String>) -> Self {
        self.permissions_policy = Some(value.into());
        self
    }

    pub fn without_permissions_policy(mut self) -> Self {
        self.permissions_policy = None;
        self
    }

    pub fn content_security_policy(mut self, value: impl Into<String>) -> Self {
        self.content_security_policy = Some(value.into());
        self
    }

    pub fn without_content_security_policy(mut self) -> Self {
        self.content_security_policy = None;
        self
    }

    pub fn strict_transport_security(mut self, value: impl Into<String>) -> Self {
        self.strict_transport_security = Some(value.into());
        self
    }

    pub fn without_strict_transport_security(mut self) -> Self {
        self.strict_transport_security = None;
        self
    }

    pub fn override_existing(mut self, yes: bool) -> Self {
        self.override_existing = yes;
        self
    }
}

/// Return middleware that applies common HTTP security headers.
pub fn security_headers_middleware(config: SecurityHeadersConfig) -> SecurityHeadersMiddleware {
    SecurityHeadersMiddleware {
        config: Arc::new(config),
    }
}

pub struct SecurityHeadersMiddleware {
    config: Arc<SecurityHeadersConfig>,
}

impl Middleware for SecurityHeadersMiddleware {
    fn call(&self, req: Request, next: Next) -> HandlerFuture {
        let config = Arc::clone(&self.config);
        Box::pin(async move {
            let resp = next.run(req).await;
            apply_security_headers(resp, &config)
        })
    }
}

fn apply_security_headers(mut resp: Response, config: &SecurityHeadersConfig) -> Response {
    resp = set_header(
        resp,
        "x-content-type-options",
        config.content_type_options.as_deref(),
        config.override_existing,
    );
    resp = set_header(
        resp,
        "x-frame-options",
        config.frame_options.as_deref(),
        config.override_existing,
    );
    resp = set_header(
        resp,
        "referrer-policy",
        config.referrer_policy.as_deref(),
        config.override_existing,
    );
    resp = set_header(
        resp,
        "permissions-policy",
        config.permissions_policy.as_deref(),
        config.override_existing,
    );
    resp = set_header(
        resp,
        "content-security-policy",
        config.content_security_policy.as_deref(),
        config.override_existing,
    );
    set_header(
        resp,
        "strict-transport-security",
        config.strict_transport_security.as_deref(),
        config.override_existing,
    )
}

fn set_header(
    resp: Response,
    name: &str,
    value: Option<&str>,
    override_existing: bool,
) -> Response {
    let Some(value) = value else {
        return resp;
    };

    if !override_existing && resp.inner().headers().contains_key(name) {
        return resp;
    }

    resp.header(name, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use harrow_core::middleware::Middleware;
    use harrow_core::path::PathMatch;
    use harrow_core::request::full_body;
    use harrow_core::state::TypeMap;
    use std::sync::Arc;

    async fn make_request() -> Request {
        let inner = http::Request::builder()
            .method("GET")
            .uri("/")
            .body(full_body(http_body_util::Full::new(bytes::Bytes::new())))
            .unwrap();
        Request::new(inner, PathMatch::default(), Arc::new(TypeMap::new()), None)
    }

    fn ok_next() -> Next {
        Next::new(|_req| Box::pin(async { Response::text("ok") }))
    }

    #[tokio::test]
    async fn default_policy_sets_conservative_headers() {
        let mw = security_headers_middleware(SecurityHeadersConfig::default());
        let resp = mw.call(make_request().await, ok_next()).await.into_inner();

        assert_eq!(
            resp.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
        assert_eq!(
            resp.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );
        assert!(resp.headers().get("permissions-policy").is_some());
        assert!(resp.headers().get("content-security-policy").is_none());
        assert!(resp.headers().get("strict-transport-security").is_none());
    }

    #[tokio::test]
    async fn optional_policy_headers_can_be_enabled() {
        let mw = security_headers_middleware(
            SecurityHeadersConfig::default()
                .content_security_policy("default-src 'self'")
                .strict_transport_security("max-age=31536000; includeSubDomains"),
        );
        let resp = mw.call(make_request().await, ok_next()).await.into_inner();

        assert_eq!(
            resp.headers().get("content-security-policy").unwrap(),
            "default-src 'self'"
        );
        assert_eq!(
            resp.headers().get("strict-transport-security").unwrap(),
            "max-age=31536000; includeSubDomains"
        );
    }

    #[tokio::test]
    async fn existing_headers_are_preserved_by_default() {
        let mw = security_headers_middleware(SecurityHeadersConfig::default());
        let next = Next::new(|_req| {
            Box::pin(async { Response::text("ok").header("x-frame-options", "SAMEORIGIN") })
        });
        let resp = mw.call(make_request().await, next).await.into_inner();

        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "SAMEORIGIN");
    }

    #[tokio::test]
    async fn existing_headers_can_be_overridden() {
        let mw =
            security_headers_middleware(SecurityHeadersConfig::default().override_existing(true));
        let next = Next::new(|_req| {
            Box::pin(async { Response::text("ok").header("x-frame-options", "SAMEORIGIN") })
        });
        let resp = mw.call(make_request().await, next).await.into_inner();

        assert_eq!(resp.headers().get("x-frame-options").unwrap(), "DENY");
    }

    #[tokio::test]
    async fn headers_can_be_disabled() {
        let mw = security_headers_middleware(
            SecurityHeadersConfig::default()
                .without_frame_options()
                .without_permissions_policy(),
        );
        let resp = mw.call(make_request().await, ok_next()).await.into_inner();

        assert!(resp.headers().get("x-frame-options").is_none());
        assert!(resp.headers().get("permissions-policy").is_none());
        assert!(resp.headers().get("x-content-type-options").is_some());
    }
}
