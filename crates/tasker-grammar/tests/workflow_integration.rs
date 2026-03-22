//! End-to-end composition integration tests for the 3 modeled workflows (TAS-336).
//!
//! This is the Phase 1 acceptance gate. It proves that:
//! - All 6 capability executors compose correctly via the `CompositionExecutor`
//! - The `CompositionValidator` validates each workflow with zero errors
//! - Checkpoint boundaries are created at the correct mutation points
//! - Checkpoint resume produces the same final output
//! - Negative test cases are caught by the validator
//!
//! These tests run with NO infrastructure — pure data, no database, no messaging.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};

use tasker_grammar::capabilities::acquire::AcquireExecutor;
use tasker_grammar::capabilities::assert::AssertExecutor;
use tasker_grammar::capabilities::emit::EmitExecutor;
use tasker_grammar::capabilities::persist::PersistExecutor;
use tasker_grammar::capabilities::transform::TransformExecutor;
use tasker_grammar::capabilities::validate::ValidateExecutor;
use tasker_grammar::executor::CompositionExecutor;
use tasker_grammar::fixtures::{self, WorkflowFixture};
use tasker_grammar::types::{
    CapabilityDeclaration, CapabilityInvocation, CompositionError, CompositionSpec,
    GrammarCategoryKind, MutationProfile, OutcomeDeclaration,
};
use tasker_grammar::validation::CompositionValidator;
use tasker_grammar::{
    standard_capability_registry, ExplainAnalyzer, ExplanationTrace, ExpressionEngine,
    InMemoryOperationProvider, InMemoryOperations, OperationProvider, SimulationInput,
};

// ---------------------------------------------------------------------------
// Test infrastructure
// ---------------------------------------------------------------------------

fn engine() -> ExpressionEngine {
    ExpressionEngine::with_defaults()
}

fn make_executor(
    acquire_fixtures: HashMap<String, Vec<Value>>,
) -> (CompositionExecutor, Arc<InMemoryOperations>) {
    let ops = Arc::new(InMemoryOperations::new(acquire_fixtures));
    let provider =
        Arc::new(InMemoryOperationProvider::new(ops.clone())) as Arc<dyn OperationProvider>;

    let executor = CompositionExecutor::builder()
        .register("transform", TransformExecutor::new(engine()))
        .register("validate", ValidateExecutor::new())
        .register("assert", AssertExecutor::new(engine()))
        .register("persist", PersistExecutor::new(engine(), provider.clone()))
        .register("acquire", AcquireExecutor::new(engine(), provider.clone()))
        .register("emit", EmitExecutor::new(engine(), provider))
        .build();

    (executor, ops)
}

/// Run a closure within a tokio runtime.
/// Persist/acquire/emit executors use `block_in_place` which requires a tokio context.
fn with_runtime<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async { tokio::task::block_in_place(f) })
}

