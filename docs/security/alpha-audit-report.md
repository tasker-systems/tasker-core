# Focused Architectural and Security Audit Report

**Audit Date**: 2026-02-05
**Auditor**: Claude (Opus 4.6 / Sonnet 4.5 sub-agents)
**Status**: Complete

---

## Executive Summary

This audit evaluates all Tasker Core crates for alpha readiness across security, error handling, resilience, and architecture dimensions. Findings are categorized by severity (Critical/High/Medium/Low/Info) per the methodology defined in the audit specification.

### Alpha Readiness Verdict

**ALPHA READY** with targeted fixes. No Critical vulnerabilities found. The High-severity items (dependency CVE, input validation gaps, shutdown timeouts) are straightforward fixes that can be completed in a single sprint.

### Consolidated Finding Counts (All Crates)

| Severity | Count | Status |
|----------|-------|--------|
| Critical | 0 | None found |
| High | 9 | Must fix before alpha |
| Medium | 22 | Document as known limitations |
| Low | 13 | Track for post-alpha |

### High-Severity Findings (Must Fix Before Alpha)

| ID | Finding | Crate | Fix Effort | Remediation |
|----|---------|-------|------------|-------------|
| S-1 | Queue name validation missing | tasker-shared | Small | Queue name validation |
| S-2 | SQL error details exposed to clients | tasker-shared | Medium | Error message sanitization |
| S-3 | `#[allow]` → `#[expect]` (systemic) | All | Small (batch) | Lint compliance cleanup |
| P-1 | NOTIFY channel name unvalidated | tasker-pgmq | Small | Queue name validation |
| O-1 | No actor panic recovery | tasker-orchestration | Medium | Shutdown and recovery hardening |
| O-2 | Graceful shutdown lacks timeout | tasker-orchestration | Small | Shutdown and recovery hardening |
| W-1 | checkpoint_yield blocks FFI without timeout | tasker-worker | Small | FFI checkpoint timeout |
| X-1 | `bytes` v1.11.0 CVE (RUSTSEC-2026-0007) | Workspace | Trivial | Dependency upgrade |
| P-2 | CLI migration SQL generation unescaped | tasker-pgmq | Small | Queue name validation |

---

## Crate 1: tasker-shared

**Overall Rating**: A- (Strong foundation with targeted improvements needed)

The `tasker-shared` crate is the largest and most foundational crate in the workspace. It provides core types, error handling, messaging abstraction, security services, circuit breakers, configuration management, database utilities, and shared models. The crate demonstrates strong security practices overall.

### Strengths

- **Zero unsafe code** across the entire crate
- **Excellent cryptographic hygiene**: Constant-time API key comparison via `subtle::ConstantTimeEq` (`src/types/api_key_auth.rs:53-62`), JWKS hardening with SSRF prevention (blocks private IPs, cloud metadata endpoints, requires HTTPS), algorithm allowlist enforcement (no `alg: none`)
- **Comprehensive input validation**: JSONB validation with size/depth/key count limits (`src/validation.rs`), namespace validation with PostgreSQL identifier rules, XSS sanitization
- **100% SQLx macro usage**: All database queries use compile-time verified `sqlx::query!` macros, zero string interpolation in SQL
- **Lock-free circuit breakers**: Atomic state management (`AtomicU8` for state, `AtomicU64` for metrics), proper memory ordering, correct state machine transitions
- **All MPSC channels bounded and config-driven**: Full bounded-channel compliance
- **Exemplary config security**: Environment variable allowlist with regex validation, TOML injection prevention via `escape_toml_string()`, fail-fast on validation errors
- **No hardcoded secrets**: All sensitive values come from env vars or file paths
- **Well-organized API surface**: Feature-gated modules (web-api, grpc-api), selective re-exports

### Finding S-1 (HIGH): Queue Name Validation Missing

**Location**: `tasker-shared/src/messaging/service/router.rs:96-97`

