# TAS-338: Expression Syntax Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `check_expression_syntax` to validate jaq expressions in both flat string and `ExpressionField` object shapes, covering all expression-bearing config fields including `validate_success`, `result_shape`, and emit metadata.

**Architecture:** A free function `extract_expression` handles both `"expr"` (flat string) and `{"expression": "expr"}` (ExpressionField object) shapes. The existing `check_expression_syntax` method gets updated field lists and uses the helper for extraction. Emit metadata gets special handling for its nested structure. Test registry schemas are updated to accept object-type expression fields.

**Tech Stack:** Rust, serde_json, jaq-core (via ExpressionEngine)

**Spec:** `docs/superpowers/specs/2026-03-20-tas-338-expression-syntax-validation-design.md`

---

## File Structure

| File | Responsibility |
|------|---------------|
| `crates/tasker-grammar/src/validation/validator.rs` | Expression extraction helper (free function) + rewritten `check_expression_syntax` |
| `crates/tasker-grammar/src/validation/tests.rs` | Updated test registry schemas + updated existing tests + new tests for all expression fields |

No new files. No SDK changes. All work is in the grammar crate's validation module.

---

## Task 1: Add expression extraction helper and extend `check_expression_syntax`

**Files:**
- Modify: `crates/tasker-grammar/src/validation/validator.rs`

- [ ] **Step 1: Add the `extract_expression` free function**

Add this function **before** the `impl<'a> CompositionValidator<'a>` block (before line 168 in `validator.rs`). It must be a free function, not a method, because it takes no `&self`:

```rust
/// Extract a jaq expression string from a config field value.
///
/// Handles two shapes:
/// - Flat string: `"filter": ".context.name"` → `Some(".context.name")`
/// - ExpressionField object: `"data": {"expression": ".prev"}` → `Some(".prev")`
/// - Anything else: `None`
fn extract_expression(value: &Value) -> Option<&str> {
    // Flat string (used by transform filter, assert filter)
    if let Some(s) = value.as_str() {
        return Some(s);
    }
    // ExpressionField object (used by persist/acquire/emit fields)
    value
        .as_object()
        .and_then(|obj| obj.get("expression"))
        .and_then(|v| v.as_str())
}
```

- [ ] **Step 2: Rewrite `check_expression_syntax` with extended field lists and extraction helper**

Replace the existing `check_expression_syntax` method (lines 398-438) with:

```rust
/// Validate jaq expression syntax in configs.
fn check_expression_syntax(
    &self,
    spec: &CompositionSpec,
    resolved: &[Option<&CapabilityDeclaration>],
    findings: &mut Vec<ValidationFinding>,
) {
    for (idx, (invocation, decl_opt)) in
        spec.invocations.iter().zip(resolved.iter()).enumerate()
    {
        let Some(decl) = decl_opt else {
            continue;
        };

        // Determine which config fields contain jaq expressions based on category
        let expression_fields: &[&str] = match decl.grammar_category {
            GrammarCategoryKind::Transform => &["filter"],
            GrammarCategoryKind::Assert => &["filter"],
            GrammarCategoryKind::Persist => &["data", "validate_success", "result_shape"],
            GrammarCategoryKind::Acquire => &["params", "validate_success", "result_shape"],
            GrammarCategoryKind::Emit => {
                &["payload", "condition", "validate_success", "result_shape"]
            }
            GrammarCategoryKind::Validate => &[],
        };

        for field in expression_fields {
            if let Some(value) = invocation.config.get(*field) {
                let (expr, field_path) = match extract_expression(value) {
                    Some(e) if value.is_string() => (e, format!("config.{field}")),
                    Some(e) => (e, format!("config.{field}.expression")),
                    None => continue,
                };
                if let Err(e) = self.expression_engine.validate_syntax(expr) {
                    findings.push(ValidationFinding {
                        severity: Severity::Error,
                        code: "INVALID_EXPRESSION".to_owned(),
                        invocation_index: Some(idx),
                        message: format!(
                            "invocation {} ({}) has invalid jaq expression in '{}': {e}",
                            idx, invocation.capability, field_path
                        ),
                        field_path: Some(field_path),
                    });
                }
            }
        }

        // Emit metadata expressions (nested one level deeper).
        // Metadata fields are always ExpressionField objects in practice.
        if matches!(decl.grammar_category, GrammarCategoryKind::Emit) {
            if let Some(metadata) =
                invocation.config.get("metadata").and_then(Value::as_object)
            {
                for meta_field in &["correlation_id", "idempotency_key"] {
                    if let Some(value) = metadata.get(*meta_field) {
                        if let Some(expr) = extract_expression(value) {
                            let field_path =
                                format!("config.metadata.{meta_field}.expression");
                            if let Err(e) =
                                self.expression_engine.validate_syntax(expr)
                            {
                                findings.push(ValidationFinding {
                                    severity: Severity::Error,
                                    code: "INVALID_EXPRESSION".to_owned(),
                                    invocation_index: Some(idx),
                                    message: format!(
                                        "invocation {} ({}) has invalid jaq expression in '{}': {e}",
                                        idx, invocation.capability, field_path
                                    ),
                                    field_path: Some(field_path),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --package tasker-grammar --all-features`
