use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Instant;

use rolly::constants::fields;
use tracing::Instrument;

use harrow_core::middleware::Next;
use harrow_core::request::Request;
use harrow_core::response::Response;

use harrow_o11y::O11yConfig;

// --- Fast request-ID generation (atomic counter, no RNG) ----------------

/// URL-safe alphabet (64 characters = 6 bits per character).
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Hex digits for trace ID encoding.
const HEX: &[u8; 16] = b"0123456789abcdef";

/// Global monotonic counter — one relaxed atomic increment per request.
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Default config — allocated once, reused forever.
static DEFAULT_CONFIG: LazyLock<Arc<O11yConfig>> =
    LazyLock::new(|| Arc::new(O11yConfig::default()));

// --- HTTP server metrics -------------------------------------------------

struct HttpServerMetrics {
    duration: rolly::metrics::Histogram,
    errors: rolly::metrics::Counter,
}

const DURATION_BOUNDARIES: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
];

static HTTP_METRICS: LazyLock<HttpServerMetrics> = LazyLock::new(|| {
    let registry = rolly::metrics::global_registry();
    HttpServerMetrics {
        duration: registry.histogram(
            rolly::constants::metrics::REQUEST_DURATION,
            "Duration of HTTP server requests",
            DURATION_BOUNDARIES,
        ),
        errors: registry.counter(
            rolly::constants::metrics::ERROR_COUNT,
            "Count of HTTP server errors (5xx)",
        ),
    }
});

/// Generate a unique 11-character URL-safe request ID.
///
/// Atomic counter + base64 encoding.
/// No RNG, no syscalls — one relaxed atomic fetch-add and bit ops.
#[inline]
fn generate_request_id() -> String {
    let n = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut buf = [0u8; 11];
    let mut i = 0;
    while i < 11 {
        buf[i] = ALPHABET[((n >> (i * 6)) & 0x3F) as usize];
        i += 1;
    }
    // SAFETY: every byte comes from ALPHABET which is pure ASCII.
    unsafe { String::from_utf8_unchecked(buf.to_vec()) }
}

/// Derive a W3C-compliant 128-bit trace ID from a request ID using blake3.
///
/// blake3 XOF → 16 bytes → 32-char hex string.
/// Deterministic: same request ID always produces the same trace ID.
#[inline]
fn derive_trace_id(request_id: &str) -> String {
    let mut trace_bytes = [0u8; 16];
    let mut hasher = blake3::Hasher::new();
    hasher.update(request_id.as_bytes());
    hasher.finalize_xof().fill(&mut trace_bytes);

    let mut hex_buf = [0u8; 32];
    for (i, &b) in trace_bytes.iter().enumerate() {
        hex_buf[i * 2] = HEX[(b >> 4) as usize];
        hex_buf[i * 2 + 1] = HEX[(b & 0x0F) as usize];
    }
    // SAFETY: all bytes come from HEX which is ASCII.
    unsafe { String::from_utf8_unchecked(hex_buf.to_vec()) }
}

