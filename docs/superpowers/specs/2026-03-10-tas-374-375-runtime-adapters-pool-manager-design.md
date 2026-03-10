# TAS-374/375: Runtime Adapters & ResourcePoolManager Design

*March 2026*

*Branch: `jcoletaylor/tas-374-375-runtime-adapters`*

---

## Overview

Implement the adapter layer in tasker-runtime (TAS-374) and the ResourcePoolManager with eviction and backpressure (TAS-375). Together these deliver the bridge between tasker-grammar operation traits and tasker-secure resource handles, plus lifecycle management for dynamic resource pools.

### Design References

- `docs/composition-architecture/operation-shape-constraints.md` — persist/acquire shape constraints
- `docs/research/resource-handle-traits-and-seams.md` — adapter pattern, pool manager, crate topology
- `crates/tasker-grammar/src/operations/` — operation traits, constraint/result types
- `crates/tasker-secure/src/resource/` — ResourceHandle, PostgresHandle, HttpHandle, ResourceRegistry

---

## Prerequisite Changes (tasker-secure)

Three small, backward-compatible changes to tasker-secure are required before adapter/pool-manager implementation:

1. **Add `Hash` derive to `ResourceType`** — needed for `HashMap<ResourceType, Factory>` in AdapterRegistry. `ResourceType` currently derives `Debug, Clone, PartialEq, Eq, Deserialize` but not `Hash`. All variants are hashable (enum of unit variants + one `Custom { type_name: String }`).

2. **Add `remove()` to `ResourceRegistry`** — needed for eviction. `pub async fn remove(&self, name: &str) -> Option<Arc<dyn ResourceHandle>>`. The registry currently only supports `register`, `get`, `list_resources`, and `refresh_resource`. Without removal, the eviction subsystem cannot reclaim resources.

3. **Add `patch()` to `HttpHandle`** — needed for `PersistMode::Update` → PATCH mapping. Follows the existing pattern of `get()`, `post()`, `put()`, `delete()`. Trivial one-method addition.

These are all additive changes with no breaking impact on existing consumers.

---

## TAS-374: Adapters & AdapterRegistry

### Adapter Matrix

| Adapter | Module | Wraps | Implements | Translation |
|---------|--------|-------|------------|-------------|
| `PostgresPersistAdapter` | `postgres` | `Arc<PostgresHandle>` | `PersistableResource` | JSON → INSERT/UPDATE/UPSERT/DELETE + RETURNING * |
| `PostgresAcquireAdapter` | `postgres` | `Arc<PostgresHandle>` | `AcquirableResource` | Params → SELECT with filters, pagination |
| `HttpPersistAdapter` | `http` | `Arc<HttpHandle>` | `PersistableResource` | JSON → POST/PUT/PATCH/DELETE |
| `HttpAcquireAdapter` | `http` | `Arc<HttpHandle>` | `AcquirableResource` | Params → GET with query string |
| `HttpEmitAdapter` | `http` | `Arc<HttpHandle>` | `EmittableResource` | Payload → POST (webhook) |
| `MessagingEmitAdapter` | `messaging` | `Arc<MessagingProvider>` | `EmittableResource` | Payload → `send_message(topic, ...)` |

### Key Decision: MessagingEmitAdapter over PgmqEmitAdapter

PGMQ is a PostgreSQL-level queue — not a separate resource handle type. The existing `MessagingProvider` in tasker-shared already abstracts over PGMQ and RabbitMQ with connection pool sharing, circuit breaker integration, and push notification support. A `MessagingEmitAdapter` wrapping `Arc<MessagingProvider>` reuses all of this and works with both backends immediately, avoiding a redundant `PgmqHandle` that would duplicate `PgmqClient`.

### SQL Generation (Pure Functions)

Extracted as testable pure functions in `adapters/sql_gen.rs`:

- `build_insert(entity, columns, constraints)` → SQL string + bind positions
- `build_update(entity, columns, identity_keys)` → SQL string + bind positions
- `build_upsert(entity, columns, identity_keys, conflict_strategy)` → SQL string
- `build_delete(entity, identity_keys)` → SQL string
- `build_select(entity, columns, params, constraints)` → SQL string + bind values

All functions return structured output (SQL string + parameter metadata) without executing anything. This enables:
- Unit testing of SQL generation without a database
- Future template-time SQL validation via `sqlparser` crate

### Identifier Sanitization (Belt and Suspenders)

Every entity name and column name passes through two checks:

1. **Regex validation**: `^[a-zA-Z_][a-zA-Z0-9_]{0,62}$` — rejects anything exotic with a clear `ValidationFailed` error (62-char limit accounts for PostgreSQL's 63-byte NAMEDATALEN after quote wrapping)
2. **Double-quote wrapping**: `quote_identifier()` wraps in PostgreSQL-style `"identifiers"` for defense-in-depth

Both checks are applied in the SQL generation functions. HTTP adapters apply only the regex check (no SQL quoting needed, but still reject invalid identifiers in URL paths).

### Postgres Value Binding

Data arrives as `serde_json::Value`. Values are bound using sqlx's parameter binding with type mapping:

- `Value::String` → text
- `Value::Number` (integer) → int8
- `Value::Number` (float) → float8
- `Value::Bool` → bool
- `Value::Null` → NULL
- `Value::Array` / `Value::Object` → jsonb

Single-row operations only in v1. No batch inserts.

### PersistMode Addition to tasker-grammar

`PersistConstraints` gains a `mode` field to express all four operation types:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum PersistMode {
    #[default]
    Insert,
    Update,
    Upsert,
    Delete,
}
```

Added to `PersistConstraints` with `#[serde(default)]` for backward compatibility. The Postgres adapter switches on mode to generate the appropriate SQL; the HTTP adapter maps mode to HTTP method (Insert→POST, Update→PATCH, Upsert→PUT, Delete→DELETE).

### AdapterRegistry (Closure-Based Factories)

```rust
type PersistFactory = Box<dyn Fn(Arc<dyn ResourceHandle>) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> + Send + Sync>;
type AcquireFactory = Box<dyn Fn(Arc<dyn ResourceHandle>) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError> + Send + Sync>;
type EmitFactory = Box<dyn Fn(Arc<dyn ResourceHandle>) -> Result<Arc<dyn EmittableResource>, ResourceOperationError> + Send + Sync>;

pub struct AdapterRegistry {
    persist_factories: HashMap<ResourceType, PersistFactory>,
    acquire_factories: HashMap<ResourceType, AcquireFactory>,
    emit_factories: HashMap<ResourceType, EmitFactory>,
}
```

- `standard()` registers built-in adapters: Postgres persist + acquire (`#[cfg(feature = "postgres")]`), HTTP persist + acquire + emit (`#[cfg(feature = "http")]`). Messaging emit adapter is registered separately since it wraps `MessagingProvider`, not a `ResourceHandle`.
- `register_persist()` / `register_acquire()` / `register_emit()` for custom resource types
- `as_persistable(handle)` / `as_acquirable(handle)` / `as_emittable(handle)` do lookup by `handle.resource_type()` + factory invocation
- Factory closures use `handle.as_any().downcast_ref::<ConcreteHandle>()` to get the typed handle

The `MessagingEmitAdapter` doesn't fit the `ResourceHandle`-based factory pattern (it wraps `MessagingProvider`, not a handle). The registry exposes a separate `messaging_emitter()` method, or the `RuntimeOperationProvider` (TAS-377) handles the messaging path directly. This keeps the factory pattern clean for handle-based adapters.

### HTTP Adapter Details

**HttpPersistAdapter:**
- Mode → HTTP method: Insert→POST, Update→PATCH, Upsert→PUT, Delete→DELETE
- URL: `{base_url}/{entity}` for Insert, `{base_url}/{entity}/{pk_value}` for Update/Upsert/Delete
- PK value extracted from data using `constraints.upsert_key` (identity keys)
- Body: JSON serialization of data object
- Response: parsed as JSON into `PersistResult.data`

**HttpAcquireAdapter:**
- Maps `params` JSON object to query string parameters
- `constraints.limit` → `?limit=N`, `constraints.offset` → `?offset=N`
- `constraints.timeout_ms` → `request.timeout(Duration)`
- Response status mapping: 401/403 → `AuthorizationFailed`, 404 → `EntityNotFound`, other errors → `Other`
- Response body parsed as JSON into `AcquireResult.data`

**HttpEmitAdapter:**
- POST to `{base_url}/{topic}`
- Body: JSON payload
- Metadata attributes mapped to HTTP headers
- `correlation_id` → `X-Correlation-ID` header
- Response: `EmitResult { confirmed: status.is_success(), data: response_json }`

---

## TAS-375: ResourcePoolManager

### Structure

```rust
pub struct ResourcePoolManager {
    registry: Arc<ResourceRegistry>,
    config: PoolManagerConfig,
    origins: RwLock<HashMap<String, ResourceOrigin>>,
    metrics: RwLock<HashMap<String, ResourceAccessMetrics>>,
    pool_metrics: PoolManagerMetrics,
}
```

No `definition_sources` or `secrets` — the pool manager manages what's already registered. Dynamic resource resolution is TAS-376's concern.

### ResourceAccessMetrics

```rust
pub struct ResourceAccessMetrics {
    pub creation_time: Instant,
    pub last_accessed: Instant,
    pub access_count: u64,
    pub active_checkouts: u64,       // plain u64 — writes are under RwLock
    pub estimated_connections: u32,
}
```

### PoolManagerMetrics (Observability)

```rust
pub struct PoolManagerMetrics {
    pub total_pools: AtomicU64,
    pub static_pools: AtomicU64,
    pub dynamic_pools: AtomicU64,
    pub estimated_total_connections: AtomicU64,
    pub admission_rejections: AtomicU64,
    pub evictions_performed: AtomicU64,
}
```

`admission_rejections` is the key autoscaling signal — sustained rate means the worker needs more capacity.

### Core Operations

**`register(name, handle, origin, estimated_connections)`:**
1. Run admission check (pool count + connection budget)
2. If at capacity with `AdmissionStrategy::EvictOne`: find eviction candidate, evict, then admit
3. If at capacity with `AdmissionStrategy::Reject`: return `ResourceError` (retriable)
4. Delegate to `registry.register(name, handle)`
5. Create metrics entry, track origin

**`get(name)`:**
- **Fast path**: registry has it → touch metrics (`last_accessed`, `access_count`), return handle
- **Miss**: not in registry → return `ResourceError::ResourceNotFound`
- Note: the scaffold method is named `get_or_initialize` — renamed to `get` since the "initialize" path (dynamic resolution) is deferred to TAS-376. The scaffold will be updated during implementation.

**`sweep()` (eviction):**
1. Iterate metrics, skip `ResourceOrigin::Static`
2. Filter to **eligible** candidates: `now - last_accessed > idle_timeout` AND `active_checkouts == 0`
3. Rank eligible candidates by configured strategy (LRU/LFU/FIFO)
4. Evict candidates until below capacity or no more eligible
5. Return `(candidates_found, evicted_count)`

**Critical invariant: active resources are never eviction candidates.** A resource within the idle timeout window or with active checkouts is protected. When at capacity with no eligible candidates, the pool manager signals backpressure via admission rejection rather than force-evicting active resources.

**`evict(name)`:**
- Remove from `origins` and `metrics` tracking
- Remove from registry (or mark as logically evicted if registry doesn't support removal)
- Update `pool_metrics.evictions_performed`

**`current_pools()`:**
- Returns `PoolManagerMetrics` snapshot + per-resource summaries with origin and metrics
- Safe for MCP/telemetry exposure

**`refresh_resource(name)`:**
- Delegates to `registry.refresh_resource(name)`
- Resets metrics on success

---

## Test Strategy

### Unit Tests (Primary)

**SQL generation** (`sql_gen.rs`):
- `build_insert` with various column counts, verify SQL string and bind positions
- `build_upsert` with Update/Skip/Reject conflict strategies
- `build_update` and `build_delete` with single and composite PKs
- `build_select` with filters, pagination, ordering
- Identifier validation: valid names pass, SQL injection attempts rejected
- Edge cases: empty columns, reserved words, unicode rejection

**HTTP request construction:**
- Mode → method mapping
- URL construction with entity and PK values
- Query parameter building from params JSON
- Timeout application from constraints
- Header mapping from emit metadata

**AdapterRegistry:**
- `standard()` registers expected resource types
- `as_persistable` returns correct adapter for Postgres/HTTP handles
- `as_persistable` returns error for unsupported resource type
- Custom factory registration and invocation

**ResourcePoolManager:**
- Admission control: register up to `max_pools`, verify rejection on next
- Connection budget enforcement
- Eviction sweep: register dynamic resources with past idle timeout, verify eviction
- Static protection: `ResourceOrigin::Static` resources never evicted
- Active protection: resources with recent access or active checkouts survive sweep
- Metrics tracking: `access_count` and `last_accessed` update correctly
- `EvictOne` admission: at capacity, new registration evicts idle dynamic resource
- Backpressure: at capacity with no eligible candidates, admission rejected
- `PoolManagerMetrics` counters increment correctly

### Test Infrastructure

- `InMemoryResourceHandle` from tasker-secure for pool manager tests
- Extracted SQL generation functions tested as pure string operations
- No database, HTTP server, or messaging infrastructure required

---

## Deferred Work & Suggested Follow-ups

### 1. Structured Acquire Filters (Grammar Type Enhancement)

**What**: `FilterOperator` enum (eq/neq/gt/gte/lt/lte/in/not_in/is_null/like), typed column selection, order_by fields in `AcquireConstraints`.

**Why deferred**: The Postgres acquire adapter can parse filters from the `params` JSON value (already in trait signature) for now. Promoting to typed fields is needed when `CompositionValidator` validates filter declarations at template time.

**Suggested scope**: Pairs naturally with SQL validation at template time (item 6).

### 2. Nested Relationship Support (Persist & Acquire)

**What**: One-level nesting — parent + child inserts with FK propagation, joined queries with result assembly into nested JSON.

**Why deferred**: Adds transaction scoping, FK value extraction, and multi-statement SQL generation. Meaningful increment of complexity beyond flat single-entity operations.

**Suggested scope**: Own ticket. Depends on this PR's SQL generation foundation.

### 3. Batch Persist

**What**: Array-of-objects handling with configurable max batch size (1000 default), chunking logic.

**Why deferred**: Single-row operations cover the common case. Batch requires generating multi-row INSERT VALUES and handling partial failure semantics.

**Suggested scope**: Low priority until real workloads surface the need.

### 4. Background Eviction Sweep Timer

**What**: `tokio::spawn` + `tokio::time::interval` calling `sweep()` at `config.sweep_interval`.

**Why deferred**: Runtime integration concern — the sweep task needs to be spawned in the composition worker startup sequence, which is TAS-377/tasker-rs territory.

**Suggested scope**: Part of TAS-377 (RuntimeOperationProvider) or composition worker startup.

### 5. Handle Guard with active_checkouts

**What**: RAII guard type returned from `get_or_initialize` that increments `active_checkouts` on creation and decrements on drop, ensuring accurate liveness tracking.

**Why deferred**: Adds a wrapper type around `Arc<dyn ResourceHandle>` that callers must hold. May land in this PR if time permits.

**Suggested scope**: First follow-up if not in this PR.

### 6. SQL Validation at Template Time

**What**: Use `sqlparser` crate to parse generated SQL from composition declarations during `CompositionValidator` validation, confirming syntactic validity without a database connection.

**Why deferred**: Requires structured acquire filters (item 1) to be fully useful. The SQL generation functions being pure and public makes this a natural integration point.

**Suggested scope**: After structured acquire filters land. Could validate persist SQL immediately.

### 7. Definition Resolution Chain (TAS-376)

**What**: `ResourceDefinitionSource` implementations (StaticConfigSource, SopsFileWatcher) that let pool manager resolve unknown resource_refs dynamically.

**Why deferred**: Already has its own ticket. Pool manager delivers full value (metrics, eviction, admission) without dynamic resolution.

**Suggested scope**: TAS-376, independent of this PR.

### 8. HTTP Response Header Parsing

**What**: Parse `X-Total-Count`, `Link`, or similar pagination headers from HTTP acquire responses into `AcquireResult.total_count`.

**Why deferred**: API-specific behavior that varies across services. Current implementation returns `total_count: None` for HTTP.

**Suggested scope**: Add when real API integrations surface the need, possibly as configurable header names per resource.

---

## File Changes Summary

### New Files
- `crates/tasker-runtime/src/adapters/sql_gen.rs` — pure SQL generation functions + identifier sanitization
- `crates/tasker-runtime/src/adapters/messaging.rs` — `MessagingEmitAdapter`
- `crates/tasker-runtime/src/adapters/registry.rs` — `AdapterRegistry` with closure-based factories
- Test files for each module

### Modified Files
- `crates/tasker-runtime/src/adapters/mod.rs` — add messaging, registry, sql_gen modules
- `crates/tasker-runtime/src/adapters/postgres.rs` — implement `persist()` and `acquire()` using sql_gen
- `crates/tasker-runtime/src/adapters/http.rs` — implement all three adapters
- `crates/tasker-runtime/src/pool_manager/mod.rs` — full ResourcePoolManager implementation
- `crates/tasker-runtime/src/pool_manager/metrics.rs` — add `active_checkouts`, `PoolManagerMetrics`
- `crates/tasker-runtime/src/pool_manager/lifecycle.rs` — may add `AdmissionStrategy::EvictOne` handling
- `crates/tasker-runtime/src/lib.rs` — update re-exports
- `crates/tasker-runtime/Cargo.toml` — add `tasker-shared` dependency (for `MessagingProvider`), add `sqlx` dependency (feature-gated behind `postgres`)
- `crates/tasker-grammar/src/operations/types.rs` — add `PersistMode` enum to `PersistConstraints`
- `crates/tasker-secure/src/resource/types.rs` — add `Hash` derive to `ResourceType`
- `crates/tasker-secure/src/resource/registry.rs` — add `remove()` method to `ResourceRegistry`
- `crates/tasker-secure/src/resource/http.rs` — add `patch()` method to `HttpHandle`

### Scaffold Alignment Note

The existing scaffold stubs in `postgres.rs`, `http.rs`, and `adapters/mod.rs` have `unimplemented!("TAS-375: ...")` markers. The adapter *implementations* are TAS-374 work; the pool manager is TAS-375. These markers will be corrected during implementation.
