# ADR-013: Cache-Aside with Graceful Degradation

**Status**: Accepted
**Date**: 2026-01
**Tickets**: TAS-156, TAS-168

## Context

Task template lookups and analytics queries hit PostgreSQL on every request. While PostgreSQL handles the load, caching provides latency reduction and load shedding benefits. The challenge: caching must never become a failure point. If Redis is down or misconfigured, Tasker must continue operating normally — PostgreSQL is always the source of truth.

Two caching contexts emerged with different requirements:

- **Task templates** (TAS-156): High-read, low-write. Cache internal to `TaskTemplateRegistry`. Redis or no-cache (in-memory adds stale-data risk in multi-instance deployments).
- **Analytics responses** (TAS-168): Moderate-read, time-bounded freshness. Can safely use in-memory (Moka) because analytics staleness is acceptable.

## Decision

Implement **cache-aside pattern** with **graceful degradation** and **type-safe provider constraints**:

- **Cache internal to services**: Cache boundary lives inside `TaskTemplateRegistry` and analytics services, not as a cross-cutting middleware. Reads and writes are co-located.
- **Three-variant `CacheProvider` enum**: `Redis`, `Moka` (in-memory), `NoOp` (pass-through)
- **Type-safe constraints**: `ConstrainedCacheProvider` enum dispatch restricts which providers each service can use:
  - Templates: Redis or NoOp only (prevents stale in-memory caches across instances)
  - Analytics: Redis, Moka, or NoOp (staleness is acceptable)
- **Best-effort cache writes**: Cache write failures are logged but never propagated. Database writes always succeed independently of cache state.
- **Graceful degradation**: If Redis is unreachable at startup, the system logs a warning and falls back to NoOp. It never fails to start due to cache unavailability.
- **Configuration-driven opt-in**: Caching is off by default. PostgreSQL remains the only hard dependency.
- **Worker boot invalidation**: Template registration automatically invalidates relevant cache entries.

## Consequences

### Positive

- Tasker never fails to start or serve requests due to cache issues
- Type-safe constraints prevent misuse (e.g., accidentally using Moka for templates in a multi-instance deployment)
- Cache-aside co-location means cache invalidation logic lives next to the write path
- Zero-cost enum dispatch — no trait objects or dynamic dispatch

### Negative

- Three provider variants and constraint types add cognitive overhead
- Cache-aside requires explicit invalidation on writes (no automatic TTL-based refresh)
- Redis becomes an optional operational dependency to monitor

### Neutral

- NoOp provider means "no caching" is a first-class configuration, not a special case
- Moka (in-memory) is useful for single-instance deployments and testing

## Alternatives Considered

### Alternative 1: Transparent Caching Middleware

HTTP-level or database-level caching middleware that caches all responses. Rejected because template caching requires domain-aware invalidation (on registration, not on TTL), and analytics caching has different freshness requirements than template caching.

### Alternative 2: Redis as Hard Dependency

Require Redis for all deployments. Rejected because Tasker's "PostgreSQL is the only hard dependency" principle is a key simplicity guarantee. Redis should improve performance, not be required for correctness.

### Alternative 3: Trait Objects for Cache Provider

`Box<dyn CacheProvider>` for runtime polymorphism. Rejected for the same reasons as ADR-010 (messaging): the provider set is small and known at compile time, enum dispatch is zero-cost and provides exhaustive matching.

## References

- [TAS-156 Spec](../ticket-specs/) (archived)
- [TAS-168 Spec](../ticket-specs/) (archived)
- [Caching Guide](../guides/caching.md)
