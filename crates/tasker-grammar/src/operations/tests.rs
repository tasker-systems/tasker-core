use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;

use super::*;

// ─── Constraint and result type tests ───────────────────────────────────

#[test]
fn persist_constraints_default_has_no_upsert_key() {
    let constraints = PersistConstraints::default();
    assert!(constraints.upsert_key.is_none());
    assert!(constraints.on_conflict.is_none());
    assert!(constraints.idempotency_key.is_none());
}

#[test]
fn persist_constraints_serialization_roundtrip() {
    let constraints = PersistConstraints {
        upsert_key: Some(vec!["id".into()]),
        on_conflict: Some(ConflictStrategy::Update),
        idempotency_key: Some("key-123".into()),
    };
    let json = serde_json::to_value(&constraints).unwrap();
    let back: PersistConstraints = serde_json::from_value(json).unwrap();
    assert_eq!(back.upsert_key.unwrap(), vec!["id".to_string()]);
}

#[test]
fn acquire_constraints_default_has_no_limits() {
    let constraints = AcquireConstraints::default();
    assert!(constraints.limit.is_none());
    assert!(constraints.offset.is_none());
    assert!(constraints.timeout_ms.is_none());
}

#[test]
fn emit_metadata_default_has_no_fields() {
    let metadata = EmitMetadata::default();
    assert!(metadata.correlation_id.is_none());
    assert!(metadata.idempotency_key.is_none());
    assert!(metadata.attributes.is_none());
}

#[test]
fn persist_result_holds_data_and_count() {
    let result = PersistResult {
        data: json!({"id": 1}),
        affected_count: Some(1),
    };
    assert_eq!(result.affected_count, Some(1));
    assert_eq!(result.data["id"], 1);
}

#[test]
fn acquire_result_holds_data_and_total() {
    let result = AcquireResult {
        data: json!([{"id": 1}, {"id": 2}]),
        total_count: Some(2),
    };
    assert_eq!(result.total_count, Some(2));
    assert!(result.data.is_array());
}

#[test]
fn emit_result_holds_confirmation() {
    let result = EmitResult {
        data: json!({"message_id": "msg-001"}),
        confirmed: true,
    };
    assert!(result.confirmed);
}

#[test]
fn conflict_strategy_serialization() {
    let strategy = ConflictStrategy::Update;
    let json = serde_json::to_value(&strategy).unwrap();
    assert_eq!(json, json!("Update"));
    let back: ConflictStrategy = serde_json::from_value(json).unwrap();
    assert!(matches!(back, ConflictStrategy::Update));
}

// ─── ResourceOperationError tests ───────────────────────────────────────

#[test]
fn resource_operation_error_entity_not_found_display() {
    let err = ResourceOperationError::EntityNotFound {
        entity: "orders".into(),
    };
    assert!(err.to_string().contains("orders"));
}

#[test]
fn resource_operation_error_conflict_display() {
    let err = ResourceOperationError::Conflict {
        entity: "orders".into(),
        reason: "duplicate key".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("orders"));
    assert!(msg.contains("duplicate key"));
}

#[test]
fn resource_operation_error_timeout_display() {
    let err = ResourceOperationError::Timeout { timeout_ms: 5000 };
    assert!(err.to_string().contains("5000"));
}

#[test]
fn resource_operation_error_other_with_source() {
    let source = std::io::Error::other("disk full");
    let err = ResourceOperationError::Other {
        message: "write failed".into(),
        source: Some(Box::new(source)),
    };
    assert!(err.to_string().contains("write failed"));
}

// ─── InMemoryOperations tests ───────────────────────────────────────────

#[tokio::test]
async fn in_memory_persist_captures_operation() {
    let ops = InMemoryOperations::new(HashMap::new());
    let constraints = PersistConstraints::default();
    let data = json!({"id": 1, "name": "test"});

    let result = ops
        .persist("orders", data.clone(), &constraints)
        .await
        .unwrap();

    assert_eq!(result.affected_count, Some(1));

    let captured = ops.captured_persists().await;
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].entity, "orders");
    assert_eq!(captured[0].data, data);
}

#[tokio::test]
async fn in_memory_persist_captures_multiple_operations() {
    let ops = InMemoryOperations::new(HashMap::new());
    let constraints = PersistConstraints::default();

    ops.persist("orders", json!({"id": 1}), &constraints)
        .await
        .unwrap();
    ops.persist("customers", json!({"id": 2}), &constraints)
        .await
        .unwrap();

    let captured = ops.captured_persists().await;
    assert_eq!(captured.len(), 2);
    assert_eq!(captured[0].entity, "orders");
    assert_eq!(captured[1].entity, "customers");
}

