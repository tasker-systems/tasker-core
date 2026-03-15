//! Tests for `ResourceHandle` trait and `ResourceHandleExt` downcast helpers.

use std::sync::Arc;

use tasker_secure::resource::{ResourceError, ResourceHandle, ResourceHandleExt, ResourceType};
use tasker_secure::secrets::SecretsProvider;
use tasker_secure::testing::InMemorySecretsProvider;

/// Minimal test implementation of `ResourceHandle`.
#[derive(Debug)]
struct TestHandle {
    name: String,
    resource_type: ResourceType,
}

impl TestHandle {
    fn new(name: &str, resource_type: ResourceType) -> Self {
        Self {
            name: name.to_string(),
            resource_type,
        }
    }
}

#[async_trait::async_trait]
impl ResourceHandle for TestHandle {
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn resource_handle_name_and_type() {
    let handle = TestHandle::new("primary_db", ResourceType::Postgres);
    assert_eq!(handle.resource_name(), "primary_db");
    assert_eq!(handle.resource_type(), &ResourceType::Postgres);
}

#[tokio::test]
async fn resource_handle_health_check() {
    let handle = TestHandle::new("db", ResourceType::Postgres);
    handle.health_check().await.unwrap();
}

#[test]
fn resource_handle_as_any_downcast() {
    let handle = TestHandle::new("my_db", ResourceType::Postgres);
    let any_ref = handle.as_any();
    let downcasted = any_ref.downcast_ref::<TestHandle>().unwrap();
    assert_eq!(downcasted.resource_name(), "my_db");
}

#[test]
fn resource_handle_ext_returns_none_for_wrong_type() {
    let handle = TestHandle::new("db", ResourceType::Postgres);
    // TestHandle is not a PostgresHandle or HttpHandle, so both should return None.
    assert!(handle.as_postgres().is_none());
    assert!(handle.as_http().is_none());
}

#[tokio::test]
async fn resource_handle_dyn_dispatch() {
    let handle: Arc<dyn ResourceHandle> =
        Arc::new(TestHandle::new("arc_db", ResourceType::Messaging));
    assert_eq!(handle.resource_name(), "arc_db");
    assert_eq!(handle.resource_type(), &ResourceType::Messaging);
    handle.health_check().await.unwrap();

    let secrets = InMemorySecretsProvider::new(std::collections::HashMap::new());
    handle.refresh_credentials(&secrets).await.unwrap();
}

#[test]
fn resource_handle_debug() {
    let handle = TestHandle::new("debug_db", ResourceType::Http);
    let debug_str = format!("{handle:?}");
    assert!(
        debug_str.contains("TestHandle"),
        "Debug output should contain type name, got: {debug_str}"
    );
}
