use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use harrow_core::middleware::{Middleware, Next};
use harrow_core::request::Request;
use harrow_core::response::Response;

/// Middleware that enforces a request timeout.
///
/// If the downstream handler chain does not complete within `duration`,
/// the request is cancelled and a **408 Request Timeout** is returned.
pub struct TimeoutMiddleware {
    duration: Duration,
}

/// Create a [`TimeoutMiddleware`] that aborts requests exceeding `duration`.
pub fn timeout_middleware(duration: Duration) -> TimeoutMiddleware {
    TimeoutMiddleware { duration }
}

impl Middleware for TimeoutMiddleware {
    fn call(&self, req: Request, next: Next) -> Pin<Box<dyn Future<Output = Response> + Send>> {
        let duration = self.duration;
        Box::pin(async move {
            match tokio::time::timeout(duration, next.run(req)).await {
                Ok(response) => response,
                Err(_elapsed) => {
                    Response::new(http::StatusCode::REQUEST_TIMEOUT, "request timeout")
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harrow_core::middleware::Middleware;
    use harrow_core::path::PathMatch;
    use harrow_core::state::TypeMap;
    use std::sync::Arc;

    async fn make_request() -> Request {
        let inner = http::Request::builder()
            .method("GET")
            .uri("/")
            .body(harrow_core::request::full_body(http_body_util::Full::new(
                bytes::Bytes::new(),
            )))
            .unwrap();
        Request::new(inner, PathMatch::default(), Arc::new(TypeMap::new()), None)
    }

    fn ok_next() -> Next {
        Next::new(|_req| Box::pin(async { Response::ok() }))
    }

    fn slow_next(delay: Duration) -> Next {
        Next::new(move |_req| {
            Box::pin(async move {
                tokio::time::sleep(delay).await;
                Response::ok()
            })
        })
    }

    #[tokio::test]
    async fn timeout_fires_returns_408() {
        let mw = timeout_middleware(Duration::from_millis(10));
        let req = make_request().await;
        let resp = mw.call(req, slow_next(Duration::from_millis(200))).await;
        assert_eq!(resp.status_code(), http::StatusCode::REQUEST_TIMEOUT);
    }

    #[tokio::test]
    async fn fast_handler_passes_through() {
        let mw = timeout_middleware(Duration::from_secs(1));
        let req = make_request().await;
        let resp = mw.call(req, ok_next()).await;
        assert_eq!(resp.status_code(), http::StatusCode::OK);
    }

    #[tokio::test]
    async fn zero_duration_immediately_times_out() {
        let mw = timeout_middleware(Duration::ZERO);
        let req = make_request().await;
        let resp = mw.call(req, slow_next(Duration::from_millis(10))).await;
        assert_eq!(resp.status_code(), http::StatusCode::REQUEST_TIMEOUT);
    }
}