Expected: Compiles successfully

- [ ] **Step 4: Verify existing tests still pass**

Run: `cargo test --package tasker-grammar --all-features -- validation::tests --nocapture`
Expected: All existing tests pass (the extraction helper handles flat strings, so transform/assert tests continue to work; persist/emit tests still work because flat strings are still supported)

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-grammar/src/validation/validator.rs
git commit -m "feat(TAS-338): add expression extraction helper and extend check_expression_syntax

Support both flat string and ExpressionField object shapes. Add
validate_success, result_shape fields for persist/acquire/emit.
Add emit metadata expression validation."
```

---

## Task 2: Update test registry schemas and existing tests to use ExpressionField object shape

**Files:**
- Modify: `crates/tasker-grammar/src/validation/tests.rs`

**Context:** The test registry's config schemas declare expression-bearing fields as `"type": "string"` (e.g., `"data": {"type": "string"}`). When we change test data to use `{"expression": "..."}` objects, the `check_config_schemas` validation pass fires `CONFIG_SCHEMA_VIOLATION` before expression syntax validation runs. We must update the registry schemas to accept both string and object types for expression-bearing fields.

- [ ] **Step 1: Update `make_registry()` config schemas to accept ExpressionField objects**

In `make_registry()` (starts at line 19), update the config schemas for persist, acquire, and emit to accept both string and object for expression-bearing fields:

For **persist** (around line 109-116), change:
```rust
config_schema: json!({
    "type": "object",
    "required": ["resource"],
    "properties": {
        "resource": { "type": "object" },
        "data": {},
        "validate_success": { "type": "object" },
        "result_shape": { "type": "object" },
        "constraints": { "type": "object" }
    }
}),
```

For **acquire** (around line 88-96), change:
```rust
config_schema: json!({
    "type": "object",
    "required": ["resource"],
    "properties": {
        "resource": { "type": "object" },
        "params": {},
        "validate_success": { "type": "object" },
        "result_shape": { "type": "object" },
        "constraints": { "type": "object" }
    }
}),
```

For **emit** (around line 132-139), change:
```rust
config_schema: json!({
    "type": "object",
    "required": ["event_name"],
    "properties": {
        "event_name": { "type": "string" },
        "payload": {},
        "condition": {},
        "validate_success": { "type": "object" },
        "result_shape": { "type": "object" },
        "metadata": { "type": "object" }
    }
}),
```

Note: Using `{}` (empty schema, accepts any type) for fields that can be either flat strings or ExpressionField objects. Also removed `"data"` and `"payload"` from `required` lists since the expression shape is an object not a string, and added the new fields (`validate_success`, `result_shape`, `metadata`) to properties.

- [ ] **Step 2: Update `persist_data_expression_validated` test (around line 497)**

Change the config from:
```rust
"data": "broken expression {{{ syntax"
```
to:
```rust
"data": { "expression": "broken expression {{{ syntax" }
```

- [ ] **Step 3: Update `emit_payload_expression_validated` test (around line 522)**

Change the config from:
```rust
"payload": "broken {{{ syntax"
```
to:
```rust
"payload": { "expression": "broken {{{ syntax" }
```

- [ ] **Step 4: Update `emit_condition_expression_validated` test (around line 548)**

Change the config from:
```rust
"payload": ".prev",
"condition": "broken {{{ syntax"
```
to:
```rust
"payload": { "expression": ".prev" },
"condition": { "expression": "broken {{{ syntax" }
```

- [ ] **Step 5: Run updated tests to verify they pass**

Run: `cargo test --package tasker-grammar --all-features -- validation::tests --nocapture`
Expected: All tests pass. The updated schemas accept object values and the extraction helper handles ExpressionField objects.

- [ ] **Step 6: Commit**

```bash
git add crates/tasker-grammar/src/validation/tests.rs
git commit -m "test(TAS-338): update registry schemas and tests for ExpressionField shape

