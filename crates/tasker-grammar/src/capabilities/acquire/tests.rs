use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;

use crate::expression::ExpressionEngine;
use crate::operations::{InMemoryOperationProvider, InMemoryOperations};
use crate::types::{CapabilityError, CapabilityExecutor, CompositionEnvelope, ExecutionContext};

use super::AcquireExecutor;

// ─── Helpers ─────────────────────────────────────────────────────────────

fn test_ctx() -> ExecutionContext {
    ExecutionContext {
        step_name: "acquire_step".into(),
        attempt: 1,
        checkpoint_state: None,
    }
}

fn make_executor(
    fixtures: HashMap<String, Vec<serde_json::Value>>,
) -> (AcquireExecutor, Arc<InMemoryOperations>) {
    let ops = Arc::new(InMemoryOperations::new(fixtures));
    let provider = Arc::new(InMemoryOperationProvider::new(ops.clone()));
    let engine = ExpressionEngine::with_defaults();
    (AcquireExecutor::new(engine, provider), ops)
}

fn customer_fixtures() -> HashMap<String, Vec<serde_json::Value>> {
    let mut fixtures = HashMap::new();
    fixtures.insert(
        "customer_profile".to_string(),
        vec![
            json!({"id": 1, "name": "Alice", "tier": "gold", "ltv": 5000}),
            json!({"id": 2, "name": "Bob", "tier": "silver", "ltv": 1200}),
        ],
    );
    fixtures.insert(
        "orders".to_string(),
        vec![
            json!({"id": 101, "customer_id": 1, "total": 45.67, "status": "pending"}),
            json!({"id": 102, "customer_id": 1, "total": 123.45, "status": "confirmed"}),
            json!({"id": 103, "customer_id": 2, "total": 67.89, "status": "pending"}),
        ],
    );
    fixtures
}

fn default_envelope() -> serde_json::Value {
    json!({
        "context": {"customer_id": "C-100", "category": "electronics"},
        "deps": {},
        "step": {},
        "prev": {
            "customer_id": 1,
            "status": "pending"
        }
    })
}

// ─── Config parsing and validation ───────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn simple_resource_string_config_parses() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile"
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    // Without result_shape, returns raw data (the array)
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn structured_resource_config_parses() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": {
            "ref": "orders-db",
            "entity": "orders"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_config_missing_resource_returns_error() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "params": {
            "expression": ".prev"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
}

// ─── Params expression evaluation ────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn params_expression_evaluates_against_envelope() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = json!({
        "context": {"customer_id": "C-100"},
        "deps": {"lookup": {"tier": "gold"}},
        "step": {},
        "prev": {"customer_id": 1}
    });
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "params": {
            "expression": "{customer_id: .prev.customer_id, context_id: .context.customer_id}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    // Should succeed — params expression is evaluated but InMemory ignores the actual params
    assert!(result.is_array());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn params_expression_error_returns_expression_evaluation_error() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "params": {
            "expression": ".prev | invalid_function_that_does_not_exist"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ExpressionEvaluation(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_params_expression_sends_empty_object() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile"
    });

    // Should work fine — no params means empty object sent
    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
}

// ─── validate_success expression ────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_success_passes_when_truthy() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "validate_success": {
            "expression": ".row_count > 0"
        }
    });

    // InMemory returns 2 records, so row_count=2 > 0 is truthy
    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_success_fails_when_falsy() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "validate_success": {
            // InMemory returns 2 records, so row_count > 100 is false
            "expression": ".row_count > 100"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::Execution(_)));
    assert!(err.to_string().contains("validate_success"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_success_can_check_total_count() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "validate_success": {
            "expression": ".total_count >= 2"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
}

// ─── result_shape expression ────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn result_shape_reshapes_output() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "result_shape": {
            "expression": "{first_name: (.data[0]).name, count: .row_count, total: .total_count}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["first_name"], json!("Alice"));
    assert_eq!(result["count"], json!(2));
    assert_eq!(result["total"], json!(2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_result_shape_returns_raw_data_array() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile"
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    // Without result_shape, returns the raw data array
    assert!(result.is_array());
    let arr = result.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["name"], json!("Alice"));
    assert_eq!(arr[1]["name"], json!("Bob"));
}

// ─── Constraint configuration ───────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn limit_constraint_restricts_results() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "constraints": {
            "limit": 2
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
    // InMemory respects limit
    assert_eq!(result.as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn timeout_constraint_parsed() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "constraints": {
            "timeout_ms": 5000
        }
    });

    // Should parse and execute fine (timeout is advisory in InMemory)
    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retry_constraint_parsed() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "constraints": {
            "timeout_ms": 5000,
            "retry": {
                "max_attempts": 3,
                "backoff_ms": 100
            }
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
}

// ─── Static filter configuration ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn filter_config_parsed() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "filter": {
            "status": {"eq": "pending"},
            "created_at": {"gte": "2026-01-01"}
        }
    });

    // Filter is parsed and stored but InMemory doesn't apply it
    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn select_columns_config_parsed() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "select": {
            "columns": ["id", "customer_id", "total", "status"]
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
}

