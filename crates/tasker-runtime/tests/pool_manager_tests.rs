//! Tests for ResourcePoolManager lifecycle management.

use std::sync::Arc;
use std::time::Duration;

use tasker_grammar::operations::ResourceOperationError;
use tasker_runtime::pool_manager::{
    AdmissionStrategy, EvictionStrategy, PoolManagerConfig, ResourceOrigin, ResourcePoolManager,
};
use tasker_runtime::ResourceHandleResolver;
use tasker_secure::testing::InMemoryResourceHandle;
use tasker_secure::{ResourceHandle, ResourceRegistry, ResourceType};

fn test_secrets() -> Arc<dyn tasker_secure::SecretsProvider> {
    Arc::new(tasker_secure::testing::InMemorySecretsProvider::new(
        std::collections::HashMap::new(),
    ))
}

fn test_config() -> PoolManagerConfig {
    PoolManagerConfig {
        max_pools: 3,
        max_total_connections: 30,
        idle_timeout: Duration::from_millis(100),
        sweep_interval: Duration::from_secs(60),
        eviction_strategy: EvictionStrategy::Lru,
        admission_strategy: AdmissionStrategy::Reject,
    }
}

fn make_handle(name: &str) -> Arc<InMemoryResourceHandle> {
    Arc::new(InMemoryResourceHandle::new(name, ResourceType::Postgres))
}

#[tokio::test]
async fn get_returns_registered_resource() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    let handle = make_handle("db1");
    manager
        .register("db1", handle, ResourceOrigin::Static, 10)
        .await
        .unwrap();

    let result = manager.get("db1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn get_returns_not_found_for_missing() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    let result = manager.get("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_updates_access_metrics() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    let handle = make_handle("db1");
    manager
        .register("db1", handle, ResourceOrigin::Dynamic, 10)
        .await
        .unwrap();

    manager.get("db1").await.unwrap();
    manager.get("db1").await.unwrap();
    manager.get("db1").await.unwrap();

    let metrics = manager.pool_metrics().snapshot();
    assert_eq!(metrics.total_pools, 1);
}

#[tokio::test]
async fn admission_rejected_when_at_max_pools() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    for i in 0..3 {
        let name = format!("db{i}");
        manager
            .register(&name, make_handle(&name), ResourceOrigin::Dynamic, 10)
            .await
            .unwrap();
    }

    let result = manager
        .register("db3", make_handle("db3"), ResourceOrigin::Dynamic, 10)
        .await;
    assert!(result.is_err(), "Should reject when at max_pools");

    let metrics = manager.pool_metrics().snapshot();
    assert_eq!(metrics.admission_rejections, 1);
}

#[tokio::test]
async fn connection_budget_enforcement() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let mut config = test_config();
    config.max_pools = 10;
    config.max_total_connections = 25;
    let manager = ResourcePoolManager::new(registry, config);

    manager
        .register("db1", make_handle("db1"), ResourceOrigin::Dynamic, 10)
        .await
        .unwrap();
    manager
        .register("db2", make_handle("db2"), ResourceOrigin::Dynamic, 10)
        .await
        .unwrap();

    let result = manager
        .register("db3", make_handle("db3"), ResourceOrigin::Dynamic, 10)
        .await;
    assert!(
        result.is_err(),
        "Should reject when exceeding connection budget"
    );
}

#[tokio::test]
async fn static_resources_never_evicted() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let mut config = test_config();
    config.idle_timeout = Duration::from_millis(1);
    let manager = ResourcePoolManager::new(registry, config);

    manager
        .register(
            "static_db",
            make_handle("static_db"),
            ResourceOrigin::Static,
            10,
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(10)).await;

    let (_, evicted) = manager.sweep().await;
    assert_eq!(evicted, 0, "Static resources should never be evicted");

    assert!(manager.get("static_db").await.is_ok());
}

#[tokio::test]
async fn sweep_evicts_idle_dynamic_resources() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let mut config = test_config();
    config.idle_timeout = Duration::from_millis(1);
    let manager = ResourcePoolManager::new(registry, config);

    manager
        .register(
            "dynamic_db",
            make_handle("dynamic_db"),
            ResourceOrigin::Dynamic,
            10,
        )
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(10)).await;

    let (_, evicted) = manager.sweep().await;
    assert!(evicted > 0, "Idle dynamic resource should be evicted");

    assert!(manager.get("dynamic_db").await.is_err());
}

#[tokio::test]
async fn sweep_preserves_recently_accessed() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let mut config = test_config();
    config.idle_timeout = Duration::from_millis(50);
    let manager = ResourcePoolManager::new(registry, config);

    manager
        .register(
            "active_db",
            make_handle("active_db"),
            ResourceOrigin::Dynamic,
            10,
        )
        .await
        .unwrap();

    manager.get("active_db").await.unwrap();

    let (_, evicted) = manager.sweep().await;
    assert_eq!(evicted, 0, "Recently accessed should not be evicted");
}

#[tokio::test]
async fn evict_one_admission_strategy() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let mut config = test_config();
    config.max_pools = 2;
    config.admission_strategy = AdmissionStrategy::EvictOne;
    config.idle_timeout = Duration::from_millis(1);
    let manager = ResourcePoolManager::new(registry, config);

    manager
        .register("db1", make_handle("db1"), ResourceOrigin::Dynamic, 10)
        .await
        .unwrap();
    manager
        .register("db2", make_handle("db2"), ResourceOrigin::Dynamic, 10)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(10)).await;

    let result = manager
        .register("db3", make_handle("db3"), ResourceOrigin::Dynamic, 10)
        .await;
    assert!(result.is_ok(), "EvictOne should make room");

    let metrics = manager.pool_metrics().snapshot();
    assert_eq!(metrics.evictions_performed, 1);
}

#[tokio::test]
async fn current_pools_returns_summaries() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    manager
        .register("db1", make_handle("db1"), ResourceOrigin::Static, 10)
        .await
        .unwrap();
    manager
        .register("db2", make_handle("db2"), ResourceOrigin::Dynamic, 5)
        .await
        .unwrap();

    let pools = manager.current_pools();
    assert_eq!(pools.len(), 2);
}

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

#[tokio::test]
async fn get_or_initialize_calls_source_when_not_found() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    let handle = make_handle("new-db");
    let resolver = MockResolver {
        handle: handle.clone(),
    };

    let result = manager.get_or_initialize("new-db", Some(&resolver)).await;
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
    let result = manager.get_or_initialize("broken", Some(&resolver)).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("initialization failed"), "Got: {msg}");
}
