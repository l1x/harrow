use bytes::Bytes;
use http::StatusCode;
use http_body_util::Full;

/// Harrow's response wrapper. Built via chained methods, no builder traits.
pub struct Response {
    inner: http::Response<Full<Bytes>>,
}

impl Response {
    /// Create a response with the given status and body.
    pub fn new(status: StatusCode, body: impl Into<Bytes>) -> Self {
        let body = Full::new(body.into());
        let inner = http::Response::builder()
            .status(status)
            .body(body)
            .expect("valid response");
        Self { inner }
    }

    /// 200 OK with empty body.
    pub fn ok() -> Self {
        Self::new(StatusCode::OK, Bytes::new())
    }

    /// 200 OK with a text body.
    pub fn text(body: impl Into<String>) -> Self {
        let body: String = body.into();
        let mut resp = Self::new(StatusCode::OK, body);
        resp.set_header("content-type", "text/plain; charset=utf-8");
        resp
    }

    /// 200 OK with a JSON body.
    #[cfg(feature = "json")]
    pub fn json(value: &impl serde::Serialize) -> Self {
        match serde_json::to_vec(value) {
            Ok(bytes) => {
                let mut resp = Self::new(StatusCode::OK, bytes);
                resp.set_header("content-type", "application/json");
                resp
            }
            Err(_) => Self::new(StatusCode::INTERNAL_SERVER_ERROR, "serialization error"),
        }
    }

    /// Set the status code.
    pub fn status(mut self, status: u16) -> Self {
        *self.inner.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::OK);
        self
    }

    /// Set a header.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.set_header(name, value);
        self
    }

    fn set_header(&mut self, name: &str, value: &str) {
        if let (Ok(name), Ok(value)) = (
            http::header::HeaderName::from_bytes(name.as_bytes()),
            http::header::HeaderValue::from_str(value),
        ) {
            self.inner.headers_mut().insert(name, value);
        }
    }

    /// The HTTP status code.
    pub fn status_code(&self) -> StatusCode {
        self.inner.status()
    }

    /// Consume and return the inner `http::Response`.
    pub fn into_inner(self) -> http::Response<Full<Bytes>> {
        self.inner
    }
}

/// Trait for types that can be converted into a `Response`.
/// Implement this on your error types to enable `Result<Response, E>` handlers.
pub trait IntoResponse {
    fn into_response(self) -> Response;
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

impl<E: IntoResponse> IntoResponse for Result<Response, E> {
    fn into_response(self) -> Response {
        match self {
            Ok(r) => r,
            Err(e) => e.into_response(),
        }
    }
}