fn make_registry() -> HashMap<String, CapabilityDeclaration> {
    let mut registry = HashMap::new();

    registry.insert(
        "transform".to_owned(),
        CapabilityDeclaration {
            name: "transform".to_owned(),
            grammar_category: GrammarCategoryKind::Transform,
            description: "Pure data transformation".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["output", "filter"],
                "properties": {
                    "output": { "type": "object" },
                    "filter": { "type": "string" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "validate".to_owned(),
        CapabilityDeclaration {
            name: "validate".to_owned(),
            grammar_category: GrammarCategoryKind::Validate,
            description: "JSON Schema validation".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["schema"],
                "properties": {
                    "schema": { "type": "object" },
                    "on_failure": { "type": "string" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "assert".to_owned(),
        CapabilityDeclaration {
            name: "assert".to_owned(),
            grammar_category: GrammarCategoryKind::Assert,
            description: "Execution gate".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["filter", "error"],
                "properties": {
                    "filter": { "type": "string" },
                    "error": { "type": "string" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "acquire".to_owned(),
        CapabilityDeclaration {
            name: "acquire".to_owned(),
            grammar_category: GrammarCategoryKind::Acquire,
            description: "Fetch data".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["resource"],
                "properties": {
                    "resource": {},
                    "params": { "type": "object" },
                    "constraints": { "type": "object" },
                    "validate_success": { "type": "object" },
                    "result_shape": { "type": "object" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "persist".to_owned(),
        CapabilityDeclaration {
            name: "persist".to_owned(),
            grammar_category: GrammarCategoryKind::Persist,
            description: "Write state".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["resource", "data"],
                "properties": {
                    "resource": {},
                    "data": {},
                    "constraints": { "type": "object" },
                    "mode": { "type": "string" },
                    "identity": { "type": "object" },
                    "validate_success": { "type": "object" },
                    "result_shape": { "type": "object" }
                }
            }),
            mutation_profile: MutationProfile::Mutating {
                supports_idempotency_key: true,
            },
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "emit".to_owned(),
        CapabilityDeclaration {
            name: "emit".to_owned(),
            grammar_category: GrammarCategoryKind::Emit,
            description: "Fire domain events".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["event_name", "payload"],
                "properties": {
                    "event_name": { "type": "string" },
                    "payload": { "type": "object" },
                    "condition": { "type": "string" },
                    "metadata": { "type": "object" },
                    "event_version": { "type": "string" },
                    "resource": { "type": "string" },
                    "result_shape": { "type": "object" },
                    "validate_success": { "type": "object" }
                }
            }),
            mutation_profile: MutationProfile::Mutating {
                supports_idempotency_key: true,
            },
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry
}

fn validate_spec(spec: &CompositionSpec) -> tasker_grammar::validation::ValidationResult {
    let registry = make_registry();
    let e = engine();
    let validator = CompositionValidator::new(&registry, &e);
    validator.validate(spec)
}

fn standard_registry() -> HashMap<String, CapabilityDeclaration> {
    standard_capability_registry()
}

fn validate_with_standard_registry(
    spec: &CompositionSpec,
) -> tasker_grammar::validation::ValidationResult {
    let registry = standard_registry();
    let e = engine();
    let validator = CompositionValidator::new(&registry, &e);
    validator.validate(spec)
}

fn explain_spec(spec: &CompositionSpec) -> ExplanationTrace {
    let registry = standard_registry();
    let e = engine();
    let analyzer = ExplainAnalyzer::new(&registry, &e);
    analyzer.analyze(spec)
}

fn explain_spec_with_simulation(
    spec: &CompositionSpec,
    simulation: &SimulationInput,
) -> ExplanationTrace {
    let registry = standard_registry();
    let e = engine();
    let analyzer = ExplainAnalyzer::new(&registry, &e);
    analyzer.analyze_with_simulation(spec, simulation)
}

// ===========================================================================
// WORKFLOW 1: E-commerce Order Processing
// ===========================================================================

mod ecommerce {
    use super::*;

    fn fixture() -> WorkflowFixture {
        fixtures::ecommerce_order_processing()
    }

    #[test]
    fn execution_produces_expected_output() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _ops) = make_executor(acquire_fixtures);
            let result = executor
                .execute(&spec, input, "process_order", 1)
                .expect("e-commerce composition should execute successfully");

            let output = &result.output;
            assert!(
                output.get("event_id").is_some(),
                "missing event_id: {output}"
            );
            assert_eq!(
                output.get("event_name").and_then(Value::as_str),
                Some("order.confirmed"),
            );
        });
    }

    #[test]
    fn intermediate_outputs_are_correct() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _ops) = make_executor(acquire_fixtures);
            let result = executor
                .execute(&spec, input, "process_order", 1)
                .expect("execution should succeed");

            // Check first checkpoint (persist at index 4)
            let persist_cp = &result.checkpoints[0];
            assert_eq!(persist_cp.completed_invocation_index, 4);
            let all = &persist_cp.all_invocation_outputs;

            // Index 1: reshape — line_items with computed totals
            let reshape = all.get(&1).expect("missing output for index 1");
            let line_items = reshape["line_items"]
                .as_array()
                .expect("missing line_items");
            assert_eq!(line_items.len(), 3);
            let first_total = line_items[0]["line_total"].as_f64().unwrap();
            assert!(
                (first_total - 89.97).abs() < 0.01,
                "first line total: {first_total}"
            );

            // Index 2: totals
            let totals = all.get(&2).expect("missing output for index 2");
            let subtotal = totals["subtotal"].as_f64().unwrap();
            assert!((subtotal - 304.91).abs() < 0.01, "subtotal: {subtotal}");
            let shipping = totals["shipping"].as_f64().unwrap();
            assert!(
                shipping.abs() < 0.01,
                "free shipping for >= 100: {shipping}"
            );

            // Index 3: routing
            let routing = &all.get(&3).expect("missing output for index 3")["routing"];
            assert_eq!(routing["priority"].as_str(), Some("normal"));
            assert_eq!(routing["warehouse"].as_str(), Some("west"));
            assert_eq!(routing["fraud_review"].as_bool(), Some(false));
        });
    }

    #[test]
    fn cross_step_references_resolve_correctly() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, ops) = make_executor(acquire_fixtures);
            let _result = executor
                .execute(&spec, input, "process_order", 1)
                .expect("execution should succeed");

            // Emit payload references .context.customer_id and .prev.order_id
            let rt = tokio::runtime::Handle::current();
            let emitted = rt.block_on(ops.captured_emits());
            assert_eq!(emitted.len(), 1);
            assert_eq!(
                emitted[0]
                    .payload
                    .get("customer_id")
                    .and_then(Value::as_str),
                Some("cust-12345"),
            );
            assert!(emitted[0].payload.get("order_id").is_some());

            // Persist data references .prev from routing transform
            let persisted = rt.block_on(ops.captured_persists());
            assert_eq!(persisted.len(), 1);
            assert_eq!(persisted[0].data["status"].as_str(), Some("confirmed"));
        });
    }

    #[test]
    fn checkpoints_created_at_mutation_boundaries() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _ops) = make_executor(acquire_fixtures);
            let result = executor
                .execute(&spec, input, "process_order", 1)
                .expect("execution should succeed");

            assert_eq!(result.checkpoints.len(), 2, "persist + emit checkpoints");

            assert_eq!(result.checkpoints[0].completed_invocation_index, 4);
            assert_eq!(result.checkpoints[0].completed_capability, "persist");
            assert!(result.checkpoints[0].was_mutation);

            assert_eq!(result.checkpoints[1].completed_invocation_index, 5);
            assert_eq!(result.checkpoints[1].completed_capability, "emit");
            assert!(result.checkpoints[1].was_mutation);
        });
    }

    #[test]
    fn checkpoint_resume_produces_same_final_output() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _) = make_executor(acquire_fixtures.clone());
            let full = executor
                .execute(&spec, input.clone(), "process_order", 1)
                .expect("full execution should succeed");

            // Resume from persist checkpoint — should only run emit
            let (executor2, _) = make_executor(acquire_fixtures);
            let resumed = executor2
                .resume(&spec, &full.checkpoints[0], &input, "process_order", 2)
                .expect("resume should succeed");

            assert_eq!(resumed.output["event_name"], full.output["event_name"]);
            assert!(resumed.output.get("event_id").is_some());
            assert_eq!(
                resumed.checkpoints.len(),
                1,
                "only emit checkpoint after resume"
            );
        });
    }

    #[test]
    fn composition_passes_validation() {
        let WorkflowFixture { spec, .. } = fixture();
        let result = validate_spec(&spec);
        assert!(result.is_valid(), "errors: {:?}", result.errors());
    }

    #[test]
    fn empty_items_fails_validation_step() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                acquire_fixtures,
                ..
            } = fixture();
            let invalid = fixtures::ecommerce_order_processing_invalid_empty_items();

            let (executor, _) = make_executor(acquire_fixtures);
            let err = executor
                .execute(&spec, invalid, "process_order", 1)
                .expect_err("empty items should fail");

            match err {
                CompositionError::InvocationFailure {
                    invocation_index,
                    capability,
                    ..
                } => {
                    assert_eq!(invocation_index, 0);
                    assert_eq!(capability, "validate");
                }
                other => panic!("expected InvocationFailure, got: {other:?}"),
            }
        });
    }

    #[test]
    fn missing_address_fails_validation_step() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                acquire_fixtures,
                ..
            } = fixture();
            let invalid = fixtures::ecommerce_order_processing_invalid_missing_address();

            let (executor, _) = make_executor(acquire_fixtures);
            let err = executor
                .execute(&spec, invalid, "process_order", 1)
                .expect_err("missing address should fail");

            match err {
                CompositionError::InvocationFailure {
                    invocation_index,
                    capability,
                    ..
                } => {
                    assert_eq!(invocation_index, 0);
                    assert_eq!(capability, "validate");
                }
                other => panic!("expected InvocationFailure, got: {other:?}"),
            }
        });
    }
}

