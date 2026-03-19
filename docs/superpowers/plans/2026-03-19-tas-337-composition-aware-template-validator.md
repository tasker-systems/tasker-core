# TAS-337: Composition-Aware Template Validator — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate tasker-grammar's CompositionValidator into the tasker-sdk template validation pipeline so composition blocks in task templates are validated at design time.

**Architecture:** Three crates touched in sequence: (1) tasker-grammar gets a `vocabulary` module exporting the standard 6-capability registry, (2) tasker-shared's `StepDefinition` gets an `Option<serde_json::Value>` composition field, (3) tasker-sdk gets a `composition_validator` module that bridges grammar validation into the template validation pipeline with unified `Severity` types.

**Tech Stack:** Rust, serde_json, tasker-grammar (CompositionValidator, ExpressionEngine, CapabilityRegistry)

**Spec:** `docs/superpowers/specs/2026-03-19-tas-337-composition-aware-template-validator-design.md`

---

## File Structure

### New files

| File | Responsibility |
|------|----------------|
| `crates/tasker-grammar/src/vocabulary.rs` | Standard 6-capability registry factory function |
| `crates/tasker-sdk/src/composition_validator/mod.rs` | Composition validation: standalone + step-context + finding translation |
| `crates/tasker-sdk/src/composition_validator/tests.rs` | Unit tests for all composition validation paths |

### Modified files

| File | Change |
|------|--------|
| `crates/tasker-grammar/src/lib.rs` | Export `vocabulary` module and `standard_capability_registry` |
| `crates/tasker-grammar/src/validation/schema_compat.rs` | Make `check_schema_compatibility` `pub` (was `pub(crate)`) |
| `crates/tasker-grammar/src/validation/mod.rs` | Re-export `check_schema_compatibility` |
| `crates/tasker-shared/src/models/core/task_template/mod.rs` | Add `composition: Option<Value>` to `StepDefinition` |
| `crates/tasker-sdk/Cargo.toml` | Add `tasker-grammar` dependency |
| `crates/tasker-sdk/src/lib.rs` | Add `pub mod composition_validator` |
| `crates/tasker-grammar/src/types/validation.rs` | Add `#[serde(rename_all = "lowercase")]` to `Severity` for serialization compatibility |
| `crates/tasker-sdk/src/template_validator/mod.rs` | Replace local `Severity` with grammar's, add composition validation pass, add `validate_with_registry()` |
| `crates/tasker-shared/src/models/core/task_template/mod.rs` | Add `composition: None` to ~5 test struct literals |
| `crates/tasker-shared/src/models/core/task_template/event_validator.rs` | Add `composition: None` to test struct literal |
| `crates/tasker-shared/src/types/base.rs` | Add `composition: None` to test struct literal |
| `crates/tasker-shared/src/config/orchestration/batch_processing.rs` | Add `composition: None` to ~3 test struct literals |
| `crates/tasker-orchestration/src/orchestration/lifecycle/task_initialization/workflow_step_creator.rs` | Add `composition: None` to ~2 test struct literals |

---

## Task 1: Standard capability vocabulary in tasker-grammar

**Files:**
- Create: `crates/tasker-grammar/src/vocabulary.rs`
- Modify: `crates/tasker-grammar/src/lib.rs`

- [ ] **Step 1: Write the test for standard_capability_registry**

Add to bottom of `crates/tasker-grammar/src/vocabulary.rs` (we'll create the file with both code and tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_registry_contains_all_six_capabilities() {
        let registry = standard_capability_registry();
        let expected = ["transform", "validate", "assert", "persist", "acquire", "emit"];
        for name in &expected {
            assert!(
                registry.contains_key(*name),
                "missing capability: {name}"
            );
        }
        assert_eq!(registry.len(), 6);
    }

    #[test]
    fn standard_registry_capabilities_have_correct_categories() {
        use crate::types::GrammarCategoryKind;
        let registry = standard_capability_registry();
        assert!(matches!(
            registry["transform"].grammar_category,
            GrammarCategoryKind::Transform
        ));
        assert!(matches!(
            registry["validate"].grammar_category,
            GrammarCategoryKind::Validate
        ));
        assert!(matches!(
            registry["assert"].grammar_category,
            GrammarCategoryKind::Assert
        ));
        assert!(matches!(
            registry["persist"].grammar_category,
            GrammarCategoryKind::Persist
        ));
        assert!(matches!(
            registry["acquire"].grammar_category,
            GrammarCategoryKind::Acquire
        ));
        assert!(matches!(
            registry["emit"].grammar_category,
            GrammarCategoryKind::Emit
        ));
    }

    #[test]
    fn standard_registry_config_schemas_are_objects() {
        let registry = standard_capability_registry();
        for (name, decl) in &registry {
            assert_eq!(
                decl.config_schema.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "capability {name} config_schema should be an object"
            );
        }
    }
}
```

- [ ] **Step 2: Write the standard_capability_registry implementation**

Create `crates/tasker-grammar/src/vocabulary.rs`:

```rust
//! Standard capability vocabulary for the Tasker action grammar.
//!
//! The [`standard_capability_registry`] function returns the canonical set of
//! built-in capabilities. Use this as the default registry for composition
//! validation in tooling (tasker-sdk, tasker-ctl, tasker-mcp).

