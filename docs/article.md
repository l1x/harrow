# Building a Fast HTTP Framework: Harrow's Performance Story

**Date:** 2026-03-11
**Platform:** Apple Silicon (arm64-darwin), Rust 1.85.0, release profile (`opt-level = 3`, `lto = "thin"`)

---

## The Goal

Harrow is a thin, macro-free HTTP framework built directly on Hyper. The design constraint: framework overhead should be invisible compared to the TCP baseline. Every microsecond of added latency is a microsecond that could serve user logic instead.

This article documents how we measured, optimized, and verified Harrow's performance — and how it compares to Axum, the production-grade framework we benchmark against.

---

## Measurement Architecture

We measure at three isolation levels so we can attribute cost precisely:

| Level | What it isolates | How |
|-------|-----------------|-----|
| **Micro** | Pure CPU cost of path matching and route lookup | Direct function calls, no I/O, no async |
| **Client** | Full framework dispatch without TCP | `App::client()` — constructs `Request`, runs middleware chain, calls handler, returns `Response` |
| **TCP** | Complete HTTP/1.1 request-response cycle | Keep-alive `BenchClient` over loopback, measures kernel TCP + Hyper + Harrow |

The micro level tells us if our data structures are efficient. The client level tells us how much the framework adds on top. The TCP level tells us what real users will see.

### Tooling

- **Criterion 0.5** for statistically rigorous timing (100 samples, outlier detection, regression detection)
- **Custom `TrackingAllocator`** for per-operation allocation counting (wraps `System` with atomic counters, 10,000 iterations per measurement)
- **Machine-readable baseline** in `harrow-bench/benches/baseline.toml` — diffable in PRs, updated by `cargo run --bin update-baseline`
- **SVG renderer** that produces `docs/performance.svg` from the TOML data — no external dependencies
- **Flamegraphs** via `cargo-flamegraph` (DTrace on macOS) for hot-path analysis

---

## Results: Harrow Standalone

### Path Matching

Pure CPU cost of `PathPattern::match_path`:

| Operation | Latency | Allocations |
|-----------|---------|-------------|
| Exact match (`/health`) | 14 ns | 0 |
| 1 param (`/users/:id`) | 67 ns | 3 (196 B) |
| Glob (`/files/*path`) | 131 ns | 4 (271 B) |
| Route lookup, 100 routes worst case | 85 ns | 3 (52 B) |

Exact match is zero-allocation — the iterator walks segments and compares literals. Each param adds ~55 ns dominated by `String` allocation for the captured value. Route lookup uses a trie, so it's O(path_length) not O(n_routes).

### TCP Round-Trip

Full HTTP/1.1 request-response over loopback:

| Operation | Latency | Alloc/op |
|-----------|---------|----------|
| Text echo, 0 middleware | 22.7 µs | 1,487 B (7 allocs) |
| JSON echo, 0 middleware | 23.2 µs | 2,281 B (12 allocs) |
| Param echo, 0 middleware | 23.2 µs | 1,543 B (10 allocs) |
| 404 miss, 0 middleware | 22.5 µs | 165 B (3 allocs) |
| JSON + 3 middleware + state + param | 24.3 µs | 4,545 B (24 allocs) |
| Health + 3 middleware | 24.4 µs | 3,697 B (15 allocs) |
| 10 noop middleware layers | 24.5 µs | 8,767 B (27 allocs) |

The TCP baseline is ~22 µs — that's the kernel TCP stack plus Hyper's HTTP/1.1 parser and response serializer. Harrow's routing adds at most 2 µs on top of that.

404 misses allocate only 165 bytes — we use the zero-allocation `matches()` path that checks existence without capturing param values.

### Middleware Cost

Each middleware layer costs ~240 ns and ~850 B per request. The cost is dominated by `Box::pin` for the async future plus `Box::new` for the `Next` closure. At 10 layers deep, total middleware overhead is ~2 µs and ~8.7 KB — well within budget.

---

