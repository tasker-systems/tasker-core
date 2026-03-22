# TAS-345: Tooling Pipeline Validation Tests — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `workflow_integration.rs` with 14 tests that validate the 3 modeled workflows through the composition tooling pipeline (CompositionValidator + ExplainAnalyzer with `standard_capability_registry()`), plus negative tests for actionable error messages. (30 existing + 14 new = 44 total)

**Architecture:** All tests use `tasker_grammar` types directly (no SDK dependency). New helper functions (`validate_with_standard_registry`, `explain_spec`, `explain_spec_with_simulation`) wrap the standard registry + ExplainAnalyzer for reuse. Tests are added to existing per-workflow modules and negative_validation/bulk modules.

**Tech Stack:** Rust, `tasker_grammar` (CompositionValidator, ExplainAnalyzer, ExpressionEngine, standard_capability_registry, SimulationInput), `serde_json`

**Spec:** `docs/superpowers/specs/2026-03-21-tas-345-tooling-pipeline-validation-design.md`

---

### Task 1: Add test infrastructure helpers

**Files:**
- Modify: `crates/tasker-grammar/tests/workflow_integration.rs:1-33` (imports) and `crates/tasker-grammar/tests/workflow_integration.rs:34-59` (helper section)

- [ ] **Step 1: Add new imports and helper functions**

Add these imports alongside the existing ones at the top of the file:

```rust
use tasker_grammar::{
    ExplainAnalyzer, ExplanationTrace, SimulationInput,
    standard_capability_registry,
};
```

Then add these helpers after the existing `validate_spec` function (after line 220):

```rust
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
```

- [ ] **Step 2: Verify the file compiles**

Run: `cargo check --all-features -p tasker-grammar --tests`
Expected: Compiles with zero errors. If there are unused import warnings, that's fine — they'll be used in subsequent tasks.

- [ ] **Step 3: Commit**

```bash
git add crates/tasker-grammar/tests/workflow_integration.rs
git commit -m "test(TAS-345): add standard registry helpers for tooling pipeline tests"
```

---

### Task 2: E-commerce validate + explain tests

**Files:**
- Modify: `crates/tasker-grammar/tests/workflow_integration.rs` — `mod ecommerce` section

- [ ] **Step 1: Write the validate test**

Add to `mod ecommerce` (after the existing `composition_passes_validation` test):

```rust
    #[test]
    fn validate_report_with_standard_registry_is_clean() {
        let WorkflowFixture { spec, .. } = fixture();
        let result = validate_with_standard_registry(&spec);
        assert!(
            result.is_valid(),
            "ecommerce should pass standard registry validation: {:?}",
            result.errors()
        );
        assert_eq!(result.error_count(), 0);
    }
```

- [ ] **Step 2: Write the static explain test**

Add to `mod ecommerce`:

```rust
    #[test]
    fn explain_static_trace_is_accurate() {
        let WorkflowFixture { spec, .. } = fixture();
        let trace = explain_spec(&spec);

        // Correct invocation count
        assert_eq!(trace.invocations.len(), 6);

        // Correct capability sequence
        let capabilities: Vec<&str> = trace
            .invocations
            .iter()
            .map(|inv| inv.capability.as_str())
            .collect();
        assert_eq!(
            capabilities,
            vec!["validate", "transform", "transform", "transform", "persist", "emit"]
        );

        // Correct checkpoint positions (persist=4, emit=5)
        let checkpoints: Vec<usize> = trace
            .invocations
            .iter()
            .filter(|inv| inv.checkpoint)
            .map(|inv| inv.index)
            .collect();
        assert_eq!(checkpoints, vec![4, 5]);

        // Envelope .prev availability — tracks static output_schema propagation:
        // Only Transform sets output_schema. Validate/Assert preserve prev state.
        // Persist/Acquire/Emit reset prev to None.
        // Sequence: validate(no schema), transform(sets), transform(sets), transform(sets), persist(resets), emit(resets)
        assert!(!trace.invocations[0].envelope_available.has_prev, "validate[0]: no prior output");
        assert!(!trace.invocations[1].envelope_available.has_prev, "transform[1]: validate has no output_schema");
        assert!(trace.invocations[2].envelope_available.has_prev, "transform[2]: transform[1] set output_schema");
        assert!(trace.invocations[3].envelope_available.has_prev, "transform[3]: transform[2] set output_schema");
        assert!(trace.invocations[4].envelope_available.has_prev, "persist[4]: transform[3] set output_schema");
        assert!(!trace.invocations[5].envelope_available.has_prev, "emit[5]: persist reset prev");

        // prev_source is set wherever has_prev is true
        assert!(trace.invocations[2].envelope_available.prev_source.is_some());
        assert!(trace.invocations[3].envelope_available.prev_source.is_some());
        assert!(trace.invocations[4].envelope_available.prev_source.is_some());

        // Expressions are extracted for transform invocations (they have filter expressions)
        for idx in [1, 2, 3] {
            assert!(
                !trace.invocations[idx].expressions.is_empty(),
                "transform at index {idx} should have expressions"
            );
        }

        // Not simulated
        assert!(!trace.simulated);

        // No error-level validation findings
        let errors: Vec<_> = trace
            .validation
            .iter()
            .filter(|f| f.severity == tasker_grammar::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }
```

