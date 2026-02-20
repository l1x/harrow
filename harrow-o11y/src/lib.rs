pub mod o11y_middleware;
pub mod request_id;

use std::time::Duration;

/// Configuration for Harrow's built-in observability.
pub struct O11yConfig {
    pub tracing_enabled: bool,
    pub metrics_enabled: bool,
    pub request_id_enabled: bool,
    pub request_id_header: String,
}

impl Default for O11yConfig {
    fn default() -> Self {
        Self {
            tracing_enabled: true,
            metrics_enabled: true,
            request_id_enabled: true,
            request_id_header: "x-request-id".to_string(),
        }
    }
}

impl O11yConfig {
    pub fn disable_tracing(mut self) -> Self {
        self.tracing_enabled = false;
        self
    }

    pub fn disable_metrics(mut self) -> Self {
        self.metrics_enabled = false;
        self
    }

    pub fn disable_request_id(mut self) -> Self {
        self.request_id_enabled = false;
        self
    }

    pub fn request_id_header(mut self, header: impl Into<String>) -> Self {
        self.request_id_header = header.into();
        self
    }
}

/// Record a request completion in metrics.
#[cfg_attr(feature = "profiling", inline(never))]
pub fn record_request(route_pattern: &str, method: &str, status: u16, duration: Duration) {
    let labels = [
        ("route", route_pattern.to_string()),
        ("method", method.to_string()),
        ("status", status.to_string()),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_request_duration_seconds", &labels)
        .record(duration.as_secs_f64());

    if status >= 400 && status < 500 {
        metrics::counter!("http_client_errors_total", &labels).increment(1);
    } else if status >= 500 {
        metrics::counter!("http_server_errors_total", &labels).increment(1);
    }
}
