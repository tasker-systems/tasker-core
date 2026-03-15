//! Integration test: StaticConfigSource → mock resolver → ResourcePoolManager.
//!
//! Validates the full pipeline without infrastructure. Uses a test
//! ResourceHandleResolver (not DefinitionBasedResolver) because from_config
//! establishes real connections.

use std::collections::HashMap;
use std::sync::Arc;

use tasker_grammar::operations::ResourceOperationError;
use tasker_runtime::pool_manager::{PoolManagerConfig, ResourcePoolManager};
use tasker_runtime::sources::static_config::StaticConfigSource;
use tasker_runtime::{ResourceDefinitionSource, ResourceHandleResolver};
use tasker_secure::testing::{InMemoryResourceHandle, InMemorySecretsProvider};
use tasker_secure::{
    ResourceConfig, ResourceDefinition, ResourceHandle, ResourceRegistry, ResourceType,
};

fn test_secrets() -> Arc<dyn tasker_secure::SecretsProvider> {
    Arc::new(InMemorySecretsProvider::new(HashMap::new()))
}

/// Test resolver that uses a StaticConfigSource to verify the definition exists,
/// then returns an InMemoryResourceHandle.
#[derive(Debug)]
struct StubSourceResolver {
    source: Arc<dyn ResourceDefinitionSource>,
}

#[async_trait::async_trait]
impl ResourceHandleResolver for StubSourceResolver {
    async fn resolve(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceOperationError> {
        let definition = self.source.resolve(resource_ref).await.ok_or_else(|| {
            ResourceOperationError::EntityNotFound {
                entity: resource_ref.to_string(),
            }
        })?;

        Ok(Arc::new(InMemoryResourceHandle::new(
            &definition.name,
            definition.resource_type.clone(),
        )))
    }
}

#[tokio::test]
async fn static_source_feeds_pool_manager_lazy_init() {
    // 1. Set up a StaticConfigSource with definitions
    let source: Arc<dyn ResourceDefinitionSource> = Arc::new(StaticConfigSource::new(vec![
        ResourceDefinition {
            name: "orders-db".to_string(),
            resource_type: ResourceType::Postgres,
            config: ResourceConfig::default(),
            secrets_provider: None,
        },
        ResourceDefinition {
            name: "payment-api".to_string(),
            resource_type: ResourceType::Http,
            config: ResourceConfig::default(),
            secrets_provider: None,
        },
    ]));

    // 2. Create resolver that checks the source before returning a handle
    let resolver = StubSourceResolver {
        source: source.clone(),
    };

    // 3. Create pool manager with no pre-registered resources
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let pool = ResourcePoolManager::new(registry, PoolManagerConfig::default());

    // 4. Lazy init through pool manager — resource not pre-registered
    let handle = pool
        .get_or_initialize("orders-db", Some(&resolver))
        .await
        .unwrap();
    assert_eq!(handle.resource_name(), "orders-db");

    // 5. Second call should find it already registered
    let handle2 = pool.get("orders-db").await.unwrap();
    assert_eq!(handle2.resource_name(), "orders-db");

    // 6. Resource NOT in source should fail
    let err = pool
        .get_or_initialize("nonexistent", Some(&resolver))
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("initialization failed"), "Got: {msg}");
}

#[tokio::test]
async fn static_source_list_names_matches_definitions() {
    let source = StaticConfigSource::new(vec![
        ResourceDefinition {
            name: "db1".to_string(),
            resource_type: ResourceType::Postgres,
            config: ResourceConfig::default(),
            secrets_provider: None,
        },
        ResourceDefinition {
            name: "api1".to_string(),
            resource_type: ResourceType::Http,
            config: ResourceConfig::default(),
            secrets_provider: None,
        },
    ]);

    let mut names = source.list_names().await;
    names.sort();
    assert_eq!(names, vec!["api1", "db1"]);
}
