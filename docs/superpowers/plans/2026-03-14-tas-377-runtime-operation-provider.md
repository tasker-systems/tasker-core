# TAS-377: RuntimeOperationProvider Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement RuntimeOperationProvider bridging ResourcePoolManager + AdapterRegistry into the OperationProvider interface that tasker-grammar executors consume.

**Architecture:** RuntimeOperationProvider resolves named resources through a pool manager (with optional lazy initialization via ResourceHandleResolver), wraps handles in adapters via AdapterRegistry, and caches resolved adapters per-composition via an SWMR AdapterCache. One provider instance per composition execution.

**Tech Stack:** Rust, async-trait, tokio::sync::RwLock, tasker-grammar (OperationProvider trait), tasker-secure (ResourceHandle, ResourceRegistry, ResourceError)

**Spec:** `docs/superpowers/specs/2026-03-14-tas-377-runtime-operation-provider-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/tasker-runtime/src/cache.rs` | Create | AdapterCache — SWMR cache for resolved adapters |
| `crates/tasker-runtime/src/sources/mod.rs` | Modify | Add ResourceHandleResolver trait |
| `crates/tasker-runtime/src/pool_manager/mod.rs` | Modify | Add get_or_initialize() method |
| `crates/tasker-runtime/src/provider.rs` | Modify | Full RuntimeOperationProvider implementation |
| `crates/tasker-runtime/src/lib.rs` | Modify | Export new public types |
| `crates/tasker-runtime/tests/runtime_provider_tests.rs` | Create | Integration tests for full resolution flow |

---

## Chunk 1: AdapterCache

### Task 1: AdapterCache — SWMR cache for resolved adapters

**Files:**
- Create: `crates/tasker-runtime/src/cache.rs`
- Modify: `crates/tasker-runtime/src/lib.rs` (add `mod cache;`)

- [ ] **Step 1: Write the failing test — cache miss returns None**

Add the test directly in `cache.rs` as a `#[cfg(test)] mod tests` block. We write the struct and test together since the type doesn't exist yet.

Create `crates/tasker-runtime/src/cache.rs`:

```rust
//! Per-composition adapter cache with SWMR (Single Writer, Multiple Reader) semantics.
//!
//! Caches resolved adapters keyed by resource reference string. Once written,
//! entries are read-only — consumers clone the `Arc`. One cache per
//! `RuntimeOperationProvider` instance (i.e., per composition execution).

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use tokio::sync::RwLock;

use tasker_grammar::operations::{AcquirableResource, EmittableResource, PersistableResource};

/// SWMR cache for resolved operation trait adapters.
///
/// Three separate maps (persist, acquire, emit) keyed by resource reference string.
/// Uses `tokio::sync::RwLock` for async-compatible SWMR: first resolution takes
/// the write lock briefly, all subsequent reads only need the read lock to clone
/// the `Arc`.
pub(crate) struct AdapterCache {
    persist: RwLock<HashMap<String, Arc<dyn PersistableResource>>>,
    acquire: RwLock<HashMap<String, Arc<dyn AcquirableResource>>>,
    emit: RwLock<HashMap<String, Arc<dyn EmittableResource>>>,
}

impl fmt::Debug for AdapterCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Trait objects aren't Debug, so show map sizes/keys only
        let persist_keys: Vec<String> = self
            .persist
            .try_read()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let acquire_keys: Vec<String> = self
            .acquire
            .try_read()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let emit_keys: Vec<String> = self
            .emit
            .try_read()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();

        f.debug_struct("AdapterCache")
            .field("persist_keys", &persist_keys)
            .field("acquire_keys", &acquire_keys)
            .field("emit_keys", &emit_keys)
            .finish()
    }
}

impl AdapterCache {
    /// Create a new empty cache.
    pub(crate) fn new() -> Self {
        Self {
            persist: RwLock::new(HashMap::new()),
            acquire: RwLock::new(HashMap::new()),
            emit: RwLock::new(HashMap::new()),
        }
    }

    /// Look up a cached persistable adapter. Returns `None` on cache miss.
    pub(crate) async fn get_persistable(
        &self,
        key: &str,
    ) -> Option<Arc<dyn PersistableResource>> {
        self.persist.read().await.get(key).cloned()
    }

    /// Cache a persistable adapter for the given resource reference.
    pub(crate) async fn insert_persistable(
        &self,
        key: String,
        adapter: Arc<dyn PersistableResource>,
    ) {
        self.persist.write().await.insert(key, adapter);
    }

    /// Look up a cached acquirable adapter. Returns `None` on cache miss.
    pub(crate) async fn get_acquirable(
        &self,
        key: &str,
    ) -> Option<Arc<dyn AcquirableResource>> {
        self.acquire.read().await.get(key).cloned()
    }

    /// Cache an acquirable adapter for the given resource reference.
    pub(crate) async fn insert_acquirable(
        &self,
        key: String,
        adapter: Arc<dyn AcquirableResource>,
    ) {
        self.acquire.write().await.insert(key, adapter);
    }

    /// Look up a cached emittable adapter. Returns `None` on cache miss.
    pub(crate) async fn get_emittable(
        &self,
        key: &str,
    ) -> Option<Arc<dyn EmittableResource>> {
        self.emit.read().await.get(key).cloned()
    }

    /// Cache an emittable adapter for the given resource reference.
    pub(crate) async fn insert_emittable(
        &self,
        key: String,
        adapter: Arc<dyn EmittableResource>,
    ) {
        self.emit.write().await.insert(key, adapter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tasker_grammar::operations::types::*;
    use tasker_grammar::operations::ResourceOperationError;

    /// Minimal test double implementing PersistableResource.
    struct MockPersist;

    #[async_trait::async_trait]
    impl PersistableResource for MockPersist {
        async fn persist(
            &self,
            _entity: &str,
            _data: serde_json::Value,
            _constraints: &PersistConstraints,
        ) -> Result<PersistResult, ResourceOperationError> {
            Ok(PersistResult {
                data: serde_json::json!({"mock": true}),
                affected_count: Some(1),
            })
        }
    }

    /// Minimal test double implementing AcquirableResource.
    struct MockAcquire;

    #[async_trait::async_trait]
    impl AcquirableResource for MockAcquire {
        async fn acquire(
            &self,
            _entity: &str,
            _params: serde_json::Value,
            _constraints: &AcquireConstraints,
        ) -> Result<AcquireResult, ResourceOperationError> {
            Ok(AcquireResult {
                data: serde_json::json!([]),
                total_count: Some(0),
            })
        }
    }

    /// Minimal test double implementing EmittableResource.
    struct MockEmit;

    #[async_trait::async_trait]
    impl EmittableResource for MockEmit {
        async fn emit(
            &self,
            _topic: &str,
            _payload: serde_json::Value,
            _metadata: &EmitMetadata,
        ) -> Result<EmitResult, ResourceOperationError> {
            Ok(EmitResult {
                data: serde_json::json!({}),
                confirmed: true,
            })
        }
    }

    #[tokio::test]
    async fn cache_miss_returns_none() {
        let cache = AdapterCache::new();
        assert!(cache.get_persistable("missing").await.is_none());
        assert!(cache.get_acquirable("missing").await.is_none());
        assert!(cache.get_emittable("missing").await.is_none());
    }

    #[tokio::test]
    async fn insert_and_retrieve_persistable() {
        let cache = AdapterCache::new();
        let adapter: Arc<dyn PersistableResource> = Arc::new(MockPersist);
        cache
            .insert_persistable("db1".to_string(), adapter.clone())
            .await;

        let cached = cache.get_persistable("db1").await;
        assert!(cached.is_some());
        assert!(Arc::ptr_eq(&adapter, &cached.unwrap()));
    }

    #[tokio::test]
    async fn insert_and_retrieve_acquirable() {
        let cache = AdapterCache::new();
        let adapter: Arc<dyn AcquirableResource> = Arc::new(MockAcquire);
        cache
            .insert_acquirable("db1".to_string(), adapter.clone())
            .await;

        let cached = cache.get_acquirable("db1").await;
        assert!(cached.is_some());
        assert!(Arc::ptr_eq(&adapter, &cached.unwrap()));
    }

    #[tokio::test]
    async fn insert_and_retrieve_emittable() {
        let cache = AdapterCache::new();
        let adapter: Arc<dyn EmittableResource> = Arc::new(MockEmit);
        cache
            .insert_emittable("events".to_string(), adapter.clone())
            .await;

        let cached = cache.get_emittable("events").await;
        assert!(cached.is_some());
        assert!(Arc::ptr_eq(&adapter, &cached.unwrap()));
    }

    #[tokio::test]
    async fn independent_keying_across_operation_types() {
        let cache = AdapterCache::new();
        let persist: Arc<dyn PersistableResource> = Arc::new(MockPersist);
        cache
            .insert_persistable("resource1".to_string(), persist)
            .await;

        // Same key in a different operation type should miss
        assert!(cache.get_acquirable("resource1").await.is_none());
        assert!(cache.get_emittable("resource1").await.is_none());
    }

    #[tokio::test]
    async fn multiple_reads_return_same_arc() {
        let cache = AdapterCache::new();
        let adapter: Arc<dyn PersistableResource> = Arc::new(MockPersist);
        cache
            .insert_persistable("db1".to_string(), adapter.clone())
            .await;

        let read1 = cache.get_persistable("db1").await.unwrap();
        let read2 = cache.get_persistable("db1").await.unwrap();
        assert!(Arc::ptr_eq(&read1, &read2));
        assert!(Arc::ptr_eq(&adapter, &read1));
    }

    #[tokio::test]
    async fn debug_shows_cached_keys() {
        let cache = AdapterCache::new();
        let adapter: Arc<dyn PersistableResource> = Arc::new(MockPersist);
        cache
            .insert_persistable("orders-db".to_string(), adapter)
            .await;

        let debug = format!("{cache:?}");
        assert!(debug.contains("AdapterCache"));
        assert!(debug.contains("orders-db"));
    }
}
```