- [ ] **Step 3: Write the simulation explain test**

Add to `mod ecommerce`:

```rust
    #[test]
    fn explain_with_simulation_evaluates_expressions() {
        let WorkflowFixture { spec, input, .. } = fixture();

        let simulation = SimulationInput {
            context: input.context,
            deps: json!({}),
            step: json!({"name": "process_order", "attempt": 1}),
            mock_outputs: HashMap::from([
                (4, json!({"order_id": "ORD-001", "status": "confirmed"})),
                (5, json!({"event_id": "evt-001", "event_name": "order.confirmed"})),
            ]),
        };

        let trace = explain_spec_with_simulation(&spec, &simulation);

        assert!(trace.simulated);

        // Transform invocations should have simulated output
        for idx in [1, 2, 3] {
            assert!(
                trace.invocations[idx].simulated_output.is_some(),
                "transform at index {idx} should have simulated output"
            );
        }

        // Side-effecting invocations with mock outputs should be flagged
        assert!(
            trace.invocations[4].mock_output_used,
            "persist at index 4 should use mock output"
        );
        assert!(
            trace.invocations[5].mock_output_used,
            "emit at index 5 should use mock output"
        );

        // Expression references should have simulated results
        for inv in &trace.invocations {
            for expr in &inv.expressions {
                if !expr.expression.is_empty() {
                    assert!(
                        expr.simulated_result.is_some(),
                        "expression '{}' at invocation {} should have simulated result",
                        expr.expression,
                        inv.index
                    );
                }
            }
        }
    }
```

- [ ] **Step 4: Run the ecommerce tests**

Run: `cargo nextest run --all-features -p tasker-grammar -E 'test(ecommerce)'`
Expected: All ecommerce tests pass (7 existing + 3 new = 10).

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-grammar/tests/workflow_integration.rs
git commit -m "test(TAS-345): add ecommerce validate and explain pipeline tests"
```

---

### Task 3: Reconciliation validate + explain tests

**Files:**
- Modify: `crates/tasker-grammar/tests/workflow_integration.rs` — `mod reconciliation` section

- [ ] **Step 1: Write the validate test**

Add to `mod reconciliation`:

```rust
    #[test]
    fn validate_report_with_standard_registry_is_clean() {
        let WorkflowFixture { spec, .. } = fixture();
        let result = validate_with_standard_registry(&spec);
        assert!(
            result.is_valid(),
            "reconciliation should pass standard registry validation: {:?}",
            result.errors()
        );
        assert_eq!(result.error_count(), 0);
    }
