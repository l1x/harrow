use std::future::Future;
use std::pin::Pin;

use crate::request::Request;
use crate::response::Response;

/// The concrete handler function type. A boxed async function from Request to Response.
/// No traits to implement, no generics to satisfy.
pub type HandlerFn =
    Box<dyn Fn(Request) -> Pin<Box<dyn Future<Output = Response> + Send>> + Send + Sync>;

/// Wrap a plain async function into a boxed `HandlerFn`.
///
/// ```ignore
/// async fn my_handler(req: Request) -> Response {
///     Response::ok()
/// }
/// let handler: HandlerFn = harrow_core::handler::wrap(my_handler);
/// ```
pub fn wrap<F, Fut>(f: F) -> HandlerFn
where
    F: Fn(Request) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Response> + Send + 'static,
{
    Box::new(move |req| Box::pin(f(req)))
}