- [ ] **Step 2: Add module declaration to lib.rs**

In `crates/tasker-runtime/src/lib.rs`, add `mod cache;` after the existing `pub mod` declarations (line 29). This is `pub(crate)` by default since the module itself isn't `pub`.

Add after line 29 (`pub mod sources;`):

```rust
mod cache;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo nextest run --features test-messaging --lib -p tasker-runtime -E 'test(cache::tests)'`

Expected: All 7 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/tasker-runtime/src/cache.rs crates/tasker-runtime/src/lib.rs
git commit -m "feat(TAS-377): add AdapterCache with SWMR semantics for per-composition adapter caching"
```

---

## Chunk 2: ResourceHandleResolver trait + get_or_initialize

### Task 2: ResourceHandleResolver trait in sources module

**Files:**
- Modify: `crates/tasker-runtime/src/sources/mod.rs`
- Modify: `crates/tasker-runtime/src/lib.rs` (add re-export)

- [ ] **Step 1: Add ResourceHandleResolver trait to sources/mod.rs**

Add the following after the existing `ResourceDefinitionSource` trait (after line 44) in `crates/tasker-runtime/src/sources/mod.rs`:

```rust
use std::sync::Arc;

use tasker_grammar::operations::ResourceOperationError;
use tasker_secure::ResourceHandle;