Queue names are constructed via `format!` with unvalidated namespace input:
```rust
fn step_queue(&self, namespace: &str) -> String {
    format!("{}_{}_queue", self.worker_queue_prefix, namespace)
}
```

The `MessagingError::InvalidQueueName` variant exists (`src/messaging/errors.rs:56`) but is **never raised**. Neither the router nor the provider implementations (`pgmq.rs:134-139`, `rabbitmq.rs:276-375`) validate queue names before passing them to native queue APIs.

**Risk**: PGMQ creates PostgreSQL tables named after queues — special characters in namespace could cause SQL issues at the DDL level. RabbitMQ queue creation could fail with unexpected characters.

**Recommendation**: Add `validate_queue_name()` that enforces alphanumeric + underscore/hyphen, 1-255 chars. Call it in `DefaultMessageRouter` methods and/or `ensure_queue()`.

### Finding S-2 (HIGH): SQL Error Details Exposed to Clients

**Location**: `tasker-shared/src/errors.rs:71-74, 431-437`

```rust
impl From<sqlx::Error> for TaskerError {
    fn from(err: sqlx::Error) -> Self {
        TaskerError::DatabaseError(err.to_string())
    }
}
```

`sqlx::Error::to_string()` can expose SQL query details, table/column names, constraint names, and potentially connection string information. These error messages may propagate to API responses.

**Recommendation**: Create a sanitized error mapper that logs full details internally but returns generic messages to API clients (e.g., "Database operation failed" with an internal error ID for correlation).

### Finding S-3 (HIGH): `#[allow]` Used Instead of `#[expect]` (Lint Policy Violation)

**Locations**:
- `src/messaging/execution_types.rs:383` — `#[allow(clippy::too_many_arguments)]`
- `src/web/authorize.rs:194` — `#[allow(dead_code)]`
- `src/utils/serde.rs:46-47` — `#[allow(dead_code)]`

Project lint policy mandates `#[expect(lint_name, reason = "...")]` instead of `#[allow]`. This is a policy compliance issue.

**Recommendation**: Convert all `#[allow]` to `#[expect]` with documented reasons.

### Finding S-4 (MEDIUM): `unwrap_or_default()` Violations of Tenet #11 (Fail Loudly)

**Locations** (20+ instances across crate):
- `src/messaging/execution_types.rs:120,186,213` — Step execution status defaults to empty string
- `src/database/sql_functions.rs:377,558` — Query results default to empty vectors
- `src/registry/task_handler_registry.rs:214,268,656,700,942` — Config schema fields default silently
- `src/proto/conversions.rs:32` — Invalid timestamps silently default to UNIX epoch

**Risk**: Required fields silently defaulting to empty values can mask real errors and produce incorrect behavior that's hard to debug.

**Recommendation**: Audit all `unwrap_or_default()` usages. Replace with explicit error returns for required fields. Keep `unwrap_or_default()` only for truly optional fields with documented rationale.

### Finding S-5 (MEDIUM): Error Context Loss in `.map_err(|_| ...)`

14 instances where original error context is discarded:
- `src/messaging/service/providers/rabbitmq.rs:544` — Discards parse error
- `src/messaging/service/providers/in_memory.rs:305,331,368` — 3 instances
- `src/state_machine/task_state_machine.rs:114` — Discards parse error
- `src/state_machine/actions.rs:256,372,434,842` — 4 instances discarding publisher errors
- `src/config/config_loader.rs:220,417` — 2 instances discarding env var errors
- `src/database/sql_functions.rs:1032` — Discards decode error
- `src/types/auth.rs:283` — Discards parse error

**Recommendation**: Include original error via `.map_err(|e| SomeError::new(context, e.to_string()))`.

### Finding S-6 (MEDIUM): Production `expect()` Calls

