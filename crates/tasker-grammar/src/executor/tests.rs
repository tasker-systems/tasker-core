//! Tests for the `CompositionExecutor`.
//!
//! Covers the test matrix from TAS-334:
//! * Linear 3-step composition: validate → transform → transform
//! * Cross-step references: later invocation reads earlier invocation's output
//! * Checkpoint creation at mutating capability boundaries
//! * Resume from checkpoint (skip already-completed invocations)
//! * Capability failure mid-chain produces structured error with step context
//! * Empty composition (zero invocations) returns input unchanged

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};

use crate::capabilities::assert::AssertExecutor;
use crate::capabilities::persist::PersistExecutor;
use crate::capabilities::transform::TransformExecutor;
use crate::capabilities::validate::ValidateExecutor;
use crate::expression::ExpressionEngine;
use crate::operations::{InMemoryOperationProvider, InMemoryOperations};
use crate::types::{
    CapabilityInvocation, CompositionCheckpoint, CompositionError, CompositionSpec,
    OutcomeDeclaration,
};

use super::{CompositionExecutor, CompositionInput};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn engine() -> ExpressionEngine {
    ExpressionEngine::with_defaults()
}

fn default_input(context: Value) -> CompositionInput {
    CompositionInput {
        context,
        deps: json!({}),
        step: json!({"name": "test_step"}),
    }
}

fn make_executor() -> CompositionExecutor {
    let ops = Arc::new(InMemoryOperations::new(HashMap::new()));
    let provider = Arc::new(InMemoryOperationProvider::new(ops));

    CompositionExecutor::builder()
        .register("transform", TransformExecutor::new(engine()))
        .register("validate", ValidateExecutor::new())
        .register("assert", AssertExecutor::new(engine()))
        .register("persist", PersistExecutor::new(engine(), provider))
        .build()
}

