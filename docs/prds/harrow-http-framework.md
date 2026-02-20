# Harrow: A Thin, Macro-Free HTTP Framework over Hyper

**Status:** Draft
**Date:** 2026-02-19
**Author:** l1x

---

## 1. Problem Statement

Axum, the dominant Rust HTTP framework built on Hyper, introduces several pain points:

- **Macro magic and type-level gymnastics.** Extractors, handler trait bounds, and the `#[debug_handler]` escape hatch all stem from heavy use of generics and hidden trait implementations. Compile errors are notoriously opaque.
- **Route table opacity.** There is no first-class way to inspect, enumerate, or export the registered route table at runtime or build time. You cannot generate OpenAPI route listings, print a startup summary, or feed routes into monitoring config without external tooling.
- **Observability is bolted on.** Tracing, metrics, and health checks require layering Tower middleware, often with boilerplate that varies per project. There is no unified o11y story out of the box.
- **Abstraction cost.** Tower's `Service` trait, `Layer` composition, `BoxCloneService`, and the resulting deep type nesting add compile-time and cognitive overhead that is not justified for many services.

Harrow aims to be the framework you reach for when you want Hyper's raw performance with a thin, explicit, zero-macro API surface that treats observability and route introspection as first-class features.

---

## 2. Goals

| Priority | Goal |
|----------|------|
| P0 | Zero proc-macros. All routing and handler wiring is plain Rust function calls. |
| P0 | Route table is a concrete, inspectable data structure available at runtime. |
| P0 | Built-in structured observability: tracing spans per request, latency histograms, error counters. |
| P0 | Minimal overhead over raw Hyper. Target < 1 us added latency per request on the hot path. |
| P0 | Continuous flamegraph profiling. Every milestone, PR, and CI run produces comparable flamecharts to catch regressions before they merge. |
| P1 | Compile times competitive with or better than Axum for equivalent service definitions. |
| P1 | Clear, human-readable compiler errors. No deeply nested generic bounds. |
| P1 | First-class health check, readiness, and liveness endpoints. |
| P2 | Optional OpenAPI route export from the route table. |
| P2 | Graceful shutdown with drain support. |

### Non-Goals

- Templating, server-side rendering, or asset serving.
- WebSocket support in v0.1 (may add later via an opt-in feature).
- Compatibility with Tower `Layer`/`Service` traits. Harrow defines its own middleware model. If Tower interop is needed, a thin adapter crate can bridge later.

---

## 3. Design Principles

1. **Explicit over implicit.** No hidden trait impls, no inference-dependent dispatch. If the user did not write it, it does not happen.
2. **Data over types.** Routes, middleware chains, and metadata are runtime values, not encoded in the type system.
3. **Observability is not optional.** Every request gets a trace span and basic metrics by default. You opt out, not in.
4. **Compile-time is developer time.** Minimize generic instantiation. Prefer dynamic dispatch (`Box<dyn Handler>`) on cold paths, monomorphization only where it matters for hot-path throughput.
5. **Small API surface.** A developer should be able to read the entire public API in one sitting.

---

## 4. Architecture Overview

```
                        ┌──────────────────────────┐
                        │        harrow::App        │
                        │  ┌────────────────────┐   │
                        │  │    RouteTable       │   │
  Incoming              │  │  (Vec<Route>)       │   │
  HTTP request          │  │  - method           │   │
  ──────────────►       │  │  - path pattern     │   │
  hyper::conn::auto     │  │  - handler fn       │   │
                        │  │  - metadata         │   │
                        │  └────────┬───────────┘   │
                        │           │               │
                        │  ┌────────▼───────────┐   │
                        │  │   MiddlewareChain   │   │
                        │  │  (Vec<Middleware>)   │   │
                        │  └────────┬───────────┘   │
                        │           │               │
                        │  ┌────────▼───────────┐   │
                        │  │   O11y Core         │   │
                        │  │  - tracing span     │   │
                        │  │  - metrics          │   │
                        │  │  - request id       │   │
                        │  └────────────────────┘   │
                        └──────────────────────────┘
```

### 4.1 Core Types

