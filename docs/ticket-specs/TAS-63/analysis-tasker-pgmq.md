# Coverage Analysis: tasker-pgmq

**Current Coverage**: 49.53% line (aggregate: unit + E2E merged)
**Target**: 65%
**Gap**: ~15.5 percentage points

---

## Summary

The `tasker-pgmq` crate is a PostgreSQL LISTEN/NOTIFY integration layer for PGMQ queues, providing a unified client, notification listener, event types, and a CLI migration generator. Aggregate coverage sits at 49.53% (735/1484 lines) after merging unit and E2E reports -- well below the 65% target.

The largest gaps are in `listener.rs` (12% after E2E merge), `client.rs` (34%), and `emitter.rs` (41%). However, analysis of the runtime architecture reveals that `emitter.rs` contains **vestigial code** -- `DbEmitter` and its trait `PgmqNotifyEmitter` are never used by any consumer outside the crate. All notification emission happens through SQL wrapper functions (`pgmq_send_with_notify`), bypassing the Rust emitter entirely.

Reaching the 65% target requires ~230 additional lines covered, achievable through integration tests that validate the actual runtime paths (client queue operations, listener lifecycle and event delivery) rather than inflating coverage with struct builder tests.

## Why E2E Coverage Contributed So Little

E2E coverage for tasker-pgmq was 12.15% (118/971 lines) -- far less than expected. The explanation is architectural:

### The Provider Abstraction Gap

Services never call tasker-pgmq types directly. The runtime path goes through a provider abstraction in `tasker-shared`:

```
E2E test request → orchestration/worker service
  → SystemContext::messaging_provider()
    → MessagingProvider::Pgmq(PgmqMessagingService)   ← tasker-shared
      → PgmqMessagingService::send_message()           ← tasker-shared
        → self.client.send_json_message()               ← tasker-pgmq client.rs
          → SQL: pgmq_send_with_notify($1, $2, $3)     ← database-side, not Rust
```

### Per-Module E2E Impact

| Module | Unit % | E2E % | Why E2E Didn't Help |
|--------|--------|-------|---------------------|
| `emitter.rs` | 41.2% | **0.0%** | Never called at runtime -- notifications go through SQL functions, not Rust emitter |
| `events.rs` | 38.7% | **0.0%** | Events arrive as JSON from `pg_notify` and are deserialized via serde. Rust-side constructors/builders are never called at runtime |
| `error.rs` | 25.0% | **0.0%** | Module-specific error types; E2E path uses different error propagation |
| `types.rs` | 0.0% | **0.0%** | Compatibility bridge types; never triggered in the provider path |
| `listener.rs` | 10.3% | **12.1%** | Listener IS created and connected, but event processing loop runs in background tokio tasks that received minimal events before instrumented services were killed |
| `client.rs` | 34.2% | **20.6%** | Only `send_json_message()` exercised (via provider). Read/pop/delete unused -- workers receive via event-driven notification, not polling |
| `config.rs` | 80.6% | **34.4%** | Config loaded at startup; well-covered by unit tests already |
| `channel_metrics.rs` | 69.9% | **13.5%** | Channel monitor initialized but limited events under instrumented build |

### Key Insight: Instrumentation Overhead

27 of 50 E2E tests timed out under LLVM-instrumented debug builds. The complex scenarios (batch processing, retries, domain events) that would exercise MPSC channels, fallback pollers, and the listener event loops all timed out. Only "fast path" scenarios (simple task creation/completion) completed, which limits coverage to basic client send and listener connection paths.

## Vestigial Code: DbEmitter

### Finding

`DbEmitter`, `NoopEmitter`, `PgmqNotifyEmitter` trait, and `EmitterFactory` are **never used outside `tasker-pgmq`**. Grep across the entire workspace confirms:

- **Defined in**: `tasker-pgmq/src/emitter.rs`
- **Re-exported from**: `tasker-pgmq/src/lib.rs` (`pub use emitter::{DbEmitter, NoopEmitter, PgmqNotifyEmitter}`)
- **Imported by zero external consumers**

### Why It's Vestigial

The `DbEmitter` was designed as an application-level notification emitter -- Rust code that calls `NOTIFY` via SQL. But the architecture evolved to use `pgmq_send_with_notify()`, a SQL wrapper function that atomically combines message insertion with notification emission at the database level. This SQL function includes its own payload size validation (7800-byte limit check at lines 72, 152, 211 of the migration).

