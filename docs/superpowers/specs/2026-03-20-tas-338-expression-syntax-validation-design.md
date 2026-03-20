# TAS-338: Expression Syntax Validation for Composition Templates

**Date**: 2026-03-20
**Status**: Approved
**Ticket**: TAS-338
**Roadmap Lane**: 3C (Validation Tooling)
**Predecessor**: TAS-321 (jaq-core expression engine), TAS-333 (CompositionValidator)
**Related**: TAS-337 (composition-aware template validator)

---

## Problem

The `CompositionValidator::check_expression_syntax` method validates jaq expression
syntax in composition config fields, but only checks **flat string** fields (`data`,
`params`, `payload`, `condition`, `filter`). Three capabilities — persist, acquire,
and emit — also accept jaq expressions in **nested `ExpressionField` objects**
(`validate_success` and `result_shape`), which have the shape
`{"expression": "<jaq-expression>"}`. These expressions are currently unchecked at
design time, meaning syntax errors are only caught at runtime.

All three workflow fixtures (ecommerce, payment reconciliation, customer onboarding)
use `validate_success` and `result_shape` expressions. This is a meaningful
validation gap.

## Scope

Extend `check_expression_syntax` in `crates/tasker-grammar/src/validation/validator.rs`
to validate jaq expressions in nested `ExpressionField` objects alongside existing
flat string fields. No SDK changes required — the SDK already bridges all grammar-level
findings through `validate_composition()`.

### Out of Scope

- Expression variable resolution (TAS-341: checks whether `.prev.field` actually exists)
- Expression result type validation (semantic analysis beyond syntax)
- New capability categories or config field additions

## Design

### Expression extraction helper

Add a private helper function to extract a jaq expression string from a config field
value, handling both shapes that appear in capability configs:

| Config shape | Example | Extracted expression |
|-------------|---------|---------------------|
| Flat string | `"data": ".prev.order_id"` | `".prev.order_id"` |
| ExpressionField object | `"validate_success": {"expression": ".affected_rows > 0"}` | `".affected_rows > 0"` |
| Non-expression value | `"mode": "upsert"` | `None` (skip) |

The helper returns `Option<&str>` — the jaq expression if one is found, `None` otherwise.
This keeps the main loop clean: iterate fields, extract expression, validate if present.

### Extended field lists

The per-category field lists in `check_expression_syntax` grow to include the nested
expression fields:

| Category | Fields checked |
|----------|---------------|
| Transform | `filter` |
| Assert | `filter` |
| Validate | *(none)* |
| Persist | `data`, `validate_success`, `result_shape` |
| Acquire | `params`, `validate_success`, `result_shape` |
| Emit | `payload`, `condition`, `validate_success`, `result_shape` |

### Field path in findings

For flat string fields, `field_path` remains `config.<field>` (e.g., `config.data`).
For nested ExpressionField objects, `field_path` becomes `config.<field>.expression`
(e.g., `config.validate_success.expression`) to precisely identify the location.

### Error format

Unchanged. Same `INVALID_EXPRESSION` error code, same message pattern:
```
invocation {idx} ({capability}) has invalid jaq expression in '{field}': {parse_error}
```

## Files Changed

| File | Change |
|------|--------|
| `crates/tasker-grammar/src/validation/validator.rs` | Add expression extraction helper, extend field lists in `check_expression_syntax` |
| `crates/tasker-grammar/src/validation/tests.rs` | Add test cases for nested expression field validation |

## Test Plan

**New tests** (all in `crates/tasker-grammar/src/validation/tests.rs`):

1. `persist_validate_success_expression_validated` — invalid expression in persist validate_success produces `INVALID_EXPRESSION` error
2. `persist_result_shape_expression_validated` — invalid expression in persist result_shape produces error
3. `acquire_validate_success_expression_validated` — invalid expression in acquire validate_success produces error
4. `acquire_result_shape_expression_validated` — invalid expression in acquire result_shape produces error
5. `emit_validate_success_expression_validated` — invalid expression in emit validate_success produces error
6. `emit_result_shape_expression_validated` — invalid expression in emit result_shape produces error
7. `valid_nested_expressions_pass` — valid expressions in ExpressionField objects produce no findings

**Existing coverage** (unchanged, confirms no regressions):

- `invalid_jaq_expression_produces_error` — transform filter
- `valid_jaq_expression_passes` — transform filter
- `assert_filter_syntax_validated` — assert filter
- `persist_data_expression_validated` — persist data
- `emit_payload_expression_validated` — emit payload
- `emit_condition_expression_validated` — emit condition
- Workflow fixture integration tests (3 fixtures validated end-to-end via TAS-337)

## Verification

```bash
# Grammar tests pass
cargo test --package tasker-grammar --all-features

# Full workspace compiles
cargo check --all-features --workspace

# Zero clippy warnings
cargo clippy --all-targets --all-features --workspace
```