// ─── Include relationship configuration ──────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn include_relationship_config_parsed() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "select": {
            "columns": ["id", "total", "status"],
            "include": {
                "customer": {
                    "entity": "customers",
                    "foreign_key": ["customer_id"],
                    "references": ["id"],
                    "columns": ["name", "email"]
                }
            }
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn include_with_mismatched_fk_references_fails() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "orders",
        "select": {
            "columns": ["id"],
            "include": {
                "customer": {
                    "entity": "customers",
                    "foreign_key": ["customer_id", "extra_key"],
                    "references": ["id"],
                    "columns": ["name"]
                }
            }
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ConfigValidation(_)));
    assert!(err.to_string().contains("foreign_key"));
    assert!(err.to_string().contains("references"));
}

// ─── Missing resource error ──────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_entity_returns_execution_error() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "nonexistent_table"
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::Execution(_)));
}

// ─── Combined validate_success + result_shape ───────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_success_and_result_shape_together() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "validate_success": {
            "expression": ".row_count > 0"
        },
        "result_shape": {
            "expression": "{name: (.data[0]).name, tier: (.data[0]).tier, lifetime_value: (.data[0]).ltv}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["name"], json!("Alice"));
    assert_eq!(result["tier"], json!("gold"));
    assert_eq!(result["lifetime_value"], json!(5000));
}

// ─── Full ticket example config ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ticket_example_config_works() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = json!({
        "context": {"customer_id": "C-100"},
        "deps": {},
        "step": {},
        "prev": {"customer_id": 1}
    });
    let envelope = CompositionEnvelope::new(&envelope_data);

    // This is the config shape from the ticket description
    let config = json!({
        "resource": "customer_profile",
        "params": {
            "expression": "{customer_id: .prev.customer_id}"
        },
        "constraints": {
            "timeout_ms": 5000,
            "retry": {"max_attempts": 3, "backoff_ms": 100}
        },
        "validate_success": {
            "expression": ".row_count > 0"
        },
        "result_shape": {
            "expression": "{name: (.data[0]).name, tier: (.data[0]).tier, lifetime_value: (.data[0]).ltv}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["name"], json!("Alice"));
    assert_eq!(result["tier"], json!("gold"));
    assert_eq!(result["lifetime_value"], json!(5000));
}

// ─── Full declarative acquire config (docs example) ──────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_declarative_config_with_all_options() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = json!({
        "context": {"customer_id": "C-100"},
        "deps": {},
        "step": {},
        "prev": {"customer_id": 1}
    });
    let envelope = CompositionEnvelope::new(&envelope_data);

    // Comprehensive config matching the operation-shape-constraints doc
    let config = json!({
        "resource": {
            "ref": "orders-db",
            "entity": "orders"
        },
        "select": {
            "columns": ["id", "customer_id", "total", "status"],
            "include": {
                "customer": {
                    "entity": "customers",
                    "foreign_key": ["customer_id"],
                    "references": ["id"],
                    "columns": ["name", "email"]
                }
            }
        },
        "filter": {
            "status": {"eq": "pending"},
        },
        "params": {
            "expression": "{customer_id: .context.customer_id}"
        },
        "constraints": {
            "limit": 100,
            "offset": 0,
            "timeout_ms": 5000
        },
        "validate_success": {
            "expression": ".row_count > 0"
        },
        "result_shape": {
            "expression": "{orders: .data, count: .row_count}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert_eq!(result["count"], json!(3));
    assert!(result["orders"].is_array());
    assert_eq!(result["orders"].as_array().unwrap().len(), 3);
}

// ─── Params expression accesses all envelope fields ──────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn params_expression_accesses_all_envelope_fields() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = json!({
        "context": {"tenant_id": "T-1"},
        "deps": {"lookup": {"rate": 0.08}},
        "step": {"name": "acquire_step"},
        "prev": {"customer_id": 1}
    });
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "params": {
            "expression": "{tenant: .context.tenant_id, customer_id: .prev.customer_id, rate: .deps.lookup.rate}"
        }
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
}

// ─── Edge cases ──────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn minimal_config_just_resource() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    // Minimal valid config — just a resource name
    let config = json!({
        "resource": "customer_profile"
    });

    let result = executor.execute(&envelope, &config, &ctx).unwrap();
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn result_shape_expression_error_returns_error() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "result_shape": {
            "expression": ".data | bogus_function_name"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ExpressionEvaluation(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_success_expression_error_returns_error() {
    let (executor, _ops) = make_executor(customer_fixtures());
    let ctx = test_ctx();
    let envelope_data = default_envelope();
    let envelope = CompositionEnvelope::new(&envelope_data);

    let config = json!({
        "resource": "customer_profile",
        "validate_success": {
            "expression": "invalid_function()"
        }
    });

    let err = executor.execute(&envelope, &config, &ctx).unwrap_err();
    assert!(matches!(err, CapabilityError::ExpressionEvaluation(_)));
}
