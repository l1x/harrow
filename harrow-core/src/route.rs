use std::collections::HashMap;
use std::sync::Arc;

use http::Method;

use crate::handler::{self, HandlerFn};
use crate::middleware::Middleware;
use crate::path::PathPattern;
use crate::request::Request;
use crate::response::Response;

/// Metadata attached to a route, queryable at runtime.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "json", derive(serde::Serialize))]
pub struct RouteMetadata {
    pub name: Option<String>,
    pub tags: Vec<String>,
    pub deprecated: bool,
    pub custom: HashMap<String, String>,
}

/// A single route entry. Concrete struct, not a trait object graph.
pub struct Route {
    pub method: Method,
    pub pattern: PathPattern,
    pub handler: HandlerFn,
    pub metadata: RouteMetadata,
    /// Middleware scoped to this route (from route groups).
    /// Runs after global middleware, before the handler.
    /// Stored as `Arc` so group middleware can be shared across routes cheaply.
    pub middleware: Vec<Arc<dyn Middleware>>,
}

/// The route table. A `Vec` you can iterate, filter, print, serialize.
pub struct RouteTable {
    routes: Vec<Route>,
}

impl RouteTable {
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    pub fn push(&mut self, route: Route) {
        self.routes.push(route);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Route> {
        self.routes.iter()
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// Get a route by index.
    pub fn get(&self, idx: usize) -> Option<&Route> {
        self.routes.get(idx)
    }

    /// Find the first matching route for the given method and path.
    /// Returns the route and the path match (captured params).
    #[cfg_attr(feature = "profiling", inline(never))]
    pub fn match_route(
        &self,
        method: &Method,
        path: &str,
    ) -> Option<(&Route, crate::path::PathMatch)> {
        for route in &self.routes {
            if &route.method != method {
                continue;
            }
            if let Some(path_match) = route.pattern.match_path(path) {
                return Some((route, path_match));
            }
        }
        None
    }

    /// Find the first matching route index for the given method and path.
    /// Returns the route index and path match. Used by the server to build
    /// middleware chains that reference the handler through an Arc.
    #[cfg_attr(feature = "profiling", inline(never))]
    pub fn match_route_idx(
        &self,
        method: &Method,
        path: &str,
    ) -> Option<(usize, crate::path::PathMatch)> {
        for (i, route) in self.routes.iter().enumerate() {
            if &route.method != method {
                continue;
            }
            if let Some(path_match) = route.pattern.match_path(path) {
                return Some((i, path_match));
            }
        }
        None
    }

    /// Check whether any route (regardless of method) matches this path.
    /// Zero-alloc — uses `PathPattern::matches` which doesn't capture params.
    /// Used for 405 vs 404 distinction.
    pub fn any_route_matches_path(&self, path: &str) -> bool {
        self.routes.iter().any(|r| r.pattern.matches(path))
    }
}

impl Default for RouteTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for the application. Owns route table, middleware, and state.
pub struct App {
    route_table: RouteTable,
    middleware: Vec<Box<dyn Middleware>>,
    state: crate::state::TypeMap,
}

impl App {
    pub fn new() -> Self {
        Self {
            route_table: RouteTable::new(),
            middleware: Vec::new(),
            state: crate::state::TypeMap::new(),
        }
    }

    /// Register a route (no route-level middleware).
    fn route<F, Fut>(mut self, method: Method, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route_table.push(Route {
            method,
            pattern: PathPattern::parse(pattern),
            handler: handler::wrap(handler),
            metadata: RouteMetadata::default(),
            middleware: Vec::new(), // no route-level middleware for top-level routes
        });
        self
    }

    pub fn get<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route(Method::GET, pattern, handler)
    }

    pub fn post<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route(Method::POST, pattern, handler)
    }