/// Resolves a resource reference to a live [`ResourceHandle`].
///
/// This is the extension point for TAS-376 (ResourceDefinitionSource implementations).
/// Distinct from [`ResourceDefinitionSource`], which returns configuration descriptors
/// (`ResourceDefinition`). This trait operates at a higher level — given a resource
/// reference string, it returns an initialized, ready-to-use handle.
///
/// In practice, a TAS-376 implementation would use a `ResourceDefinitionSource` internally
/// to look up the definition, then initialize the handle from it.
#[async_trait]
pub trait ResourceHandleResolver: Send + Sync + std::fmt::Debug {
    /// Resolve a resource reference to a live handle.
    async fn resolve(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceOperationError>;
}
```

Note: `async_trait` is already imported at line 12. `Arc` needs to be added to imports.

- [ ] **Step 2: Add re-export to lib.rs**

In `crates/tasker-runtime/src/lib.rs`, add `ResourceHandleResolver` to the `sources` re-export at line 38:

Change:
```rust
pub use sources::ResourceDefinitionSource;
```
To:
```rust
pub use sources::{ResourceDefinitionSource, ResourceHandleResolver};
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check --all-features -p tasker-runtime`

Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/tasker-runtime/src/sources/mod.rs crates/tasker-runtime/src/lib.rs
git commit -m "feat(TAS-377): add ResourceHandleResolver trait as extension point for lazy resource initialization"
```

### Task 3: Add get_or_initialize to ResourcePoolManager

**Files:**
- Modify: `crates/tasker-runtime/src/pool_manager/mod.rs`
- Modify: `crates/tasker-runtime/tests/pool_manager_tests.rs`

- [ ] **Step 1: Write failing tests for get_or_initialize**

Add the following tests to the end of `crates/tasker-runtime/tests/pool_manager_tests.rs`:

```rust
#[tokio::test]
async fn get_or_initialize_returns_existing_resource() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    let handle = make_handle("db1");
    manager
        .register("db1", handle, ResourceOrigin::Static, 10)
        .await
        .unwrap();

    let result = manager.get_or_initialize("db1", None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn get_or_initialize_no_source_propagates_not_found() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    let result = manager.get_or_initialize("nonexistent", None).await;
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run --features test-messaging -p tasker-runtime -E 'test(get_or_initialize)'`

Expected: FAIL — `get_or_initialize` method does not exist.

- [ ] **Step 3: Implement get_or_initialize on ResourcePoolManager**

Add the following method to `crates/tasker-runtime/src/pool_manager/mod.rs` after the `get` method (after line 163). Also add the necessary import at the top of the file.

Add to the imports section (after line 18, before line 19):

```rust
use crate::sources::ResourceHandleResolver;
```

Note: `ResourceOrigin` is already in scope via `pub use lifecycle::ResourceOrigin` at line 9.

Add the method after `get()` (after line 163):

```rust
    /// Get a resource handle by name, or initialize it via the given source.
    ///
    /// Flow:
    /// 1. Try `self.get(name)` — if found, return it
    /// 2. If `ResourceNotFound` and `source` is `Some`, call `source.resolve(name)`
    /// 3. Register the returned handle as `Dynamic` with 1 estimated connection
    /// 4. Return the handle
    /// 5. If `ResourceNotFound` and `source` is `None`, propagate the error
    pub async fn get_or_initialize(
        &self,
        name: &str,
        source: Option<&dyn ResourceHandleResolver>,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceError> {
        match self.get(name).await {
            Ok(handle) => Ok(handle),
            Err(ResourceError::ResourceNotFound { .. }) => {
                let Some(source) = source else {
                    return Err(ResourceError::ResourceNotFound {
                        name: name.to_string(),
                    });
                };

                let handle =
                    source
                        .resolve(name)
                        .await
                        .map_err(|e| ResourceError::InitializationFailed {
                            name: name.to_string(),
                            message: e.to_string(),
                        })?;

                self.register(name, handle.clone(), ResourceOrigin::Dynamic, 1)
                    .await?;

                Ok(handle)
            }
            Err(other) => Err(other),
        }
    }
```

- [ ] **Step 4: Run the two basic tests to verify they pass**

Run: `cargo nextest run --features test-messaging -p tasker-runtime -E 'test(get_or_initialize)'`

Expected: Both tests pass.

- [ ] **Step 5: Write tests for source-based resolution**

Add to `crates/tasker-runtime/tests/pool_manager_tests.rs` — first add the required imports at the top:

```rust
use tasker_grammar::operations::ResourceOperationError;
use tasker_runtime::ResourceHandleResolver;
use tasker_secure::ResourceHandle;
```

Then add the mock source and tests:

```rust
/// Mock ResourceHandleResolver for testing get_or_initialize.
#[derive(Debug)]
struct MockResolver {
    handle: Arc<dyn ResourceHandle>,
}

#[async_trait::async_trait]
impl ResourceHandleResolver for MockResolver {
    async fn resolve(
        &self,
        _resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceOperationError> {
        Ok(self.handle.clone())
    }
}

/// Mock resolver that always fails.
#[derive(Debug)]
struct FailingResolver;

#[async_trait::async_trait]
impl ResourceHandleResolver for FailingResolver {
    async fn resolve(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceOperationError> {
        Err(ResourceOperationError::Unavailable {
            message: format!("Cannot resolve '{resource_ref}'"),
        })
    }
}

#[tokio::test]
async fn get_or_initialize_calls_source_when_not_found() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    let handle = make_handle("new-db");
    let resolver = MockResolver {
        handle: handle.clone(),
    };

    let result = manager
        .get_or_initialize("new-db", Some(&resolver))
        .await;
    assert!(result.is_ok());

    // Subsequent get should find the registered resource
    let second = manager.get("new-db").await;
    assert!(second.is_ok());
}

#[tokio::test]
async fn get_or_initialize_source_error_maps_to_initialization_failed() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    let resolver = FailingResolver;
    let result = manager
        .get_or_initialize("broken", Some(&resolver))
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("initialization failed"), "Got: {msg}");
}
```

- [ ] **Step 6: Run all get_or_initialize tests**

Run: `cargo nextest run --features test-messaging -p tasker-runtime -E 'test(get_or_initialize)'`

Expected: All 4 tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/tasker-runtime/src/pool_manager/mod.rs crates/tasker-runtime/tests/pool_manager_tests.rs
git commit -m "feat(TAS-377): add get_or_initialize to ResourcePoolManager with optional ResourceHandleResolver"
```

---

## Chunk 3: RuntimeOperationProvider implementation

### Task 4: Error mapping helper + RuntimeOperationProvider

**Files:**
- Modify: `crates/tasker-runtime/src/provider.rs`

- [ ] **Step 1: Implement the full provider replacing stubs**

Replace the entire contents of `crates/tasker-runtime/src/provider.rs` with:

```rust
//! `RuntimeOperationProvider` — the production implementation of
//! `tasker_grammar::operations::OperationProvider`.
//!
//! Bridges the pool manager and adapter registry to provide grammar
//! capability executors with their operation trait objects.
//!
//! **Lifetime model:** One `RuntimeOperationProvider` per composition execution.
//! Created when a worker picks up a composition, dropped when execution completes.

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::{
    AcquirableResource, EmittableResource, OperationProvider, PersistableResource,
    ResourceOperationError,
};
use tasker_secure::ResourceError;

use crate::adapters::AdapterRegistry;
use crate::cache::AdapterCache;
use crate::pool_manager::ResourcePoolManager;
use crate::sources::ResourceHandleResolver;

/// Production implementation of `OperationProvider`.
///
/// When a grammar capability executor calls `get_persistable("orders-db")`,
/// this provider:
/// 1. Checks the per-composition [`AdapterCache`] for a cached adapter
/// 2. Asks the [`ResourcePoolManager`] to get or initialize the handle
/// 3. Asks the [`AdapterRegistry`] to wrap the handle in the right adapter
/// 4. Caches and returns the adapter as `Arc<dyn PersistableResource>`
///
/// The executor never sees handles, pools, or adapters — just the
/// operation trait it tested against `InMemoryOperations`.
#[derive(Debug)]
pub struct RuntimeOperationProvider {
    pool_manager: Arc<ResourcePoolManager>,
    adapter_registry: Arc<AdapterRegistry>,
    source: Option<Arc<dyn ResourceHandleResolver>>,
    cache: AdapterCache,
}

impl RuntimeOperationProvider {
    /// Create a new provider without a resource handle resolver.
    ///
    /// Resources must be pre-registered in the pool manager before
    /// composition execution starts.
    pub fn new(
        pool_manager: Arc<ResourcePoolManager>,
        adapter_registry: Arc<AdapterRegistry>,
    ) -> Self {
        Self {
            pool_manager,
            adapter_registry,
            source: None,
            cache: AdapterCache::new(),
        }
    }

