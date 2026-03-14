# TAS-377: RuntimeOperationProvider Design Specification

**Date:** 2026-03-14
**Status:** Approved
**Ticket:** TAS-377
**Lane:** 2D (Phase 2 convergence point)
**Depends on:** 2A (AdapterRegistry ✅), 2B (ResourcePoolManager ✅)
**Blocks:** Phase 3B (CompositionExecutionContext)

---

## Overview

RuntimeOperationProvider is the production implementation of the `OperationProvider` trait from tasker-grammar. It bridges two completed subsystems — `ResourcePoolManager` (pool lifecycle, eviction, admission control) and `AdapterRegistry` (factory-based adapter dispatch) — into the single interface that grammar capability executors consume.

Grammar executors call `context.operations.get_persistable("orders-db")` and receive the same `Arc<dyn PersistableResource>` they tested against with `InMemoryOperations`. The only difference at runtime is that the trait object is backed by a `PostgresPersistAdapter` wrapping a live `PostgresHandle`.

**Lifetime model:** One `RuntimeOperationProvider` per composition execution. Created when a worker picks up a composition, dropped when execution completes. This scopes resource resolution and caching to the composition boundary — not per-action (too granular, redundant resolution) and not per-task-template (too broad, assumes a single worker handles all steps).

---

## Architecture

### Resolution Flow

```
Grammar executor
  → context.operations.get_persistable("orders-db")
    → RuntimeOperationProvider
      → AdapterCache (check cache first)
      → ResourcePoolManager.get_or_initialize("orders-db")
        → try get() from registry
        → if NotFound + source configured: source.resolve() → register()
      → AdapterRegistry.as_persistable(handle)
      → cache result
      → return Arc<dyn PersistableResource>
```

### Types Introduced

| Type | Location | Purpose |
|------|----------|---------|
| `RuntimeOperationProvider` | `provider.rs` (existing stub) | Implements `OperationProvider`, holds pool manager + adapter registry + cache |
| `AdapterCache` | `cache.rs` (new) | SWMR per-composition cache for resolved adapters |
| `ResourceDefinitionSource` | `sources/traits.rs` (new) | Minimal trait — extension point for TAS-376 |

### Types Modified

| Type | Location | Change |
|------|----------|--------|
| `ResourcePoolManager` | `pool_manager/mod.rs` | Add `get_or_initialize()` method |

---

## AdapterCache

SWMR (Single Writer, Multiple Reader) cache storing resolved adapters keyed by resource reference string. Once written, entries are read-only — consumers clone the `Arc`.

### Structure

```rust
pub(crate) struct AdapterCache {
    persist: RwLock<HashMap<String, Arc<dyn PersistableResource>>>,
    acquire: RwLock<HashMap<String, Arc<dyn AcquirableResource>>>,
    emit: RwLock<HashMap<String, Arc<dyn EmittableResource>>>,
}
```

Three separate maps rather than one with an enum wrapper — avoids downcasting and keeps the read path type-safe. `RwLock` provides SWMR: first call for a given resource takes the write lock briefly, all subsequent calls only need the read lock to clone the `Arc`.

### API

- `new() -> Self`
- `get_persistable(&self, key: &str) -> Option<Arc<dyn PersistableResource>>` — read lock only
- `insert_persistable(&self, key: String, adapter: Arc<dyn PersistableResource>)` — write lock
- Same pattern for `get_acquirable`/`insert_acquirable` and `get_emittable`/`insert_emittable`

### Design Rationale