fn make_spec(invocations: Vec<CapabilityInvocation>) -> CompositionSpec {
    CompositionSpec {
        name: Some("test_composition".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test composition".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations,
    }
}

// ---------------------------------------------------------------------------
// Empty composition
// ---------------------------------------------------------------------------

#[test]
fn empty_composition_returns_context_unchanged() {
    let executor = make_executor();
    let spec = make_spec(vec![]);
    let input = default_input(json!({"greeting": "hello"}));

    let result = executor.execute(&spec, input, "s", 1).unwrap();
    assert_eq!(result.output, json!({"greeting": "hello"}));
    assert!(result.checkpoints.is_empty());
}

// ---------------------------------------------------------------------------
// Single-step composition
// ---------------------------------------------------------------------------

#[test]
fn single_transform_invocation() {
    let executor = make_executor();
    let spec = make_spec(vec![CapabilityInvocation {
        capability: "transform".to_owned(),
        config: json!({
            "filter": "{doubled: (.context.value * 2)}",
            "output": {"type": "object"}
        }),
        checkpoint: false,
    }]);

    let input = default_input(json!({"value": 21}));
    let result = executor.execute(&spec, input, "s", 1).unwrap();

    assert_eq!(result.output, json!({"doubled": 42}));
    assert!(result.checkpoints.is_empty());
}

// ---------------------------------------------------------------------------
// Linear 3-step composition: validate → transform → transform
// ---------------------------------------------------------------------------

#[test]
fn linear_three_step_validate_transform_transform() {
    let executor = make_executor();
    let spec = make_spec(vec![
        // Step 0: validate input has required fields
        CapabilityInvocation {
            capability: "validate".to_owned(),
            config: json!({
                "schema": {
                    "type": "object",
                    "required": ["price", "quantity"],
                    "properties": {
                        "price": {"type": "number"},
                        "quantity": {"type": "integer"}
                    }
                }
            }),
            checkpoint: false,
        },
        // Step 1: compute subtotal from validated data
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{subtotal: (.prev.price * .prev.quantity), price: .prev.price, quantity: .prev.quantity}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
        // Step 2: compute final total with tax
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{subtotal: .prev.subtotal, tax: (.prev.subtotal * 0.1), total: (.prev.subtotal * 1.1)}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
    ]);

    let input = default_input(json!({"price": 10.0, "quantity": 5}));
    let result = executor.execute(&spec, input, "s", 1).unwrap();

    assert_eq!(result.output["subtotal"], json!(50.0));
    assert_eq!(result.output["tax"], json!(5.0));
    // Use approximate comparison for floating point arithmetic
    let total = result.output["total"].as_f64().unwrap();
    assert!((total - 55.0).abs() < 1e-10, "expected ~55.0, got {total}");
}

// ---------------------------------------------------------------------------
// Cross-step references: later invocation reads earlier invocation's output
// ---------------------------------------------------------------------------

#[test]
fn cross_step_reference_via_deps_invocations() {
    let executor = make_executor();
    let spec = make_spec(vec![
        // Step 0: produce a base value
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{base: 100}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
        // Step 1: produce a multiplier
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{multiplier: 3}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
        // Step 2: reference step 0's output via .deps.invocations
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{result: (.deps.invocations.\"0\".base * .prev.multiplier)}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
    ]);

    let input = default_input(json!({}));
    let result = executor.execute(&spec, input, "s", 1).unwrap();

    assert_eq!(result.output["result"], json!(300));
}

// ---------------------------------------------------------------------------
// Checkpoint creation at mutating capability boundaries
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn checkpoint_emitted_for_persist_invocation() {
    let ops = Arc::new(InMemoryOperations::new(HashMap::new()));
    let provider = Arc::new(InMemoryOperationProvider::new(ops.clone()));

    let executor = CompositionExecutor::builder()
        .register("transform", TransformExecutor::new(engine()))
        .register("persist", PersistExecutor::new(engine(), provider))
        .build();

    let spec = make_spec(vec![
        // Step 0: prepare data
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{name: .context.name, amount: .context.amount}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
        // Step 1: persist (checkpoint boundary)
        CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": "orders",
                "data": {"expression": ".prev"},
            }),
            checkpoint: true,
        },
        // Step 2: format result
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{status: \"persisted\", data: .prev}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
    ]);

    let input = default_input(json!({"name": "order-1", "amount": 99}));
    let result = executor.execute(&spec, input, "s", 1).unwrap();

    // Final output should include the formatted result
    assert_eq!(result.output["status"], json!("persisted"));

    // One checkpoint should have been emitted (for the persist step)
    assert_eq!(result.checkpoints.len(), 1);
    let cp = &result.checkpoints[0];
    assert_eq!(cp.completed_invocation_index, 1);
    assert_eq!(cp.completed_capability, "persist");
    assert!(cp.was_mutation);
    // Checkpoint should have outputs for steps 0 and 1
    assert_eq!(cp.all_invocation_outputs.len(), 2);
    assert!(cp.all_invocation_outputs.contains_key(&0));
    assert!(cp.all_invocation_outputs.contains_key(&1));
}

// ---------------------------------------------------------------------------
// Resume from checkpoint (skip already-completed invocations)
// ---------------------------------------------------------------------------

#[test]
fn resume_from_checkpoint_skips_completed_invocations() {
    let executor = make_executor();
    let spec = make_spec(vec![
        // Step 0: validate (would be skipped on resume)
        CapabilityInvocation {
            capability: "validate".to_owned(),
            config: json!({
                "schema": {
                    "type": "object",
                    "required": ["value"],
                    "properties": {"value": {"type": "number"}}
                }
            }),
            checkpoint: false,
        },
        // Step 1: first transform (checkpoint, would be skipped on resume)
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{intermediate: (.context.value * 2)}",
                "output": {"type": "object"}
            }),
            checkpoint: true,
        },
        // Step 2: second transform (will execute on resume)
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{final_result: (.prev.intermediate + 10)}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
    ]);

    // Simulate a checkpoint after step 1 completed
    let checkpoint = CompositionCheckpoint {
        completed_invocation_index: 1,
        completed_capability: "transform".to_owned(),
        invocation_output: json!({"intermediate": 42}),
        all_invocation_outputs: {
            let mut map = HashMap::new();
            map.insert(0, json!({"value": 21}));
            map.insert(1, json!({"intermediate": 42}));
            map
        },
        was_mutation: false,
    };

    let input = default_input(json!({"value": 21}));
    let result = executor.resume(&spec, &checkpoint, &input, "s", 2).unwrap();

    // Should have executed only step 2, using checkpoint's prev
    assert_eq!(result.output, json!({"final_result": 52}));
}

#[test]
fn resume_from_final_invocation_returns_checkpoint_output() {
    let executor = make_executor();
    let spec = make_spec(vec![CapabilityInvocation {
        capability: "transform".to_owned(),
        config: json!({"filter": "{done: true}", "output": {"type": "object"}}),
        checkpoint: true,
    }]);

    // Checkpoint at the last (and only) invocation
    let checkpoint = CompositionCheckpoint {
        completed_invocation_index: 0,
        completed_capability: "transform".to_owned(),
        invocation_output: json!({"done": true}),
        all_invocation_outputs: {
            let mut map = HashMap::new();
            map.insert(0, json!({"done": true}));
            map
        },
        was_mutation: false,
    };

    let input = default_input(json!({}));
    let result = executor.resume(&spec, &checkpoint, &input, "s", 2).unwrap();
    assert_eq!(result.output, json!({"done": true}));
    assert!(result.checkpoints.is_empty());
}

// ---------------------------------------------------------------------------
// Capability failure mid-chain produces structured error
// ---------------------------------------------------------------------------