```rust
/// A plain async function that handles a request.
/// No traits to implement, no generics to satisfy.
type HandlerFn = Box<dyn Fn(Request) -> Pin<Box<dyn Future<Output = Response> + Send>> + Send + Sync>;

/// A single route entry. This is a concrete struct, not a trait object graph.
struct Route {
    method: Method,
    pattern: PathPattern,
    handler: HandlerFn,
    metadata: RouteMetadata,
}

/// Metadata attached to each route, queryable at runtime.
struct RouteMetadata {
    name: Option<String>,
    tags: Vec<String>,
    deprecated: bool,
    custom: HashMap<String, String>,
}

/// The route table. It is a Vec you can iterate, filter, print, serialize.
struct RouteTable {
    routes: Vec<Route>,
}

/// The application. Owns the route table, middleware chain, and o11y config.
struct App {
    route_table: RouteTable,
    middleware: Vec<Box<dyn Middleware>>,
    o11y: O11yConfig,
}
```

### 4.2 Handler Signatures

Handlers are plain async functions. Parameter extraction is explicit — the user destructures from `Request`:

```rust
async fn get_user(req: Request) -> Response {
    let user_id: u64 = req.param("id").parse().unwrap_or(0);
    let db = req.state::<DbPool>();
    // ...
    Response::json(&user).status(200)
}
```

No extractor traits. No `FromRequest`. The `Request` wrapper provides ergonomic methods (`param`, `query`, `body_json`, `state`) that return `Result` types with clear errors.

### 4.3 Routing API

```rust
let app = App::new()
    .get("/health", health_handler)
    .get("/users/:id", get_user)
    .post("/users", create_user)
    .delete("/users/:id", delete_user)
    .group("/api/v1", |g| {
        g.get("/items", list_items)
         .get("/items/:id", get_item)
    })
    .with_metadata("/users/:id", |m| {
        m.name("user_detail").tag("users")
    });
```

### 4.4 Route Table Introspection

```rust
// Print all routes at startup
for route in app.route_table().iter() {
    println!("{} {} [{}]", route.method, route.pattern, route.metadata.name.as_deref().unwrap_or("-"));
}

// Export as JSON for external tooling
let json = serde_json::to_string_pretty(app.route_table())?;

// Filter routes by tag
let user_routes: Vec<&Route> = app.route_table()
    .iter()
    .filter(|r| r.metadata.tags.contains(&"users".into()))
    .collect();
```

### 4.5 Middleware Model

Middleware is a plain async function that wraps the next handler:

```rust
async fn logging_middleware(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let resp = next.run(req).await;
    tracing::info!(elapsed = ?start.elapsed(), status = resp.status().as_u16());
    resp
}

let app = App::new()
    .middleware(logging_middleware)
    .get("/ping", ping_handler);
```

No `Layer`. No `Service`. No `BoxCloneService`. A middleware is a function with a known signature.

### 4.6 Built-in Observability

Every request automatically gets:

| Feature | Implementation |
|---------|---------------|
| **Trace span** | `tracing::info_span!("http_request", method, path, request_id)` wrapping the handler. |
| **Request ID** | Generated or propagated from `x-request-id` header. Available via `req.request_id()`. |
| **Latency histogram** | Per-route histogram exported to a metrics registry (`metrics` crate). |
| **Error counter** | Counts 4xx and 5xx responses per route. |
| **Route label** | Metrics are tagged with the route pattern (not the resolved path) to avoid cardinality explosion. |

Opt-out:

```rust
let app = App::new()
    .o11y(O11yConfig::default().disable_metrics());
```

### 4.7 Startup Diagnostics

On `app.serve(addr)`, Harrow logs:

```
harrow listening on 0.0.0.0:8080
  GET  /health             [health]
  GET  /users/:id          [user_detail]  tags: users
  POST /users              [create_user]  tags: users
  DEL  /users/:id          [delete_user]  tags: users
  GET  /api/v1/items       [list_items]   tags: items
  GET  /api/v1/items/:id   [get_item]     tags: items
  middleware: [logging, auth, o11y]
```

---

## 5. Path Matching

A custom, simple path matcher — no regex engine:

| Pattern | Matches | Captures |
|---------|---------|----------|
| `/users` | exact | — |
| `/users/:id` | single segment | `id` |
| `/files/*path` | tail glob | `path` (rest of URL) |

Path matching is a linear scan of the route table in v0.1. This is fast enough for typical service route counts (< 200 routes). If needed, a radix tree can be added behind the same `RouteTable` interface later.

---

## 6. State / Dependency Injection

Application state is stored in a type-map on `App` and accessible via `Request`:

```rust
let pool = DbPool::connect("postgres://...").await?;

let app = App::new()
    .state(pool)
    .state(AppConfig::from_env())
    .get("/users/:id", get_user);

// Inside handler:
async fn get_user(req: Request) -> Response {
    let db = req.state::<DbPool>();
    // ...
}
```

`state::<T>()` returns `&T`. Panics if `T` was not registered — this is a programmer error caught immediately at startup if the handler is exercised by a smoke test. No `Option`, no `Result` — if you need it, register it.

---

## 7. Error Handling

Handlers return `Response` directly. For fallible operations, a `into_response` conversion on `Result<Response, E>` where `E: IntoResponse` allows:

```rust
async fn get_user(req: Request) -> Result<Response, AppError> {
    let id: u64 = req.param("id").parse()?;
    let user = db.find_user(id).await?;
    Ok(Response::json(&user))
}
```

`AppError` is user-defined and implements `IntoResponse`. Harrow provides a default `ProblemDetail` (RFC 9457) response builder but does not impose it.

---

## 8. Graceful Shutdown

```rust
app.serve_with_shutdown(addr, shutdown_signal()).await?;
```

On signal, Harrow:
1. Stops accepting new connections.
2. Waits for in-flight requests to complete (configurable timeout).
3. Returns from `serve_with_shutdown`.

---

## 9. Crate Structure

```
harrow/
  harrow-core/       # Route table, Request/Response wrappers, middleware trait
  harrow-o11y/       # Tracing + metrics integration (optional feature)
  harrow-server/     # Hyper binding, connection handling, graceful shutdown
  harrow-bench/      # Standalone load driver + criterion benchmarks (3 workloads)
  harrow/            # Facade crate re-exporting everything
  scripts/
    profile.sh       # Run all workloads under cargo-flamegraph, output SVGs
    profile-diff.sh  # Diff current flamegraphs against a saved baseline
  flamegraphs/       # .gitignore-d, local output directory
```

Feature flags on the facade crate:

| Feature | Default | Contents |
|---------|---------|----------|
| `o11y` | on | Tracing spans + metrics |
| `json` | on | `serde_json` body parsing/response helpers |
| `tls` | off | rustls integration |
| `http2` | on | HTTP/2 support via hyper |
| `profiling` | off | Adds `#[inline(never)]` markers on key functions for cleaner flamegraph frames |

---

## 10. Performance Targets

Measured on a simple JSON echo handler (`/echo` — parse JSON body, return it):

| Metric | Target |
|--------|--------|
| Added latency over raw Hyper | < 1 us p99 |
| Requests/sec (single core, 64 connections) | > 95% of raw Hyper throughput |
| Binary size (release, stripped, minimal features) | < 2 MB |
| Compile time (clean build) | < 30s on M-series Apple Silicon |

Benchmarks tracked in CI via `criterion`.

---

## 11. Flamegraph-Driven Performance Verification

Every change to Harrow must be provably non-regressing. Flamegraphs are not a debugging afterthought — they are a continuous verification artifact produced on every CI run and reviewable in every PR.

### 11.1 Toolchain

