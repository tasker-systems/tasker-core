use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;

use crate::expression::ExpressionEngine;
use crate::operations::{InMemoryOperationProvider, InMemoryOperations};
use crate::types::{CapabilityError, CapabilityExecutor, CompositionEnvelope, ExecutionContext};

use super::PersistExecutor;

// ─── Helpers ─────────────────────────────────────────────────────────────

fn test_ctx() -> ExecutionContext {
    ExecutionContext {
        step_name: "persist_step".into(),
        attempt: 1,
        checkpoint_state: None,
    }
}

fn make_executor(
    fixtures: HashMap<String, Vec<serde_json::Value>>,
) -> (PersistExecutor, Arc<InMemoryOperations>) {
    let ops = Arc::new(InMemoryOperations::new(fixtures));
    let provider = Arc::new(InMemoryOperationProvider::new(ops.clone()));
    let engine = ExpressionEngine::with_defaults();
    (PersistExecutor::new(engine, provider), ops)
}

fn default_envelope() -> serde_json::Value {
    json!({
        "context": {"customer_id": "C-100"},
        "deps": {},
        "step": {},
        "prev": {
            "order_id": 42,
            "computed_total": 99.99,
            "status": "confirmed"
        }
    })
}

// ─── Config parsing and validation ───────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn simple_resource_string_config_parses() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "data": {
            "expression": "{id: .prev.order_id, total: .prev.computed_total}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["id"], json!(42));
    assert_eq!(result["total"], json!(99.99));

    let captured = ops.captured_persists().await;
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].entity, "orders");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn structured_resource_config_parses() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": {
            "ref": "orders-db",
            "entity": "orders"
        },
        "mode": "insert",
        "data": {
            "expression": "{id: .prev.order_id, status: .prev.status}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["id"], json!(42));

    let captured = ops.captured_persists().await;
    assert_eq!(captured[0].entity, "orders");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_config_missing_data_returns_error() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders"
        // missing "data" field
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_config_missing_resource_returns_error() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "data": {
            "expression": ".prev"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

// ─── Mode validation ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_mode_requires_identity() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "mode": "update",
        "data": {
            "expression": "{id: .prev.order_id, status: \"shipped\"}"
        }
        // missing identity
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
    assert!(err.to_string().contains("identity"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_mode_requires_identity() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "mode": "delete",
        "data": {
            "expression": "{id: .prev.order_id}"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upsert_mode_requires_identity() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "mode": "upsert",
        "data": {
            "expression": "{id: .prev.order_id}"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn insert_mode_works_without_identity() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "mode": "insert",
        "data": {
            "expression": "{total: .prev.computed_total}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["total"], json!(99.99));

    let captured = ops.captured_persists().await;
    assert_eq!(captured.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn update_mode_with_identity_succeeds() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "mode": "update",
        "data": {
            "expression": "{id: .prev.order_id, status: \"shipped\"}"
        },
        "identity": {
            "primary_key": ["id"]
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["status"], json!("shipped"));

    let captured = ops.captured_persists().await;
    assert_eq!(captured.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn upsert_mode_with_identity_succeeds() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": {
            "ref": "orders-db",
            "entity": "orders"
        },
        "mode": "upsert",
        "data": {
            "expression": "{id: .prev.order_id, total: .prev.computed_total, status: \"confirmed\"}"
        },
        "identity": {
            "primary_key": ["id"]
        },
        "constraints": {
            "on_conflict": "Update"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["id"], json!(42));

    let captured = ops.captured_persists().await;
    assert_eq!(captured.len(), 1);
    // upsert_key should fall back to identity.primary_key
    assert_eq!(
        captured[0].constraints.upsert_key,
        Some(vec!["id".to_string()])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_mode_with_identity_succeeds() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "mode": "delete",
        "data": {
            "expression": "{id: .prev.order_id}"
        },
        "identity": {
            "primary_key": ["id"]
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["id"], json!(42));

    let captured = ops.captured_persists().await;
    assert_eq!(captured.len(), 1);
}

// ─── Data expression evaluation ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn data_expression_accesses_context_fields() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = json!({
        "context": {"tenant_id": "T-1"},
        "deps": {"lookup": {"rate": 0.08}},
        "step": {},
        "prev": {"subtotal": 100}
    });
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "invoices",
        "data": {
            "expression": "{tenant: .context.tenant_id, subtotal: .prev.subtotal, tax_rate: .deps.lookup.rate}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["tenant"], json!("T-1"));
    assert_eq!(result["subtotal"], json!(100));
    assert_eq!(result["tax_rate"], json!(0.08));

    let captured = ops.captured_persists().await;
    assert_eq!(captured[0].data["tenant"], json!("T-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn data_expression_error_returns_expression_evaluation_error() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "data": {
            "expression": ".prev | invalid_function_that_does_not_exist"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ExpressionEvaluation(_)));
}

// ─── validate_success expression ────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_success_passes_when_truthy() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "data": {
            "expression": "{id: .prev.order_id}"
        },
        "validate_success": {
            "expression": ".affected_rows > 0"
        }
    });

    // InMemoryOperations returns affected_count=1, so affected_rows=1 > 0 is truthy
    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["id"], json!(42));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_success_fails_when_falsy() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "data": {
            "expression": "{id: .prev.order_id}"
        },
        "validate_success": {
            // InMemory returns affected_count=1, so this will be false
            "expression": ".affected_rows > 100"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::Execution(_)));
    assert!(err.to_string().contains("validate_success"));
}