// ===========================================================================
// WORKFLOW 2: Payment Reconciliation
// ===========================================================================

mod reconciliation {
    use super::*;

    fn fixture() -> WorkflowFixture {
        fixtures::payment_reconciliation()
    }

    #[test]
    fn execution_produces_expected_output() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _) = make_executor(acquire_fixtures);
            let result = executor
                .execute(&spec, input, "reconcile_payments", 1)
                .expect("reconciliation should succeed");

            let output = &result.output;
            assert_eq!(output["matched_count"].as_i64(), Some(3));
            assert_eq!(output["unmatched_count"].as_i64(), Some(1));
            assert_eq!(output["status"].as_str(), Some("completed"));
        });
    }

    #[test]
    fn intermediate_matching_and_variance_are_correct() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _) = make_executor(acquire_fixtures);
            let result = executor
                .execute(&spec, input, "reconcile_payments", 1)
                .expect("execution should succeed");

            let cp = &result.checkpoints[0];
            let all = &cp.all_invocation_outputs;

            // Index 2: matching
            let matching = all.get(&2).expect("missing matching output");
            assert_eq!(matching["matched"].as_array().unwrap().len(), 3);
            assert_eq!(matching["unmatched"].as_array().unwrap().len(), 1);

            // Index 3: discrepancies — variance = 250.50 - 250.00 = 0.50
            let disc = all.get(&3).expect("missing discrepancies output");
            let variance = disc["total_variance"].as_f64().unwrap();
            assert!(
                (variance - 0.50).abs() < 0.01,
                "variance should be ~0.50, got {variance}"
            );
        });
    }

    #[test]
    fn acquire_data_flows_through_pipeline() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, ops) = make_executor(acquire_fixtures);
            let _ = executor
                .execute(&spec, input, "reconcile_payments", 1)
                .expect("execution should succeed");

            let rt = tokio::runtime::Handle::current();
            let persisted = rt.block_on(ops.captured_persists());
            assert_eq!(persisted.len(), 1);
            assert_eq!(persisted[0].data["status"].as_str(), Some("completed"));
            assert_eq!(
                persisted[0].data["reconciliation_date"].as_str(),
                Some("2026-03-10"),
            );
        });
    }

    #[test]
    fn checkpoints_created_at_persist_only() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _) = make_executor(acquire_fixtures);
            let result = executor
                .execute(&spec, input, "reconcile_payments", 1)
                .expect("execution should succeed");

            assert_eq!(result.checkpoints.len(), 1, "only persist checkpoint");
            assert_eq!(result.checkpoints[0].completed_invocation_index, 5);
            assert_eq!(result.checkpoints[0].completed_capability, "persist");
            assert!(result.checkpoints[0].was_mutation);
        });
    }

    #[test]
    fn checkpoint_resume_at_final_step_returns_output() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _) = make_executor(acquire_fixtures.clone());
            let full = executor
                .execute(&spec, input.clone(), "reconcile_payments", 1)
                .expect("full execution should succeed");

            // Resume from last checkpoint (persist is final invocation)
            let (executor2, _) = make_executor(acquire_fixtures);
            let resumed = executor2
                .resume(&spec, &full.checkpoints[0], &input, "reconcile_payments", 2)
                .expect("resume should succeed");

            assert_eq!(
                resumed.output["matched_count"],
                full.output["matched_count"]
            );
            assert_eq!(
                resumed.output["total_variance"],
                full.output["total_variance"]
            );
        });
    }

    #[test]
    fn composition_passes_validation() {
        let WorkflowFixture { spec, .. } = fixture();
        let result = validate_spec(&spec);
        assert!(result.is_valid(), "errors: {:?}", result.errors());
    }

    #[test]
    fn large_variance_fails_assert() {
        with_runtime(|| {
            let WorkflowFixture { spec, .. } = fixture();
            let (input, acquire) = fixtures::payment_reconciliation_large_variance();

            let (executor, _) = make_executor(acquire);
            let err = executor
                .execute(&spec, input, "reconcile_payments", 1)
                .expect_err("large variance should fail assert");

            match err {
                CompositionError::InvocationFailure {
                    invocation_index,
                    capability,
                    ..
                } => {
                    assert_eq!(invocation_index, 4);
                    assert_eq!(capability, "assert");
                }
                other => panic!("expected InvocationFailure at assert, got: {other:?}"),
            }
        });
    }

    #[test]
    fn too_many_unmatched_fails_assert() {
        with_runtime(|| {
            let WorkflowFixture { spec, .. } = fixture();
            let (input, acquire) = fixtures::payment_reconciliation_too_many_unmatched();

            let (executor, _) = make_executor(acquire);
            let err = executor
                .execute(&spec, input, "reconcile_payments", 1)
                .expect_err("too many unmatched should fail assert");

            match err {
                CompositionError::InvocationFailure {
                    invocation_index,
                    capability,
                    ..
                } => {
                    assert_eq!(invocation_index, 4);
                    assert_eq!(capability, "assert");
                }
                other => panic!("expected InvocationFailure at assert, got: {other:?}"),
            }
        });
    }
}

