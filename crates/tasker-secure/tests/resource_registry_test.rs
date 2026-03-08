//! Tests for `ResourceRegistry`.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use tasker_secure::resource::{ResourceError, ResourceHandle, ResourceRegistry, ResourceType};
use tasker_secure::secrets::SecretsProvider;
use tasker_secure::testing::InMemorySecretsProvider;

// ── Stub handle for testing ────────────────────────────────────────────────

/// A minimal `ResourceHandle` for registry tests.
#[derive(Debug)]
struct StubHandle {
    name: String,
    resource_type: ResourceType,
}

impl StubHandle {
    fn new(name: &str, resource_type: ResourceType) -> Self {
        Self {
            name: name.to_string(),
            resource_type,
        }
    }
}

#[async_trait::async_trait]
impl ResourceHandle for StubHandle {
    fn resource_name(&self) -> &str {
        &self.name
    }

    fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }

    async fn refresh_credentials(
        &self,
        _secrets: &dyn SecretsProvider,
    ) -> Result<(), ResourceError> {
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ResourceError> {
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ── Helper ─────────────────────────────────────────────────────────────────

fn empty_secrets() -> Arc<dyn SecretsProvider> {
    Arc::new(InMemorySecretsProvider::new(HashMap::new()))
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn registry_empty() {
    let registry = ResourceRegistry::new(empty_secrets());
    assert!(registry.get("anything").is_none());
    assert!(registry.list_resources().is_empty());
}

#[tokio::test]
async fn registry_register_and_get() {
    let registry = ResourceRegistry::new(empty_secrets());
    let handle: Arc<dyn ResourceHandle> =
        Arc::new(StubHandle::new("primary_db", ResourceType::Postgres));

    registry.register("primary_db", handle).await;

    let retrieved = registry
        .get("primary_db")
        .expect("should find registered handle");
    assert_eq!(retrieved.resource_name(), "primary_db");
    assert_eq!(retrieved.resource_type(), &ResourceType::Postgres);
}

#[tokio::test]
async fn registry_get_returns_none_for_missing() {
    let registry = ResourceRegistry::new(empty_secrets());
    registry
        .register(
            "db",
            Arc::new(StubHandle::new("db", ResourceType::Postgres)),
        )
        .await;

    assert!(registry.get("nonexistent").is_none());
}

#[tokio::test]
async fn registry_list_resources_shows_names_and_types() {
    let registry = ResourceRegistry::new(empty_secrets());

    registry
        .register(
            "primary_db",
            Arc::new(StubHandle::new("primary_db", ResourceType::Postgres)),
        )
        .await;
    registry
        .register(
            "api_endpoint",
            Arc::new(StubHandle::new("api_endpoint", ResourceType::Http)),
        )
        .await;

    let mut summaries = registry.list_resources();
    summaries.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].name, "api_endpoint");
    assert_eq!(summaries[0].resource_type, ResourceType::Http);
    assert_eq!(summaries[1].name, "primary_db");
    assert_eq!(summaries[1].resource_type, ResourceType::Postgres);
}

#[tokio::test]
async fn registry_list_resources_never_exposes_credentials() {
    let registry = ResourceRegistry::new(empty_secrets());
    registry
        .register(
            "secret_db",
            Arc::new(StubHandle::new("secret_db", ResourceType::Postgres)),
        )
        .await;

    let summaries = registry.list_resources();
    let debug_output = format!("{summaries:?}");

    // ResourceSummary only contains name, type, healthy — no host/port/creds.
    assert!(
        !debug_output.contains("password"),
        "Summary debug must not contain credentials"
    );
    assert!(
        !debug_output.contains("secret_ref"),
        "Summary debug must not contain secret references"
    );
}

#[tokio::test]
async fn registry_refresh_resource() {
    let registry = ResourceRegistry::new(empty_secrets());
    registry
        .register(
            "db",
            Arc::new(StubHandle::new("db", ResourceType::Postgres)),
        )
        .await;

    registry
        .refresh_resource("db")
        .await
        .expect("refresh should succeed for stub handle");
}

#[tokio::test]
async fn registry_refresh_missing_resource() {
    let registry = ResourceRegistry::new(empty_secrets());

    let err = registry
        .refresh_resource("nonexistent")
        .await
        .expect_err("refresh should fail for missing resource");

    match err {
        ResourceError::ResourceNotFound { name } => {
            assert_eq!(name, "nonexistent");
        }
        other => panic!("expected ResourceNotFound, got: {other:?}"),
    }
}