    /// Create a new provider with a resource handle resolver for lazy initialization.
    ///
    /// When a resource is not found in the pool manager, the resolver will be
    /// called to initialize it on demand.
    pub fn with_source(
        pool_manager: Arc<ResourcePoolManager>,
        adapter_registry: Arc<AdapterRegistry>,
        source: Arc<dyn ResourceHandleResolver>,
    ) -> Self {
        Self {
            pool_manager,
            adapter_registry,
            source: Some(source),
            cache: AdapterCache::new(),
        }
    }
}

#[async_trait]
impl OperationProvider for RuntimeOperationProvider {
    async fn get_persistable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> {
        if let Some(adapter) = self.cache.get_persistable(resource_ref).await {
            return Ok(adapter);
        }

        let handle = self
            .pool_manager
            .get_or_initialize(resource_ref, self.source.as_deref())
            .await
            .map_err(map_resource_error)?;

        let adapter = self.adapter_registry.as_persistable(handle)?;
        self.cache
            .insert_persistable(resource_ref.to_string(), adapter.clone())
            .await;
        Ok(adapter)
    }

    async fn get_acquirable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError> {
        if let Some(adapter) = self.cache.get_acquirable(resource_ref).await {
            return Ok(adapter);
        }

        let handle = self
            .pool_manager
            .get_or_initialize(resource_ref, self.source.as_deref())
            .await
            .map_err(map_resource_error)?;

        let adapter = self.adapter_registry.as_acquirable(handle)?;
        self.cache
            .insert_acquirable(resource_ref.to_string(), adapter.clone())
            .await;
        Ok(adapter)
    }

