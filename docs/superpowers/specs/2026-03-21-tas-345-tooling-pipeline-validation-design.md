# TAS-345: Validate 3 Modeled Workflows Through Composition Tooling Pipeline

**Date**: 2026-03-21
**Ticket**: TAS-345
**Phase**: 2D — Acceptance gate between Phase 2 (validation tooling) and Phase 3 (worker integration)

## Purpose

TAS-345 is the confidence gate for the Action Grammar's validation tooling pipeline. All prerequisite tooling is complete (CompositionValidator, ExplainAnalyzer, SDK wrappers, CLI commands). This ticket validates that the 3 real-world workflows from Phase 1E pass through the tooling cleanly, that the explain traces are structurally accurate, and that intentionally broken compositions produce actionable error messages.

## Approach

Extend `crates/tasker-grammar/tests/workflow_integration.rs` with new tests that exercise the `CompositionValidator`, `ExplainAnalyzer`, and `standard_capability_registry()` directly from `tasker-grammar`. These tests use the same underlying types and logic as the SDK wrapper functions but avoid cross-crate dependency issues (`tasker-sdk` depends on `tasker-grammar`, not the reverse).

No new fixture files — broken compositions are constructed programmatically by mutating existing `CompositionSpec` structs.

## Workflow Reference

| Workflow | Capabilities (in order) | Checkpoints | Invocation count |
|----------|------------------------|-------------|------------------|
| E-commerce order processing | validate, transform, transform, transform, persist, emit | indices 4, 5 | 6 |
| Payment reconciliation | acquire, validate, transform, transform, assert, persist | index 5 | 6 |
| Customer onboarding | acquire, validate, transform, transform, persist, emit | indices 4, 5 | 6 |

## What We're Testing

### 1. Per-Workflow Validate Coverage

For each of the 3 workflows, add a test that validates the fixture's `CompositionSpec` using `CompositionValidator` with `standard_capability_registry()` and asserts:

- `result.is_valid() == true`
- `result.error_count() == 0`
- Zero errors in findings

This differs from the existing `composition_passes_validation()` tests which use a hand-built registry. The new tests exercise the `standard_capability_registry()`, matching what the SDK and CLI use in production.

### 2. Per-Workflow Explain Coverage (Static Mode)

For each workflow, call `ExplainAnalyzer::analyze()` and assert:

- Correct invocation count (all 3 workflows have 6 invocations)
- Correct capability sequence per the Workflow Reference table above
- Correct checkpoint positions per the Workflow Reference table above
- Envelope `.prev` availability: `has_prev == false` for first invocation, `has_prev == true` for all subsequent
- `prev_source` descriptions reference the correct prior invocation (e.g., "output of invocation 0 (validate)")
- Expressions are extracted (non-empty `expressions` vec for invocations that use jaq expressions in their config)
- `simulated == false`
- Zero error-level validation findings

### 3. Per-Workflow Explain Coverage (Simulated Mode)

For each workflow, call `ExplainAnalyzer::analyze_with_simulation()` with representative sample data and assert:

- `simulated == true`
- Transform invocations have `simulated_output` populated (`Some(...)`)
- Side-effecting invocations with mock outputs have `mock_output_used == true`
- Expression references have `simulated_result` populated where expressions were evaluated

**Simulation data sources:**
- `context`: Reuse `CompositionInput.context` from the existing `WorkflowFixture`
- `deps`: `json!({})`
- `step`: `json!({"name": "<step_name>", "attempt": 1})`
- `mock_outputs` per workflow:
  - **E-commerce**: index 4 (persist) → `{"order_id": "ORD-001", "status": "confirmed"}`, index 5 (emit) → `{"event_id": "evt-001", "event_name": "order.confirmed"}`
  - **Reconciliation**: index 0 (acquire) → use the fixture's `acquire_fixtures` data, index 5 (persist) → `{"report_id": "RPT-001", "status": "completed"}`
  - **Onboarding**: index 0 (acquire) → use the fixture's `acquire_fixtures` data, index 4 (persist) → `{"customer_id": "cust-67890", "status": "active"}`, index 5 (emit) → `{"event_id": "evt-002", "event_name": "customer.onboarded"}`