#[test]
fn failure_mid_chain_produces_structured_error() {
    let executor = make_executor();
    let spec = make_spec(vec![
        // Step 0: succeeds
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{name: .context.name}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
        // Step 1: assert fails
        CapabilityInvocation {
            capability: "assert".to_owned(),
            config: json!({
                "filter": ".prev.name == \"Bob\"",
                "error": "Name must be Bob"
            }),
            checkpoint: false,
        },
        // Step 2: never reached
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{unreachable: true}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
    ]);

    let input = default_input(json!({"name": "Alice"}));
    let err = executor.execute(&spec, input, "s", 1).unwrap_err();

    match err {
        CompositionError::InvocationFailure {
            invocation_index,
            capability,
            ..
        } => {
            assert_eq!(invocation_index, 1);
            assert_eq!(capability, "assert");
        }
        other => panic!("expected InvocationFailure, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Missing capability produces validation error
// ---------------------------------------------------------------------------

#[test]
fn missing_capability_produces_error() {
    let executor = make_executor();
    let spec = make_spec(vec![CapabilityInvocation {
        capability: "nonexistent".to_owned(),
        config: json!({}),
        checkpoint: false,
    }]);

    let input = default_input(json!({}));
    let err = executor.execute(&spec, input, "s", 1).unwrap_err();

    match err {
        CompositionError::Validation(msg) => {
            assert!(msg.contains("nonexistent"), "error message: {msg}");
        }
        other => panic!("expected Validation error, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Validation failure at first step
// ---------------------------------------------------------------------------

#[test]
fn validation_failure_at_first_invocation() {
    let executor = make_executor();
    let spec = make_spec(vec![CapabilityInvocation {
        capability: "validate".to_owned(),
        config: json!({
            "schema": {
                "type": "object",
                "required": ["email"],
                "properties": {"email": {"type": "string"}}
            }
        }),
        checkpoint: false,
    }]);

    // Input missing required "email" field
    let input = default_input(json!({"name": "Alice"}));
    let err = executor.execute(&spec, input, "s", 1).unwrap_err();

    match err {
        CompositionError::InvocationFailure {
            invocation_index, ..
        } => {
            assert_eq!(invocation_index, 0);
        }
        other => panic!("expected InvocationFailure at index 0, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Multiple checkpoints in a single composition
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multiple_checkpoints_collected_in_order() {
    let ops = Arc::new(InMemoryOperations::new(HashMap::new()));
    let provider = Arc::new(InMemoryOperationProvider::new(ops));

    let executor = CompositionExecutor::builder()
        .register("transform", TransformExecutor::new(engine()))
        .register("persist", PersistExecutor::new(engine(), provider))
        .build();

    let spec = make_spec(vec![
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({"filter": "{a: 1}", "output": {"type": "object"}}),
            checkpoint: true, // non-mutating but explicitly marked
        },
        CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": "orders",
                "data": {"expression": ".prev"},
            }),
            checkpoint: true,
        },
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({"filter": "{b: 2}", "output": {"type": "object"}}),
            checkpoint: false,
        },
    ]);

    let input = default_input(json!({}));
    let result = executor.execute(&spec, input, "s", 1).unwrap();

    assert_eq!(result.checkpoints.len(), 2);
    assert_eq!(result.checkpoints[0].completed_invocation_index, 0);
    assert!(!result.checkpoints[0].was_mutation); // transform is non-mutating
    assert_eq!(result.checkpoints[1].completed_invocation_index, 1);
    assert!(result.checkpoints[1].was_mutation); // persist is mutating
}

// ---------------------------------------------------------------------------
// Context and deps are accessible from all invocations
// ---------------------------------------------------------------------------

#[test]
fn context_and_deps_accessible_throughout_chain() {
    let executor = make_executor();
    let spec = make_spec(vec![
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{from_context: .context.origin}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
        CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "filter": "{from_deps: .deps.upstream.value, from_context: .context.origin, prev_origin: .prev.from_context}",
                "output": {"type": "object"}
            }),
            checkpoint: false,
        },
    ]);

    let input = CompositionInput {
        context: json!({"origin": "api"}),
        deps: json!({"upstream": {"value": 42}}),
        step: json!({"name": "s"}),
    };

    let result = executor.execute(&spec, input, "s", 1).unwrap();
    assert_eq!(result.output["from_deps"], json!(42));
    assert_eq!(result.output["from_context"], json!("api"));
    assert_eq!(result.output["prev_origin"], json!("api"));
}

// ---------------------------------------------------------------------------
// Builder and registry queries
// ---------------------------------------------------------------------------

#[test]
fn registered_capabilities_lists_all_names() {
    let executor = make_executor();
    let mut names = executor.registered_capabilities();
    names.sort();
    assert!(names.contains(&"transform"));
    assert!(names.contains(&"validate"));
    assert!(names.contains(&"assert"));
    assert!(names.contains(&"persist"));
}

#[test]
fn has_capability_returns_correct_values() {
    let executor = make_executor();
    assert!(executor.has_capability("transform"));
    assert!(!executor.has_capability("nonexistent"));
}
