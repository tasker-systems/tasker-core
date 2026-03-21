# TAS-345: Validate 3 Modeled Workflows Through Composition Tooling Pipeline

**Date**: 2026-03-21
**Ticket**: TAS-345
**Phase**: 2D — Acceptance gate between Phase 2 (validation tooling) and Phase 3 (worker integration)

## Purpose

TAS-345 is the confidence gate for the Action Grammar's validation tooling pipeline. All prerequisite tooling is complete (CompositionValidator, ExplainAnalyzer, SDK wrappers, CLI commands). This ticket validates that the 3 real-world workflows from Phase 1E pass through the tooling cleanly, that the explain traces are structurally accurate, and that intentionally broken compositions produce actionable error messages.

## Approach

Extend `crates/tasker-grammar/tests/workflow_integration.rs` with new tests that exercise the SDK-level `validate_composition_yaml()` and `explain_composition()` functions. No new fixture files — broken compositions are constructed programmatically by mutating existing `CompositionSpec` structs.

## What We're Testing

### 1. Per-Workflow Validate Coverage

For each of the 3 workflows (ecommerce, reconciliation, onboarding), add a test that calls `validate_composition_yaml()` on the YAML serialization of the fixture's `CompositionSpec` and asserts:

- `report.valid == true`
- `report.findings` contains zero errors
- `report.summary` matches the "Composition is valid" pattern

This differs from the existing `composition_passes_validation()` tests which use the internal `CompositionValidator` directly with a hand-built registry. The new tests exercise the full SDK path including YAML serialization/deserialization and the `standard_capability_registry()`.

### 2. Per-Workflow Explain Coverage (Static Mode)

For each workflow, call `explain_composition(yaml, None)` and assert:

- Correct invocation count (ecommerce: 6, reconciliation: 6, onboarding: 6)
- Correct capability sequence (e.g., ecommerce: validate, transform, transform, transform, persist, emit)
- Correct checkpoint positions (e.g., ecommerce: indices 4 and 5)
- Envelope `.prev` availability: false for first invocation, true for subsequent
- Expressions are extracted (non-empty `expressions` vec for invocations that use expressions)
- `simulated == false`
- `findings` contains zero errors

### 3. Per-Workflow Explain Coverage (Simulated Mode)

For each workflow, call `explain_composition(yaml, Some(simulation_input))` with representative sample data and assert:

- `simulated == true`
- Transform invocations have `simulated_output` populated (not None)
- Side-effecting invocations with mock outputs have `mock_output_used == true`
- Expression references have `simulated_result` populated where expressions were evaluated

The simulation inputs reuse the same test data from the existing `WorkflowFixture` inputs. Mock outputs for side-effecting capabilities (persist, acquire, emit) are provided as realistic JSON values matching what those operations would return.

### 4. Negative Validation — Actionable Error Messages

Extend the `negative_validation` module with tests that programmatically mutate valid specs and assert the errors are actionable:

| Scenario | Mutation | Assertion |
|----------|----------|-----------|
| Missing capability | Change a capability name to `"nonexistent_capability"` | Error message contains the unknown capability name |
| Schema mismatch | Change a transform's output_schema to an incompatible type | Error references producer/consumer incompatibility |
| Invalid expression | Replace a filter expression with `"{invalid syntax [[[" ` | Error includes the expression text and parse error |
| Invalid input mapping | Replace an expression referencing `.context.X` with `.context.nonexistent_field_zzz` | Warning or info about unresolvable reference |

Each test verifies:
- The report is `valid == false` (for errors) or contains warnings (for softer checks)
- The finding includes enough information for an engineer to fix the problem (capability name, field path, expression text)

### 5. Edge Cases

Additional tests folded into the per-workflow modules:

- **Cross-step references**: Verify explain traces show `.prev` source descriptions that reference the correct prior invocation
- **Checkpoint detection**: Verify explain correctly identifies mutating capabilities with checkpoint markers
- **Output schema threading**: Verify `prev_schema` in envelope snapshots reflects the prior invocation's declared output schema

## What We're NOT Doing

- No CLI-level tests (SDK coverage is sufficient for the gate)
- No new YAML fixture files (programmatic mutations only)
- No changes to validator, analyzer, or SDK implementation
- No changes to existing tests

## Test Organization

All new tests go in `workflow_integration.rs`:

```
mod ecommerce {
    // existing tests...
    fn validate_report_is_clean()
    fn explain_static_trace_is_accurate()
    fn explain_with_simulation_evaluates_expressions()
}

mod reconciliation {
    // existing tests...
    fn validate_report_is_clean()
    fn explain_static_trace_is_accurate()
    fn explain_with_simulation_evaluates_expressions()
}

mod onboarding {
    // existing tests...
    fn validate_report_is_clean()
    fn explain_static_trace_is_accurate()
    fn explain_with_simulation_evaluates_expressions()
}

mod negative_validation {
    // existing tests...
    fn missing_capability_produces_actionable_error()
    fn schema_mismatch_produces_actionable_error()
    fn invalid_expression_produces_actionable_error()
    fn invalid_input_mapping_produces_actionable_error()
}
```

## Dependencies

- `tasker_sdk::grammar_query::{validate_composition_yaml, explain_composition, SimulationInput}` — SDK functions under test
- `serde_yaml` — for serializing `CompositionSpec` to YAML string (round-trip test)
- Existing `tasker_grammar::fixtures` — for workflow specs and test data

## Success Criteria

1. All 3 workflows pass `validate_composition_yaml()` with zero errors
2. `explain_composition()` produces correct invocation traces for all 3 workflows
3. Simulated explain evaluates expressions against sample data
4. Each of the 4 negative scenarios produces an error message actionable enough to fix the problem
5. All new tests pass alongside the existing 34 tests