Update test registry config schemas to accept ExpressionField objects
alongside flat strings. Update persist_data, emit_payload, and
emit_condition tests to use real {\"expression\": \"...\"} shape."
```

---

## Task 3: Add tests for validate_success and result_shape expressions

**Files:**
- Modify: `crates/tasker-grammar/src/validation/tests.rs` (append after the expression syntax tests section, around line 573)

- [ ] **Step 1: Write the 6 new tests for validate_success and result_shape**

Add after the existing `emit_condition_expression_validated` test:

```rust
#[test]
fn persist_validate_success_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "data": { "expression": ".prev" },
                "validate_success": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result.findings.iter().find(|f| f.code == "INVALID_EXPRESSION").unwrap();
    assert_eq!(finding.field_path.as_deref(), Some("config.validate_success.expression"));
}

#[test]
fn persist_result_shape_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "data": { "expression": ".prev" },
                "result_shape": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result.findings.iter().find(|f| f.code == "INVALID_EXPRESSION").unwrap();
    assert_eq!(finding.field_path.as_deref(), Some("config.result_shape.expression"));
}

#[test]
fn acquire_validate_success_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "acquire".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "validate_success": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result.findings.iter().find(|f| f.code == "INVALID_EXPRESSION").unwrap();
    assert_eq!(finding.field_path.as_deref(), Some("config.validate_success.expression"));
}

#[test]
fn acquire_result_shape_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "acquire".to_owned(),
            config: json!({
                "resource": { "type": "database" },
                "result_shape": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: false,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result.findings.iter().find(|f| f.code == "INVALID_EXPRESSION").unwrap();
    assert_eq!(finding.field_path.as_deref(), Some("config.result_shape.expression"));
}

#[test]
fn emit_validate_success_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "emit".to_owned(),
            config: json!({
                "event_name": "test.event",
                "payload": { "expression": ".prev" },
                "validate_success": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result.findings.iter().find(|f| f.code == "INVALID_EXPRESSION").unwrap();
    assert_eq!(finding.field_path.as_deref(), Some("config.validate_success.expression"));
}

#[test]
fn emit_result_shape_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "emit".to_owned(),
            config: json!({
                "event_name": "test.event",
                "payload": { "expression": ".prev" },
                "result_shape": { "expression": "broken {{{ syntax" }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result.findings.iter().find(|f| f.code == "INVALID_EXPRESSION").unwrap();
    assert_eq!(finding.field_path.as_deref(), Some("config.result_shape.expression"));
}
```

- [ ] **Step 2: Run the new tests**

Run: `cargo test --package tasker-grammar --all-features -- validation::tests --nocapture`
Expected: All tests pass including the 6 new ones

- [ ] **Step 3: Commit**

```bash
git add crates/tasker-grammar/src/validation/tests.rs
git commit -m "test(TAS-338): add validate_success and result_shape expression tests

Cover all 6 combinations: persist/acquire/emit x validate_success/result_shape.
Each test verifies INVALID_EXPRESSION error with correct field_path."
```

---

## Task 4: Add tests for emit metadata expressions and valid-expressions-pass

**Files:**
- Modify: `crates/tasker-grammar/src/validation/tests.rs` (append after Task 3 tests)

- [ ] **Step 1: Write metadata and valid-expressions tests**

Append:

```rust
#[test]
fn emit_metadata_correlation_id_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "emit".to_owned(),
            config: json!({
                "event_name": "test.event",
                "payload": { "expression": ".prev" },
                "metadata": {
                    "correlation_id": { "expression": "broken {{{ syntax" }
                }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result.findings.iter().find(|f| f.code == "INVALID_EXPRESSION").unwrap();
    assert_eq!(
        finding.field_path.as_deref(),
        Some("config.metadata.correlation_id.expression")
    );
}

#[test]
fn emit_metadata_idempotency_key_expression_validated() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "emit".to_owned(),
            config: json!({
                "event_name": "test.event",
                "payload": { "expression": ".prev" },
                "metadata": {
                    "idempotency_key": { "expression": "broken {{{ syntax" }
                }
            }),
            checkpoint: true,
        }],
    };

    let result = validator.validate(&spec);
    assert!(has_error(&result, "INVALID_EXPRESSION"));
    let finding = result.findings.iter().find(|f| f.code == "INVALID_EXPRESSION").unwrap();
    assert_eq!(
        finding.field_path.as_deref(),
        Some("config.metadata.idempotency_key.expression")
    );
}

