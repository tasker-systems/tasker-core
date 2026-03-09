use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;

use crate::expression::ExpressionEngine;
use crate::operations::{InMemoryOperationProvider, InMemoryOperations};
use crate::types::{CapabilityError, CapabilityExecutor, CompositionEnvelope, ExecutionContext};

use super::EmitExecutor;

// ─── Helpers ─────────────────────────────────────────────────────────────

fn test_ctx() -> ExecutionContext {
    ExecutionContext {
        step_name: "emit_step".into(),
        attempt: 1,
        checkpoint_state: None,
    }
}

fn make_executor() -> (EmitExecutor, Arc<InMemoryOperations>) {
    let ops = Arc::new(InMemoryOperations::new(HashMap::new()));
    let provider = Arc::new(InMemoryOperationProvider::new(ops.clone()));
    let engine = ExpressionEngine::with_defaults();
    (EmitExecutor::new(engine, provider), ops)
}

fn order_envelope() -> serde_json::Value {
    json!({
        "context": {"request_id": "req-abc-123", "tenant_id": "T-1"},
        "deps": {
            "customer_profile": {"id": 42, "name": "Alice", "tier": "gold"}
        },
        "step": {"name": "emit_step"},
        "prev": {
            "order_id": 101,
            "total": 45.67,
            "payment_status": "captured",
            "customer_id": 42
        }
    })
}

// ─── Basic event emission ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_event_with_correct_name_and_payload() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": "{order_id: .prev.order_id, total: .prev.total}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    // Without result_shape, returns the confirmation data
    assert_eq!(result["message_id"], json!("test-msg-001"));

    // Verify the event was captured with correct topic and payload
    let emits = ops.captured_emits().await;
    assert_eq!(emits.len(), 1);
    assert_eq!(emits[0].topic, "order.confirmed");
    assert_eq!(emits[0].payload["order_id"], json!(101));
    assert_eq!(emits[0].payload["total"], json!(45.67));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn event_version_included_in_result_envelope() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "event_version": "1.0",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        },
        "result_shape": {
            "expression": "{event: .event_name, version: .event_version, confirmed: .confirmed}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["event"], json!("order.confirmed"));
    assert_eq!(result["version"], json!("1.0"));
    assert_eq!(result["confirmed"], json!(true));
}

// ─── Conditional emission ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn condition_true_emits_event() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "condition": {
            "expression": ".prev.payment_status == \"captured\""
        },
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["message_id"], json!("test-msg-001"));

    let emits = ops.captured_emits().await;
    assert_eq!(emits.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn condition_false_skips_emission() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "condition": {
            "expression": ".prev.payment_status == \"pending\""
        },
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["emitted"], json!(false));
    assert_eq!(result["reason"], json!("condition not met"));

    // No event should have been emitted
    let emits = ops.captured_emits().await;
    assert_eq!(emits.len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn condition_null_skips_emission() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "condition": {
            "expression": ".prev.nonexistent_field"
        },
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["emitted"], json!(false));

    let emits = ops.captured_emits().await;
    assert_eq!(emits.len(), 0);
}

// ─── Payload construction from composition envelope ──────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn payload_expression_accesses_all_envelope_fields() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": "{order_id: .prev.order_id, total: .prev.total, customer_id: .deps.customer_profile.id, tenant: .context.tenant_id}"
        }
    });

    executor.execute(&envelope, &config, &ctx).unwrap();

    let emits = ops.captured_emits().await;
    assert_eq!(emits.len(), 1);
    assert_eq!(emits[0].payload["order_id"], json!(101));
    assert_eq!(emits[0].payload["total"], json!(45.67));
    assert_eq!(emits[0].payload["customer_id"], json!(42));
    assert_eq!(emits[0].payload["tenant"], json!("T-1"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn payload_expression_error_returns_error() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": ".prev | bogus_function"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ExpressionEvaluation(_)));
}

// ─── Metadata expressions ────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metadata_correlation_id_from_expression() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        },
        "metadata": {
            "correlation_id": {
                "expression": ".context.request_id"
            }
        }
    });

    executor.execute(&envelope, &config, &ctx).unwrap();

    let emits = ops.captured_emits().await;
    assert_eq!(emits.len(), 1);
    assert_eq!(
        emits[0].metadata.correlation_id.as_deref(),
        Some("req-abc-123")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metadata_idempotency_key_from_expression() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        },
        "metadata": {
            "idempotency_key": {
                "expression": "\"order-confirmed-\" + (.prev.order_id | tostring)"
            }
        }
    });

    executor.execute(&envelope, &config, &ctx).unwrap();

    let emits = ops.captured_emits().await;
    assert_eq!(emits.len(), 1);
    assert_eq!(
        emits[0].metadata.idempotency_key.as_deref(),
        Some("order-confirmed-101")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metadata_both_correlation_and_idempotency() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        },
        "metadata": {
            "correlation_id": {
                "expression": ".context.request_id"
            },
            "idempotency_key": {
                "expression": "\"emit-\" + (.prev.order_id | tostring)"
            }
        }
    });

    executor.execute(&envelope, &config, &ctx).unwrap();

    let emits = ops.captured_emits().await;
    assert_eq!(
        emits[0].metadata.correlation_id.as_deref(),
        Some("req-abc-123")
    );
    assert_eq!(
        emits[0].metadata.idempotency_key.as_deref(),
        Some("emit-101")
    );
}

