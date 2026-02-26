# ADR-011: Task Identity Strategy Pattern

**Status**: Implemented
**Date**: 2026-01
**Ticket**: TAS-154

## Context

Task deduplication and identity management needed a flexible, domain-aware approach. Different use cases have fundamentally different identity semantics:

- **Idempotent operations** (e.g., "process order #123") should deduplicate on context
- **Caller-controlled operations** (e.g., API clients providing their own idempotency keys) need explicit key support
- **Fire-and-forget operations** (e.g., "send notification") should always create a new task

A one-size-fits-all approach (always hash context, or always generate unique IDs) would force awkward workarounds in legitimate use cases.

## Decision

Implement three **identity strategies** configured per task template, with per-request override:

1. **STRICT** (default): Hash task context to generate `identity_hash`. Identical context produces identical hash, triggering UNIQUE constraint → 409 Conflict response.

2. **CALLER_PROVIDED**: Require an explicit `idempotency_key` on the `TaskRequest`. The caller controls deduplication semantics entirely. Time-bounded deduplication (e.g., "deduplicate within 1 hour") is a user-space concern — callers include a time bucket in their key.

3. **ALWAYS_UNIQUE**: Generate a UUIDv7 as the identity hash. Every request creates a new task, no deduplication.

Key design decisions:

- **Per-request override**: Any request can include `idempotency_key` to override the template's default strategy
- **409 Conflict response**: Duplicates return 409 (not 200 with existing task) to prevent UUID probing attacks
- **UNIQUE constraint on `identity_hash`**: Database-level enforcement, not application-level
- **Thundering herd safe**: Tested with 50 concurrent identical requests — exactly one succeeds

## Consequences

### Positive

- Each use case gets appropriate deduplication semantics without workarounds
- Database-level UNIQUE constraint prevents races that application-level checks would miss
- 409 Conflict response is security-conscious (doesn't leak existing task UUIDs)
- Callers control time-bounded deduplication without Tasker needing TTL infrastructure

### Negative

- Three strategies require documentation and understanding from API consumers
- CALLER_PROVIDED shifts deduplication correctness responsibility to the caller
- No built-in TTL-based deduplication (deliberate — keeps Tasker simple)

### Neutral

- Default (STRICT) provides safe behavior without configuration
- Strategy is per-template, so most users configure once and forget

## Alternatives Considered

### Alternative 1: Always Hash Context

Use context hashing for all tasks. Rejected because fire-and-forget tasks would require injecting unique values (timestamps, UUIDs) into context just to avoid deduplication — an awkward anti-pattern.

### Alternative 2: TTL-Based Deduplication

Built-in time-windowed deduplication (e.g., "deduplicate identical tasks within 1 hour"). Rejected because it adds infrastructure complexity (scheduled cleanup jobs, configurable TTLs per template). CALLER_PROVIDED with time-bucketed keys achieves the same result without Tasker needing TTL management.

### Alternative 3: Return 200 with Existing Task on Duplicate

Return the existing task on duplicate instead of 409. Rejected for security reasons — returning an existing task UUID to an unauthenticated duplicate request enables UUID probing.

## References

- [TAS-154 Spec](../ticket-specs/) (archived)
- [Identity Strategy Guide](../guides/identity-strategy.md)
- [Idempotency and Atomicity](../architecture/idempotency-and-atomicity.md)