The `build_payload()` logic in `DbEmitter` (size validation, metadata stripping) duplicates protections that already exist in SQL. The `PgmqNotifyEmitter` trait defines an interface (`emit_queue_created`, `emit_message_ready`, etc.) that no runtime code path calls.

### Recommendation

**Remove `emitter.rs` entirely** (or extract `build_payload` if the config-driven metadata stripping is needed elsewhere). This would:

- Remove 131 lines from the coverage denominator, immediately improving coverage ratio
- Eliminate a misleading public API surface that suggests functionality the system doesn't use
- Remove the `PgmqNotifyEmitter` trait that no code implements outside this module
- Clean up `NoopEmitter` which only exists as a test double for the unused `DbEmitter`

The `max_payload_size` and `include_metadata` config fields in `PgmqNotifyConfig` are only used by `DbEmitter.build_payload()` and the CLI migration generator (`bin/cli.rs`). The CLI uses them for generating SQL that includes the database-side size checks, so those config fields should remain -- but the Rust-side emitter code that consumes them is redundant.

**Impact on coverage**: Removing 131 lines from the denominator (currently 54 covered / 131 total) changes the math:

- Before removal: 735 / 1484 = 49.53%
- After removal: 681 / 1353 = 50.33%
- New lines needed for 65%: `0.65 * 1353 = 880` → need 199 more lines (vs 230 before)

### Action Item

Before removing, verify:

1. No downstream consumers outside this repo import `DbEmitter` / `PgmqNotifyEmitter`
2. The CLI migration generator (`bin/cli.rs`) doesn't depend on emitter types (it uses `PgmqNotifyConfig` directly -- confirmed)
3. No planned features depend on the Rust-side emitter pattern

This should be tracked as a separate ticket (cleanup/refactor), not part of TAS-63 coverage work.

## File Coverage Overview

| File | Lines Covered | Lines Total | Aggregate % | E2E Contribution |
|------|--------------|-------------|-------------|------------------|
| `types.rs` | 0 | 12 | 0.00% | None |
| `listener.rs` | 27 | 252 | ~12% | +1 line from E2E |
| `error.rs` | 5 | 20 | 25.00% | None |
| `client.rs` | 82 | 240 | 34.17% | E2E covered send path only |
| `events.rs` | 94 | 243 | 38.68% | None |
| `emitter.rs` | 54 | 131 | 41.22% | None (vestigial) |
| `channel_metrics.rs` | 146 | 209 | 69.86% | Minimal |
| `config.rs` | 104 | 129 | 80.62% | Startup loading |
| `bin/cli.rs` | 223 | 276 | 80.80% | N/A (separate binary) |

## Revised Test Plan: High-Value Tests Only

Organized by what gives **real system confidence** vs what just moves the coverage number. Tests that validate "can a struct hold a value" are excluded.

### Tier 1: Critical Runtime Guarantees (~135-155 lines)

These test things that, if broken, would silently corrupt production behavior.

**1. Client Queue Lifecycle (integration, `client.rs`, ~40 lines)**

Create queue -> send message -> read it back -> verify content matches -> delete -> verify gone -> drop queue. Validates the PGMQ SQL wrappers work end-to-end. The `send_json_message` path exercises `pgmq_send_with_notify`, testing the atomic send+notify SQL function from Rust.

**Why this matters**: This is the primary message flow. If `send_json_message` silently fails to call the SQL wrapper correctly, messages are lost.

**2. Listener Connect -> Subscribe -> Receive -> Parse (integration, `listener.rs`, ~60-80 lines)**

Connect to PG -> listen on a channel -> send `NOTIFY` from a separate connection -> verify the listener receives and correctly parses the event to `PgmqNotifyEvent`. Tests both the connection lifecycle and the actual event delivery path.

**Why this matters**: This is the real-time notification path that EventDriven and Hybrid deployment modes depend on. If broken, the system silently degrades to polling-only with no error indication.

**3. Client send_with_transaction Atomicity (integration, `client.rs`, ~15 lines)**

Begin transaction -> send via `send_with_transaction` -> commit -> read message -> verify present. This validates the transactional send path used during step processing.

**Why this matters**: Step result publishing uses transactional sends to ensure message delivery is atomic with database state changes. If this breaks, steps can complete in the database but their results never reach the orchestrator.

**4. Client read_specific_message Deserialization (integration, `client.rs`, ~20 lines)**

Send a typed message -> read it back with `read_specific_message` -> verify deserialized type matches. Also test with a message that fails deserialization to verify the error path.