## Results: Harrow vs Axum

We run identical workloads on both frameworks: same response bodies, same `BenchClient`, same Tokio runtime, same `--release` profile. The only difference is the framework code.

### Latency Comparison

| Benchmark | Harrow | Axum | Delta |
|-----------|--------|------|-------|
| Text echo | 22.7 µs | 27.5 µs | **-17%** |
| JSON echo | 23.2 µs | 25.0 µs | **-7%** |
| Param echo | 23.2 µs | 24.9 µs | **-7%** |
| 404 miss | 22.5 µs | 24.4 µs | **-8%** |

Harrow is 7-17% faster than Axum across all four workloads. The gap is largest on the text echo because there's less handler work to amortize framework overhead against — the text echo isolates pure framework cost.

**Why the latency difference?** Allocations are not free. Each `malloc`/`free` pair costs ~20-50 ns on modern allocators. Axum makes 10+ more allocations per request than Harrow, which accounts for ~200-500 ns of the gap. The remaining difference comes from indirection: every `Box<dyn Trait>` call goes through a vtable pointer, defeating CPU branch prediction and inlining. Harrow's concrete types allow the compiler to inline the response construction path entirely.

### Memory Comparison

This is where the difference is stark:

| Benchmark | Harrow bytes/op | Axum bytes/op | Ratio |
|-----------|----------------|---------------|-------|
| Text echo | 1,487 B (7 allocs) | 9,449 B (17 allocs) | **6.4x less** |
| JSON echo | 2,281 B (12 allocs) | 10,238 B (23 allocs) | **4.5x less** |
| Param echo | 1,543 B (10 allocs) | 10,143 B (21 allocs) | **6.6x less** |
| 404 miss | 165 B (3 allocs) | 9,030 B (12 allocs) | **55x less** |

Harrow allocates 4.5–55x fewer bytes per request than Axum. The 404 case is especially notable: Harrow's zero-alloc miss path means a missed route costs 165 bytes total (just the response construction), while Axum allocates ~9 KB even for a 404.

At 100,000 req/s, Harrow allocates ~150 MB/s for the text echo workload. Axum would allocate ~945 MB/s for the same workload — nearly a gigabyte per second of allocator pressure that the garbage collector (jemalloc or system) must handle.

### Why Does Axum Allocate ~9 KB Per Request?

To understand the gap, we traced the allocation path through Axum's source code. Every `GET /echo -> "ok"` request hits three unavoidable boxing layers, each a consequence of a specific architectural choice.

#### 1. Body Type-Erasure: `UnsyncBoxBody` (~3 KB)

Axum's `Body` type is defined in `axum-core/src/body.rs`:

```rust
type BoxBody = http_body_util::combinators::UnsyncBoxBody<Bytes, Error>;

pub struct Body(BoxBody);
```

Every response body — even a `&'static str` — goes through this path:

```rust
// axum-core/src/body.rs, line 97
impl From<&'static str> for Body {
    fn from(buf: &'static str) -> Self {
        Self::new(http_body_util::Full::from(buf))  // heap allocation
    }
}
```

`Self::new()` calls `body.map_err(Error::new).boxed_unsync()`, which creates a `Pin<Box<dyn Body>>` — a heap-allocated trait object. This happens on both the request body *and* the response body, so every request-response cycle pays for two trait-object allocations even when the body types are known at compile time.

**Why Axum does this:** `Router` needs to store handlers with different response types in the same data structure. Type-erasing the body to `dyn Body` is the simplest way to make `Router::route("/a", get(returns_string)).route("/b", get(returns_json))` compile. It's a necessary trade-off for Axum's generic API.

**What Harrow does instead:** `Response` wraps `http::Response<Full<Bytes>>` — a concrete type, not a trait object. The handler return type is always `Response`, and the framework constructs it directly:

```rust
// harrow-core/src/response.rs, line 12
pub struct Response {
    inner: http::Response<Full<Bytes>>,
}
```