- `src/macros.rs:65` — Panics if Tokio task spawning fails
- `src/cache/provider.rs:399,429,459,489,522` — Multiple `expect("checked in should_use")` calls

**Risk**: Panics in production code. While guarded by preconditions, they bypass error propagation.

**Recommendation**: Replace with `Result` propagation or add detailed safety comments explaining invariant guarantees.

### Finding S-7 (MEDIUM): Database Pool Config Lacks Validation

Database pool configuration (`PoolConfig`) does not have a `validate()` method. Unlike circuit breaker config which validates ranges (failure_threshold > 0, timeout <= 300s), pool config relies on sqlx to reject invalid values at runtime.

**Recommendation**: Add validation: `max_connections > 0`, `min_connections <= max_connections`, `acquire_timeout_seconds > 0`.

### Finding S-8 (MEDIUM): Individual Query Timeouts Missing

While database pools have `acquire_timeout` configured (`src/database/pools.rs:169-170`), individual `sqlx::query!` calls lack explicit timeout wrappers. Long-running queries rely solely on pool-level timeouts.

**Recommendation**: Consider PostgreSQL `statement_timeout` at the connection level, or add `tokio::time::timeout()` wrappers around critical query paths.

### Finding S-9 (LOW): Message Size Limits Not Enforced

Messaging deserialization uses `serde_json::from_slice()` without explicit size limits. While PGMQ has implicit limits from PostgreSQL column sizes, a very large message could cause memory issues during deserialization.

**Recommendation**: Add configurable message size limits at the provider level.

### Finding S-10 (LOW): File Path Exposure in Config Errors

`src/services/security_service.rs:184-187` — Configuration errors include filesystem paths. Only occurs during startup (not exposed to API clients in normal operation).

### Finding S-11 (LOW): Timestamp Conversion Silently Defaults to Epoch

`src/proto/conversions.rs:32` — `DateTime::from_timestamp().unwrap_or_default()` silently converts invalid timestamps to 1970-01-01 instead of returning an error.

### Finding S-12 (LOW): `cargo-machete` Ignore List Has 19 Entries

`Cargo.toml:12-39` — Most are legitimately feature-gated or used via macros, but the list should be periodically audited to prevent dependency bloat.

### Finding S-13 (LOW): Global Wildcard Permission Rejection Undocumented

`src/types/permissions.rs` — The `permission_matches()` function correctly rejects global wildcard (`*`) permissions but this behavior isn't documented in user-facing comments.

---

## Crate 2: tasker-pgmq

**Overall Rating**: B+ (Good with one high-priority fix needed)

The `tasker-pgmq` crate is a PGMQ wrapper providing PostgreSQL LISTEN/NOTIFY support for event-driven message processing. ~3,345 source lines across 9 files. No dependencies on `tasker-shared` (clean separation).

### Strengths

- **No unsafe code** across the entire crate
- **Payload uses parameterized queries**: Message payloads bound via `$1` parameter in NOTIFY
- **Payload size validation**: Enforces pg_notify 8KB limit
- **Comprehensive thiserror error types** with context preservation
- **Bounded channels**: All MPSC channels bounded
- **Good test coverage**: 6 integration test files covering major flows
- **Clean separation from tasker-shared**: No duplication, standalone library

### Finding P-1 (HIGH): SQL Injection via NOTIFY Channel Name

**Location**: `tasker-pgmq/src/emitter.rs:122`

```rust
let sql = format!("NOTIFY {}, $1", channel);
sqlx::query(&sql).bind(payload).execute(&self.pool)
```

PostgreSQL's `NOTIFY` does not support parameterized channel identifiers. The channel name is interpolated directly via `format!`. Channel names flow from `config.build_channel_name()` which concatenates `channels_prefix` (from TOML config) with base channel names and namespace strings.