/// Built-in observability middleware.
///
/// Creates a tracing span with standard HTTP fields that rolly's OtlpLayer
/// picks up automatically for OTLP export.
///
/// - **Request ID**: from incoming header (e.g. CloudFront `x-amz-cf-id`) or
///   generated via atomic counter (11 chars, no RNG). Echoed in the response.
/// - **Trace ID**: derived from the request ID via blake3 (128-bit, 32-char hex,
///   W3C compliant). Deterministic — same request ID always yields the same trace.
///
/// Reads `Arc<O11yConfig>` from application state; falls back to a static
/// default when absent.
pub async fn o11y_middleware(mut req: Request, next: Next) -> Response {
    let config = req
        .try_state::<Arc<O11yConfig>>()
        .cloned()
        .unwrap_or_else(|| Arc::clone(&DEFAULT_CONFIG));

    // Extract or generate request ID.
    let request_id = req
        .header(&config.request_id_header)
        .map(|s| s.to_string())
        .unwrap_or_else(generate_request_id);

    // Derive W3C trace ID from request ID.
    let trace_id = derive_trace_id(&request_id);

    // Capture metric labels before `next.run()` consumes the request.
    let record_metrics = config.otlp_metrics_endpoint.is_some();
    let method_str = req.method().as_str().to_string();
    let route_for_metrics = req
        .route_pattern_arc()
        .unwrap_or_else(|| Arc::from("<unmatched>"));

    // Build span — borrows req fields as &str (zero allocation).
    let span = {
        let method = req.method().as_str();
        let path = req.path();
        let route = req.route_pattern().unwrap_or_else(|| req.path());
        tracing::info_span!(
            "http_request",
            { fields::TRACE_ID } = trace_id.as_str(),
            { fields::HTTP_METHOD } = method,
            { fields::HTTP_URI } = path,
            route = route,
            request_id = request_id.as_str(),
            { fields::HTTP_STATUS_CODE } = tracing::field::Empty,
            { fields::HTTP_LATENCY_MS } = tracing::field::Empty,
        )
    };

    req.set_request_id(request_id.clone());

    let start = Instant::now();
    let span_handle = span.clone();
    let resp = next.run(req).instrument(span).await;

    let elapsed = start.elapsed();

    // Record response fields and metrics inside the span scope so rolly
    // can capture trace/span exemplars on the histogram and counter.
    let status = resp.status_code().as_u16();
    span_handle.in_scope(|| {
        span_handle.record(fields::HTTP_STATUS_CODE, status);
        span_handle.record(fields::HTTP_LATENCY_MS, elapsed.as_secs_f64() * 1000.0);

        if record_metrics {
            let metrics = &*HTTP_METRICS;
            let status_str = status.to_string();
            let attrs: &[(&str, &str)] = &[
                ("http.request.method", &method_str),
                ("http.response.status_code", &status_str),
                ("http.route", &route_for_metrics),
            ];
            metrics.duration.observe(elapsed.as_secs_f64(), attrs);
            if status >= 400 {
                metrics.errors.add(1, attrs);
            }
        }
    });

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
        assert_eq!(rid.len(), 11);
        assert!(rid.is_ascii());
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

    #[tokio::test]
    async fn generated_ids_are_unique() {
        use std::collections::HashSet;
        let mut ids = HashSet::new();
        for _ in 0..1000 {
            let id = generate_request_id();
            assert!(ids.insert(id), "duplicate request ID generated");
        }
    }

    #[tokio::test]
    async fn derive_trace_id_is_valid_w3c() {
        let trace = derive_trace_id("test-request-id");
        assert_eq!(trace.len(), 32);
        assert!(trace.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[tokio::test]
    async fn derive_trace_id_is_deterministic() {
        let a = derive_trace_id("same-input");
        let b = derive_trace_id("same-input");
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn derive_trace_id_differs_for_different_inputs() {
        let a = derive_trace_id("request-1");
        let b = derive_trace_id("request-2");
        assert_ne!(a, b);
    }

    // -- Helpers for order-independent metric assertions --------------------
    //
    // HTTP_METRICS is backed by the process-global rolly registry and counters/
    // histograms are cumulative, so tests cannot assume a clean slate.  Each
    // test uses a unique (method, route) fingerprint and searches data points
    // by attribute match rather than by index.

    /// Build a request with a specific method, URI, route pattern, and state.
    async fn make_metrics_request(
        method: &str,
        uri: &str,
        route_pattern: Option<&str>,
        state: TypeMap,
    ) -> Request {
        let inner = http::Request::builder()
            .method(method)
            .uri(uri)
            .body(harrow_core::request::full_body(http_body_util::Full::new(
                bytes::Bytes::new(),
            )))
            .unwrap();
        Request::new(
            inner,
            PathMatch::default(),
            Arc::new(state),
            route_pattern.map(Arc::from),
        )
    }

    fn metrics_config() -> TypeMap {
        let config = O11yConfig::default().otlp_metrics_endpoint("http://localhost:4318");
        let mut state = TypeMap::new();
        state.insert(Arc::new(config));
        state
    }

    /// Search counter data points for one matching all given (key, value) pairs.
    fn find_counter_dp<'a>(
        snapshots: &'a [rolly::metrics::MetricSnapshot],
        counter_name: &str,
        attrs: &[(&str, &str)],
    ) -> Option<&'a (rolly::metrics::Attrs, i64, Option<rolly::metrics::Exemplar>)> {
        for snap in snapshots {
            if let rolly::metrics::MetricSnapshot::Counter {
                name, data_points, ..
            } = snap
            {
                if name == counter_name {
                    return data_points.iter().find(|(dp_attrs, _, _)| {
                        attrs
                            .iter()
                            .all(|(k, v)| dp_attrs.iter().any(|(dk, dv)| dk == k && dv == v))
                    });
                }
            }
        }
        None
    }

    /// Search histogram data points for one matching all given (key, value) pairs.
    fn find_histogram_dp<'a>(
        snapshots: &'a [rolly::metrics::MetricSnapshot],
        hist_name: &str,
        attrs: &[(&str, &str)],
    ) -> Option<&'a rolly::metrics::HistogramDataPoint> {
        for snap in snapshots {
            if let rolly::metrics::MetricSnapshot::Histogram {
                name, data_points, ..
            } = snap
            {
                if name == hist_name {
                    return data_points.iter().find(|dp| {
                        attrs
                            .iter()
                            .all(|(k, v)| dp.attrs.iter().any(|(dk, dv)| dk == k && dv == v))
                    });
                }
            }
        }
        None
    }

    #[tokio::test]
    async fn metrics_not_recorded_without_endpoint() {
        init_tracing();
        // Use a unique (method, route) pair that no other test will produce.
        // No otlp_metrics_endpoint → record_metrics is false.
        let req = make_metrics_request(
            "TRACE",
            "/disabled-sentinel",
            Some("/disabled-sentinel"),
            TypeMap::new(),
        )
        .await;
        let resp = Middleware::call(&o11y_middleware, req, ok_next()).await;
        assert_eq!(resp.status_code(), http::StatusCode::OK);

        // The global registry must NOT contain a histogram entry for this
        // unique (TRACE, /disabled-sentinel) attribute set.
        let global = rolly::metrics::global_registry();
        let snapshots = global.collect();
        let dp = find_histogram_dp(
            &snapshots,
            rolly::constants::metrics::REQUEST_DURATION,
            &[
                ("http.request.method", "TRACE"),
                ("http.route", "/disabled-sentinel"),
            ],
        );
        assert!(
            dp.is_none(),
            "no histogram data point should exist for disabled-metrics request"
        );
    }

    #[tokio::test]
    async fn metrics_recorded_with_endpoint_configured() {
        init_tracing();
        // Unique fingerprint: OPTIONS + /metrics-ok-test
        let req = make_metrics_request(
            "OPTIONS",
            "/metrics-ok-test",
            Some("/metrics-ok-test"),
            metrics_config(),
        )
        .await;
        let resp = Middleware::call(&o11y_middleware, req, ok_next()).await;
        assert_eq!(resp.status_code(), http::StatusCode::OK);

        let global = rolly::metrics::global_registry();
        let snapshots = global.collect();

        // Duration histogram must contain our data point.
        let dp = find_histogram_dp(
            &snapshots,
            rolly::constants::metrics::REQUEST_DURATION,
            &[
                ("http.request.method", "OPTIONS"),
                ("http.response.status_code", "200"),
                ("http.route", "/metrics-ok-test"),
            ],
        );
        assert!(
            dp.is_some(),
            "expected histogram data point for OPTIONS /metrics-ok-test 200"
        );

        // 200 OK must NOT produce an error counter entry for this fingerprint.
        let err_dp = find_counter_dp(
            &snapshots,
            rolly::constants::metrics::ERROR_COUNT,
            &[
                ("http.request.method", "OPTIONS"),
                ("http.response.status_code", "200"),
            ],
        );
        assert!(
            err_dp.is_none(),
            "200 OK should not produce an error counter entry"
        );
    }

    #[tokio::test]
    async fn metrics_error_counter_increments_on_4xx() {
        init_tracing();
        // Unique fingerprint: PUT + /err-4xx-test → 404
        let req = make_metrics_request("PUT", "/err-4xx-test", None, metrics_config()).await;
        let not_found_next =
            Next::new(|_req| Box::pin(async { Response::new(http::StatusCode::NOT_FOUND, "") }));
        let resp = Middleware::call(&o11y_middleware, req, not_found_next).await;
        assert_eq!(resp.status_code(), http::StatusCode::NOT_FOUND);

        let global = rolly::metrics::global_registry();
        let snapshots = global.collect();

        let dp = find_counter_dp(
            &snapshots,
            rolly::constants::metrics::ERROR_COUNT,
            &[
                ("http.request.method", "PUT"),
                ("http.response.status_code", "404"),
            ],
        );
        assert!(dp.is_some(), "404 should increment error counter");
        assert!(dp.unwrap().1 >= 1, "counter value must be >= 1");
    }

    #[tokio::test]
    async fn metrics_error_counter_increments_on_5xx() {
        init_tracing();
        // Unique fingerprint: PATCH + /err-5xx-test → 500
        let req = make_metrics_request("PATCH", "/err-5xx-test", None, metrics_config()).await;
        let server_error_next = Next::new(|_req| {
            Box::pin(async { Response::new(http::StatusCode::INTERNAL_SERVER_ERROR, "") })
        });
        let resp = Middleware::call(&o11y_middleware, req, server_error_next).await;
        assert_eq!(resp.status_code(), http::StatusCode::INTERNAL_SERVER_ERROR);

        let global = rolly::metrics::global_registry();
        let snapshots = global.collect();

        let dp = find_counter_dp(
            &snapshots,
            rolly::constants::metrics::ERROR_COUNT,
            &[
                ("http.request.method", "PATCH"),
                ("http.response.status_code", "500"),
            ],
        );
        assert!(dp.is_some(), "500 should increment error counter");
        assert!(dp.unwrap().1 >= 1, "counter value must be >= 1");
    }

    #[tokio::test]
    async fn metrics_duration_histogram_has_correct_labels() {
        init_tracing();
        // Unique fingerprint: POST + /users/:id → 200
        let req =
            make_metrics_request("POST", "/users/42", Some("/users/:id"), metrics_config()).await;
        let resp = Middleware::call(&o11y_middleware, req, ok_next()).await;
        assert_eq!(resp.status_code(), http::StatusCode::OK);

        let global = rolly::metrics::global_registry();
        let snapshots = global.collect();

        let dp = find_histogram_dp(
            &snapshots,
            rolly::constants::metrics::REQUEST_DURATION,
            &[
                ("http.request.method", "POST"),
                ("http.response.status_code", "200"),
                ("http.route", "/users/:id"),
            ],
        );
        assert!(
            dp.is_some(),
            "expected histogram data point with method=POST, route=/users/:id, status=200"
        );
        let dp = dp.unwrap();
        assert!(dp.count >= 1, "histogram count must be >= 1");
        assert!(dp.sum > 0.0, "histogram sum must be > 0");
    }
}