#[tokio::test]
async fn in_memory_acquire_returns_fixture_data() {
    let mut fixtures = HashMap::new();
    fixtures.insert(
        "orders".to_string(),
        vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})],
    );
    let ops = InMemoryOperations::new(fixtures);
    let constraints = AcquireConstraints::default();

    let result = ops
        .acquire("orders", json!({}), &constraints)
        .await
        .unwrap();

    assert_eq!(result.data, json!([{"id": 1}, {"id": 2}, {"id": 3}]));
    assert_eq!(result.total_count, Some(3));
}

#[tokio::test]
async fn in_memory_acquire_respects_limit() {
    let mut fixtures = HashMap::new();
    fixtures.insert(
        "orders".to_string(),
        vec![json!({"id": 1}), json!({"id": 2}), json!({"id": 3})],
    );
    let ops = InMemoryOperations::new(fixtures);
    let constraints = AcquireConstraints {
        limit: Some(2),
        ..Default::default()
    };

    let result = ops
        .acquire("orders", json!({}), &constraints)
        .await
        .unwrap();

    assert_eq!(result.data, json!([{"id": 1}, {"id": 2}]));
    assert_eq!(result.total_count, Some(3));
}

#[tokio::test]
async fn in_memory_acquire_entity_not_found() {
    let ops = InMemoryOperations::new(HashMap::new());
    let constraints = AcquireConstraints::default();

    let err = ops
        .acquire("nonexistent", json!({}), &constraints)
        .await
        .unwrap_err();

    assert!(matches!(err, ResourceOperationError::EntityNotFound { .. }));
}

#[tokio::test]
async fn in_memory_emit_captures_operation() {
    let ops = InMemoryOperations::new(HashMap::new());
    let metadata = EmitMetadata {
        correlation_id: Some("corr-001".into()),
        ..Default::default()
    };
    let payload = json!({"event": "order_created"});

    let result = ops
        .emit("order-events", payload.clone(), &metadata)
        .await
        .unwrap();

    assert!(result.confirmed);

    let captured = ops.captured_emits().await;
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].topic, "order-events");
    assert_eq!(captured[0].payload, payload);
    assert_eq!(captured[0].metadata.correlation_id, Some("corr-001".into()));
}

// ─── OperationProvider tests ────────────────────────────────────────────

#[tokio::test]
async fn in_memory_provider_returns_persistable() {
    let ops = Arc::new(InMemoryOperations::new(HashMap::new()));
    let provider = InMemoryOperationProvider::new(ops);

    let persistable = provider.get_persistable("any-resource").await.unwrap();
    let result = persistable
        .persist("orders", json!({"id": 1}), &PersistConstraints::default())
        .await
        .unwrap();

    assert_eq!(result.affected_count, Some(1));
}

#[tokio::test]
async fn in_memory_provider_returns_acquirable() {
    let mut fixtures = HashMap::new();
    fixtures.insert("products".to_string(), vec![json!({"id": 1})]);
    let ops = Arc::new(InMemoryOperations::new(fixtures));
    let provider = InMemoryOperationProvider::new(ops);

    let acquirable = provider.get_acquirable("any-resource").await.unwrap();
    let result = acquirable
        .acquire("products", json!({}), &AcquireConstraints::default())
        .await
        .unwrap();

    assert_eq!(result.data, json!([{"id": 1}]));
}

#[tokio::test]
async fn in_memory_provider_returns_emittable() {
    let ops = Arc::new(InMemoryOperations::new(HashMap::new()));
    let provider = InMemoryOperationProvider::new(ops);

    let emittable = provider.get_emittable("any-resource").await.unwrap();
    let result = emittable
        .emit("topic", json!({"x": 1}), &EmitMetadata::default())
        .await
        .unwrap();

    assert!(result.confirmed);
}

// ─── Trait object safety tests ──────────────────────────────────────────

#[tokio::test]
async fn operation_traits_are_object_safe() {
    let ops = InMemoryOperations::new(HashMap::new());

    // These lines prove the traits are object-safe (can be used as dyn)
    let persistable: Arc<dyn PersistableResource> = Arc::new(ops.clone());
    let acquirable: Arc<dyn AcquirableResource> = Arc::new(ops.clone());
    let emittable: Arc<dyn EmittableResource> = Arc::new(ops);

    let _ = persistable
        .persist("t", json!({}), &PersistConstraints::default())
        .await;
    let _ = acquirable
        .acquire("t", json!({}), &AcquireConstraints::default())
        .await;
    let _ = emittable
        .emit("t", json!({}), &EmitMetadata::default())
        .await;
}

#[tokio::test]
async fn operation_provider_is_object_safe() {
    let ops = Arc::new(InMemoryOperations::new(HashMap::new()));
    let provider: Arc<dyn OperationProvider> = Arc::new(InMemoryOperationProvider::new(ops));

    let _ = provider.get_persistable("x").await;
    let _ = provider.get_acquirable("x").await;
    let _ = provider.get_emittable("x").await;
}