**Risk**: While the NOTIFY command has limited injection surface (it's not a general SQL execution vector), malformed channel names could cause PostgreSQL errors, unexpected channel routing, or denial of service. The channels_prefix comes from config (lower risk), but namespace strings flow from queue operations.

**Recommendation**: Add channel name validation — allow only `[a-zA-Z0-9_.]+`, max 63 chars (PostgreSQL identifier limit). Apply in `build_channel_name()` and/or `notify_channel()`.

### Finding P-2 (HIGH): CLI Migration SQL Generation with Unescaped Input

**Location**: `tasker-pgmq/src/bin/cli.rs:179-353`

User-provided regex patterns and channel prefixes are directly interpolated into SQL migration strings when generating migration files. While these are generated files that should be reviewed before application, the lack of escaping creates a risk if the generation process is automated.

**Recommendation**: Validate inputs against strict patterns before interpolation. Add a warning comment in generated files that they should be reviewed.

### Finding P-3 (MEDIUM): `unwrap_or_default()` on Database Results (Tenet #11)

**Location**: `tasker-pgmq/src/client.rs:164`

```rust
.read_batch(queue_name, visibility_timeout, l).await?.unwrap_or_default()
```

When `read_batch` returns `None`, this silently produces an empty vector instead of failing loudly. Could mask permission errors, connection failures, or other serious issues.

**Recommendation**: Return explicit error on unexpected `None`.

### Finding P-4 (MEDIUM): RwLock Poison Handling Masks Panics

**Location**: `tasker-pgmq/src/listener.rs` (22 instances)

```rust
self.stats.write().unwrap_or_else(|p| p.into_inner())
```

Silently recovers from poisoned RwLock without logging. Could propagate corrupted state from a panicked thread.

**Recommendation**: Log warning on poison recovery, or switch to `parking_lot::RwLock` (doesn't poison).

### Finding P-5 (MEDIUM): Hardcoded Pool Size

**Location**: `tasker-pgmq/src/client.rs:41-44`

```rust
let pool = sqlx::postgres::PgPoolOptions::new()
    .max_connections(20)  // Hard-coded
    .connect(database_url).await?;
```

Pool size should be configurable for different deployment scenarios.

### Finding P-6 (MEDIUM): Missing Async Operation Timeouts

Database operations in `client.rs`, `emitter.rs`, and `listener.rs` lack explicit `tokio::time::timeout()` wrappers. Relies solely on pool-level acquire timeouts.

### Finding P-7 (LOW): Error Context Loss in Regex Compilation

**Location**: `tasker-pgmq/src/config.rs:169`

```rust
Regex::new(&self.queue_naming_pattern)
    .map_err(|_| PgmqNotifyError::invalid_pattern(&self.queue_naming_pattern))
```

Original regex error details discarded.

### Finding P-8 (LOW): `#[allow]` Instead of `#[expect]` (Lint Policy)

**Location**: `tasker-pgmq/src/emitter.rs:299-320` — 3 instances of `#[allow(dead_code)]` on `EmitterFactory`.

---

## Crate 3: tasker-orchestration

**Overall Rating**: A- (Strong security with targeted resilience improvements needed)

The `tasker-orchestration` crate handles core orchestration logic: actors, state machines, REST + gRPC APIs, and auth middleware. This is the largest service crate and the primary attack surface.

### Strengths

- **Zero unsafe code** across the entire crate
- **Excellent auth architecture**: Constant-time API key comparison, JWT algorithm allowlist, JWKS SSRF prevention, auth before body parsing
- **gRPC/REST auth parity verified**: All 6 gRPC task methods enforce identical permissions to REST counterparts
- **No auth bypass found**: All API v1 routes wrapped in `authorize()`, health/metrics public by design
- **Database-level atomic claiming**: `FOR UPDATE SKIP LOCKED` prevents concurrent state corruption
- **State transitions enforce ownership**: No API endpoint allows direct state manipulation
- **Sanitized error responses**: No stack traces, database errors genericized, consistent JSON format
- **Backpressure checked before resource operations**: 503 with Retry-After header
- **Full bounded-channel compliance**: All MPSC channels bounded and config-driven (0 unbounded channels)
- **HTTP request timeout**: `TimeoutLayer` with configurable 30s default

### Finding O-1 (HIGH): No Actor Panic Recovery

**Location**: `tasker-orchestration/src/actors/command_processor_actor.rs:139`

Actors spawn via `spawn_named!` but have no supervisor/restart logic. If `OrchestrationCommandProcessorActor` panics, the entire orchestration processing stops. Recovery requires full process restart.

**Recommendation**: Implement panic-catching wrapper with logged restart, or document that process-level supervision (systemd, k8s) handles this.

### Finding O-2 (HIGH): Graceful Shutdown Lacks Timeout

**Locations**:
- `tasker-orchestration/src/orchestration/bootstrap.rs:177-213`
- `tasker-orchestration/src/bin/server.rs:68-82`

Shutdown calls `coordinator.lock().await.stop().await` and `orchestration_handle.stop().await` with no timeout. If the event coordinator or actors hang during shutdown, the server never completes graceful shutdown.

**Recommendation**: Add 30-second timeout with force-kill fallback.

### Finding O-3 (HIGH): `#[allow]` Instead of `#[expect]` (Lint Policy)

21 instances of `#[allow]` found across the crate (most without `reason =` clause):
- `src/actors/traits.rs:67,81`
- `src/web/extractors.rs:6`
- `src/health/channel_status.rs:87`
- `src/grpc/conversions.rs:42`
- And 16 more locations

### Finding O-4 (MEDIUM): Request Validation Not Enforced at Handler Layer

**Location**: `src/web/handlers/tasks.rs:47`

`TaskRequest` has `#[derive(Validate)]` with constraints (name length 1-255, namespace length 1-255, priority range -100 to 100) but handlers accept `Json<TaskRequest>` without calling `.validate()`. Validation happens later at the service layer.

**Impact**: Oversized payloads are deserialized before rejection. Not a security vulnerability per se, but the defense-in-depth pattern would catch malformed input earlier.

**Recommendation**: Add `.validate()` at handler entry or use `Valid<Json<TaskRequest>>` extractor.

### Finding O-5 (MEDIUM): Actor Shutdown May Lose In-Flight Work

**Location**: `tasker-orchestration/src/actors/registry.rs:216-259`

Shutdown uses `Arc::get_mut()` which only works if no other references exist. If `get_mut` fails, `stopped()` is silently skipped. In-flight work may be lost.

### Finding O-6 (MEDIUM): Database Query Timeouts Missing

Same pattern as tasker-shared (Finding S-8). Individual `sqlx::query!` calls lack explicit timeout wrappers:
- `src/services/health/service.rs:284` — health check query
- `src/orchestration/backoff_calculator.rs:232,245,290,345,368` — multiple queries

Pool-level acquire timeout (30s) provides partial mitigation.

### Finding O-7 (MEDIUM): `unwrap_or_default()` on Config Fields

- `src/orchestration/event_systems/unified_event_coordinator.rs:89` — event system config
- `src/orchestration/bootstrap.rs:581` — namespace config
- `src/grpc/services/config.rs:96-97` — `jwt_issuer` and `jwt_audience` default to empty strings

### Finding O-8 (MEDIUM): Error Context Loss

~12 instances of `.map_err(|_| ...)` discarding error context:
- `src/orchestration/bootstrap.rs:203` — oneshot send error
- `src/web/handlers/health.rs:53` — timeout error
- `src/web/handlers/tasks.rs:113` — UUID parse error

### Finding O-9 (MEDIUM): Hardcoded Magic Numbers

- `src/services/task_service.rs:257-259` — `per_page > 100` validation
- `src/orchestration/event_systems/orchestration_event_system.rs:142` — 24h max message age
- `src/services/analytics_query_service.rs:229` — 30.0s slow step threshold

### Finding O-10 (LOW): gRPC Internal Error May Leak Details

**Location**: `src/grpc/conversions.rs:152-153`

`tonic::Status::internal(error.to_string())` — depending on error `Display` implementations, could expose implementation details in gRPC error messages.

### Finding O-11 (LOW): CORS Allows Any Origin

**Location**: `src/web/mod.rs`

```rust
CorsLayer::new()
    .allow_origin(tower_http::cors::Any)
    .allow_methods(tower_http::cors::Any)
    .allow_headers(tower_http::cors::Any)
```

Acceptable for alpha/API service, but should be configurable for production deployments.

## Crate 4: tasker-worker

**Overall Rating**: A- (Strong FFI safety with one notable gap)

The `tasker-worker` crate handles handler dispatch, FFI integration, and completion processing. Despite complex FFI requirements, it achieves this with **zero unsafe blocks** in the crate itself.

### Strengths

- **Zero unsafe code** despite handling Ruby/Python FFI integration
- **All SQL queries via sqlx macros** — no string interpolation
- **Handler panic containment**: `catch_unwind()` + `AssertUnwindSafe` wraps all handler calls
- **Error classification preserved**: Permanent/Retryable distinction maintained across FFI boundary
- **Fire-and-forget callbacks**: Spawned into runtime, 5s timeout, no deadlock risk
- **FFI completion circuit breaker**: Latency-based, 100ms threshold, lock-free metrics
- **All MPSC channels bounded** — full bounded-channel compliance
- **No production unwrap()/expect()** in core paths

### Finding W-1 (HIGH): `checkpoint_yield` Blocks FFI Thread Without Timeout

**Location**: `tasker-worker/src/worker/handlers/ffi_dispatch_channel.rs:904`

```rust
let result = self.config.runtime_handle.block_on(async {
    self.handle_checkpoint_yield_async(/* ... */).await
});
```

Uses `block_on` which blocks the Ruby/Python thread while persisting checkpoint data to the database. No timeout wrapper. If the database is slow, this blocks the FFI thread indefinitely, potentially exhausting the thread pool.

**Recommendation**: Add `tokio::time::timeout()` around the `block_on` body (configurable, suggest 10s default).

### Finding W-2 (MEDIUM): Starvation Detection is Warning-Only

**Location**: `tasker-worker/src/worker/handlers/ffi_dispatch_channel.rs:772-793`

`check_starvation_warnings()` logs warnings but doesn't enforce any action. Also requires manual invocation by the caller — no automatic monitoring loop.

### Finding W-3 (MEDIUM): FFI Thread Safety Documentation Gap

The `FfiDispatchChannel` uses `Arc<Mutex<mpsc::Receiver>>` (thread-safe) but lacks documentation about thread-safety guarantees, `poll()` contention behavior, and `block_on` safety in FFI context.

### Finding W-4 (MEDIUM): `#[allow]` vs `#[expect]` (Lint Policy)

5 instances in `web/middleware/mod.rs` and `web/middleware/request_id.rs`.

### Finding W-5 (MEDIUM): Missing Database Query Timeouts

Same systemic pattern as other crates. Checkpoint service and step claim queries lack explicit timeout wrappers.

### Finding W-6 (LOW): `unwrap_or_default()` in `worker/core.rs`

Several instances, appear to be for optional fields (likely legitimate), but warrants review.

---

## Crates 5-6: tasker-client & tasker-cli

**Overall Rating**: A (Excellent — cleanest crates in the workspace)

These client crates demonstrate the strongest compliance across all audit dimensions. Notably, **lint policy compliant** (using `#[expect]` already). No Critical or High findings.

### Strengths

- **No unsafe code** in either crate
- **No hardcoded credentials** — all auth from env vars or config files
- **RSA key generation validates minimum 2048-bit** keys
- **Proper error context preservation** in all `From` conversions
- **Complete transport abstraction**: REST and gRPC both implement 11/11 methods
- **HTTP/gRPC timeouts configured**: 30s request, 10s connect
- **Exponential backoff retry** for `create_task` with configurable max retries
- **Lint policy compliant** — uses `#[expect]` with reasons
- **User-facing CLI errors informative** without leaking internals

### Finding C-1 (MEDIUM): TLS Certificate Validation Not Explicitly Enforced

**Location**: `tasker-client/src/api_clients/orchestration_client.rs:220`

HTTP client uses `reqwest::Client::builder()` without explicitly setting `.danger_accept_invalid_certs(false)`. Default is secure, but explicit enforcement prevents accidental changes.

### Finding C-2 (MEDIUM): Default URLs Use HTTP

**Location**: `tasker-client/src/config.rs:276`

Default `base_url` is `http://localhost:8080`. Credentials transmitted over HTTP are vulnerable to interception. Appropriate for local dev, but should warn when HTTP is used with authentication enabled.

### Finding C-3 (MEDIUM): Retry Logic Only on `create_task`

Other operations (`get_task`, `list_tasks`, etc.) do not retry on transient failures. Should either extend retry logic or document the limitation.

### Finding C-4 (LOW): Production `expect()` in Config Initialization

`tasker-client/src/api_clients/orchestration_client.rs:123` — panics if config is malformed. Acceptable during startup but could return `Result` instead.

---

## Crates 7-10: Language Workers (Rust, Ruby, Python, TypeScript)

**Overall Rating**: A- (Strong FFI engineering, no critical gaps)

All 4 language workers share common architecture via `FfiDispatchChannel` for poll-based event dispatch. Audited ~22,000 lines of Rust FFI code plus language wrappers.

### Strengths

- **TypeScript: Comprehensive panic handling** — `catch_unwind` on all critical FFI functions, errors converted to JSON error responses
- **Ruby/Python: Managed FFI** via Magnus and PyO3 — these frameworks handle panic unwinding automatically via their exception systems
- **Error classification preserved across all FFI boundaries**: Permanent/Retryable distinction maintained
- **Fire-and-forget callbacks**: No deadlock risk identified
- **Starvation detection functional** in all workers
- **Proper Arc usage** for thread-safe shared ownership across FFI
- **TypeScript C FFI: Correct string memory management** with `into_raw()`/`from_raw()` pattern and `free_rust_string()` for caller cleanup
- **Checkpoint support uniformly implemented** across all 4 workers
- **Consistent error hierarchy** across all languages

### Finding LW-1 (MEDIUM): TypeScript FFI Missing Safety Documentation

**Location**: `workers/typescript/src-rust/lib.rs:38`

`#![allow(clippy::missing_safety_doc)]` — suppresses docs for 9 `unsafe extern "C"` functions. Should use `#[expect]` per lint policy and add `# Safety` sections.

### Finding LW-2 (MEDIUM): Rust Worker `#[allow(dead_code)]` (Lint Policy)

**Location**: `workers/rust/src/event_subscribers/logging_subscriber.rs:60,98,132`

3 instances of `#[allow(dead_code)]` instead of `#[expect]`.

### Finding LW-3 (LOW): Ruby Bootstrap Uses `expect()` on Ruby Runtime

**Location**: `workers/ruby/ext/tasker_core/src/bridge.rs:19-20`, `bootstrap.rs:29-30`

`Ruby::get().expect("Ruby runtime should be available")` — safe in practice (guaranteed by Magnus FFI contract) but could use `?` for defensive programming.

### Finding LW-4 (LOW): Timeout Cleanup Requires Manual Polling

`cleanup_timeouts()` exists in all FFI workers but documentation doesn't specify recommended polling frequency. Workers must call this periodically.

### Finding LW-5 (LOW): Ruby Tokio Thread Pool Hardcoded to 8

**Location**: `workers/ruby/ext/tasker_core/src/bootstrap.rs:74-79`

Hardcoded `.worker_threads(8)` for M2/M4 Pro compatibility. Python/TypeScript use defaults. Consider making configurable.

---

## Cross-Cutting Concerns

### Dependency Audit (`cargo audit`)

**Finding X-1 (HIGH): `bytes` v1.11.0 Integer Overflow (RUSTSEC-2026-0007)**

Published 2026-02-03. Integer overflow in `BytesMut::reserve`. Fix: upgrade to `bytes >= 1.11.1`. This is a transitive dependency used by tokio, hyper, axum, tonic, reqwest, sqlx — deeply embedded.

**Recommendation**: Add to workspace `Cargo.toml`: `bytes = "1.11.1"`

### Finding X-2 (LOW): `rustls-pemfile` Unmaintained (RUSTSEC-2025-0134)

Transitive from `lapin` (RabbitMQ) → `amq-protocol` → `tcp-stream` → `rustls-pemfile`. No action available from this project; depends on upstream `lapin` update.

### Clippy Compliance

**Zero warnings** across entire workspace with `--all-targets --all-features`. Excellent.

### Systemic: `#[allow]` vs `#[expect]` (Lint Policy)

**27 instances** of `#[allow]` found across all crates. Distribution:
- tasker-shared: ~5 instances
- tasker-pgmq: 3 instances
- tasker-orchestration: 21 instances (highest)
- tasker-worker: 5 instances
- tasker-client/cli: 0 (compliant)
- Language workers: ~3 instances

**Recommendation**: Batch fix in a single PR — mechanical replacement of `#[allow]` → `#[expect]` with `reason` strings.

### Systemic: Database Query Timeouts

Found across tasker-shared, tasker-orchestration, tasker-worker, and tasker-pgmq. Individual `sqlx::query!` calls lack explicit `tokio::time::timeout()` wrappers. Pool-level acquire timeouts (30s) provide partial mitigation.

**Recommendation**: Consider PostgreSQL `statement_timeout` at the connection level as a blanket fix, or add `tokio::time::timeout()` around critical query paths.

### Systemic: `unwrap_or_default()` on Required Fields (Tenet #11)

Found across tasker-shared (20+ instances), tasker-orchestration (3 instances), tasker-pgmq (1 instance). Silent failures on required fields violate the Fail Loudly principle.

**Recommendation**: Audit all instances and replace with explicit error handling for required fields.

---

## Appendix: Methodology

Each crate was evaluated across these dimensions:
1. **Security** — Input validation, SQL safety, auth checks, unsafe blocks, crypto, secrets
2. **Error Handling** — Fail Loudly (Tenet #11), context preservation, structured errors
3. **Resilience** — Bounded channels, timeouts, circuit breakers, backpressure
4. **Architecture** — API surface, documentation consistency, test coverage, dead code
5. **FFI-Specific** (language workers) — Error classification, deadlock risk, starvation detection, memory safety

Severity definitions follow the audit specification.

---

## Appendix: Remediation Tracking

Remediation work items for all High-severity findings:

| Work Item | Findings | Priority | Summary |
|-----------|----------|----------|---------|
| Dependency upgrade | X-1 | Urgent | Upgrade `bytes` to fix RUSTSEC-2026-0007 CVE |
| Queue name validation | S-1, P-1, P-2 | High | Add queue name and NOTIFY channel validation |
| Lint compliance cleanup | S-3, O-3, W-4, LW-1, LW-2, P-8 | Medium | Replace `#[allow]` with `#[expect]` workspace-wide |
| Shutdown and recovery hardening | O-1, O-2 | High | Add shutdown timeout and actor panic recovery |
| FFI checkpoint timeout | W-1 | High | Add timeout to `checkpoint_yield` `block_on` |
| Error message sanitization | S-2 | High | Sanitize database error messages in API responses |