**Why this matters**: Notification-driven message consumption uses `read_specific_message` to fetch the full message after receiving a lightweight notification. Deserialization failures here would silently drop messages.

### Tier 2: Operational Correctness (~85-95 lines)

**5. Listener State Machine (integration, `listener.rs`, ~30-40 lines)**

Connect -> verify `is_healthy() == true`, `stats().connected == true` -> subscribe to channels -> verify `listening_channels()` returns them -> disconnect -> verify `is_healthy() == false`, channels cleared, stats updated.

**Why this matters**: Prevents resource leaks and stale subscriptions. If `disconnect()` doesn't clear state, reconnection attempts may fail or duplicate subscriptions.

**6. Listener NotConnected Guards (integration, `listener.rs`, ~15 lines)**

Call `listen_channel()`, `unlisten_channel()`, `start_listening()` without calling `connect()` first -> verify `NotConnected` error returned.

**Why this matters**: If these silently succeed without a connection, the application believes it's listening when it isn't -- a silent failure that's hard to diagnose in production.

**7. Client Namespace Operations (integration, `client.rs`, ~20 lines)**

Test `initialize_namespace_queues` creates expected queues -> `process_namespace_queue` reads from correct queue name -> `complete_message` deletes from correct queue.

**Why this matters**: These helper methods construct queue names from namespace strings. If the name construction is wrong, messages go to/from the wrong queues.

**8. Client extract_namespace Edge Cases (unit, `client.rs`, ~10 lines)**

Test with matching pattern, non-matching pattern, fallback `_queue` suffix stripping, empty string, no named group in regex.

**Why this matters**: Namespace extraction drives message routing decisions. Edge cases in regex matching could cause silent misrouting.

### What to Skip (Coverage Without Confidence)

| Category | Lines | Why Skip |
|----------|-------|----------|
| Event builder tests (`events.rs`) | ~80-100 | Simple setter chains on data structs. Builders are never called at runtime -- events come from JSON deserialization. |
| Error factory tests (`error.rs`) | ~15 | Simple string constructors. Zero confidence value. |
| types.rs From impls | ~12 | Trivial type conversions for a compatibility bridge. |
| NoopEmitter method tests (`emitter.rs`) | ~10 | Testing that no-ops return Ok. If we remove DbEmitter, NoopEmitter goes too. |
| channel_metrics remaining gaps | ~25 | Already at 69.9%. Uncovered parts are derive-adjacent accessors. |
| Event deserialization roundtrips | ~30 | Serde derive handles this; if the struct compiles with `Serialize`/`Deserialize`, roundtrips work. |

## Coverage Projection

### With emitter.rs removal + Tier 1+2 tests

| Step | Lines Covered | Lines Total | Coverage |
|------|--------------|-------------|----------|
| Current (aggregate) | 735 | 1,484 | 49.53% |
| Remove emitter.rs (-54 covered, -131 total) | 681 | 1,353 | 50.33% |
| Tier 1 tests (+135-155 lines) | ~826 | 1,353 | ~61% |
| Tier 2 tests (+85-95 lines) | ~916 | 1,353 | ~67-68% |

### Without emitter.rs removal (tests only)

| Step | Lines Covered | Lines Total | Coverage |
|------|--------------|-------------|----------|
| Current (aggregate) | 735 | 1,484 | 49.53% |
| Tier 1 tests (+135-155 lines) | ~880 | 1,484 | ~59% |
| Tier 2 tests (+85-95 lines) | ~970 | 1,484 | ~65-66% |

Either path reaches the 65% target. The emitter removal provides margin and cleans up the API surface.

## Key Dependencies for Integration Tests

- PostgreSQL instance with PGMQ extension installed
- `DATABASE_URL` environment variable set
- Feature flag `test-messaging` or `test-services` enabled
- SQL functions `pgmq_send_with_notify` and `pgmq_read_specific_message` installed
- For listener event delivery tests: ability to send `NOTIFY` from a separate connection (use `sqlx::query("SELECT pg_notify($1, $2)")`)

## Action Items

1. **Write Tier 1 integration tests** (items 1-4) -- validates critical runtime guarantees
2. **Write Tier 2 integration tests** (items 5-8) -- validates operational correctness
3. **Investigate DbEmitter removal** (separate ticket) -- remove vestigial emitter module and clean up `pub use` exports in `lib.rs`
4. **Re-run aggregate coverage** after tests to confirm 65% threshold met