    async fn get_emittable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn EmittableResource>, ResourceOperationError> {
        if let Some(adapter) = self.cache.get_emittable(resource_ref).await {
            return Ok(adapter);
        }

        let handle = self
            .pool_manager
            .get_or_initialize(resource_ref, self.source.as_deref())
            .await
            .map_err(map_resource_error)?;

        let adapter = self.adapter_registry.as_emittable(handle)?;
        self.cache
            .insert_emittable(resource_ref.to_string(), adapter.clone())
            .await;
        Ok(adapter)
    }
}

/// Map a `ResourceError` (tasker-secure domain) to a `ResourceOperationError`
/// (tasker-grammar domain).
fn map_resource_error(err: ResourceError) -> ResourceOperationError {
    match err {
        ResourceError::ResourceNotFound { name } => {
            ResourceOperationError::EntityNotFound { entity: name }
        }
        ResourceError::InitializationFailed { name, message } => {
            ResourceOperationError::Unavailable {
                message: format!("Resource '{name}' initialization failed: {message}"),
            }
        }
        ResourceError::HealthCheckFailed { name, message } => {
            ResourceOperationError::Unavailable {
                message: format!("Resource '{name}' health check failed: {message}"),
            }
        }
        ResourceError::CredentialRefreshFailed { name, message } => {
            ResourceOperationError::Unavailable {
                message: format!("Resource '{name}' credential refresh failed: {message}"),
            }
        }
        ResourceError::WrongResourceType {
            name,
            expected,
            actual,
        } => ResourceOperationError::ValidationFailed {
            message: format!(
                "Resource '{name}' type mismatch: expected {expected}, got {actual}"
            ),
        },
        ResourceError::MissingConfigKey { resource, key } => {
            ResourceOperationError::ValidationFailed {
                message: format!("Resource '{resource}' missing required config key: '{key}'"),
            }
        }
        ResourceError::SecretResolution { resource, source } => {
            ResourceOperationError::Unavailable {
                message: format!("Resource '{resource}' secret resolution failed: {source}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_resource_not_found() {
        let err = ResourceError::ResourceNotFound {
            name: "db1".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(
            matches!(mapped, ResourceOperationError::EntityNotFound { entity } if entity == "db1")
        );
    }

    #[test]
    fn map_initialization_failed() {
        let err = ResourceError::InitializationFailed {
            name: "db1".to_string(),
            message: "connection refused".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(matches!(mapped, ResourceOperationError::Unavailable { message } if message.contains("connection refused")));
    }

    #[test]
    fn map_wrong_resource_type() {
        let err = ResourceError::WrongResourceType {
            name: "db1".to_string(),
            expected: "Postgres".to_string(),
            actual: "Http".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(matches!(mapped, ResourceOperationError::ValidationFailed { message } if message.contains("type mismatch")));
    }

    #[test]
    fn map_missing_config_key() {
        let err = ResourceError::MissingConfigKey {
            resource: "db1".to_string(),
            key: "host".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(matches!(mapped, ResourceOperationError::ValidationFailed { message } if message.contains("host")));
    }

    #[test]
    fn map_health_check_failed() {
        let err = ResourceError::HealthCheckFailed {
            name: "db1".to_string(),
            message: "timeout".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(matches!(mapped, ResourceOperationError::Unavailable { message } if message.contains("timeout")));
    }

    #[test]
    fn map_credential_refresh_failed() {
        let err = ResourceError::CredentialRefreshFailed {
            name: "db1".to_string(),
            message: "expired".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(matches!(mapped, ResourceOperationError::Unavailable { message } if message.contains("expired")));
    }

    #[test]
    fn map_secret_resolution() {
        let err = ResourceError::SecretResolution {
            resource: "db1".to_string(),
            source: tasker_secure::SecretsError::ProviderUnavailable {
                message: "vault sealed".to_string(),
            },
        };
        let mapped = map_resource_error(err);
        assert!(matches!(mapped, ResourceOperationError::Unavailable { message } if message.contains("vault sealed")));
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check --all-features -p tasker-runtime`

Expected: Compiles. Note: `self.source.as_deref()` on `Option<Arc<dyn ResourceHandleResolver>>` should work because `Arc<T>` implements `Deref<Target = T>`. If the compiler rejects this, use `self.source.as_ref().map(|s| &**s)` instead.

- [ ] **Step 3: Run the error mapping unit tests**

Run: `cargo nextest run --features test-messaging --lib -p tasker-runtime -E 'test(provider::tests)'`

Expected: All 7 error mapping tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/tasker-runtime/src/provider.rs
git commit -m "feat(TAS-377): implement RuntimeOperationProvider with error mapping and adapter caching"
```

---

## Chunk 4: Integration tests

### Task 5: Integration tests for full resolution flow

**Files:**
- Create: `crates/tasker-runtime/tests/runtime_provider_tests.rs`

- [ ] **Step 1: Create the integration test file**

Create `crates/tasker-runtime/tests/runtime_provider_tests.rs`:

```rust
//! Integration tests for RuntimeOperationProvider.
//!
//! Tests the full resolution flow: provider → pool manager → adapter registry → cache.
//! Uses InMemoryResourceHandle and custom adapter factories — no infrastructure required.

use std::collections::HashMap;
use std::sync::Arc;

use tasker_grammar::operations::{
    AcquirableResource, EmittableResource, OperationProvider, PersistableResource,
    ResourceOperationError,
};
use tasker_grammar::operations::types::*;
use tasker_runtime::AdapterRegistry;
use tasker_runtime::pool_manager::{PoolManagerConfig, ResourceOrigin, ResourcePoolManager};
use tasker_runtime::{ResourceHandleResolver, RuntimeOperationProvider};
use tasker_secure::testing::InMemoryResourceHandle;
use tasker_secure::{ResourceHandle, ResourceRegistry, ResourceType};

fn test_secrets() -> Arc<dyn tasker_secure::SecretsProvider> {
    Arc::new(tasker_secure::testing::InMemorySecretsProvider::new(
        HashMap::new(),
    ))
}

/// Simple adapter wrapping InMemoryResourceHandle as PersistableResource.
struct TestPersistAdapter;

#[async_trait::async_trait]
impl PersistableResource for TestPersistAdapter {
    async fn persist(
        &self,
        entity: &str,
        data: serde_json::Value,
        _constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        Ok(PersistResult {
            data: serde_json::json!({ "entity": entity, "data": data }),
            affected_count: Some(1),
        })
    }
}

/// Simple adapter wrapping InMemoryResourceHandle as AcquirableResource.
struct TestAcquireAdapter;

#[async_trait::async_trait]
impl AcquirableResource for TestAcquireAdapter {
    async fn acquire(
        &self,
        entity: &str,
        _params: serde_json::Value,
        _constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        Ok(AcquireResult {
            data: serde_json::json!([{ "entity": entity }]),
            total_count: Some(1),
        })
    }
}

/// Simple adapter wrapping InMemoryResourceHandle as EmittableResource.
struct TestEmitAdapter;

#[async_trait::async_trait]
impl EmittableResource for TestEmitAdapter {
    async fn emit(
        &self,
        topic: &str,
        payload: serde_json::Value,
        _metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError> {
        Ok(EmitResult {
            data: serde_json::json!({ "topic": topic, "payload": payload }),
            confirmed: true,
        })
    }
}

/// Build an AdapterRegistry with test factories for the Custom("test") resource type.
fn test_adapter_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    let test_type = ResourceType::Custom {
        type_name: "test".to_string(),
    };

    registry.register_persist(
        test_type.clone(),
        Box::new(|_handle| Ok(Arc::new(TestPersistAdapter))),
    );
    registry.register_acquire(
        test_type.clone(),
        Box::new(|_handle| Ok(Arc::new(TestAcquireAdapter))),
    );
    registry.register_emit(
        test_type,
        Box::new(|_handle| Ok(Arc::new(TestEmitAdapter))),
    );

    registry
}

fn make_test_handle(name: &str) -> Arc<InMemoryResourceHandle> {
    Arc::new(InMemoryResourceHandle::new(
        name,
        ResourceType::Custom {
            type_name: "test".to_string(),
        },
    ))
}

// ─── Basic resolution flow ───────────────────────────────────────

#[tokio::test]
async fn get_persistable_resolves_through_pool_and_registry() {
    let secrets = test_secrets();
    let resource_registry = Arc::new(ResourceRegistry::new(secrets));
    let pool = Arc::new(ResourcePoolManager::new(
        resource_registry,
        PoolManagerConfig::default(),
    ));

    let handle = make_test_handle("orders-db");
    pool.register("orders-db", handle, ResourceOrigin::Static, 10)
        .await
        .unwrap();

    let adapter_registry = Arc::new(test_adapter_registry());
    let provider = RuntimeOperationProvider::new(pool, adapter_registry);

    let result = provider.get_persistable("orders-db").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn get_acquirable_resolves_through_pool_and_registry() {
    let secrets = test_secrets();
    let resource_registry = Arc::new(ResourceRegistry::new(secrets));
    let pool = Arc::new(ResourcePoolManager::new(
        resource_registry,
        PoolManagerConfig::default(),
    ));

    let handle = make_test_handle("orders-db");
    pool.register("orders-db", handle, ResourceOrigin::Static, 10)
        .await
        .unwrap();

    let adapter_registry = Arc::new(test_adapter_registry());
    let provider = RuntimeOperationProvider::new(pool, adapter_registry);

    let result = provider.get_acquirable("orders-db").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn get_emittable_resolves_through_pool_and_registry() {
    let secrets = test_secrets();
    let resource_registry = Arc::new(ResourceRegistry::new(secrets));
    let pool = Arc::new(ResourcePoolManager::new(
        resource_registry,
        PoolManagerConfig::default(),
    ));

    let handle = make_test_handle("events");
    pool.register("events", handle, ResourceOrigin::Static, 1)
        .await
        .unwrap();

    let adapter_registry = Arc::new(test_adapter_registry());
    let provider = RuntimeOperationProvider::new(pool, adapter_registry);

    let result = provider.get_emittable("events").await;
    assert!(result.is_ok());
}

// ─── Caching behavior ───────────────────────────────────────────

#[tokio::test]
async fn second_call_returns_cached_adapter() {
    let secrets = test_secrets();
    let resource_registry = Arc::new(ResourceRegistry::new(secrets));
    let pool = Arc::new(ResourcePoolManager::new(
        resource_registry,
        PoolManagerConfig::default(),
    ));

    let handle = make_test_handle("orders-db");
    pool.register("orders-db", handle, ResourceOrigin::Static, 10)
        .await
        .unwrap();

    let adapter_registry = Arc::new(test_adapter_registry());
    let provider = RuntimeOperationProvider::new(pool, adapter_registry);

    let first = provider.get_persistable("orders-db").await.unwrap();
    let second = provider.get_persistable("orders-db").await.unwrap();

    // Same Arc pointer — came from cache, not re-resolved
    assert!(Arc::ptr_eq(&first, &second));
}

// ─── Error propagation ──────────────────────────────────────────

#[tokio::test]
async fn resource_not_found_maps_to_entity_not_found() {
    let secrets = test_secrets();
    let resource_registry = Arc::new(ResourceRegistry::new(secrets));
    let pool = Arc::new(ResourcePoolManager::new(
        resource_registry,
        PoolManagerConfig::default(),
    ));

    let adapter_registry = Arc::new(test_adapter_registry());
    let provider = RuntimeOperationProvider::new(pool, adapter_registry);

    let result = provider.get_persistable("nonexistent").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, ResourceOperationError::EntityNotFound { entity } if entity == "nonexistent")
    );
}

#[tokio::test]
async fn no_adapter_registered_maps_to_validation_failed() {
    let secrets = test_secrets();
    let resource_registry = Arc::new(ResourceRegistry::new(secrets));
    let pool = Arc::new(ResourcePoolManager::new(
        resource_registry,
        PoolManagerConfig::default(),
    ));

    // Register a Pgmq handle — no adapter factory exists for Pgmq
    let handle = Arc::new(InMemoryResourceHandle::new("queue", ResourceType::Pgmq));
    pool.register("queue", handle, ResourceOrigin::Static, 1)
        .await
        .unwrap();

    let adapter_registry = Arc::new(test_adapter_registry());
    let provider = RuntimeOperationProvider::new(pool, adapter_registry);

    let result = provider.get_persistable("queue").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(
        err,
        ResourceOperationError::ValidationFailed { .. }
    ));
}

// ─── ResourceHandleResolver (lazy init) ─────────────────────────

/// Mock resolver that creates InMemoryResourceHandle on demand.
#[derive(Debug)]
struct TestResolver;

#[async_trait::async_trait]
impl ResourceHandleResolver for TestResolver {
    async fn resolve(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceOperationError> {
        Ok(Arc::new(InMemoryResourceHandle::new(
            resource_ref,
            ResourceType::Custom {
                type_name: "test".to_string(),
            },
        )))
    }
}

#[tokio::test]
async fn with_source_lazily_initializes_missing_resource() {
    let secrets = test_secrets();
    let resource_registry = Arc::new(ResourceRegistry::new(secrets));
    let pool = Arc::new(ResourcePoolManager::new(
        resource_registry,
        PoolManagerConfig::default(),
    ));

    let adapter_registry = Arc::new(test_adapter_registry());
    let resolver: Arc<dyn ResourceHandleResolver> = Arc::new(TestResolver);

    let provider =
        RuntimeOperationProvider::with_source(pool.clone(), adapter_registry, resolver);

    // Resource not pre-registered — resolver should initialize it
    let result = provider.get_persistable("lazy-db").await;
    assert!(result.is_ok());

    // Pool manager should now have it registered
    let handle = pool.get("lazy-db").await;
    assert!(handle.is_ok());
}

// ─── Debug ──────────────────────────────────────────────────────

#[tokio::test]
async fn debug_output_is_meaningful() {
    let secrets = test_secrets();
    let resource_registry = Arc::new(ResourceRegistry::new(secrets));
    let pool = Arc::new(ResourcePoolManager::new(
        resource_registry,
        PoolManagerConfig::default(),
    ));
    let adapter_registry = Arc::new(test_adapter_registry());
    let provider = RuntimeOperationProvider::new(pool, adapter_registry);

    let debug = format!("{provider:?}");
    assert!(debug.contains("RuntimeOperationProvider"));
    assert!(debug.contains("AdapterCache"));
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo nextest run --features test-messaging -p tasker-runtime -E 'test(runtime_provider)'`

Expected: All 8 tests pass.

- [ ] **Step 3: Run the full tasker-runtime test suite**

Run: `cargo nextest run --features test-messaging -p tasker-runtime`

Expected: All tests pass (existing + new).

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --all-targets --all-features -p tasker-runtime`

Expected: Zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-runtime/tests/runtime_provider_tests.rs
git commit -m "test(TAS-377): integration tests for RuntimeOperationProvider resolution flow"
```

### Task 6: Final verification

- [ ] **Step 1: Run full workspace check**

Run: `cargo check --all-features`

Expected: Compiles with no errors.

- [ ] **Step 2: Run full workspace clippy**

Run: `cargo clippy --all-targets --all-features --workspace`

Expected: Zero warnings.

- [ ] **Step 3: Run cargo fmt**

Run: `cargo fmt --check`

Expected: No formatting issues.

- [ ] **Step 4: Run test-no-infra to verify no infrastructure needed**

Run: `cargo make test-no-infra`

Expected: All tests pass, including all new TAS-377 tests.
