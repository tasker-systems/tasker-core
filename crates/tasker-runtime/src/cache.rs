//! SWMR (single-writer, multiple-reader) cache for resolved adapter trait objects.
//!
//! [`AdapterCache`] stores resolved [`PersistableResource`], [`AcquirableResource`],
//! and [`EmittableResource`] trait objects keyed by resource reference name. This
//! avoids repeated resolution through the pool manager and adapter registry during
//! a single composition execution.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use tasker_grammar::operations::{AcquirableResource, EmittableResource, PersistableResource};
use tokio::sync::RwLock;

/// Per-composition cache of resolved adapter trait objects.
///
/// Each composition execution creates one `AdapterCache`. Lookups (`get_*`) take
/// a read lock; inserts (`insert_*`) take a write lock. Because compositions are
/// typically single-threaded with respect to resolution, write contention is
/// minimal while concurrent reads during execution are lock-free.
pub(crate) struct AdapterCache {
    persist: RwLock<HashMap<String, Arc<dyn PersistableResource>>>,
    acquire: RwLock<HashMap<String, Arc<dyn AcquirableResource>>>,
    emit: RwLock<HashMap<String, Arc<dyn EmittableResource>>>,
}

impl fmt::Debug for AdapterCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Trait objects don't implement Debug, so show map keys only.
        // Use try_read to avoid blocking in Debug formatting.
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
    /// Create an empty cache.
    pub(crate) fn new() -> Self {
        Self {
            persist: RwLock::new(HashMap::new()),
            acquire: RwLock::new(HashMap::new()),
            emit: RwLock::new(HashMap::new()),
        }
    }

    // -- PersistableResource --------------------------------------------------

    /// Look up a cached [`PersistableResource`] by resource reference name.
    pub(crate) async fn get_persistable(&self, key: &str) -> Option<Arc<dyn PersistableResource>> {
        self.persist.read().await.get(key).cloned()
    }

    /// Cache a resolved [`PersistableResource`] under the given key.
    pub(crate) async fn insert_persistable(
        &self,
        key: String,
        adapter: Arc<dyn PersistableResource>,
    ) {
        self.persist.write().await.insert(key, adapter);
    }

    // -- AcquirableResource ---------------------------------------------------

    /// Look up a cached [`AcquirableResource`] by resource reference name.
    pub(crate) async fn get_acquirable(&self, key: &str) -> Option<Arc<dyn AcquirableResource>> {
        self.acquire.read().await.get(key).cloned()
    }

    /// Cache a resolved [`AcquirableResource`] under the given key.
    pub(crate) async fn insert_acquirable(
        &self,
        key: String,
        adapter: Arc<dyn AcquirableResource>,
    ) {
        self.acquire.write().await.insert(key, adapter);
    }

    // -- EmittableResource ----------------------------------------------------

    /// Look up a cached [`EmittableResource`] by resource reference name.
    pub(crate) async fn get_emittable(&self, key: &str) -> Option<Arc<dyn EmittableResource>> {
        self.emit.read().await.get(key).cloned()
    }

    /// Cache a resolved [`EmittableResource`] under the given key.
    pub(crate) async fn insert_emittable(&self, key: String, adapter: Arc<dyn EmittableResource>) {
        self.emit.write().await.insert(key, adapter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tasker_grammar::operations::{
        AcquireConstraints, AcquireResult, EmitMetadata, EmitResult, PersistConstraints,
        PersistResult, ResourceOperationError,
    };

    // -- Test doubles ---------------------------------------------------------

    struct StubPersist;

    #[async_trait]
    impl PersistableResource for StubPersist {
        async fn persist(
            &self,
            _entity: &str,
            _data: serde_json::Value,
            _constraints: &PersistConstraints,
        ) -> Result<PersistResult, ResourceOperationError> {
            Ok(PersistResult {
                data: serde_json::Value::Null,
                affected_count: Some(1),
            })
        }
    }

    struct StubAcquire;

    #[async_trait]
    impl AcquirableResource for StubAcquire {
        async fn acquire(
            &self,
            _entity: &str,
            _params: serde_json::Value,
            _constraints: &AcquireConstraints,
        ) -> Result<AcquireResult, ResourceOperationError> {
            Ok(AcquireResult {
                data: serde_json::Value::Array(vec![]),
                total_count: Some(0),
            })
        }
    }

    struct StubEmit;

    #[async_trait]
    impl EmittableResource for StubEmit {
        async fn emit(
            &self,
            _topic: &str,
            _payload: serde_json::Value,
            _metadata: &EmitMetadata,
        ) -> Result<EmitResult, ResourceOperationError> {
            Ok(EmitResult {
                data: serde_json::Value::String("evt-1".to_string()),
                confirmed: true,
            })
        }
    }

    // -- Tests ----------------------------------------------------------------

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
        let adapter: Arc<dyn PersistableResource> = Arc::new(StubPersist);

        cache
            .insert_persistable("orders-db".to_string(), adapter.clone())
            .await;

        let retrieved = cache.get_persistable("orders-db").await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn insert_and_retrieve_acquirable() {
        let cache = AdapterCache::new();
        let adapter: Arc<dyn AcquirableResource> = Arc::new(StubAcquire);

        cache
            .insert_acquirable("products-api".to_string(), adapter.clone())
            .await;

        let retrieved = cache.get_acquirable("products-api").await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn insert_and_retrieve_emittable() {
        let cache = AdapterCache::new();
        let adapter: Arc<dyn EmittableResource> = Arc::new(StubEmit);

        cache
            .insert_emittable("events-bus".to_string(), adapter.clone())
            .await;

        let retrieved = cache.get_emittable("events-bus").await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn types_are_independently_keyed() {
        let cache = AdapterCache::new();

        // Insert only a persistable under "shared-key".
        let persist_adapter: Arc<dyn PersistableResource> = Arc::new(StubPersist);
        cache
            .insert_persistable("shared-key".to_string(), persist_adapter)
            .await;

        // Same key should miss for other types.
        assert!(cache.get_acquirable("shared-key").await.is_none());
        assert!(cache.get_emittable("shared-key").await.is_none());

        // The persistable should still be found.
        assert!(cache.get_persistable("shared-key").await.is_some());
    }

    #[tokio::test]
    async fn multiple_reads_return_same_arc() {
        let cache = AdapterCache::new();
        let adapter: Arc<dyn PersistableResource> = Arc::new(StubPersist);

        cache
            .insert_persistable("db".to_string(), adapter.clone())
            .await;

        let a = cache.get_persistable("db").await.unwrap();
        let b = cache.get_persistable("db").await.unwrap();

        // Both reads should return Arcs pointing to the same allocation.
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[tokio::test]
    async fn debug_output_shows_keys() {
        let cache = AdapterCache::new();

        cache
            .insert_persistable("pg-main".to_string(), Arc::new(StubPersist))
            .await;
        cache
            .insert_acquirable("http-api".to_string(), Arc::new(StubAcquire))
            .await;
        cache
            .insert_emittable("rabbit".to_string(), Arc::new(StubEmit))
            .await;

        let debug_str = format!("{cache:?}");

        assert!(debug_str.contains("pg-main"), "should contain persist key");
        assert!(debug_str.contains("http-api"), "should contain acquire key");
        assert!(debug_str.contains("rabbit"), "should contain emit key");
        assert!(
            debug_str.contains("AdapterCache"),
            "should contain struct name"
        );
    }
}