// ===========================================================================
// WORKFLOW 3: Customer Onboarding
// ===========================================================================

mod onboarding {
    use super::*;

    fn fixture() -> WorkflowFixture {
        fixtures::customer_onboarding()
    }

    #[test]
    fn execution_produces_expected_output() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _) = make_executor(acquire_fixtures);
            let result = executor
                .execute(&spec, input, "onboard_customer", 1)
                .expect("onboarding should succeed");

            let output = &result.output;
            assert!(output.get("event_id").is_some());
            assert_eq!(output["event_name"].as_str(), Some("customer.onboarded"));
        });
    }

    #[test]
    fn tier_classification_is_correct() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _) = make_executor(acquire_fixtures);
            let result = executor
                .execute(&spec, input, "onboard_customer", 1)
                .expect("execution should succeed");

            let all = &result.checkpoints[0].all_invocation_outputs;

            // Index 2: tier classification — gold for $7500/25 purchases
            let tier = all.get(&2).expect("missing tier output");
            assert_eq!(tier["tier"].as_str(), Some("gold"));
            assert_eq!(tier["discount_pct"].as_i64(), Some(15));
            assert_eq!(tier["tier_benefits"]["free_shipping"].as_bool(), Some(true));
            assert_eq!(
                tier["tier_benefits"]["priority_support"].as_bool(),
                Some(false)
            );

            // Index 3: reshaped profile
            let reshaped = all.get(&3).expect("missing reshaped output");
            assert_eq!(reshaped["display_name"].as_str(), Some("Jane Doe"));
            assert_eq!(reshaped["onboarding_status"].as_str(), Some("completed"));
            assert_eq!(
                reshaped["onboarded_at"].as_str(),
                Some("2026-03-10T08:30:00Z"),
            );
        });
    }

    #[test]
    fn emit_references_persist_output_correctly() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, ops) = make_executor(acquire_fixtures);
            let _ = executor
                .execute(&spec, input, "onboard_customer", 1)
                .expect("execution should succeed");

            let rt = tokio::runtime::Handle::current();
            let emitted = rt.block_on(ops.captured_emits());
            assert_eq!(emitted.len(), 1);

            let payload = &emitted[0].payload;
            assert_eq!(payload["customer_id"].as_str(), Some("cust-67890"));
            assert_eq!(payload["tier"].as_str(), Some("gold"));
            assert_eq!(payload["display_name"].as_str(), Some("Jane Doe"));

            let metadata = &emitted[0].metadata;
            assert_eq!(
                metadata.idempotency_key.as_deref(),
                Some("onboard-cust-67890")
            );
            assert_eq!(metadata.correlation_id.as_deref(), Some("cust-67890"));
        });
    }

    #[test]
    fn checkpoints_created_at_persist_and_emit() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _) = make_executor(acquire_fixtures);
            let result = executor
                .execute(&spec, input, "onboard_customer", 1)
                .expect("execution should succeed");

            assert_eq!(result.checkpoints.len(), 2, "persist + emit checkpoints");
            assert_eq!(result.checkpoints[0].completed_invocation_index, 4);
            assert_eq!(result.checkpoints[0].completed_capability, "persist");
            assert_eq!(result.checkpoints[1].completed_invocation_index, 5);
            assert_eq!(result.checkpoints[1].completed_capability, "emit");
        });
    }

    #[test]
    fn checkpoint_resume_from_persist_produces_correct_output() {
        with_runtime(|| {
            let WorkflowFixture {
                spec,
                input,
                acquire_fixtures,
            } = fixture();

            let (executor, _) = make_executor(acquire_fixtures.clone());
            let full = executor
                .execute(&spec, input.clone(), "onboard_customer", 1)
                .expect("full execution should succeed");

            let (executor2, _) = make_executor(acquire_fixtures);
            let resumed = executor2
                .resume(&spec, &full.checkpoints[0], &input, "onboard_customer", 2)
                .expect("resume should succeed");

            assert_eq!(resumed.output["event_name"], full.output["event_name"]);
            assert!(resumed.output.get("event_id").is_some());
        });
    }

    #[test]
    fn composition_passes_validation() {
        let WorkflowFixture { spec, .. } = fixture();
        let result = validate_spec(&spec);
        assert!(result.is_valid(), "errors: {:?}", result.errors());
    }

    #[test]
    fn incomplete_profile_fails_validation_step() {
        with_runtime(|| {
            let WorkflowFixture { spec, .. } = fixture();
            let (input, acquire) = fixtures::customer_onboarding_incomplete_profile();

            let (executor, _) = make_executor(acquire);
            let err = executor
                .execute(&spec, input, "onboard_customer", 1)
                .expect_err("incomplete profile should fail");

            match err {
                CompositionError::InvocationFailure {
                    invocation_index,
                    capability,
                    ..
                } => {
                    assert_eq!(invocation_index, 1, "should fail at validate (index 1)");
                    assert_eq!(capability, "validate");
                }
                other => panic!("expected InvocationFailure at validate, got: {other:?}"),
            }
        });
    }
}