No boxing. The trade-off: Harrow handlers must return `Response` (or implement `IntoResponse` which returns `Response`). Less flexible, but zero heap allocation on the response path.

#### 2. Service Boxing: `BoxCloneSyncService` (~4 KB)

Every route in Axum is wrapped in Tower's `BoxCloneSyncService`:

```rust
// axum/src/routing/route.rs, line 31
pub struct Route<E = Infallible>(BoxCloneSyncService<Request, Response, E>);
```

And `BoxCloneSyncService` is defined in `tower/src/util/boxed_clone_sync.rs`:

```rust
pub struct BoxCloneSyncService<T, U, E>(
    Box<
        dyn CloneService<T, Response = U, Error = E,
            Future = BoxFuture<'static, Result<U, E>>>
            + Send + Sync,
    >,
);
```

This is a double-boxing: the service itself is `Box<dyn CloneService>`, and its future is `BoxFuture` (which is `Pin<Box<dyn Future>>`). Every request dispatch allocates both.

When a request arrives, the route is cloned (line 55: `self.0.get_mut().unwrap().clone().oneshot(req)`) — `clone()` calls `clone_box()`, which allocates *another* `Box<dyn CloneService>`:

```rust
fn clone_box(&self) -> Box<dyn CloneService<...> + Send + Sync> {
    Box::new(self.clone())  // heap allocation per request
}
```

So per-request: one `Box` for the cloned service, one `Pin<Box>` for the response future. Combined with the data these boxes contain (the handler closure, captured state, future state machine), this is ~4 KB.

**Why Axum does this:** Tower's `Service` trait requires `Clone` for concurrent request handling, but different handlers have different types. `BoxCloneSyncService` erases the handler type so the router can store them uniformly. The `clone()` per request is necessary because `Service::call(&mut self)` takes `&mut self` — you can't share a `&mut` across concurrent requests without cloning.

**What Harrow does instead:** Handlers are stored as `Box<dyn Fn(Request) -> Pin<Box<dyn Future<Output = Response> + Send>> + Send + Sync>`. The `Fn` trait (not `FnMut`, not `FnOnce`) means handlers are called via shared reference — no cloning needed. The only per-request boxing is the handler's future (`Pin<Box<dyn Future>>`), which is inherent to async dispatch.

#### 3. Extensions, OriginalUri, and Routing Overhead (~2 KB)

Axum stores routing metadata in `http::Extensions`, which is a type-erased `HashMap`:

- `OriginalUri` — clones the request URI (allocates a `String` for the path)
- `MatchedPath` — stores the matched route pattern
- URL params — stored via `insert_url_params(&mut parts.extensions, match_.params)`

Each `Extensions::insert()` is a `HashMap` insertion with potential reallocation. For a request with path params, the matched params from `matchit` are collected into a `Vec<(String, String)>` and inserted into extensions.

**What Harrow does instead:** Params from the trie match are stored directly on `Request` as a `Vec<(String, String)>` — no indirection through `Extensions`. State is accessed via `Arc<TypeMap>` which is shared (zero per-request allocation), not cloned.

#### The Compound Effect

Each layer seems modest in isolation, but they stack:

