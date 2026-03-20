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
syntax in composition config fields, but only handles **flat string** values
(`if let Some(Value::String(expr)) = invocation.config.get(field)`). In real
composition configs, **all** expression-bearing fields use the `ExpressionField`
object shape `{"expression": "<jaq-expression>"}` — including `data`, `params`,
`payload`, and `condition`, not just `validate_success` and `result_shape`.

This means the existing flat-string checks are vacuous for real workflow configs.
The existing validator tests pass only because they use flat strings in test data
that don't match the actual `ExpressionField` shape used by the typed executor
configs and all three workflow fixtures.

Additionally, emit's `metadata.correlation_id` and `metadata.idempotency_key`
contain jaq expressions but are not checked at all.

## Scope

Rewrite `check_expression_syntax` in `crates/tasker-grammar/src/validation/validator.rs`
to extract jaq expressions from both flat strings and `ExpressionField` objects, add
`validate_success` and `result_shape` to the per-category field lists, and add
validation for emit metadata expression fields. No SDK changes required — the SDK
already bridges all grammar-level findings through `validate_composition()`.

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
| Flat string | `"filter": "{x: .context.name}"` | `"{x: .context.name}"` |
| ExpressionField object | `"data": {"expression": ".prev.order_id"}` | `".prev.order_id"` |
| Non-expression value | `"mode": "upsert"` | `None` (skip) |

The helper returns `Option<&str>` — the jaq expression if one is found, `None`
otherwise. This keeps the main loop clean: iterate fields, extract, validate.

### Field lists per category

| Category | Fields checked |
|----------|---------------|
| Transform | `filter` |
| Assert | `filter` |
| Validate | *(none)* |
| Persist | `data`, `validate_success`, `result_shape` |
| Acquire | `params`, `validate_success`, `result_shape` |
| Emit | `payload`, `condition`, `validate_success`, `result_shape` |

### Emit metadata expressions

Emit configs can include `metadata.correlation_id` and `metadata.idempotency_key`,
both `ExpressionField` objects. These are nested one level deeper than other fields
(`config.metadata.correlation_id.expression`). Add explicit handling: if
`config.metadata` exists and is an object, check `correlation_id` and
`idempotency_key` within it using the same extraction helper.

### Field path in findings

The `field_path` always reflects the actual location of the expression:

| Field shape | `field_path` value |
|------------|-------------------|
| Flat string `filter` | `config.filter` |
| ExpressionField `data` | `config.data.expression` |
| ExpressionField `validate_success` | `config.validate_success.expression` |
| Nested metadata | `config.metadata.correlation_id.expression` |

### Error format

Same `INVALID_EXPRESSION` error code. Message pattern unchanged:
```
invocation {idx} ({capability}) has invalid jaq expression in '{field_path}': {parse_error}
```

The `field_path` in the message now uses the full dotted path (e.g.,
`config.validate_success.expression`) rather than the simple field name, so users
can locate the broken expression precisely.

## Files Changed

| File | Change |
|------|--------|
| `crates/tasker-grammar/src/validation/validator.rs` | Add extraction helper, rewrite `check_expression_syntax` with extended field lists and metadata handling |
| `crates/tasker-grammar/src/validation/tests.rs` | Update existing tests to use `ExpressionField` shape, add new tests for validate_success/result_shape/metadata |

## Test Plan

**Updated existing tests** (use `ExpressionField` object shape instead of flat strings):

- `persist_data_expression_validated` — use `{"expression": "broken {{{ syntax"}` shape
- `emit_payload_expression_validated` — use ExpressionField shape
- `emit_condition_expression_validated` — use ExpressionField shape

**New tests** (all in `crates/tasker-grammar/src/validation/tests.rs`):

1. `persist_validate_success_expression_validated` — invalid expression in persist validate_success
2. `persist_result_shape_expression_validated` — invalid expression in persist result_shape
3. `acquire_validate_success_expression_validated` — invalid expression in acquire validate_success
4. `acquire_result_shape_expression_validated` — invalid expression in acquire result_shape
5. `emit_validate_success_expression_validated` — invalid expression in emit validate_success
6. `emit_result_shape_expression_validated` — invalid expression in emit result_shape
7. `emit_metadata_correlation_id_expression_validated` — invalid expression in metadata.correlation_id
8. `emit_metadata_idempotency_key_expression_validated` — invalid expression in metadata.idempotency_key
9. `valid_nested_expressions_pass` — valid ExpressionField objects produce no findings

**Existing coverage** (unchanged, confirms no regressions):

- `invalid_jaq_expression_produces_error` — transform filter (flat string, correct for transform)
- `valid_jaq_expression_passes` — transform filter
- `assert_filter_syntax_validated` — assert filter (flat string, correct for assert)
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