// ===========================================================================
// CROSS-WORKFLOW: Negative validation tests (Test scenarios 7-10)
// ===========================================================================

mod negative_validation {
    use super::*;

    #[test]
    fn missing_capability_caught_by_validator() {
        let spec = CompositionSpec {
            name: Some("missing_cap_test".to_owned()),
            outcome: OutcomeDeclaration {
                description: "Test".to_owned(),
                output_schema: json!({"type": "object"}),
            },
            invocations: vec![CapabilityInvocation {
                capability: "nonexistent_capability".to_owned(),
                config: json!({}),
                checkpoint: false,
            }],
        };

        let result = validate_spec(&spec);
        assert!(!result.is_valid(), "should fail with missing capability");
        assert!(
            result
                .errors()
                .iter()
                .any(|f| f.message.contains("nonexistent_capability")
                    || f.code.contains("MISSING")
                    || f.code.contains("UNKNOWN")),
            "error should reference unknown capability: {:?}",
            result.errors()
        );
    }

    #[test]
    fn schema_mismatch_caught_by_validator() {
        let spec = CompositionSpec {
            name: Some("schema_mismatch_test".to_owned()),
            outcome: OutcomeDeclaration {
                description: "Test".to_owned(),
                output_schema: json!({"type": "object"}),
            },
            invocations: vec![
                CapabilityInvocation {
                    capability: "transform".to_owned(),
                    config: json!({
                        "filter": "\"hello\"",
                        "output": {"type": "string"}
                    }),
                    checkpoint: false,
                },
                CapabilityInvocation {
                    capability: "validate".to_owned(),
                    config: json!({
                        "schema": {
                            "type": "object",
                            "required": ["name"],
                            "properties": { "name": {"type": "string"} }
                        }
                    }),
                    checkpoint: false,
                },
            ],
        };

        let result = validate_spec(&spec);
        assert!(
            !result.findings.is_empty(),
            "schema mismatch should produce findings"
        );
    }