    pub fn put<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route(Method::PUT, pattern, handler)
    }

    pub fn delete<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route(Method::DELETE, pattern, handler)
    }

    pub fn patch<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route(Method::PATCH, pattern, handler)
    }

    /// Attach metadata to the most recently added route matching this pattern.
    pub fn with_metadata(mut self, pattern: &str, f: impl FnOnce(&mut RouteMetadata)) -> Self {
        if let Some(route) = self
            .route_table
            .routes
            .iter_mut()
            .rev()
            .find(|r| r.pattern.as_str() == pattern)
        {
            f(&mut route.metadata);
        }
        self
    }

    /// Add a global middleware. Runs on every request before route-level middleware.
    pub fn middleware<M: Middleware + 'static>(mut self, m: M) -> Self {
        self.middleware.push(Box::new(m));
        self
    }

    /// Register application state of type `T`.
    pub fn state<T: Send + Sync + 'static>(mut self, val: T) -> Self {
        self.state.insert(val);
        self
    }

    /// Create a route group with a shared prefix and optional scoped middleware.
    ///
    /// Routes defined inside the group get the prefix prepended and any
    /// middleware added to the group attached. Group middleware runs after
    /// global middleware but before the handler.
    ///
    /// ```ignore
    /// let app = App::new()
    ///     .get("/health", health)
    ///     .group("/api/v1", |g| {
    ///         g.middleware(auth_middleware)
    ///          .get("/users", list_users)
    ///          .get("/users/:id", get_user)
    ///     });
    /// ```
    pub fn group(mut self, prefix: &str, f: impl FnOnce(Group) -> Group) -> Self {
        let g = Group::new(prefix);
        let g = f(g);
        for route in g.into_routes() {
            self.route_table.push(route);
        }
        self
    }

    /// Access the route table for introspection.
    pub fn route_table(&self) -> &RouteTable {
        &self.route_table
    }

    /// Consume the builder, returning the parts needed by the server.
    pub fn into_parts(
        self,
    ) -> (
        RouteTable,
        Vec<Box<dyn Middleware>>,
        crate::state::TypeMap,
    ) {
        (self.route_table, self.middleware, self.state)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Route Group
// ---------------------------------------------------------------------------

/// A group of routes sharing a common path prefix and scoped middleware.
///
/// Created via `App::group()` or `Group::group()` for nesting.
pub struct Group {
    prefix: String,
    middleware: Vec<Arc<dyn Middleware>>,
    routes: Vec<Route>,
}

impl Group {
    fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.trim_end_matches('/').to_string(),
            middleware: Vec::new(),
            routes: Vec::new(),
        }
    }

    /// Add middleware scoped to this group. Runs after global middleware,
    /// before the handler, only for routes in this group.
    pub fn middleware<M: Middleware + 'static>(mut self, m: M) -> Self {
        self.middleware.push(Arc::new(m));
        self
    }

    /// Register a route within this group. The group prefix is prepended.
    /// Group middleware is attached later in `into_routes()`.
    fn route<F, Fut>(mut self, method: Method, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        let full_pattern = format!("{}{}", self.prefix, pattern);
        self.routes.push(Route {
            method,
            pattern: PathPattern::parse(&full_pattern),
            handler: handler::wrap(handler),
            metadata: RouteMetadata::default(),
            middleware: Vec::new(),
        });
        self
    }

    pub fn get<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route(Method::GET, pattern, handler)
    }

    pub fn post<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route(Method::POST, pattern, handler)
    }

    pub fn put<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route(Method::PUT, pattern, handler)
    }

    pub fn delete<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route(Method::DELETE, pattern, handler)
    }

    pub fn patch<F, Fut>(self, pattern: &str, handler: F) -> Self
    where
        F: Fn(Request) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Response> + Send + 'static,
    {
        self.route(Method::PATCH, pattern, handler)
    }

    /// Nest a sub-group. The sub-group's prefix is appended to this group's
    /// prefix, and middleware from both groups is combined (outer group first).
    ///
    /// ```ignore
    /// app.group("/api", |g| {
    ///     g.middleware(auth)
    ///      .group("/v1", |v1| {
    ///          v1.middleware(rate_limit)
    ///            .get("/users", list_users)  // /api/v1/users — auth + rate_limit
    ///      })
    ///      .get("/health", health)           // /api/health — auth only
    /// })
    /// ```
    pub fn group(mut self, prefix: &str, f: impl FnOnce(Group) -> Group) -> Self {
        let nested_prefix = format!("{}{}", self.prefix, prefix.trim_end_matches('/'));
        let sub = Group::new(&nested_prefix);
        let sub = f(sub);
        for mut route in sub.into_routes() {
            // Prepend this group's middleware before the sub-group's middleware.
            let mut combined: Vec<Arc<dyn Middleware>> = Vec::new();
            for mw in &self.middleware {
                combined.push(Arc::clone(mw));
            }
            combined.append(&mut route.middleware);
            route.middleware = combined;
            self.routes.push(route);
        }
        self
    }

    /// Consume the group, attaching group middleware to each route.
    fn into_routes(self) -> Vec<Route> {
        let mut routes = self.routes;
        for route in &mut routes {
            // Prepend group middleware before any existing per-route middleware
            // (which may come from nested sub-groups).
            let mut combined: Vec<Arc<dyn Middleware>> = Vec::new();
            for mw in &self.middleware {
                combined.push(Arc::clone(mw));
            }
            combined.append(&mut route.middleware);
            route.middleware = combined;
        }
        routes
    }
}