- `RwLock<HashMap>` over `DashMap`: simpler, no extra dependency, perfect fit for rare-write/many-read profile
- Realistically a composition resolves 2-5 resources; the cache avoids redundant pool manager + adapter registry round-trips, not high-throughput optimization
- Three separate maps: type-safe, no downcasting, independent keying (same resource ref as persistable doesn't satisfy acquirable lookup)

---

## ResourceDefinitionSource Trait

Minimal extension point for TAS-376. Defines the seam for lazy resource initialization without over-determining the implementation.

### Definition

```rust
#[async_trait]
pub trait ResourceDefinitionSource: Send + Sync {
    async fn resolve(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceOperationError>;
}
```

Single method — given a resource reference string, return a handle. No metadata, no type information, no connection estimates. Those concerns belong to TAS-376 when the trait gets its real implementations.

### Location

`crates/tasker-runtime/src/sources/traits.rs` — alongside the existing `sources/` module where `StaticConfigSource` and `SopsFileWatcher` stubs live.

---

## ResourcePoolManager Extension

### New Method

```rust
pub async fn get_or_initialize(
    &self,
    name: &str,
    source: Option<&dyn ResourceDefinitionSource>,
) -> Result<Arc<dyn ResourceHandle>, ResourceError>
```

### Flow

1. Try `self.get(name)` — if found, return it
2. If `NotFound` and `source` is `Some`, call `source.resolve(name)`
3. Register the returned handle via `self.register(name, handle, Dynamic, 1)`
4. Return the handle
5. If `NotFound` and `source` is `None`, propagate the `NotFound` error

### Design Decisions

- `source` is a parameter, not stored on the pool manager — keeps the pool manager decoupled from the source concept, avoids changing its constructor
- `Dynamic` origin: lazily-initialized resources are evictable by definition
- `1` connection estimate: conservative default. When TAS-376 implements richer sources, the source trait can be expanded to return metadata
- The `resolve()` call returns `ResourceOperationError` which needs mapping to `ResourceError` for the `register()` call — or alternatively, `get_or_initialize` can work with the error types directly since it sits on the boundary

---

## RuntimeOperationProvider

### Struct

```rust
pub struct RuntimeOperationProvider {
    pool_manager: Arc<ResourcePoolManager>,
    adapter_registry: Arc<AdapterRegistry>,
    source: Option<Arc<dyn ResourceDefinitionSource>>,
    cache: AdapterCache,
}
```

### Constructors

```rust
impl RuntimeOperationProvider {
    pub fn new(
        pool_manager: Arc<ResourcePoolManager>,
        adapter_registry: Arc<AdapterRegistry>,
    ) -> Self

    pub fn with_source(
        pool_manager: Arc<ResourcePoolManager>,
        adapter_registry: Arc<AdapterRegistry>,
        source: Arc<dyn ResourceDefinitionSource>,
    ) -> Self
}
```

Two constructors — `new()` for the current state (no source), `with_source()` for when TAS-376 lands. Both create a fresh `AdapterCache` internally.

### OperationProvider Implementation

All three methods follow the same pattern:

```rust
#[async_trait]
impl OperationProvider for RuntimeOperationProvider {
    async fn get_persistable(&self, resource_ref: &str)
        -> Result<Arc<dyn PersistableResource>, ResourceOperationError>
    {
        // 1. Check cache
        if let Some(adapter) = self.cache.get_persistable(resource_ref) {
            return Ok(adapter);
        }
        // 2. Resolve handle through pool manager
        let handle = self.pool_manager
            .get_or_initialize(resource_ref, self.source.as_deref())
            .map_err(map_resource_error)?;
        // 3. Wrap with adapter
        let adapter = self.adapter_registry.as_persistable(handle)?;
        // 4. Cache and return
        self.cache.insert_persistable(resource_ref.to_string(), adapter.clone());
        Ok(adapter)
    }

    // get_acquirable and get_emittable follow the identical pattern
    // with their respective cache methods and adapter registry calls
}
```

---

## Error Mapping

Private helper function in `provider.rs`:

```rust
fn map_resource_error(err: ResourceError) -> ResourceOperationError
```

### Mapping Table

| ResourceError | ResourceOperationError | Rationale |
|---|---|---|
| `NotFound { name }` | `EntityNotFound { entity: name }` | Direct semantic match |
| `AlreadyExists { name }` | `Conflict { entity: name, reason }` | Registration conflict |
| `ConnectionFailed { .. }` | `Unavailable { message }` | Infrastructure unavailability |
| `PoolExhausted { .. }` | `Unavailable { message }` | Capacity unavailability |
| `Timeout { .. }` | `Timeout { timeout_ms }` | Direct match |
| Everything else | `Other { message, source }` | Catch-all preserving original error |

Original error messages are preserved in all cases for debugging context. The `Other` variant wraps the source error for chain-of-cause inspection.

---

## Testing Strategy

All tests use in-memory handles and test doubles — no infrastructure required. Fits the `test-no-infra` tier.

### AdapterCache Unit Tests

- Insert and retrieve each adapter type
- Cache miss returns `None`
- Multiple reads after single write (SWMR verification)
- Independent keying — same resource ref cached as persistable doesn't satisfy acquirable lookup

### get_or_initialize Unit Tests

- Existing resource returned directly
- Source called when resource not found
- Source `None` propagates NotFound
- Registered handle retrievable on subsequent `get()` calls

### RuntimeOperationProvider Unit Tests

- Uses `InMemoryResourceHandle` from tasker-secure's test-utils for the pool manager
- Custom test adapter factory registered in `AdapterRegistry` wrapping in-memory handles
- Test cases:
  - Successful resolution flow (pool manager → adapter registry → cache)
  - Cache hit on second call (verify same `Arc` pointer returned)
  - Resource not found propagates as `EntityNotFound`
  - Adapter not registered propagates as `ValidationFailed`
  - With mock `ResourceDefinitionSource` — verifies lazy initialization when `get()` returns NotFound

### Integration Test File

`crates/tasker-runtime/tests/runtime_provider_tests.rs` — end-to-end flow through all three layers.

---

## Files Changed

| File | Action | Content |
|------|--------|---------|
| `crates/tasker-runtime/src/provider.rs` | Modify | Full `RuntimeOperationProvider` implementation replacing stubs |
| `crates/tasker-runtime/src/cache.rs` | Create | `AdapterCache` SWMR cache |
| `crates/tasker-runtime/src/sources/traits.rs` | Modify | Add `ResourceDefinitionSource` trait |
| `crates/tasker-runtime/src/pool_manager/mod.rs` | Modify | Add `get_or_initialize()` method |
| `crates/tasker-runtime/src/lib.rs` | Modify | Export new public types |
| `crates/tasker-runtime/tests/runtime_provider_tests.rs` | Create | Integration tests |