    #[test]
    fn invalid_jaq_expression_caught_by_validator() {
        let spec = CompositionSpec {
            name: Some("invalid_expr_test".to_owned()),
            outcome: OutcomeDeclaration {
                description: "Test".to_owned(),
                output_schema: json!({"type": "object"}),
            },
            invocations: vec![CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "filter": "{invalid syntax [[[",
                    "output": {"type": "object"}
                }),
                checkpoint: false,
            }],
        };

        let result = validate_spec(&spec);
        assert!(!result.is_valid(), "invalid expression should fail");
        assert!(
            result.errors().iter().any(|f| f.code.contains("EXPRESSION")
                || f.message.contains("expression")
                || f.message.contains("parse")),
            "should mention expression/parse: {:?}",
            result.errors()
        );
    }

    #[test]
    fn unchecked_mutation_produces_warning() {
        let spec = CompositionSpec {
            name: Some("unchecked_mutation_test".to_owned()),
            outcome: OutcomeDeclaration {
                description: "Test".to_owned(),
                output_schema: json!({"type": "object"}),
            },
            invocations: vec![CapabilityInvocation {
                capability: "persist".to_owned(),
                config: json!({
                    "resource": "test-db",
                    "data": { "expression": ".prev" }
                }),
                checkpoint: false,
            }],
        };

        let result = validate_spec(&spec);
        assert!(
            result
                .findings
                .iter()
                .any(|f| f.message.to_lowercase().contains("checkpoint")
                    || f.code.contains("CHECKPOINT")),
            "should flag missing checkpoint: {:?}",
            result.findings
        );
    }
}

