# Harrow 1.0 PRD

**Status:** Draft  
**Date:** 2026-04-21  
**Scope:** Public product definition for the `1.0` line

---

## 1. Product Definition

Harrow is a thin, macro-free HTTP framework for Rust with:

- plain `async fn(Request) -> Response` handlers
- an inspectable route table
- first-party opt-in middleware
- first-party opt-in observability
- custom HTTP/1 server backends

The `1.0` goal is not to expose every runtime/backend experiment. It is to ship
a **small, explicit, stable public framework** that is easy to understand and
fast enough to compete with the best Rust HTTP stacks on real workloads.

---

## 2. 1.0 Goals

### P0

- Stable, documented public API for the `harrow` crate
- Stable HTTP/1 behavior on the supported backends
- Clear support policy for each backend
- Publish/release flow that works without crate-specific exceptions
- Verified correctness for routing, middleware dispatch, codec boundaries, and
  HTTP/1 lifecycle behavior

### P1

- Maintain near-`ntex` performance for the hot Tokio path
- Reduce remaining large-payload response overhead
- Keep backend-specific complexity out of the public `harrow` crate

### P2

- Expose additional backends only if their support level is explicit and
  justified

---

## 3. Non-Goals For 1.0

- Making every workspace crate a first-class public product
- Stabilizing Meguri as a fully supported backend unless it earns that status
- Broad protocol scope expansion beyond the current HTTP/1-centered design
- Preserving every transitional API shape from the pre-1.0 rewrite

---

## 4. Supported Backends

### Tokio

**Support level:** First-class

Tokio is the default documented backend for Harrow `1.0`.

What `1.0` means here:

- cross-platform support
- custom HTTP/1 transport
- local-worker runtime direction
- production-ready public API
- benchmarked and verified as part of the normal release story

### Monoio

**Support level:** First-class, Linux-specific

Monoio is part of the `1.0` product, but explicitly as the advanced
Linux/io_uring backend.

What `1.0` means here:

- Linux-only performance-oriented backend
- public runtime entrypoint is intentionally smaller than the backend crate's
  internal/advanced surface
- HTTP/1 behavior should match Tokio where shared Harrow behavior is expected

### Meguri

**Support level:** Experimental until explicitly promoted

Meguri remains a workspace backend, not part of the stable `harrow` `1.0`
surface unless a dedicated decision is made.

That promotion requires:

- explicit public API decision
- documented support level
- current benchmark position
- parity expectations that we are willing to support

---

## 5. Public API Scope

The stable `harrow` `1.0` surface should emphasize:

- `App`
- `Request`
- `Response`
- `ProblemDetail`
- route grouping and route introspection
- feature-gated middleware re-exports
- feature-gated observability wiring
- explicit backend runtime modules

The public API should avoid exposing backend-specific lifecycle controls unless
they are clearly part of the intended stable product.

### Tokio API

Tokio should keep:

- simple entrypoint for common use
- explicit multi-worker production entrypoint
- config type

### Monoio API

The `harrow` crate should expose only the smaller public Monoio surface:

- `run`
- `run_with_config`
- `ServerConfig`

Advanced lifecycle/testing hooks can remain in `harrow-server-monoio`.

---

## 6. Observability

Harrow `1.0` includes first-party request-level observability:

- request ID propagation/generation
- trace ID derivation
- tracing spans
- latency/error metrics via the `o11y` integration

This is intentionally **request-level observability**, not a promise of full
transport-internals telemetry for every backend.

---

## 7. Verification Requirements

Before `1.0`, Harrow should have:

- route/path property tests
- middleware-ordering and fast/slow-path checks
- codec proptests for framing/roundtrip invariants
- codec fuzz targets for malformed input boundaries
- a maintained narrow formal model for the HTTP/1 lifecycle

The goal is not maximal formalism. The goal is confidence on the real bug
surfaces Harrow owns.

---

## 8. Performance Requirements

For `1.0`, performance work should focus on:

- keeping Tokio close to `ntex` on hot HTTP/1 cases
- reducing remaining large-JSON response construction overhead
- validating the runtime matrix periodically, not rediscovering old conclusions

Performance claims should be backed by:

- current benchmark summaries
- current methodology
- clear artifact provenance

---

## 9. Release Requirements

Before calling the project `1.0`-ready:

1. The root crate must publish and verify normally.
2. The public backend support policy must be explicit.
3. The stable API surface must be smaller and clearer than the full workspace
   implementation surface.
4. The docs must have one clear source of truth for product scope.
5. The remaining open issues must be mostly feature/completeness work, not
   architectural ambiguity.

---

## 10. Immediate Follow-On Work

The current highest-value work toward `1.0` is:

1. simplify the public runtime/API surface, especially Monoio
2. keep Meguri explicitly experimental until a promotion decision is made
3. complete codec fuzz/proptest hardening
4. reduce large-JSON response overhead
5. clean up the issue tracker to reflect the real product plan rather than the
   migration history