| Tool | Role |
|------|------|
| [`cargo-flamegraph`](https://github.com/flamegraph-rs/flamegraph) | Generates SVG flamegraphs from `perf` (Linux) or `dtrace` (macOS) profiles. |
| [`inferno`](https://github.com/jonhoo/inferno) | Rust-native folded-stack processing. Used in CI where SVG diffing is needed. `inferno-flamegraph` and `inferno-diff-folded` are the key binaries. |
| `criterion` | Micro-benchmarks that serve as the workloads being profiled. |
| Custom `harrow-bench` binary | A standalone load driver (wrk2-style) that sends sustained traffic to a running Harrow server for macro-level profiling. |

### 11.2 What Gets Profiled

Three standard workloads, each producing its own flamegraph:

| Workload | Description | What it catches |
|----------|-------------|-----------------|
| **echo** | JSON echo handler, no middleware, no state. Pure routing + serialization hot path. | Overhead in core request dispatch, path matching, response construction. |
| **middleware-chain** | 5-deep middleware stack (logging, auth check, request ID, rate limit stub, compression stub) around a trivial handler. | Cost of middleware traversal, `Next` chaining, per-middleware allocations. |
| **full-stack** | Realistic service: state injection, path params, JSON body parse, DB stub (async sleep), structured error responses, all o11y enabled. | End-to-end overhead under realistic conditions. Allocation pressure, span creation cost, metrics recording. |

### 11.3 CI Pipeline Integration

```
┌─────────────┐     ┌──────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  PR opened  │────►│  cargo bench     │────►│  Profile each    │────►│  Diff against   │
│             │     │  (criterion)     │     │  workload with   │     │  main baseline  │
│             │     │                  │     │  cargo-flamegraph │     │  flamegraphs    │
└─────────────┘     └──────────────────┘     └──────────────────┘     └────────┬────────┘
                                                                               │
                                                                    ┌──────────▼──────────┐
                                                                    │  Post artifacts to  │
                                                                    │  PR as comment:     │
                                                                    │  - SVG flamegraphs  │
                                                                    │  - Diff flamegraph  │
                                                                    │  - criterion report │
                                                                    │  - Pass/fail gate   │
                                                                    └─────────────────────┘
```

**Steps in detail:**

1. **Baseline capture.** On every merge to `main`, CI runs all three workloads and stores the folded stacks and SVG flamegraphs as versioned artifacts (e.g., `flamegraphs/main/<commit-sha>/echo.folded`).

2. **PR profiling.** On every PR, CI runs the same workloads on the PR branch.

3. **Differential flamegraph.** `inferno-diff-folded` compares the PR's folded stacks against the `main` baseline, producing a red/blue differential SVG:
   - **Red** = frames that got hotter (more samples).
   - **Blue** = frames that got cooler (fewer samples).

4. **Regression gate.** CI fails the PR if:
   - Any criterion benchmark regresses by more than **3%** (configurable via `HARROW_PERF_THRESHOLD`).
   - Any new frame appears in the hot path that was not present in the baseline (flagged for manual review, not auto-fail).

5. **Artifact posting.** A CI bot comments on the PR with:
   - Links to the three workload flamegraphs (SVG, viewable in-browser).
   - The differential flamegraph.
   - A summary table of criterion results (mean, stddev, change %).

### 11.4 Local Developer Workflow

Developers can reproduce CI profiling locally:

```bash
# Generate a flamegraph for the echo workload
cargo flamegraph --bench echo_bench -o flamegraphs/echo.svg

# Run all three workloads and generate flamegraphs
./scripts/profile.sh

# Compare against a saved baseline
./scripts/profile-diff.sh baseline/main flamegraphs/
# Outputs: flamegraphs/diff-echo.svg, diff-middleware-chain.svg, diff-full-stack.svg
```

The `scripts/profile.sh` script:
- Builds in release mode with debug symbols (`profile.release.debug = true` in `Cargo.toml`).
- Runs each criterion benchmark under `cargo-flamegraph`.
- Launches `harrow-bench` against a local server for the macro-level profile.
- Outputs all SVGs to `flamegraphs/`.

### 11.5 Cargo Configuration

```toml
# Cargo.toml — workspace root
[profile.bench]
debug = true          # Required for meaningful flamegraph symbols
opt-level = 3
lto = "thin"

[profile.release]
debug = 1             # Line-level debug info for production profiling
opt-level = 3
lto = "fat"
codegen-units = 1
```

### 11.6 Flamegraph Storage and History

- `flamegraphs/` directory is `.gitignore`-d. CI artifacts are stored externally (S3, GCS, or GitHub Actions artifact storage).
- A lightweight manifest (`flamegraphs/manifest.json`) tracks which commit produced which baseline, enabling historical comparison across releases.
- On tagged releases (v0.1, v0.2, ...), flamegraphs are archived permanently and linked from the release notes, giving a visual performance history of the project.

### 11.7 What We Look For in Review

When reviewing a PR's flamegraph diff:

| Signal | Action |
|--------|--------|
| New `alloc::` frames in hot path | Investigate. Likely an unnecessary allocation introduced. |
| Wider `tracing` frames | Check if new spans/events were added. Acceptable if intentional o11y, flag if accidental. |
| `clone()` or `to_string()` appearing in dispatch | Likely a regression. Request path should be zero-copy where possible. |
| Middleware traversal frame growth | Check if `Next` chaining changed. Should be constant-cost per middleware layer. |
| `serde` frames growing | Check if serialization path changed. May indicate a schema change, not a regression. |
| Differential flamegraph is entirely blue | Celebrate. |

### 11.8 Milestone Gates

Each milestone (v0.1, v0.2, v0.3) has a performance gate:

| Milestone | Gate |
|-----------|------|
| **v0.1** | Flamegraph of `echo` workload shows Harrow frames occupy < 5% of total samples (95%+ is Hyper/tokio/kernel). Baseline flamegraphs established for all three workloads. |
| **v0.2** | No workload regresses by more than 3% vs v0.1 baseline. Route groups and serialization do not introduce new hot-path allocations. |
| **v0.3** | TLS and timeout handling do not appear in the `echo` workload flamegraph (they should only activate when configured). Full-stack workload remains within 5% of v0.2. |

---

## 12. What Harrow Intentionally Omits

| Feature | Rationale |
|---------|-----------|
| Proc macros | Core design principle. |
| Tower compatibility | Adds type complexity for interop most services don't need. Adapter crate possible later. |
| Built-in templating | Not a web application framework. Use `askama` or `maud` externally. |
| Cookie/session management | Belongs in middleware, not core. |
| WebSocket (v0.1) | Can be added as a feature-gated module later. |
| ORM/database integration | Out of scope. Bring your own `sqlx`, `diesel`, etc. |

---

## 12. Milestones

### v0.1 — Foundation
- Core types: `App`, `RouteTable`, `Route`, `Request`, `Response`.
- Path matching with `:param` and `*glob`.
- Middleware chain.
- State injection via type-map.
- Built-in o11y (tracing spans, request ID, latency histogram).
- Route table printing at startup.
- Graceful shutdown.
- Criterion benchmark suite for all three workloads (echo, middleware-chain, full-stack).
- Baseline flamegraphs for all three workloads, archived as v0.1 reference.
- `scripts/profile.sh` and `scripts/profile-diff.sh` for local profiling.
- CI pipeline producing differential flamegraphs on every PR.
- **Performance gate:** Harrow frames < 5% of total samples in the `echo` flamegraph.

### v0.2 — Ergonomics
- Route groups with shared middleware.
- `RouteTable` serialization (JSON, TOML) for external tooling.
- `ProblemDetail` (RFC 9457) error response builder.
- Query string and form body parsing helpers.
- Configurable 404/405 responses.
- **Performance gate:** No workload regresses > 3% vs v0.1. No new hot-path allocations from route groups or serialization (verified via flamegraph diff).

### v0.3 — Production Hardening
- TLS via rustls.
- Connection-level timeouts and limits.
- Request body size limits.
- Rate limiting middleware (token bucket, shipped as an example).
- OpenAPI route export (optional feature).
- **Performance gate:** TLS/timeout frames absent from `echo` flamegraph. Full-stack workload within 5% of v0.2.

---

## 13. Open Questions

1. **Should `HandlerFn` use `Box<dyn ...>` or a thin enum dispatch?** Boxing is simple but incurs an allocation per route registration (one-time cost, negligible). Enum dispatch avoids it but limits extensibility.

2. **Should state injection panic or return `Option`?** Panic is proposed for simplicity. Alternative: return `Option<&T>` or compile-time checking via a builder pattern that encodes registered types.

3. **Is linear route matching acceptable long-term?** For < 200 routes, yes. Beyond that, a radix tree (like `matchit`, which Axum uses) should be considered. Could start with linear and swap the internals without changing the public API.

4. **Should Harrow expose the underlying `hyper::Request`/`hyper::Response`?** Proposed: no. Harrow wraps them to control the API surface. An escape hatch (`req.inner()`) can expose the raw Hyper types for advanced use.

5. **Async trait vs boxed futures for middleware?** With `async fn` in traits stabilized, using `async trait` directly may simplify the middleware signature. Needs benchmarking to confirm there is no overhead regression.

---

## 14. Prior Art and Differentiation

| Framework | Macros | Route Introspection | Built-in O11y | Overhead |
|-----------|--------|---------------------|---------------|----------|
| **Axum** | No proc macros, but heavy trait generics | No first-class API | No (Tower layers) | Low |
| **Actix-web** | Proc macros for routes | Limited | No | Low |
| **Warp** | No macros, filter combinators | No | No | Low |
| **Poem** | Proc macros | OpenAPI integration | Partial | Low |
| **Harrow** | None | First-class, queryable | Built-in | Minimal |
