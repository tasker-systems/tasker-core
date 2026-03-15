//! Tests for `InMemoryResourceHandle` and `test_registry_with_fixtures`.

use std::collections::HashMap;

use serde_json::json;

use tasker_secure::resource::{ResourceHandle, ResourceType};
use tasker_secure::testing::{
    test_registry_with_fixtures, InMemoryResourceHandle, ResourceFixture,
};

// ── InMemoryResourceHandle unit tests ─────────────────────────────────────

#[test]
fn in_memory_handle_name_and_type() {
    let handle = InMemoryResourceHandle::new("test_db", ResourceType::Postgres);
    assert_eq!(handle.resource_name(), "test_db");
    assert_eq!(handle.resource_type(), &ResourceType::Postgres);
}

#[tokio::test]
async fn in_memory_handle_health_check() {
    let handle = InMemoryResourceHandle::new("test_db", ResourceType::Postgres);
    handle
        .health_check()
        .await
        .expect("health check should always succeed for in-memory handle");
}

#[test]
fn in_memory_handle_fixture_data() {
    let mut fixtures = HashMap::new();
    fixtures.insert("user_count".to_string(), json!(42));
    fixtures.insert("label".to_string(), json!("hello"));

    let handle =
        InMemoryResourceHandle::with_fixtures("analytics_db", ResourceType::Postgres, fixtures);

    assert_eq!(handle.get_fixture("user_count"), Some(&json!(42)));
    assert_eq!(handle.get_fixture("label"), Some(&json!("hello")));
    assert_eq!(handle.get_fixture("nonexistent"), None);
}

#[test]
fn in_memory_handle_persist_capture() {
    let handle = InMemoryResourceHandle::new("sink", ResourceType::Http);

    assert!(handle.persisted().is_empty());

    handle.capture_persist(json!({"id": 1, "status": "created"}));
    handle.capture_persist(json!({"id": 2, "status": "updated"}));

    let captured = handle.persisted();
    assert_eq!(captured.len(), 2);
    assert_eq!(captured[0], json!({"id": 1, "status": "created"}));
    assert_eq!(captured[1], json!({"id": 2, "status": "updated"}));
}

#[test]
fn in_memory_handle_emit_capture() {
    let handle = InMemoryResourceHandle::new("events", ResourceType::Messaging);

    assert!(handle.emitted().is_empty());

    handle.capture_emit(json!({"event": "order.placed", "order_id": 100}));

    let captured = handle.emitted();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0]["event"], "order.placed");
}

#[test]
fn in_memory_handle_clear_captures() {
    let handle = InMemoryResourceHandle::new("sink", ResourceType::Http);

    handle.capture_persist(json!("p1"));
    handle.capture_emit(json!("e1"));
    assert_eq!(handle.persisted().len(), 1);
    assert_eq!(handle.emitted().len(), 1);

    handle.clear_captures();
    assert!(handle.persisted().is_empty());
    assert!(handle.emitted().is_empty());
}

// ── test_registry_with_fixtures tests ─────────────────────────────────────

#[tokio::test]
async fn test_registry_with_fixtures_creates_usable_registry() {
    let fixtures = vec![
        ResourceFixture {
            name: "primary_db".to_string(),
            resource_type: ResourceType::Postgres,
            data: HashMap::new(),
        },
        ResourceFixture {
            name: "api_endpoint".to_string(),
            resource_type: ResourceType::Http,
            data: HashMap::new(),
        },
    ];

    let registry = test_registry_with_fixtures(fixtures).await;

    // Verify list contains both resources.
    let mut summaries = registry.list_resources();
    summaries.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].name, "api_endpoint");
    assert_eq!(summaries[0].resource_type, ResourceType::Http);
    assert_eq!(summaries[1].name, "primary_db");
    assert_eq!(summaries[1].resource_type, ResourceType::Postgres);

    // Verify get works for each.
    let db = registry.get("primary_db").expect("should find primary_db");
    assert_eq!(db.resource_name(), "primary_db");

    let api = registry
        .get("api_endpoint")
        .expect("should find api_endpoint");
    assert_eq!(api.resource_name(), "api_endpoint");
}

#[tokio::test]
async fn test_registry_in_memory_handle_downcast() {
    let mut fixture_data = HashMap::new();
    fixture_data.insert("config_key".to_string(), json!("config_value"));

    let fixtures = vec![ResourceFixture {
        name: "test_resource".to_string(),
        resource_type: ResourceType::Postgres,
        data: fixture_data,
    }];

    let registry = test_registry_with_fixtures(fixtures).await;

    let handle = registry
        .get("test_resource")
        .expect("should find test_resource");

    // Downcast via as_any to InMemoryResourceHandle.
    let in_memory = handle
        .as_any()
        .downcast_ref::<InMemoryResourceHandle>()
        .expect("should downcast to InMemoryResourceHandle");

    // Access fixture data through the downcast reference.
    assert_eq!(
        in_memory.get_fixture("config_key"),
        Some(&json!("config_value"))
    );

    // Use capture methods through the downcast reference.
    in_memory.capture_persist(json!({"written": true}));
    assert_eq!(in_memory.persisted().len(), 1);
    assert_eq!(in_memory.persisted()[0], json!({"written": true}));

    in_memory.capture_emit(json!({"event": "test"}));
    assert_eq!(in_memory.emitted().len(), 1);
}
