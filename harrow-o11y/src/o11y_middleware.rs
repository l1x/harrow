use std::time::Instant;

use harrow_core::middleware::Next;
use harrow_core::request::Request;
use harrow_core::response::Response;

use crate::request_id;

/// Built-in observability middleware.
/// Adds a tracing span, request ID, and records latency + status metrics.
#[cfg_attr(feature = "profiling", inline(never))]
pub async fn o11y_middleware(req: Request, next: Next) -> Response {
    let request_id = req
        .header("x-request-id")
        .map(|s| s.to_string())
        .unwrap_or_else(request_id::generate);

    let method = req.method().to_string();
    let path = req.path().to_string();

    let span = tracing::info_span!(
        "http_request",
        method = %method,
        path = %path,
        request_id = %request_id,
    );

    let start = Instant::now();

    let resp = {
        let _enter = span.enter();
        next.run(req).await
    };

    let duration = start.elapsed();
    let status = resp.status_code().as_u16();

    tracing::info!(
        method = %method,
        path = %path,
        status = status,
        duration_ms = duration.as_secs_f64() * 1000.0,
        request_id = %request_id,
        "request completed"
    );

    crate::record_request(&path, &method, status, duration);

    resp.header("x-request-id", &request_id)
}