#[test]
fn valid_nested_expressions_pass() {
    let registry = make_registry();
    let engine = make_engine();
    let validator = make_validator(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "acquire".to_owned(),
                config: json!({
                    "resource": { "type": "database" },
                    "params": { "expression": "{customer_id: .context.id}" },
                    "validate_success": { "expression": ".total_count > 0" },
                    "result_shape": { "expression": ".data[0]" }
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "persist".to_owned(),
                config: json!({
                    "resource": { "type": "database" },
                    "data": { "expression": "{id: .prev.order_id}" },
                    "validate_success": { "expression": ".affected_rows > 0" },
                    "result_shape": { "expression": "{persisted_id: .id}" }
                }),
                checkpoint: true,
            },
            CapabilityInvocation {
                capability: "emit".to_owned(),
                config: json!({
                    "event_name": "order.created",
                    "payload": { "expression": ".prev" },
                    "condition": { "expression": ".prev.persisted_id != null" },
                    "validate_success": { "expression": ".delivered" },
                    "result_shape": { "expression": "{event_id: .message_id}" },
                    "metadata": {
                        "correlation_id": { "expression": ".context.request_id" },
                        "idempotency_key": { "expression": ".prev.persisted_id | tostring" }
                    }
                }),
                checkpoint: true,
            },
        ],
    };

    let result = validator.validate(&spec);
    assert!(!has_finding(&result, "INVALID_EXPRESSION"));
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test --package tasker-grammar --all-features -- validation::tests --nocapture`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/tasker-grammar/src/validation/tests.rs
git commit -m "test(TAS-338): add emit metadata and valid-expressions-pass tests

Cover metadata.correlation_id and metadata.idempotency_key expression
validation. Add comprehensive valid-expressions test covering all three
capabilities with all ExpressionField shapes."
```

---

## Task 5: Final verification

- [ ] **Step 1: Run full grammar test suite**

Run: `cargo test --package tasker-grammar --all-features`
Expected: All tests pass (448+ grammar tests)

- [ ] **Step 2: Run workspace clippy**

Run: `cargo clippy --all-targets --all-features --workspace`
Expected: Zero warnings

- [ ] **Step 3: Run workspace check**

Run: `cargo check --all-features --workspace`
Expected: Clean compilation

- [ ] **Step 4: Run SDK tests to confirm no regression**

Run: `cargo test --package tasker-sdk --all-features`
Expected: All SDK tests pass (the SDK bridges grammar findings unchanged)

- [ ] **Step 5: Final commit if any cleanup needed, otherwise done**

If clippy or formatting requires changes:
```bash
cargo fmt --all
git add -A
git commit -m "chore(TAS-338): clippy and formatting fixes"
```