// ─── result_shape expression ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn result_shape_reshapes_output() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "data": {
            "expression": "{id: .prev.order_id, total: .prev.computed_total}"
        },
        "result_shape": {
            "expression": "{persisted_id: .data.id, row_count: .affected_rows}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["persisted_id"], json!(42));
    assert_eq!(result["row_count"], json!(1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_result_shape_returns_raw_data() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "data": {
            "expression": "{id: .prev.order_id}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    // Without result_shape, returns the raw persist result data
    assert_eq!(result["id"], json!(42));
}

// ─── Constraint configuration ───────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_upsert_key_in_constraints() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "mode": "upsert",
        "data": {
            "expression": "{id: .prev.order_id}"
        },
        "identity": {
            "primary_key": ["id"]
        },
        "constraints": {
            "upsert_key": ["id", "tenant_id"],
            "on_conflict": "Skip"
        }
    });

    executor.execute(&envelope, &config, &ctx).unwrap();

    let captured = ops.captured_persists().await;
    // Explicit upsert_key should take precedence over identity fallback
    assert_eq!(
        captured[0].constraints.upsert_key,
        Some(vec!["id".to_string(), "tenant_id".to_string()])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idempotency_key_passed_through() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "data": {
            "expression": "{id: .prev.order_id}"
        },
        "constraints": {
            "idempotency_key": "idem-key-123"
        }
    });

    executor.execute(&envelope, &config, &ctx).unwrap();

    let captured = ops.captured_persists().await;
    assert_eq!(
        captured[0].constraints.idempotency_key,
        Some("idem-key-123".to_string())
    );
}

// ─── Relationship validation ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relationship_with_mismatched_fk_references_fails() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "mode": "insert",
        "data": {
            "expression": "{id: .prev.order_id}"
        },
        "relationships": {
            "line_items": {
                "entity": "order_line_items",
                "foreign_key": ["order_id", "extra_key"],
                "references": ["id"],
                "mode": "insert"
            }
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
    assert!(err.to_string().contains("foreign_key"));
    assert!(err.to_string().contains("references"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn valid_relationship_declaration_succeeds() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": {
            "ref": "orders-db",
            "entity": "orders"
        },
        "mode": "insert",
        "data": {
            "expression": "{id: .prev.order_id, total: .prev.computed_total}"
        },
        "identity": {
            "primary_key": ["id"]
        },
        "relationships": {
            "line_items": {
                "entity": "order_line_items",
                "foreign_key": ["order_id"],
                "references": ["id"],
                "mode": "insert"
            }
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["id"], json!(42));

    let captured = ops.captured_persists().await;
    assert_eq!(captured.len(), 1);
}

// ─── Combined validate_success + result_shape ───────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_success_and_result_shape_together() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "data": {
            "expression": "{id: .prev.order_id, total: .prev.computed_total, status: \"confirmed\"}"
        },
        "validate_success": {
            "expression": ".affected_rows > 0"
        },
        "result_shape": {
            "expression": "{persisted_id: .data.id, timestamp: \"2026-03-09T00:00:00Z\"}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["persisted_id"], json!(42));
    assert_eq!(result["timestamp"], json!("2026-03-09T00:00:00Z"));
}

// ─── Default mode is insert ─────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn default_mode_is_insert() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    // No mode specified — should default to insert (no identity required)
    let config = json!({
        "resource": "orders",
        "data": {
            "expression": "{total: 100}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx);
    assert!(result.is_ok());
}

// ─── Full ticket example config ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ticket_example_config_works() {
    let (executor, ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    // This is the config shape from the ticket description
    let config = json!({
        "resource": "orders",
        "data": {
            "expression": "{id: .prev.order_id, total: .prev.computed_total, status: \"confirmed\"}"
        },
        "constraints": {
            "upsert_key": ["id"]
        },
        "validate_success": {
            "expression": ".affected_rows > 0"
        },
        "result_shape": {
            "expression": "{persisted_id: .data.id}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["persisted_id"], json!(42));

    let captured = ops.captured_persists().await;
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].entity, "orders");
    assert_eq!(captured[0].data["status"], json!("confirmed"));
}

// ─── Composite primary key ─────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn composite_primary_key_accepted() {
    let (executor, _ops) = make_executor(HashMap::new());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "order_lines",
        "mode": "update",
        "data": {
            "expression": "{order_id: .prev.order_id, line_num: 1, quantity: 5}"
        },
        "identity": {
            "primary_key": ["order_id", "line_num"]
        }
    });

    let result = executor.execute(&envelope, &config, &ctx);
    assert!(result.is_ok());
}
