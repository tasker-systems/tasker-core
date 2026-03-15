//! Tests for DefinitionBasedResolver — bridges ResourceDefinitionSource
//! to ResourceHandleResolver by dispatching to handle constructors.

use std::collections::HashMap;
use std::sync::Arc;

use tasker_grammar::operations::ResourceOperationError;
use tasker_runtime::sources::static_config::StaticConfigSource;
use tasker_runtime::{DefinitionBasedResolver, ResourceDefinitionSource, ResourceHandleResolver};
use tasker_secure::testing::InMemorySecretsProvider;
use tasker_secure::{ResourceConfig, ResourceDefinition, ResourceType};

fn test_secrets() -> Arc<dyn tasker_secure::SecretsProvider> {
    Arc::new(InMemorySecretsProvider::new(HashMap::new()))
}

fn make_definition(name: &str, rt: ResourceType) -> ResourceDefinition {
    ResourceDefinition {
        name: name.to_string(),
        resource_type: rt,
        config: ResourceConfig::default(),
        secrets_provider: None,
    }
}

#[tokio::test]
async fn resolve_missing_definition_returns_entity_not_found() {
    let source: Arc<dyn ResourceDefinitionSource> = Arc::new(StaticConfigSource::new(vec![]));
    let resolver = DefinitionBasedResolver::new(source, test_secrets());

    let result = resolver.resolve("nonexistent").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, ResourceOperationError::EntityNotFound { ref entity } if entity == "nonexistent"),
        "Expected EntityNotFound, got: {err:?}"
    );
}

#[tokio::test]
async fn resolve_messaging_type_returns_validation_failed() {
    let source: Arc<dyn ResourceDefinitionSource> =
        Arc::new(StaticConfigSource::new(vec![make_definition(
            "queue",
            ResourceType::Messaging,
        )]));
    let resolver = DefinitionBasedResolver::new(source, test_secrets());

    let result = resolver.resolve("queue").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, ResourceOperationError::ValidationFailed { ref message } if message.contains("No handle factory")),
        "Expected ValidationFailed with 'No handle factory', got: {err:?}"
    );
}

#[tokio::test]
async fn resolve_custom_type_returns_validation_failed() {
    let source: Arc<dyn ResourceDefinitionSource> =
        Arc::new(StaticConfigSource::new(vec![make_definition(
            "cache",
            ResourceType::Custom {
                type_name: "redis".to_string(),
            },
        )]));
    let resolver = DefinitionBasedResolver::new(source, test_secrets());

    let result = resolver.resolve("cache").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, ResourceOperationError::ValidationFailed { ref message } if message.contains("No handle factory")),
        "Expected ValidationFailed with 'No handle factory', got: {err:?}"
    );
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn resolve_postgres_with_missing_config_returns_error() {
    // Postgres from_config requires at least "host" and "database" keys.
    // An empty config should fail during initialization (before any connection).
    let source: Arc<dyn ResourceDefinitionSource> =
        Arc::new(StaticConfigSource::new(vec![make_definition(
            "bad-db",
            ResourceType::Postgres,
        )]));
    let resolver = DefinitionBasedResolver::new(source, test_secrets());

    let result = resolver.resolve("bad-db").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    // MissingConfigKey maps to ValidationFailed via map_resource_error
    assert!(
        matches!(
            err,
            ResourceOperationError::ValidationFailed { .. }
                | ResourceOperationError::Unavailable { .. }
        ),
        "Expected ValidationFailed or Unavailable from missing config, got: {err:?}"
    );
}

#[tokio::test]
async fn debug_output_is_meaningful() {
    let source: Arc<dyn ResourceDefinitionSource> = Arc::new(StaticConfigSource::new(vec![]));
    let resolver = DefinitionBasedResolver::new(source, test_secrets());

    let debug = format!("{resolver:?}");
    assert!(debug.contains("DefinitionBasedResolver"));
}