```

- [ ] **Step 2: Write the static explain test**

Add to `mod reconciliation`:

```rust
    #[test]
    fn explain_static_trace_is_accurate() {
        let WorkflowFixture { spec, .. } = fixture();
        let trace = explain_spec(&spec);

        // Correct invocation count
        assert_eq!(trace.invocations.len(), 6);

        // Correct capability sequence
        let capabilities: Vec<&str> = trace
            .invocations
            .iter()
            .map(|inv| inv.capability.as_str())
            .collect();
        assert_eq!(
            capabilities,
            vec!["acquire", "validate", "transform", "transform", "assert", "persist"]
        );

        // Correct checkpoint position (persist=5 only)
        let checkpoints: Vec<usize> = trace
            .invocations
            .iter()
            .filter(|inv| inv.checkpoint)
            .map(|inv| inv.index)
            .collect();
        assert_eq!(checkpoints, vec![5]);

        // Envelope .prev availability — tracks static output_schema propagation:
        // Only Transform sets output_schema. Validate/Assert preserve prev state.
        // Persist/Acquire/Emit reset prev to None.
        // Sequence: acquire(resets), validate(preserves None), transform(sets), transform(sets), assert(preserves), persist(resets)
        assert!(!trace.invocations[0].envelope_available.has_prev, "acquire[0]: no prior output");
        assert!(!trace.invocations[1].envelope_available.has_prev, "validate[1]: acquire reset prev");
        assert!(!trace.invocations[2].envelope_available.has_prev, "transform[2]: validate preserved None");
        assert!(trace.invocations[3].envelope_available.has_prev, "transform[3]: transform[2] set output_schema");
        assert!(trace.invocations[4].envelope_available.has_prev, "assert[4]: transform[3] set output_schema");
        assert!(trace.invocations[5].envelope_available.has_prev, "persist[5]: assert preserved prev");

        // Transform invocations have expressions
        for idx in [2, 3] {
            assert!(
                !trace.invocations[idx].expressions.is_empty(),
                "transform at index {idx} should have expressions"
            );
        }

        // Assert invocation has expressions (filter condition)
        assert!(
            !trace.invocations[4].expressions.is_empty(),
            "assert at index 4 should have expressions"
        );

        assert!(!trace.simulated);

        let errors: Vec<_> = trace
            .validation
            .iter()
            .filter(|f| f.severity == tasker_grammar::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }
```

- [ ] **Step 3: Write the simulation explain test**

Add to `mod reconciliation`. The reconciliation workflow has `acquire` at index 0, so we need a mock output for it. We use the fixture's acquire data:

```rust
    #[test]
    fn explain_with_simulation_evaluates_expressions() {
        let WorkflowFixture {
            spec,
            input,
            acquire_fixtures,
        } = fixture();

        // Build the mock output for acquire (index 0) from fixture data
        let txns = acquire_fixtures
            .get("transactions")
            .cloned()
            .unwrap_or_default();
        let txn_count = txns.len();
        let acquire_output = json!({
            "external_transactions": txns,
            "external_count": txn_count
        });

        let simulation = SimulationInput {
            context: input.context,
            deps: json!({}),
            step: json!({"name": "reconcile_payments", "attempt": 1}),
            mock_outputs: HashMap::from([
                (0, acquire_output),
                (5, json!({"report_id": "RPT-001", "status": "completed"})),
            ]),
        };

        let trace = explain_spec_with_simulation(&spec, &simulation);

        assert!(trace.simulated);

        // Acquire at index 0 should use mock output
        assert!(
            trace.invocations[0].mock_output_used,
            "acquire at index 0 should use mock output"
        );

        // Transform invocations should have simulated output
        for idx in [2, 3] {
            assert!(
                trace.invocations[idx].simulated_output.is_some(),
                "transform at index {idx} should have simulated output"
            );
        }

        // Persist at index 5 should use mock output
        assert!(
            trace.invocations[5].mock_output_used,
            "persist at index 5 should use mock output"
        );
    }
```

- [ ] **Step 4: Run the reconciliation tests**

Run: `cargo nextest run --all-features -p tasker-grammar -E 'test(reconciliation)'`
Expected: All reconciliation tests pass (existing 8 + new 3 = 11).

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-grammar/tests/workflow_integration.rs
git commit -m "test(TAS-345): add reconciliation validate and explain pipeline tests"
```

---

### Task 4: Onboarding validate + explain tests

**Files:**
- Modify: `crates/tasker-grammar/tests/workflow_integration.rs` — `mod onboarding` section

- [ ] **Step 1: Write the validate test**

Add to `mod onboarding`:

```rust
    #[test]
    fn validate_report_with_standard_registry_is_clean() {
        let WorkflowFixture { spec, .. } = fixture();
        let result = validate_with_standard_registry(&spec);
        assert!(
            result.is_valid(),
            "onboarding should pass standard registry validation: {:?}",
            result.errors()
        );
        assert_eq!(result.error_count(), 0);
    }
```

- [ ] **Step 2: Write the static explain test**

Add to `mod onboarding`:

```rust
    #[test]
    fn explain_static_trace_is_accurate() {
        let WorkflowFixture { spec, .. } = fixture();
        let trace = explain_spec(&spec);

        // Correct invocation count
        assert_eq!(trace.invocations.len(), 6);

        // Correct capability sequence
        let capabilities: Vec<&str> = trace
            .invocations
            .iter()
            .map(|inv| inv.capability.as_str())
            .collect();
        assert_eq!(
            capabilities,
            vec!["acquire", "validate", "transform", "transform", "persist", "emit"]
        );

        // Correct checkpoint positions (persist=4, emit=5)
        let checkpoints: Vec<usize> = trace
            .invocations
            .iter()
            .filter(|inv| inv.checkpoint)
            .map(|inv| inv.index)
            .collect();
        assert_eq!(checkpoints, vec![4, 5]);

        // Envelope .prev availability — tracks static output_schema propagation:
        // Only Transform sets output_schema. Validate/Assert preserve prev state.
        // Persist/Acquire/Emit reset prev to None.
        // Sequence: acquire(resets), validate(preserves None), transform(sets), transform(sets), persist(resets), emit(resets)
        assert!(!trace.invocations[0].envelope_available.has_prev, "acquire[0]: no prior output");
        assert!(!trace.invocations[1].envelope_available.has_prev, "validate[1]: acquire reset prev");
        assert!(!trace.invocations[2].envelope_available.has_prev, "transform[2]: validate preserved None");
        assert!(trace.invocations[3].envelope_available.has_prev, "transform[3]: transform[2] set output_schema");
        assert!(trace.invocations[4].envelope_available.has_prev, "persist[4]: transform[3] set output_schema");
        assert!(!trace.invocations[5].envelope_available.has_prev, "emit[5]: persist reset prev");

        // Emit has expressions (payload, metadata)
        assert!(
            !trace.invocations[5].expressions.is_empty(),
            "emit at index 5 should have expressions"
        );

        // Output schema threading: transform invocations should have output_schema
        for idx in [2, 3] {
            assert!(
                trace.invocations[idx].output_schema.is_some(),
                "transform at index {idx} should have output_schema"
            );
        }

        // prev_schema should be set where prior invocation has output_schema
        // After transform at index 2, invocation 3 should see prev_schema
        if trace.invocations[2].output_schema.is_some() {
            assert!(
                trace.invocations[3].envelope_available.prev_schema.is_some(),
                "invocation 3 should see prev_schema from transform at index 2"
            );
        }

        assert!(!trace.simulated);

        let errors: Vec<_> = trace
            .validation
            .iter()
            .filter(|f| f.severity == tasker_grammar::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }
```

- [ ] **Step 3: Write the simulation explain test**

Add to `mod onboarding`:

```rust
    #[test]
    fn explain_with_simulation_evaluates_expressions() {
        let WorkflowFixture {
            spec,
            input,
            acquire_fixtures,
        } = fixture();

        // Build the mock output for acquire (index 0) from fixture data
        let customers = acquire_fixtures
            .get("customers")
            .cloned()
            .unwrap_or_default();
        let acquire_output = if customers.is_empty() {
            json!({"error": "customer not found"})
        } else {
            customers[0].clone()
        };

        let simulation = SimulationInput {
            context: input.context,
            deps: json!({}),
            step: json!({"name": "onboard_customer", "attempt": 1}),
            mock_outputs: HashMap::from([
                (0, acquire_output),
                (4, json!({"customer_id": "cust-67890", "status": "active"})),
                (5, json!({"event_id": "evt-002", "event_name": "customer.onboarded"})),
            ]),
        };

        let trace = explain_spec_with_simulation(&spec, &simulation);

        assert!(trace.simulated);

        // Acquire at index 0 should use mock output
        assert!(
            trace.invocations[0].mock_output_used,
            "acquire at index 0 should use mock output"
        );

        // Transform invocations should have simulated output
        for idx in [2, 3] {
            assert!(
                trace.invocations[idx].simulated_output.is_some(),
                "transform at index {idx} should have simulated output"
            );
        }

        // Persist and emit should use mock outputs
        assert!(
            trace.invocations[4].mock_output_used,
            "persist at index 4 should use mock output"
        );
        assert!(
            trace.invocations[5].mock_output_used,
            "emit at index 5 should use mock output"
        );

        // Emit expressions should have simulated results
        for expr in &trace.invocations[5].expressions {
            if !expr.expression.is_empty() {
                assert!(
                    expr.simulated_result.is_some(),
                    "emit expression '{}' should have simulated result",
                    expr.expression
                );
            }
        }
    }
```

- [ ] **Step 4: Run the onboarding tests**

Run: `cargo nextest run --all-features -p tasker-grammar -E 'test(onboarding)'`
Expected: All onboarding tests pass (existing 8 + new 3 = 11).

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-grammar/tests/workflow_integration.rs
git commit -m "test(TAS-345): add onboarding validate and explain pipeline tests"
```

---

### Task 5: Negative validation tests with actionable errors

**Files:**
- Modify: `crates/tasker-grammar/tests/workflow_integration.rs` — `mod negative_validation` section

- [ ] **Step 1: Write the missing capability actionable error test**

Add to `mod negative_validation`:

```rust
    #[test]
    fn missing_capability_produces_actionable_error() {
        let WorkflowFixture { mut spec, .. } = fixtures::ecommerce_order_processing();
        // Mutate: change first capability to a nonexistent one
        spec.invocations[0].capability = "nonexistent_capability".to_owned();

        let result = validate_with_standard_registry(&spec);
        assert!(!result.is_valid(), "should fail with missing capability");

        let errors = result.errors();
        assert!(
            !errors.is_empty(),
            "should have at least one error for missing capability"
        );

        // Error must name the unknown capability so an engineer can fix it
        let has_actionable_msg = errors.iter().any(|f| {
            f.message.contains("nonexistent_capability")
        });
        assert!(
            has_actionable_msg,
            "error should name the unknown capability 'nonexistent_capability': {:?}",
            errors
        );
    }
```

- [ ] **Step 2: Write the schema mismatch actionable error test**

Add to `mod negative_validation`:

```rust
    #[test]
    fn schema_mismatch_produces_actionable_error() {
        let WorkflowFixture { mut spec, .. } = fixtures::ecommerce_order_processing();
        // Mutate: change transform at index 1's output_schema to produce a string,
        // but next transform at index 2 expects an object with fields
        if let Some(output) = spec.invocations[1].config.get_mut("output") {
            *output = json!({"type": "string"});
        }

        let result = validate_with_standard_registry(&spec);
        // Schema mismatch should produce findings (error or warning)
        let contract_findings: Vec<_> = result
            .findings
            .iter()
            .filter(|f| {
                f.code.contains("CONTRACT")
                    || f.code.contains("COMPAT")
                    || f.code.contains("SCHEMA")
                    || f.message.to_lowercase().contains("compat")
                    || f.message.to_lowercase().contains("contract")
                    || f.message.to_lowercase().contains("schema")
            })
            .collect();
        assert!(
            !contract_findings.is_empty(),
            "should flag schema incompatibility between invocations: findings={:?}",
            result.findings
        );
    }
```

- [ ] **Step 3: Write the invalid expression actionable error test**

Add to `mod negative_validation`:

```rust
    #[test]
    fn invalid_expression_produces_actionable_error() {
        let WorkflowFixture { mut spec, .. } = fixtures::ecommerce_order_processing();
        // Mutate: replace transform filter at index 1 with invalid jaq syntax
        let bad_expr = "{invalid syntax [[[";
        if let Some(filter) = spec.invocations[1].config.get_mut("filter") {
            *filter = json!(bad_expr);
        }

        let result = validate_with_standard_registry(&spec);
        assert!(!result.is_valid(), "should fail with invalid expression");

        let errors = result.errors();
        let has_actionable_msg = errors.iter().any(|f| {
            (f.code.contains("EXPRESSION") || f.message.to_lowercase().contains("expression")
                || f.message.to_lowercase().contains("parse"))
                && (f.message.contains("invalid syntax") || f.message.contains("[[[")
                    || f.field_path.is_some())
        });
        assert!(
            has_actionable_msg,
            "error should reference the bad expression text or field path: {:?}",
            errors
        );
    }
```

- [ ] **Step 4: Run the negative validation tests**

Run: `cargo nextest run --all-features -p tasker-grammar -E 'test(negative_validation)'`
Expected: All negative_validation tests pass (existing 4 + new 3 = 7).

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-grammar/tests/workflow_integration.rs
git commit -m "test(TAS-345): add negative validation tests with actionable error assertions"
```

---

### Task 6: Bulk module tests + final verification

**Files:**
- Modify: `crates/tasker-grammar/tests/workflow_integration.rs` — `mod bulk` section

- [ ] **Step 1: Write bulk validation with standard registry test**

Add to `mod bulk`:

```rust
    #[test]
    fn all_workflows_pass_validation_with_standard_registry() {
        for (name, fixture) in fixtures::all_workflow_fixtures() {
            let result = validate_with_standard_registry(&fixture.spec);
            assert!(
                result.is_valid(),
                "workflow '{name}' failed standard registry validation: {:?}",
                result.errors()
            );
        }
    }
```

- [ ] **Step 2: Write bulk explain trace test**

Add to `mod bulk`:

```rust
    #[test]
    fn all_workflows_produce_valid_explain_traces() {
        for (name, fixture) in fixtures::all_workflow_fixtures() {
            let trace = explain_spec(&fixture.spec);

            // Each workflow has 6 invocations
            assert_eq!(
                trace.invocations.len(),
                6,
                "workflow '{name}' should have 6 invocations"
            );

            // At least 1 checkpoint per workflow
            let checkpoint_count = trace.invocations.iter().filter(|i| i.checkpoint).count();
            assert!(
                checkpoint_count >= 1,
                "workflow '{name}' should have at least 1 checkpoint, got {checkpoint_count}"
            );

            // First invocation has no .prev (no prior output_schema)
            assert!(
                !trace.invocations[0].envelope_available.has_prev,
                "workflow '{name}': first invocation should not have .prev"
            );

            // At least one invocation after the first has .prev
            // (exact pattern depends on capability sequence, tested per-workflow)
            let has_prev_count = trace.invocations[1..]
                .iter()
                .filter(|i| i.envelope_available.has_prev)
                .count();
            assert!(
                has_prev_count >= 1,
                "workflow '{name}': at least one invocation should have .prev, got 0"
            );

            // No error-level findings
            let errors: Vec<_> = trace
                .validation
                .iter()
                .filter(|f| f.severity == tasker_grammar::Severity::Error)
                .collect();
            assert!(
                errors.is_empty(),
                "workflow '{name}' has unexpected errors: {errors:?}"
            );
        }
    }
```

- [ ] **Step 3: Run ALL workflow integration tests**

Run: `cargo nextest run --all-features -p tasker-grammar -E 'test(::ecommerce::) | test(::reconciliation::) | test(::onboarding::) | test(::negative_validation::) | test(::bulk::)'`
Expected: All tests pass (44 total: 30 existing + 14 new).

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --all-targets --all-features -p tasker-grammar`
Expected: Zero warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-grammar/tests/workflow_integration.rs
git commit -m "test(TAS-345): add bulk tooling pipeline validation tests

Complete TAS-345 acceptance gate: all 3 workflows pass validation and
explain trace analysis with standard_capability_registry(). Negative
tests verify actionable error messages for missing capabilities,
schema mismatches, and invalid expressions."
```