// ===========================================================================
// ALL-WORKFLOWS: Bulk fixture tests
// ===========================================================================

mod bulk {
    use super::*;

    #[test]
    fn all_workflow_fixtures_load_successfully() {
        let all = fixtures::all_workflow_fixtures();
        assert_eq!(all.len(), 3);
        for (name, fixture) in &all {
            assert!(
                fixture.spec.invocations.len() >= 5,
                "{name} too few invocations"
            );
            assert!(fixture.spec.name.is_some(), "{name} missing name");
        }
    }

    #[test]
    fn all_workflows_execute_successfully() {
        with_runtime(|| {
            for (name, fixture) in fixtures::all_workflow_fixtures() {
                let (executor, _) = make_executor(fixture.acquire_fixtures);
                let step = fixture.spec.name.as_deref().unwrap_or("test");
                let result = executor.execute(&fixture.spec, fixture.input, step, 1);
                assert!(
                    result.is_ok(),
                    "workflow '{name}' failed: {:?}",
                    result.err()
                );
            }
        });
    }

    #[test]
    fn all_workflows_pass_validation() {
        for (name, fixture) in fixtures::all_workflow_fixtures() {
            let result = validate_spec(&fixture.spec);
            assert!(
                result.is_valid(),
                "workflow '{name}' validation errors: {:?}",
                result.errors()
            );
        }
    }
}