Note: `ExplainAnalyzer::analyze_with_simulation()` is synchronous — no `with_runtime` wrapper needed.

### 4. Negative Validation — Actionable Error Messages

Extend the `negative_validation` module with tests that programmatically mutate valid specs and assert the errors are actionable:

| Scenario | Mutation | Assertion |
|----------|----------|-----------|
| Missing capability | Change a capability name to `"nonexistent_capability"` | Error message contains `"nonexistent_capability"` |
| Schema mismatch | Change a transform's output_schema to produce `"string"` type, followed by a validate expecting `"object"` with required fields | Error references contract/compatibility |
| Invalid expression | Replace a filter expression with `"{invalid syntax [[[" ` | Error includes expression text and parse error details |

Each test verifies:
- `result.is_valid() == false`
- The finding includes enough information for an engineer to fix the problem (capability name, field path, expression text)

Note: The original "invalid input mapping" scenario (referencing `.context.nonexistent_field_zzz`) is dropped. jaq evaluates missing paths as `null` at runtime rather than producing a parse-time error, and the validator performs expression syntax validation but not path existence checking. This check would require runtime simulation, which the explain-with-simulation tests already cover.

### 5. Edge Cases

Folded into the per-workflow explain tests (not separate test functions):

- **Cross-step references**: Verified via `prev_source` assertions in static explain tests
- **Checkpoint detection**: Verified via checkpoint position assertions in static explain tests
- **Output schema threading**: Verify `prev_schema` in envelope snapshots reflects the prior invocation's declared output schema where available

## What We're NOT Doing

- No CLI-level tests (grammar-level coverage is sufficient for the gate)
- No new YAML fixture files (programmatic mutations only)
- No changes to validator, analyzer, or SDK implementation
- No changes to existing tests
- No `tasker-sdk` dependency from `tasker-grammar` tests

## Test Organization

All new tests go in `workflow_integration.rs` (~15 new tests, ~50 total):

```
mod ecommerce {
    // existing 7 tests...
    fn validate_report_with_standard_registry_is_clean()
    fn explain_static_trace_is_accurate()
    fn explain_with_simulation_evaluates_expressions()
}

mod reconciliation {
    // existing 8 tests...
    fn validate_report_with_standard_registry_is_clean()
    fn explain_static_trace_is_accurate()
    fn explain_with_simulation_evaluates_expressions()
}

mod onboarding {
    // existing 8 tests...
    fn validate_report_with_standard_registry_is_clean()
    fn explain_static_trace_is_accurate()
    fn explain_with_simulation_evaluates_expressions()
}

mod negative_validation {
    // existing 4 tests...
    fn missing_capability_produces_actionable_error()
    fn schema_mismatch_produces_actionable_error()
    fn invalid_expression_produces_actionable_error()
}

mod bulk {
    // existing 3 tests...
    fn all_workflows_pass_validation_with_standard_registry()
    fn all_workflows_produce_valid_explain_traces()
}
```

## New Test Infrastructure

Add to the top-level helper section:

```rust
fn standard_registry() -> HashMap<String, CapabilityDeclaration> {
    tasker_grammar::standard_capability_registry()
}

fn validate_with_standard_registry(spec: &CompositionSpec) -> ValidationResult {
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

## Dependencies

- `tasker_grammar::{standard_capability_registry, ExplainAnalyzer, ExpressionEngine, SimulationInput}` — grammar types used directly
- `tasker_grammar::validation::CompositionValidator` — already imported in existing tests
- Existing `tasker_grammar::fixtures` — for workflow specs and test data

## Success Criteria

1. All 3 workflows pass validation with `standard_capability_registry()` (zero errors)
2. `ExplainAnalyzer::analyze()` produces correct invocation traces for all 3 workflows (capability sequence, checkpoints, envelope threading)
3. `ExplainAnalyzer::analyze_with_simulation()` evaluates expressions against sample data with populated outputs
4. Each of the 3 negative scenarios produces an error message actionable enough to fix the problem
5. All ~15 new tests pass alongside the existing 34 tests (~50 total)
