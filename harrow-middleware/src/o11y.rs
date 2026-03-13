use std::sync::Arc;
use std::time::Instant;

use rolly::constants::fields;
use tracing::Instrument;

use harrow_core::middleware::Next;
use harrow_core::request::Request;
use harrow_core::response::Response;

use harrow_o11y::O11yConfig;

/// Built-in observability middleware.
///
/// Creates a tracing span with a `trace_id` field that rolly's OtlpLayer
/// picks up automatically for OTLP export. Generates request IDs via
/// `rolly::trace_id`, records RED metric events, and echoes the request
/// ID header in the response.
///
/// Reads `Arc<O11yConfig>` from application state. If not present, falls back
/// to `O11yConfig::default()`.
pub async fn o11y_middleware(mut req: Request, next: Next) -> Response {
    let default_config = Arc::new(O11yConfig::default());
    let config = req
        .try_state::<Arc<O11yConfig>>()
        .cloned()
        .unwrap_or(default_config);

    let method = req.method().to_string();
    let path = req.path().to_string();
    let route = req
        .route_pattern()
        .unwrap_or_else(|| req.path())
        .to_string();

    // Extract request ID from header, or generate a new trace ID.
    let request_id = req
        .header(&config.request_id_header)
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let id = rolly::trace_id::generate_trace_id(None);
            rolly::trace_id::hex_encode(&id)
        });

    req.set_request_id(request_id.clone());

    // Create span with trace_id field — OtlpLayer picks this up automatically.
    let span = tracing::info_span!(
        "http_request",
        { fields::TRACE_ID } = request_id.as_str(),
        { fields::HTTP_METHOD } = %method,
        { fields::HTTP_URI } = %path,
        route = %route,
        request_id = %request_id,
    );

    let start = Instant::now();
    let resp = next.run(req).instrument(span).await;
    let duration = start.elapsed();
    let status = resp.status_code().as_u16();

    tracing::info!(
        { fields::HTTP_METHOD } = %method,
        { fields::HTTP_URI } = %path,
        route = %route,
        { fields::HTTP_STATUS_CODE } = status,
        { fields::HTTP_LATENCY_MS } = duration.as_secs_f64() * 1000.0,
        request_id = %request_id,
        "request completed"
    );

    resp.header(&config.request_id_header, &request_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use harrow_core::middleware::Middleware;
    use harrow_core::path::PathMatch;
    use harrow_core::state::TypeMap;

    async fn make_request_with_state(headers: &[(&str, &str)], state: TypeMap) -> Request {
        let mut builder = http::Request::builder().method("GET").uri("/test");
        for &(name, value) in headers {
            builder = builder.header(name, value);
        }
        let inner = builder
            .body(harrow_core::request::full_body(http_body_util::Full::new(
                bytes::Bytes::new(),
            )))
            .unwrap();
        Request::new(inner, PathMatch::default(), Arc::new(state), None)
    }

    fn ok_next() -> Next {
        Next::new(|_req| Box::pin(async { Response::ok() }))
    }

    // Ensure tracing subscriber is installed for tests (no-op if already set).
    fn init_tracing() {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    }

    #[tokio::test]
    async fn generates_request_id_when_absent() {
        init_tracing();
        let req = make_request_with_state(&[], TypeMap::new()).await;
        let resp = Middleware::call(&o11y_middleware, req, ok_next()).await;
        let inner = resp.into_inner();
        let rid = inner
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(rid.len(), 32);
        assert!(rid.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn preserves_request_id_from_header() {
        init_tracing();
        let req =
            make_request_with_state(&[("x-request-id", "incoming-id-123")], TypeMap::new()).await;
        let resp = Middleware::call(&o11y_middleware, req, ok_next()).await;
        let inner = resp.into_inner();
        assert_eq!(
            inner.headers().get("x-request-id").unwrap(),
            "incoming-id-123"
        );
    }

    #[tokio::test]
    async fn uses_config_from_state() {
        init_tracing();
        let config = O11yConfig::default().request_id_header("x-trace-id");
        let mut state = TypeMap::new();
        state.insert(Arc::new(config));
        let req = make_request_with_state(&[("x-trace-id", "custom-trace")], state).await;
        let resp = Middleware::call(&o11y_middleware, req, ok_next()).await;
        let inner = resp.into_inner();
        assert_eq!(inner.headers().get("x-trace-id").unwrap(), "custom-trace");
        // Default header should not be set.
        assert!(inner.headers().get("x-request-id").is_none());
    }

    #[tokio::test]
    async fn falls_back_to_default_config() {
        init_tracing();
        // Empty state — no O11yConfig registered.
        let req = make_request_with_state(&[], TypeMap::new()).await;
        let resp = Middleware::call(&o11y_middleware, req, ok_next()).await;
        // Should not panic and should use default "x-request-id".
        let inner = resp.into_inner();
        assert!(inner.headers().get("x-request-id").is_some());
    }

    #[tokio::test]
    async fn sets_request_id_on_request_object() {
        init_tracing();
        // Capture the request_id from inside the handler.
        let captured = Arc::new(std::sync::Mutex::new(None::<String>));
        let captured_clone = Arc::clone(&captured);
        let next = Next::new(move |req: Request| {
            let captured = Arc::clone(&captured_clone);
            Box::pin(async move {
                *captured.lock().unwrap() = req.request_id().map(String::from);
                Response::ok()
            })
        });
        let req = make_request_with_state(&[], TypeMap::new()).await;
        let resp = Middleware::call(&o11y_middleware, req, next).await;
        let inner = resp.into_inner();
        let response_rid = inner
            .headers()
            .get("x-request-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let captured_rid = captured.lock().unwrap().clone().unwrap();
        assert_eq!(captured_rid, response_rid);
    }
}
