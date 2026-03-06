pub mod o11y_middleware;

/// Configuration for Harrow's built-in observability.
///
/// When `otlp_endpoint` is `Some`, traces are exported via ro11y's OTLP exporter.
/// When `None`, only JSON stderr logging is active (local dev mode).
pub struct O11yConfig {
    pub service_name: &'static str,
    pub service_version: &'static str,
    pub environment: &'static str,
    pub otlp_endpoint: Option<&'static str>,
    pub request_id_header: String,
}

impl Default for O11yConfig {
    fn default() -> Self {
        Self {
            service_name: "harrow",
            service_version: "0.1.0",
            environment: "development",
            otlp_endpoint: None,
            request_id_header: "x-request-id".to_string(),
        }
    }
}

impl O11yConfig {
    pub fn request_id_header(mut self, header: impl Into<String>) -> Self {
        self.request_id_header = header.into();
        self
    }
}
