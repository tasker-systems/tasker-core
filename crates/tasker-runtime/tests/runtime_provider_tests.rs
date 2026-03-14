//! Integration tests for RuntimeOperationProvider.
//!
//! Tests the full resolution flow: provider → pool manager → adapter registry → cache.
//! Uses InMemoryResourceHandle and custom adapter factories — no infrastructure required.

use std::collections::HashMap;
use std::sync::Arc;

use tasker_grammar::{
    AcquirableResource, AcquireConstraints, AcquireResult, EmitMetadata, EmitResult,
    EmittableResource, OperationProvider, PersistConstraints, PersistResult, PersistableResource,
    ResourceOperationError,
};
use tasker_runtime::pool_manager::{PoolManagerConfig, ResourceOrigin, ResourcePoolManager};
use tasker_runtime::AdapterRegistry;
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
    registry.register_emit(test_type, Box::new(|_handle| Ok(Arc::new(TestEmitAdapter))));

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
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
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
    let err = match result {
        Err(e) => e,
        Ok(_) => unreachable!(),
    };
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

    let provider = RuntimeOperationProvider::with_source(pool.clone(), adapter_registry, resolver);

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