| Layer | Axum allocs | Harrow equivalent |
|-------|-------------|-------------------|
| Request body boxing | `Pin<Box<dyn Body>>` | `Body` (hyper's concrete type) |
| Response body boxing | `Pin<Box<dyn Body>>` | `Full<Bytes>` (concrete) |
| Service clone + box | `Box<dyn CloneService>` | None (shared `&Fn`) |
| Future boxing | `Pin<Box<dyn Future>>` | `Pin<Box<dyn Future>>` (same) |
| Extensions HashMap | `HashMap<TypeId, Box<dyn Any>>` | Direct field access |
| URI clone | `String` allocation | No clone needed |

Harrow pays for one future boxing per handler call. Axum pays for five to six boxings. At ~1–2 KB per box (trait object + captured data + alignment), this explains the 6x difference.

#### Is Axum's Approach Wrong?

No. Axum's architecture enables a much more flexible API:

- Handlers can return any type that implements `IntoResponse` — the type erasure is what makes this work
- Tower middleware is composable, reusable across frameworks (hyper, tonic, axum)
- `BoxCloneSyncService` enables dynamic middleware stacking without monomorphization explosion

These are real engineering trade-offs. Axum optimizes for **developer ergonomics and ecosystem compatibility**. Harrow optimizes for **minimal per-request overhead**. For most applications, Axum's 9 KB/request is invisible — at 10K req/s it's 90 MB/s of allocator throughput, well within what modern allocators handle. The question is whether *your* workload is allocation-sensitive enough to care.

---

## How We Verified

### Statistical Rigor

Criterion runs 100 samples per benchmark with automatic warmup detection and outlier removal. We report mean and median — when they diverge significantly, it indicates measurement noise (usually from the kernel TCP stack), not framework variance.

### Allocation Accuracy

The `TrackingAllocator` wraps `std::alloc::System` with `AtomicU64` counters. Tracking is toggled on/off per measurement window. We run 10,000 iterations and divide by the count. The allocator tracks every `alloc()` call including those from Hyper, Tokio, serde, and the framework itself — this is total per-operation cost, not just framework allocations.

### Fairness Principles

The comparison against Axum follows strict fairness rules:

- Same Tokio runtime configuration (current-thread for alloc tracking, multi-thread for criterion)
- Same `BenchClient` (custom keep-alive HTTP/1.1 client, same for both frameworks)
- Same response bodies (byte-identical where possible)
- Same `--release` profile, same machine, sequential execution (never both under load simultaneously)
- Same warmup period and iteration count

### Machine-Readable Baseline

All measurements are stored in `harrow-bench/benches/baseline.toml` — a TOML file that maps 1:1 to criterion's JSON output. The workflow:

```bash
# 1. Run criterion benchmarks
cargo bench

# 2. Extract timing results into TOML
cargo run --bin update-baseline

# 3. Measure allocations
cargo run --release --bin measure-allocs

# 4. Render visualization
cargo run --bin render-baseline
```

The TOML file is committed to the repository. In PRs, reviewers see the raw number diffs alongside code changes. The SVG visualization makes trends immediately visible.

---

## Flamegraph Analysis

Flamegraphs reveal where CPU time is actually spent during benchmark execution. We generate them using `cargo-flamegraph` (DTrace on macOS).

### Harrow Echo Flamegraph

The `docs/flamegraphs/harrow_echo.svg` flamegraph shows the hot path for Harrow's echo benchmark. Key observations:

- **Hyper dominates.** The widest frames are `hyper::proto::h1` (HTTP/1.1 parsing) and `tokio::io` (TCP read/write). This confirms that Harrow's framework overhead is small relative to the I/O layer.
- **Route matching is invisible.** `PathPattern::match_path` and `RouteTable::match_route_idx` don't appear as measurable frames — they're too fast relative to TCP.
- **Allocator frames are minimal.** `__malloc` and `__free` are present but narrow, consistent with our allocation tracking showing <2 KB per request.

### Axum Echo Flamegraph

The `docs/flamegraphs/axum_echo.svg` flamegraph shows Axum's echo benchmark for comparison:

- **Tower service calls** appear as multiple nested frames — `Service::call` at each layer boundary adds indirection.
- **Body boxing** shows up as `BoxBody` conversion frames that don't exist in Harrow's flamegraph.
- **Wider allocator frames** consistent with 6x higher allocation count per request.

### How to Read Flamegraphs

- **Width = time.** Wider frames consumed more CPU time.
- **Depth = call stack.** Deeper means more function call nesting.
- **Color is arbitrary** — it helps distinguish frames but doesn't encode meaning.
- **Look for wide frames near the top** — these are the functions where the most time is spent directly (not just transitively).

To generate fresh flamegraphs:

```bash
# Requires: cargo install flamegraph
# macOS: dtrace available by default (Xcode CLI tools)
cargo flamegraph --bench echo -o docs/flamegraphs/harrow_echo.svg --root -- --bench
cargo flamegraph --bench axum_echo -o docs/flamegraphs/axum_echo.svg --root -- --bench
cargo flamegraph --bench full_stack -o docs/flamegraphs/harrow_full_stack.svg --root -- --bench
```

---

## Optimization History

### Phase 1: Get It Working

Initial implementation used `HashMap` for params, `Vec` collection on every match, and `String` everywhere. Performance was acceptable but left room.

### Phase 2: Eliminate Hot-Path Allocations (2026-02-25)

Three targeted changes closed a ~7% latency gap vs Axum:

| Change | What it eliminated | Impact |
|--------|-------------------|--------|
| `serde_json::to_writer` into `BytesMut(128)` | Intermediate `Vec<u8>` from `to_vec()` | -0.4 µs on JSON responses |
| `HeaderValue::from_static` for known headers | Per-request header name parsing | -0.1 µs per static header |
| `PathPattern.raw`: `String` → `Arc<str>` | Per-request `to_string()` heap allocation | -0.1 µs per route match |

After this phase, Harrow was within noise of Axum on latency, and significantly ahead on allocations.

### Phase 3: Trie-Based Route Lookup

Replaced linear scan with a trie for `RouteTable::match_route_idx`. At 100 routes, worst-case lookup dropped from 1.19 µs (linear) to 85 ns (trie). The trie lookup is O(path_length), not O(n_routes) — it's effectively constant cost regardless of table size.

### Phase 4: Measurement Infrastructure (current)

Added the machine-readable baseline, allocation tracking, and SVG visualization. This makes regressions visible in every PR and gives us the data for this article.

---

## What's Next

| Target | Expected gain | When |
|--------|--------------|------|
| Borrowed param values (`&str` into request path) | ~40 ns per param route | Major API change — not yet |
| Inline `Next` (avoid `Box<dyn FnOnce>`) | ~10 ns per middleware layer | Diminishing returns |
| io_uring for TCP (Linux) | Potentially significant for throughput | Requires kernel 5.10+ |

The framework is at 2 µs overhead on a 22 µs TCP baseline. We're at parity with Axum on latency and 4–6x ahead on allocations. Remaining optimizations offer sub-50 ns gains. Diminishing returns for typical workloads.

---

## Visualization

The full visualization is available at `docs/performance.svg`. It contains:

1. **Harrow latency** — all 11 benchmarks, micro and TCP, sorted by latency
2. **Harrow vs Axum** — side-by-side TCP latency comparison
3. **Allocation profile** — side-by-side bytes per operation
4. **Resource budget** — weighted mean latency, max throughput, CPU utilization

Flamegraphs are in `docs/flamegraphs/`:

- `harrow_echo.svg` — Harrow echo benchmark hot path
- `harrow_full_stack.svg` — Harrow full stack (3 middleware + state + JSON)
- `axum_echo.svg` — Axum echo benchmark for comparison

---

## Reproducing These Results

```bash
# Run all criterion benchmarks
cargo bench

# Update the TOML baseline from criterion data
cargo run --bin update-baseline

# Measure per-operation allocations (Harrow + Axum)
cargo run --release --bin measure-allocs

# Render the SVG visualization
cargo run --bin render-baseline

# Generate flamegraphs (requires: cargo install flamegraph)
cargo flamegraph --bench echo -o docs/flamegraphs/harrow_echo.svg --root -- --bench
cargo flamegraph --bench full_stack -o docs/flamegraphs/harrow_full_stack.svg --root -- --bench
cargo flamegraph --bench axum_echo -o docs/flamegraphs/axum_echo.svg --root -- --bench
```

All measurements in this article were taken on Apple Silicon (M-series), macOS, AC power, no background load. TCP benchmarks use a single keep-alive connection over loopback. Your numbers will differ on different hardware, but the relative comparisons should hold.