// ─── Delivery mode configuration ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delivery_mode_async_default() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        }
    });

    // Default delivery mode should work fine
    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_object());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delivery_mode_sync_parsed() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "delivery_mode": "sync",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_object());
}

// ─── validate_success ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_success_passes_when_confirmed() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        },
        "validate_success": {
            "expression": ".confirmed"
        }
    });

    // InMemory always returns confirmed: true
    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_object());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_success_fails_when_falsy() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        },
        "validate_success": {
            // This will be false since event_version is null (not set)
            "expression": ".event_version == \"2.0\""
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::Execution(_)));
    assert!(err.to_string().contains("validate_success"));
}

// ─── result_shape ────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn result_shape_reshapes_output() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "event_version": "1.0",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        },
        "result_shape": {
            "expression": "{event_id: .data.message_id, event_name: .event_name, version: .event_version}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["event_id"], json!("test-msg-001"));
    assert_eq!(result["event_name"], json!("order.confirmed"));
    assert_eq!(result["version"], json!("1.0"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_result_shape_returns_confirmation_data() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    // Without result_shape, returns raw confirmation data
    assert_eq!(result["message_id"], json!("test-msg-001"));
}

// ─── Custom resource ref ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn custom_resource_ref_used() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "resource": "custom-event-bus",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        }
    });

    // InMemoryOperationProvider accepts any resource_ref
    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_object());

    let emits = ops.captured_emits().await;
    assert_eq!(emits.len(), 1);
}

// ─── Error cases ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_event_name_returns_config_error() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "payload": {
            "expression": ".prev"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_payload_returns_config_error() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed"
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn condition_expression_error_returns_error() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "condition": {
            "expression": "invalid_func()"
        },
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ExpressionEvaluation(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn result_shape_expression_error_returns_error() {
    let (executor, _ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "payload": {
            "expression": "{order_id: .prev.order_id}"
        },
        "result_shape": {
            "expression": ".data | nonexistent_function"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ExpressionEvaluation(_)));
}

// ─── Full ticket example config ──────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ticket_example_config_works() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    // This matches the config shape from the ticket description
    let config = json!({
        "event_name": "order.confirmed",
        "event_version": "1.0",
        "delivery_mode": "async",
        "condition": {
            "expression": ".prev.payment_status == \"captured\""
        },
        "payload": {
            "expression": "{order_id: .prev.order_id, total: .prev.total, customer_id: .deps.customer_profile.id}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["message_id"], json!("test-msg-001"));

    let emits = ops.captured_emits().await;
    assert_eq!(emits.len(), 1);
    assert_eq!(emits[0].topic, "order.confirmed");
    assert_eq!(emits[0].payload["order_id"], json!(101));
    assert_eq!(emits[0].payload["total"], json!(45.67));
    assert_eq!(emits[0].payload["customer_id"], json!(42));
}

// ─── Combined validate_success + result_shape + condition ────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_pipeline_with_all_options() {
    let (executor, ops) = make_executor();
    let ctx = test_ctx();
    let envelope_data = order_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "event_name": "order.confirmed",
        "event_version": "1.0",
        "delivery_mode": "sync",
        "resource": "domain-events",
        "condition": {
            "expression": ".prev.payment_status == \"captured\""
        },
        "payload": {
            "expression": "{order_id: .prev.order_id, total: .prev.total, customer_id: .deps.customer_profile.id}"
        },
        "metadata": {
            "correlation_id": {
                "expression": ".context.request_id"
            },
            "idempotency_key": {
                "expression": "\"order-confirmed-\" + (.prev.order_id | tostring)"
            }
        },
        "validate_success": {
            "expression": ".confirmed"
        },
        "result_shape": {
            "expression": "{event_id: .data.message_id, event: .event_name, version: .event_version}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["event_id"], json!("test-msg-001"));
    assert_eq!(result["event"], json!("order.confirmed"));
    assert_eq!(result["version"], json!("1.0"));

    let emits = ops.captured_emits().await;
    assert_eq!(emits.len(), 1);
    assert_eq!(emits[0].topic, "order.confirmed");
    assert_eq!(
        emits[0].metadata.correlation_id.as_deref(),
        Some("req-abc-123")
    );
    assert_eq!(
        emits[0].metadata.idempotency_key.as_deref(),
        Some("order-confirmed-101")
    );
}