use std::collections::HashMap;

use serde_json::json;

use crate::types::{CapabilityDeclaration, GrammarCategoryKind, MutationProfile};

/// Returns the standard 6-capability vocabulary as a `HashMap` implementing
/// [`CapabilityRegistry`](crate::validation::CapabilityRegistry).
///
/// Capabilities: `transform`, `validate`, `assert`, `persist`, `acquire`, `emit`.
pub fn standard_capability_registry() -> HashMap<String, CapabilityDeclaration> {
    let mut registry = HashMap::new();

    registry.insert(
        "transform".to_owned(),
        CapabilityDeclaration {
            name: "transform".to_owned(),
            grammar_category: GrammarCategoryKind::Transform,
            description: "Pure data transformation via jaq expression".to_owned(),
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
            description: "JSON Schema validation with coercion and failure modes".to_owned(),
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
            description: "Composable execution gate with boolean jaq filter".to_owned(),
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
        "persist".to_owned(),
        CapabilityDeclaration {
            name: "persist".to_owned(),
            grammar_category: GrammarCategoryKind::Persist,
            description: "Write state to a resource target".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["resource", "data"],
                "properties": {
                    "resource": { "type": "object" },
                    "data": { "type": "string" },
                    "constraints": { "type": "object" }
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
        "acquire".to_owned(),
        CapabilityDeclaration {
            name: "acquire".to_owned(),
            grammar_category: GrammarCategoryKind::Acquire,
            description: "Fetch data from a resource source".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["resource"],
                "properties": {
                    "resource": { "type": "object" },
                    "params": { "type": "string" },
                    "constraints": { "type": "object" }
                }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    registry.insert(
        "emit".to_owned(),
        CapabilityDeclaration {
            name: "emit".to_owned(),
            grammar_category: GrammarCategoryKind::Emit,
            description: "Fire a domain event".to_owned(),
            config_schema: json!({
                "type": "object",
                "required": ["event_name", "payload"],
                "properties": {
                    "event_name": { "type": "string" },
                    "payload": { "type": "string" },
                    "condition": { "type": "string" }
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

// Tests at bottom of file (see Step 1)
```

- [ ] **Step 3: Export the vocabulary module from lib.rs**

In `crates/tasker-grammar/src/lib.rs`, add:
- `pub mod vocabulary;` after the existing module declarations
- `pub use vocabulary::standard_capability_registry;` in the re-exports section

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p tasker-grammar --lib vocabulary`
Expected: 3 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-grammar/src/vocabulary.rs crates/tasker-grammar/src/lib.rs
git commit -m "feat(TAS-337): add standard_capability_registry in tasker-grammar vocabulary module"
```

---

## Task 2: Make schema_compat::check_schema_compatibility public

**Files:**
- Modify: `crates/tasker-grammar/src/validation/schema_compat.rs:162` (change `pub(crate)` to `pub`)
- Modify: `crates/tasker-grammar/src/validation/mod.rs` (add re-export)

- [ ] **Step 1: Change visibility of check_schema_compatibility**

In `crates/tasker-grammar/src/validation/schema_compat.rs`, line 162, change:
```rust
pub(crate) fn check_schema_compatibility(
```
to:
```rust
pub fn check_schema_compatibility(
```

- [ ] **Step 2: Re-export from validation/mod.rs**

In `crates/tasker-grammar/src/validation/mod.rs`, add:
```rust
pub use schema_compat::check_schema_compatibility;
```

- [ ] **Step 3: Re-export from crate root**

In `crates/tasker-grammar/src/lib.rs`, update the validation re-export line to include `check_schema_compatibility`:
```rust
pub use validation::{CapabilityRegistry, CompositionValidator, ValidationResult, check_schema_compatibility};
```

- [ ] **Step 4: Run tests to verify no breakage**

Run: `cargo test -p tasker-grammar --lib`
Expected: All existing tests pass (this is purely a visibility change)

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-grammar/src/validation/schema_compat.rs crates/tasker-grammar/src/validation/mod.rs crates/tasker-grammar/src/lib.rs
git commit -m "refactor(TAS-337): make check_schema_compatibility public for SDK use"
```

---

## Task 3: Add composition field to StepDefinition

**Files:**
- Modify: `crates/tasker-shared/src/models/core/task_template/mod.rs:560-562`

- [ ] **Step 1: Add the composition field**

In `crates/tasker-shared/src/models/core/task_template/mod.rs`, after the `result_schema` field (line 561) and before the closing `}` of `StepDefinition` (line 562), add:

```rust
    /// Optional composition spec for grammar-defined virtual handler steps.
    ///
    /// When present, this step's behavior is defined by the composition rather
    /// than the handler callable. The value is an opaque JSON blob at the
    /// `tasker-shared` level — typed validation against `CompositionSpec`
    /// happens in `tasker-sdk::composition_validator`.
    ///
    /// # Example
    /// ```yaml
    /// composition:
    ///   outcome:
    ///     description: "Process an order"
    ///     output_schema:
    ///       type: object
    ///       properties:
    ///         order_id: { type: string }
    ///   invocations:
    ///     - capability: validate
    ///       config:
    ///         schema: { type: object }
    /// ```
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub composition: Option<Value>,
```

- [ ] **Step 2: Fix struct literal construction sites**

Adding a new field to `StepDefinition` breaks all existing struct literal construction sites (even though `#[builder(default)]` handles builder construction). Add `composition: None,` to all models-layer `StepDefinition` struct literals:

**`crates/tasker-shared/src/models/core/task_template/mod.rs`** — 5 test sites (around lines 1692, 1712, 2072, 2098, 2121). Add `composition: None,` after `result_schema` in each.

**`crates/tasker-shared/src/models/core/task_template/event_validator.rs`** — 1 test helper (around line 323). Add `composition: None,` after `result_schema`.

**`crates/tasker-shared/src/types/base.rs`** — 1 test helper (around line 724). Add `composition: None,` after `result_schema`.

**`crates/tasker-shared/src/config/orchestration/batch_processing.rs`** — 3 test sites (around lines 111, 147, 187). Add `composition: None,` after `result_schema`.

**`crates/tasker-orchestration/src/orchestration/lifecycle/task_initialization/workflow_step_creator.rs`** — 2 test sites (around lines 208, 238). Add `composition: None,` after `result_schema`.

- [ ] **Step 3: Verify the full workspace builds**

Run: `cargo check --all-features`
Expected: Clean build. All struct literal sites now include the new field.

- [ ] **Step 4: Verify existing tests still pass**

Run: `cargo test -p tasker-shared --lib && cargo test -p tasker-sdk --lib template_parser`
Expected: All existing tests pass. Deserialization defaults `composition` to `None` for existing YAML templates.

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-shared/src/models/core/task_template/mod.rs
git commit -m "feat(TAS-337): add composition field to StepDefinition"
```

---

## Task 4: Add tasker-grammar dependency to tasker-sdk

**Files:**
- Modify: `crates/tasker-sdk/Cargo.toml`

- [ ] **Step 1: Add the dependency**

In `crates/tasker-sdk/Cargo.toml`, in the `[dependencies]` section under `# Workspace crates`, add:

```toml
tasker-grammar = { path = "../tasker-grammar" }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p tasker-sdk --all-features`
Expected: Clean build

- [ ] **Step 3: Commit**

```bash
git add crates/tasker-sdk/Cargo.toml
git commit -m "chore(TAS-337): add tasker-grammar dependency to tasker-sdk"
```

---

## Task 5: Unify Severity — replace SDK's local enum with grammar's

**Files:**
- Modify: `crates/tasker-sdk/src/template_validator/mod.rs`

This task replaces the locally-defined `Severity` enum with a re-export of `tasker_grammar::Severity`. All existing tests must continue passing.

- [ ] **Step 1: Replace the Severity enum and update imports**

In `crates/tasker-sdk/src/template_validator/mod.rs`:

1. Remove the local `Severity` enum (lines 12-18):
```rust
// DELETE this block:
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}
```

2. Add import at the top (after the existing `use` statements):
```rust
pub use tasker_grammar::Severity;
```

- [ ] **Step 2: Run existing tests to verify backward compatibility**

Run: `cargo test -p tasker-sdk --lib template_validator`
Expected: All 8 existing tests pass. The `Severity` variants are identical so all assertions work.

The grammar's `Severity` does NOT have `rename_all = "lowercase"` — it serializes as PascalCase (`"Error"`, `"Warning"`, `"Info"`). The SDK's old `Severity` used `rename_all = "lowercase"` (`"error"`, `"warning"`, `"info"`). To maintain serialization compatibility for MCP consumers, we must add the attribute to grammar's Severity.

- [ ] **Step 3: Add rename_all to grammar's Severity**

In `crates/tasker-grammar/src/types/validation.rs`, change:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
```
to:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
```

Then run: `cargo test -p tasker-grammar --lib`
Expected: All grammar tests pass. Grammar tests compare `Severity` variants via `matches!()` (not serialized strings), so this change is safe.

- [ ] **Step 4: Verify full SDK test suite**

Run: `cargo test -p tasker-sdk --lib`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-sdk/src/template_validator/mod.rs crates/tasker-grammar/src/types/validation.rs
git commit -m "refactor(TAS-337): unify Severity type — SDK re-exports from tasker-grammar"
```

---

## Task 6: Create composition_validator module with standalone validation

**Files:**
- Create: `crates/tasker-sdk/src/composition_validator/mod.rs`
- Modify: `crates/tasker-sdk/src/lib.rs`

- [ ] **Step 1: Write the failing test for validate_composition**

Create `crates/tasker-sdk/src/composition_validator/mod.rs` starting with tests:

```rust
//! Composition validation for task templates.
//!
//! Bridges `tasker-grammar`'s [`CompositionValidator`] into the SDK's template
//! validation pipeline. Provides both standalone composition validation and
//! step-context-aware validation that integrates with `template_validator`.
//!
//! **Ticket**: TAS-337

use serde_json::Value;

use tasker_grammar::validation::{CapabilityRegistry, CompositionValidator};
use tasker_grammar::{CompositionSpec, ExpressionEngine, Severity};

use crate::template_validator::ValidationFinding;

#[cfg(test)]
mod tests;
```

Create `crates/tasker-sdk/src/composition_validator/tests.rs`:

```rust
use serde_json::json;

use tasker_grammar::vocabulary::standard_capability_registry;
use tasker_grammar::Severity;

use super::*;

#[test]
fn validate_composition_valid_spec_returns_empty() {
    let registry = standard_capability_registry();
    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: tasker_grammar::OutcomeDeclaration {
            description: "Test outcome".to_owned(),
            output_schema: json!({
                "type": "object",
                "properties": { "result": { "type": "string" } },
                "required": ["result"]
            }),
        },
        invocations: vec![tasker_grammar::CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {
                    "type": "object",
                    "properties": { "result": { "type": "string" } },
                    "required": ["result"]
                },
                "filter": ".context | {result: .name}"
            }),
            checkpoint: false,
        }],
    };
    let findings = validate_composition(&spec, &registry);
    let errors: Vec<_> = findings.iter().filter(|f| f.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
}

#[test]
fn validate_composition_unknown_capability_returns_error() {
    let registry = standard_capability_registry();
    let spec = CompositionSpec {
        name: None,
        outcome: tasker_grammar::OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![tasker_grammar::CapabilityInvocation {
            capability: "nonexistent_cap".to_owned(),
            config: json!({}),
            checkpoint: false,
        }],
    };
    let findings = validate_composition(&spec, &registry);
    assert!(
        findings.iter().any(|f| f.code == "COMPOSITION_INVALID" && f.severity == Severity::Error),
        "expected COMPOSITION_INVALID error, got: {findings:?}"
    );
}

#[test]
fn validate_composition_missing_checkpoint_returns_error() {
    let registry = standard_capability_registry();
    let spec = CompositionSpec {
        name: None,
        outcome: tasker_grammar::OutcomeDeclaration {
            description: "Test".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![tasker_grammar::CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": { "type": "postgres", "target": "orders" },
                "data": ".context"
            }),
            checkpoint: false, // Missing! persist is mutating
        }],
    };
    let findings = validate_composition(&spec, &registry);
    assert!(
        findings.iter().any(|f| f.code == "COMPOSITION_INVALID" && f.severity == Severity::Error),
        "expected checkpoint error, got: {findings:?}"
    );
}
```

- [ ] **Step 2: Export the module from lib.rs**

In `crates/tasker-sdk/src/lib.rs`, add:
```rust
pub mod composition_validator;
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p tasker-sdk --lib composition_validator`
Expected: FAIL — `validate_composition` function doesn't exist yet

- [ ] **Step 4: Implement validate_composition**

Add to `crates/tasker-sdk/src/composition_validator/mod.rs` (after the imports, before `#[cfg(test)]`):

```rust
/// Translate a grammar-level `ValidationFinding` into an SDK `ValidationFinding`.
///
/// Grammar findings include `invocation_index` and `field_path` which are
/// encoded into the message for human readability. The `step` field is set
/// by the caller when validating in step context.
fn translate_finding(
    finding: &tasker_grammar::ValidationFinding,
    step_name: Option<&str>,
) -> ValidationFinding {
    let prefix = match finding.invocation_index {
        Some(idx) => format!("invocation[{idx}]: "),
        None => String::new(),
    };
    let suffix = match &finding.field_path {
        Some(path) => format!(" (at {path})"),
        None => String::new(),
    };
    let code = match finding.severity {
        Severity::Error => "COMPOSITION_INVALID",
        Severity::Warning => "COMPOSITION_WARNING",
        Severity::Info => "COMPOSITION_WARNING",
    };
    ValidationFinding {
        code: code.to_owned(),
        severity: finding.severity.clone(),
        message: format!("{prefix}{}{suffix}", finding.message),
        step: step_name.map(str::to_owned),
    }
}

/// Validate a standalone `CompositionSpec` against a capability registry.
///
/// Constructs an `ExpressionEngine` with default config internally.
/// Returns SDK-level `ValidationFinding`s with no step context.
pub fn validate_composition(
    spec: &CompositionSpec,
    registry: &dyn CapabilityRegistry,
) -> Vec<ValidationFinding> {
    let engine = ExpressionEngine::with_defaults();
    let validator = CompositionValidator::new(registry, &engine);
    let result = validator.validate(spec);

    result
        .findings
        .iter()
        .map(|f| translate_finding(f, None))
        .collect()
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p tasker-sdk --lib composition_validator`
Expected: 3 tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/tasker-sdk/src/composition_validator/mod.rs crates/tasker-sdk/src/composition_validator/tests.rs crates/tasker-sdk/src/lib.rs
git commit -m "feat(TAS-337): add composition_validator module with standalone validation"
```

---

## Task 7: Add step-context validation (validate_step_composition)

**Files:**
- Modify: `crates/tasker-sdk/src/composition_validator/mod.rs`
- Modify: `crates/tasker-sdk/src/composition_validator/tests.rs`

- [ ] **Step 1: Write failing tests for validate_step_composition**

Add to `crates/tasker-sdk/src/composition_validator/tests.rs`:

```rust
use crate::template_parser::parse_template_str;

#[test]
fn validate_step_composition_parse_error() {
    let registry = standard_capability_registry();
    let yaml = r#"
name: parse_error_test
namespace_name: test
version: "1.0.0"
steps:
  - name: bad_step
    handler:
      callable: "grammar:bad"
    composition:
      not_a_valid_composition: true
"#;
    let template = parse_template_str(yaml).unwrap();
    let step = &template.steps[0];
    let findings = validate_step_composition(step, &registry);
    assert!(
        findings.iter().any(|f| f.code == "COMPOSITION_PARSE_ERROR"),
        "expected COMPOSITION_PARSE_ERROR, got: {findings:?}"
    );
    assert!(findings.iter().all(|f| f.step.as_deref() == Some("bad_step")));
}

#[test]
fn validate_step_composition_result_schema_mismatch() {
    let registry = standard_capability_registry();
    let yaml = r#"
name: schema_mismatch_test
namespace_name: test
version: "1.0.0"
steps:
  - name: mismatched_step
    handler:
      callable: "grammar:test"
    result_schema:
      type: object
      required:
        - field_that_composition_does_not_produce
      properties:
        field_that_composition_does_not_produce:
          type: string
    composition:
      outcome:
        description: "Produces different fields"
        output_schema:
          type: object
          required:
            - actual_field
          properties:
            actual_field:
              type: integer
      invocations:
        - capability: transform
          config:
            output:
              type: object
              required:
                - actual_field
              properties:
                actual_field:
                  type: integer
            filter: ".context | {actual_field: 42}"
"#;
    let template = parse_template_str(yaml).unwrap();
    let step = &template.steps[0];
    let findings = validate_step_composition(step, &registry);
    assert!(
        findings.iter().any(|f| f.code == "COMPOSITION_RESULT_SCHEMA_MISMATCH"),
        "expected COMPOSITION_RESULT_SCHEMA_MISMATCH, got: {findings:?}"
    );
}

#[test]
fn validate_step_composition_callable_convention_warning() {
    let registry = standard_capability_registry();
    let yaml = r#"
name: callable_convention_test
namespace_name: test
version: "1.0.0"
steps:
  - name: wrong_callable
    handler:
      callable: "my_handler"
    composition:
      outcome:
        description: "Test"
        output_schema:
          type: object
          properties:
            result:
              type: string
          required:
            - result
      invocations:
        - capability: transform
          config:
            output:
              type: object
              properties:
                result:
                  type: string
              required:
                - result
            filter: ".context | {result: .name}"
"#;
    let template = parse_template_str(yaml).unwrap();
    let step = &template.steps[0];
    let findings = validate_step_composition(step, &registry);
    assert!(
        findings.iter().any(|f| f.code == "COMPOSITION_CALLABLE_CONVENTION"
            && f.severity == Severity::Warning),
        "expected COMPOSITION_CALLABLE_CONVENTION warning, got: {findings:?}"
    );
}

#[test]
fn validate_step_composition_no_composition_returns_empty() {
    let registry = standard_capability_registry();
    let yaml = r#"
name: no_composition_test
namespace_name: test
version: "1.0.0"
steps:
  - name: normal_step
    handler:
      callable: "my_handler"
"#;
    let template = parse_template_str(yaml).unwrap();
    let step = &template.steps[0];
    let findings = validate_step_composition(step, &registry);
    assert!(findings.is_empty(), "expected no findings for step without composition");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p tasker-sdk --lib composition_validator`
Expected: FAIL — `validate_step_composition` doesn't exist yet

- [ ] **Step 3: Implement validate_step_composition**

Add to `crates/tasker-sdk/src/composition_validator/mod.rs`:

```rust
use tasker_shared::models::core::task_template::StepDefinition;

/// Validate a composition in the context of a template step.
///
/// If the step has no `composition` field, returns empty.
/// Otherwise: deserializes to `CompositionSpec`, runs `CompositionValidator`,
/// checks result_schema compatibility, and checks callable convention.
/// All findings are tagged with the step name.
pub fn validate_step_composition(
    step: &StepDefinition,
    registry: &dyn CapabilityRegistry,
) -> Vec<ValidationFinding> {
    let composition_value = match &step.composition {
        Some(v) => v,
        None => return Vec::new(),
    };

    let mut findings = Vec::new();

    // Deserialize to CompositionSpec
    let spec: CompositionSpec = match serde_json::from_value(composition_value.clone()) {
        Ok(s) => s,
        Err(e) => {
            findings.push(ValidationFinding {
                code: "COMPOSITION_PARSE_ERROR".to_owned(),
                severity: Severity::Error,
                message: format!("failed to parse composition: {e}"),
                step: Some(step.name.clone()),
            });
            return findings;
        }
    };

    // Run grammar-level validation
    let grammar_findings = validate_composition(&spec, registry);
    findings.extend(
        grammar_findings
            .into_iter()
            .map(|mut f| {
                f.step = Some(step.name.clone());
                f
            }),
    );

    // Check result_schema compatibility
    if let Some(result_schema) = &step.result_schema {
        let outcome_schema = &spec.outcome.output_schema;
        let compat_findings = tasker_grammar::check_schema_compatibility(
            outcome_schema,  // producer: what the composition produces
            result_schema,   // consumer: what the step declares
            "step result_schema vs composition outcome",
            None,
        );
        for cf in &compat_findings {
            if matches!(cf.severity, Severity::Error | Severity::Warning) {
                findings.push(ValidationFinding {
                    code: "COMPOSITION_RESULT_SCHEMA_MISMATCH".to_owned(),
                    severity: cf.severity.clone(),
                    message: cf.message.clone(),
                    step: Some(step.name.clone()),
                });
            }
        }
    }

    // Check callable convention
    if !step.handler.callable.starts_with("grammar:") {
        findings.push(ValidationFinding {
            code: "COMPOSITION_CALLABLE_CONVENTION".to_owned(),
            severity: Severity::Warning,
            message: format!(
                "step has a composition but callable '{}' does not use the 'grammar:' prefix",
                step.handler.callable
            ),
            step: Some(step.name.clone()),
        });
    }

    findings
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p tasker-sdk --lib composition_validator`
Expected: All 7 tests pass (3 from Task 6 + 4 new)

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-sdk/src/composition_validator/mod.rs crates/tasker-sdk/src/composition_validator/tests.rs
git commit -m "feat(TAS-337): add validate_step_composition with parse, schema, and callable checks"
```

---

## Task 8: Integrate composition validation into template_validator

**Files:**
- Modify: `crates/tasker-sdk/src/template_validator/mod.rs`
- Modify: `crates/tasker-sdk/src/composition_validator/tests.rs` (add integration-level tests)

- [ ] **Step 1: Write failing tests for template-level composition validation**

Add to `crates/tasker-sdk/src/composition_validator/tests.rs`:

```rust
use crate::template_validator;

#[test]
fn template_validate_with_composition_step() {
    let yaml = r#"
name: composition_template
namespace_name: test
version: "1.0.0"
steps:
  - name: validate_order
    handler:
      callable: "grammar:validate_order"
    composition:
      outcome:
        description: "Validate an order"
        output_schema:
          type: object
          properties:
            valid:
              type: boolean
          required:
            - valid
      invocations:
        - capability: transform
          config:
            output:
              type: object
              properties:
                valid:
                  type: boolean
              required:
                - valid
            filter: ".context | {valid: true}"
  - name: process_order
    handler:
      callable: "process.order"
    depends_on:
      - validate_order
"#;
    let template = parse_template_str(yaml).unwrap();
    let report = template_validator::validate(&template);
    let composition_errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("COMPOSITION_") && f.severity == Severity::Error)
        .collect();
    assert!(
        composition_errors.is_empty(),
        "unexpected composition errors: {composition_errors:?}"
    );
}

#[test]
fn template_validate_catches_bad_composition() {
    let yaml = r#"
name: bad_composition_template
namespace_name: test
version: "1.0.0"
steps:
  - name: bad_step
    handler:
      callable: "grammar:bad"
    composition:
      outcome:
        description: "Bad"
        output_schema:
          type: object
      invocations:
        - capability: nonexistent_capability
          config: {}
"#;
    let template = parse_template_str(yaml).unwrap();
    let report = template_validator::validate(&template);
    assert!(
        report.findings.iter().any(|f| f.code == "COMPOSITION_INVALID"),
        "expected COMPOSITION_INVALID, got: {:?}",
        report.findings
    );
    assert!(!report.valid);
}

#[test]
fn template_validate_mixed_steps_validates_both() {
    let yaml = r#"
name: mixed_template
namespace_name: test
version: "1.0.0"
steps:
  - name: setup
    handler:
      callable: "my.setup_handler"
  - name: grammar_step
    handler:
      callable: "grammar:process"
    depends_on:
      - setup
    composition:
      outcome:
        description: "Process"
        output_schema:
          type: object
          properties:
            done:
              type: boolean
          required:
            - done
      invocations:
        - capability: transform
          config:
            output:
              type: object
              properties:
                done:
                  type: boolean
              required:
                - done
            filter: ".context | {done: true}"
"#;
    let template = parse_template_str(yaml).unwrap();
    let report = template_validator::validate(&template);
    // Structural checks still run for both steps
    assert_eq!(report.step_count, 2);
    // No composition errors
    let composition_errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("COMPOSITION_") && f.severity == Severity::Error)
        .collect();
    assert!(composition_errors.is_empty(), "unexpected: {composition_errors:?}");
}

#[test]
fn template_validate_backward_compatible_no_composition() {
    // Existing fixture with no composition fields
    let yaml =
        include_str!("../../../../tests/fixtures/task_templates/codegen_test_template.yaml");
    let template = parse_template_str(yaml).unwrap();
    let report = template_validator::validate(&template);
    assert!(report.valid);
    assert!(!report.has_cycles);
    assert_eq!(report.step_count, 5);
    // No composition findings at all
    assert!(
        !report.findings.iter().any(|f| f.code.starts_with("COMPOSITION_")),
        "should have no composition findings for template without compositions"
    );
}

#[test]
fn template_validate_with_custom_registry() {
    use tasker_grammar::{CapabilityDeclaration, GrammarCategoryKind, MutationProfile};

    let mut registry = standard_capability_registry();
    registry.insert(
        "custom_cap".to_owned(),
        CapabilityDeclaration {
            name: "custom_cap".to_owned(),
            grammar_category: GrammarCategoryKind::Transform,
            description: "Custom capability".to_owned(),
            config_schema: serde_json::json!({
                "type": "object",
                "properties": { "input": { "type": "string" } }
            }),
            mutation_profile: MutationProfile::NonMutating,
            tags: vec![],
            version: "1.0.0".to_owned(),
        },
    );

    let yaml = r#"
name: custom_registry_test
namespace_name: test
version: "1.0.0"
steps:
  - name: custom_step
    handler:
      callable: "grammar:custom"
    composition:
      outcome:
        description: "Custom"
        output_schema:
          type: object
      invocations:
        - capability: custom_cap
          config:
            input: "test"
"#;
    let template = parse_template_str(yaml).unwrap();
    let report = template_validator::validate_with_registry(&template, &registry);
    let composition_errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code == "COMPOSITION_INVALID")
        .collect();
    assert!(
        composition_errors.is_empty(),
        "custom_cap should be valid with custom registry: {composition_errors:?}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p tasker-sdk --lib composition_validator`
Expected: FAIL — `validate_with_registry` doesn't exist yet, `validate` doesn't check compositions yet

- [ ] **Step 3: Implement validate_with_registry and extend validate**

In `crates/tasker-sdk/src/template_validator/mod.rs`:

1. Add imports at the top:
```rust
use tasker_grammar::validation::CapabilityRegistry;
use tasker_grammar::vocabulary::standard_capability_registry;
```

2. Replace the `validate` function body to delegate:
```rust
/// Validate a task template and return a detailed report.
pub fn validate(template: &TaskTemplate) -> ValidationReport {
    let registry = standard_capability_registry();
    validate_with_registry(template, &registry)
}

/// Validate a task template with a custom capability registry.
///
/// The registry is used for composition validation — steps with a `composition`
/// field are validated against the registered capabilities. For batch validation
/// of many templates, prefer this over [`validate`] to amortize registry construction.
pub fn validate_with_registry(
    template: &TaskTemplate,
    registry: &dyn CapabilityRegistry,
) -> ValidationReport {
    let mut findings = Vec::new();
    let mut has_cycles = false;

    check_duplicate_step_names(template, &mut findings);
    check_dependencies(template, &mut findings);
    check_handlers(template, &mut findings);
    check_namespace_length(template, &mut findings);
    check_schemas(template, &mut findings);
    check_orphan_steps(template, &mut findings);
    check_compositions(template, registry, &mut findings);

    if let Some(cycle_findings) = check_cycles(template) {
        has_cycles = true;
        findings.extend(cycle_findings);
    }

    let valid = !findings.iter().any(|f| f.severity == Severity::Error);

    ValidationReport {
        valid,
        findings,
        step_count: template.steps.len(),
        has_cycles,
    }
}
```

3. Add the `check_compositions` function:
```rust
fn check_compositions(
    template: &TaskTemplate,
    registry: &dyn CapabilityRegistry,
    findings: &mut Vec<ValidationFinding>,
) {
    for step in &template.steps {
        if step.composition.is_some() {
            let step_findings =
                crate::composition_validator::validate_step_composition(step, registry);
            findings.extend(step_findings);
        }
    }
}
```

- [ ] **Step 4: Run all tests**

Run: `cargo test -p tasker-sdk --lib`
Expected: All tests pass — both existing template_validator tests and new composition tests

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-sdk/src/template_validator/mod.rs crates/tasker-sdk/src/composition_validator/tests.rs
git commit -m "feat(TAS-337): integrate composition validation into template_validator pipeline"
```

---

## Task 9: Integration test with workflow fixtures

**Files:**
- Modify: `crates/tasker-sdk/src/composition_validator/tests.rs`

- [ ] **Step 1: Add integration tests with all 3 workflow fixtures**

Add to `crates/tasker-sdk/src/composition_validator/tests.rs`:

```rust
/// Helper: build a template YAML string with a single composition step,
/// using serde round-tripping to avoid fragile YAML indentation.
fn template_with_composition_fixture(fixture_yaml: &str, step_name: &str) -> String {
    // Parse fixture YAML to a serde_json::Value (avoids indentation issues)
    let composition_value: serde_json::Value = serde_yaml::from_str(fixture_yaml)
        .expect("fixture YAML should parse");
    // Build a minimal template with the composition embedded
    let template_value = serde_json::json!({
        "name": format!("{step_name}_template"),
        "namespace_name": "test",
        "version": "1.0.0",
        "steps": [{
            "name": step_name,
            "handler": { "callable": format!("grammar:{step_name}") },
            "composition": composition_value
        }]
    });
    serde_yaml::to_string(&template_value).expect("should serialize to YAML")
}

#[test]
fn workflow_fixture_ecommerce_validates_in_template() {
    let fixture = include_str!(
        "../../../../crates/tasker-grammar/tests/fixtures/workflows/ecommerce_order_processing.yaml"
    );
    let yaml = template_with_composition_fixture(fixture, "process_order");
    let template = parse_template_str(&yaml).unwrap();
    let report = template_validator::validate(&template);
    let errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("COMPOSITION_") && f.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "ecommerce fixture errors: {errors:?}");
}

#[test]
fn workflow_fixture_payment_reconciliation_validates_in_template() {
    let fixture = include_str!(
        "../../../../crates/tasker-grammar/tests/fixtures/workflows/payment_reconciliation.yaml"
    );
    let yaml = template_with_composition_fixture(fixture, "reconcile_payments");
    let template = parse_template_str(&yaml).unwrap();
    let report = template_validator::validate(&template);
    let errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("COMPOSITION_") && f.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "payment fixture errors: {errors:?}");
}

#[test]
fn workflow_fixture_customer_onboarding_validates_in_template() {
    let fixture = include_str!(
        "../../../../crates/tasker-grammar/tests/fixtures/workflows/customer_onboarding.yaml"
    );
    let yaml = template_with_composition_fixture(fixture, "onboard_customer");
    let template = parse_template_str(&yaml).unwrap();
    let report = template_validator::validate(&template);
    let errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code.starts_with("COMPOSITION_") && f.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "onboarding fixture errors: {errors:?}");
}
```

- [ ] **Step 2: Run the integration tests**

Run: `cargo test -p tasker-sdk --lib workflow_fixture`
Expected: All 3 pass — each workflow fixture validates cleanly through the full pipeline

- [ ] **Step 3: Commit**

```bash
git add crates/tasker-sdk/src/composition_validator/tests.rs
git commit -m "test(TAS-337): add integration test with ecommerce workflow fixture"
```

---

## Task 10: Final verification and cleanup

**Files:** None (verification only)

- [ ] **Step 1: Run full SDK test suite**

Run: `cargo test -p tasker-sdk --lib`
Expected: All tests pass

- [ ] **Step 2: Run grammar test suite**

Run: `cargo test -p tasker-grammar --lib`
Expected: All tests pass

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --all-targets --all-features --workspace`
Expected: Zero warnings

- [ ] **Step 4: Run full workspace build check**

Run: `cargo check --all-features`
Expected: Clean build

- [ ] **Step 5: Verify no-infra tests pass**

Run: `cargo make test-no-infra`
Expected: All tests pass (composition validation is pure — no infrastructure needed)

- [ ] **Step 6: Commit any clippy fixes if needed**

```bash
git add -A
git commit -m "chore(TAS-337): clippy fixes"
```
