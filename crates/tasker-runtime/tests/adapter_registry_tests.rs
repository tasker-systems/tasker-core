//! Tests for AdapterRegistry factory dispatch.

use std::sync::Arc;

use tasker_grammar::operations::ResourceOperationError;
use tasker_runtime::adapters::registry::AdapterRegistry;
use tasker_secure::testing::InMemoryResourceHandle;
use tasker_secure::ResourceType;

/// Extract the error from a Result, panicking if it was Ok.
fn expect_err<T>(result: Result<T, ResourceOperationError>) -> ResourceOperationError {
    match result {
        Err(e) => e,
        Ok(_) => panic!("expected Err, got Ok"),
    }
}

#[test]
#[cfg(feature = "postgres")]
fn standard_registry_has_postgres_persist_factory() {
    let registry = AdapterRegistry::standard();
    let handle = Arc::new(InMemoryResourceHandle::new(
        "test_db",
        ResourceType::Postgres,
    ));
    let result = registry.as_persistable(handle);
    // InMemoryResourceHandle is not a PostgresHandle, so the downcast validation fails
    let err = expect_err(result);
    let msg = format!("{err}");
    assert!(msg.contains("Expected Postgres handle"), "Got: {msg}");
}

#[test]
#[cfg(feature = "http")]
fn standard_registry_has_http_emit_factory() {
    let registry = AdapterRegistry::standard();
    let handle = Arc::new(InMemoryResourceHandle::new("webhook", ResourceType::Http));
    let result = registry.as_emittable(handle);
    // InMemoryResourceHandle is not an HttpHandle, so the downcast validation fails
    let err = expect_err(result);
    let msg = format!("{err}");
    assert!(msg.contains("Expected HTTP handle"), "Got: {msg}");
}

#[test]
fn unknown_resource_type_returns_no_factory_error() {
    let registry = AdapterRegistry::standard();
    let handle = Arc::new(InMemoryResourceHandle::new(
        "custom",
        ResourceType::Custom {
            type_name: "redis".to_string(),
        },
    ));
    let result = registry.as_persistable(handle);
    let err = expect_err(result);
    let msg = format!("{err}");
    assert!(msg.contains("No persist adapter registered"), "Got: {msg}");
}

#[test]
fn messaging_has_no_persist_factory() {
    let registry = AdapterRegistry::standard();
    let handle = Arc::new(InMemoryResourceHandle::new(
        "queue",
        ResourceType::Messaging,
    ));
    let result = registry.as_persistable(handle);
    let err = expect_err(result);
    let msg = format!("{err}");
    assert!(msg.contains("No persist adapter registered"), "Got: {msg}");
}

#[test]
fn custom_factory_can_be_registered() {
    let mut registry = AdapterRegistry::new();
    let custom_type = ResourceType::Custom {
        type_name: "redis".to_string(),
    };

    registry.register_persist(
        custom_type.clone(),
        Box::new(|_handle| {
            Err(ResourceOperationError::ValidationFailed {
                message: "test factory called".to_string(),
            })
        }),
    );

    let handle = Arc::new(InMemoryResourceHandle::new("redis1", custom_type));
    let result = registry.as_persistable(handle);
    let err = expect_err(result);
    assert!(format!("{err}").contains("test factory called"));
}

#[test]
fn debug_shows_registered_types() {
    let registry = AdapterRegistry::standard();
    let debug = format!("{registry:?}");
    assert!(debug.contains("AdapterRegistry"));
    assert!(debug.contains("persist_types"));
    assert!(debug.contains("acquire_types"));
    assert!(debug.contains("emit_types"));
}

#[test]
fn default_creates_empty_registry() {
    let registry = AdapterRegistry::default();
    let handle = Arc::new(InMemoryResourceHandle::new("test", ResourceType::Postgres));
    // Empty registry has no factories at all
    let err = expect_err(registry.as_persistable(handle));
    let msg = format!("{err}");
    assert!(msg.contains("No persist adapter registered"), "Got: {msg}");
}
