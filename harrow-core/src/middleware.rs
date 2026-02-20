use std::future::Future;
use std::pin::Pin;

use crate::request::Request;
use crate::response::Response;

/// A middleware function. Receives the request and a `Next` handle to call
/// the remainder of the chain (or the final handler).
pub trait Middleware: Send + Sync {
    fn call(
        &self,
        req: Request,
        next: Next,
    ) -> Pin<Box<dyn Future<Output = Response> + Send>>;
}

/// Blanket impl: any matching async function is a Middleware.
impl<F, Fut> Middleware for F
where
    F: Fn(Request, Next) -> Fut + Send + Sync,
    Fut: Future<Output = Response> + Send + 'static,
{
    fn call(
        &self,
        req: Request,
        next: Next,
    ) -> Pin<Box<dyn Future<Output = Response> + Send>> {
        Box::pin((self)(req, next))
    }
}

/// Handle to the next middleware or the final handler.
pub struct Next {
    inner: Box<dyn FnOnce(Request) -> Pin<Box<dyn Future<Output = Response> + Send>> + Send>,
}

impl Next {
    pub fn new(
        f: impl FnOnce(Request) -> Pin<Box<dyn Future<Output = Response> + Send>> + Send + 'static,
    ) -> Self {
        Self { inner: Box::new(f) }
    }

    /// Call the next middleware/handler in the chain.
    pub async fn run(self, req: Request) -> Response {
        (self.inner)(req).await
    }
}
