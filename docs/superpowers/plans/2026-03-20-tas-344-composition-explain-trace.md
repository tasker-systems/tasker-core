# TAS-344: Composition Explain Trace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `composition_explain` tool that traces data flow through a composition, with optional simulated evaluation against user-provided sample data.

**Architecture:** `ExplainAnalyzer` in `tasker-grammar` performs the core analysis (static trace + optional simulation). `tasker-sdk` wraps it as `explain_composition()`. `tasker-mcp` exposes it as a Tier 1 offline tool, `tasker-ctl` as a CLI subcommand. This mirrors the existing `CompositionValidator` → `validate_composition_yaml` → `composition_validate` pattern.

**Tech Stack:** Rust, jaq-core (expression evaluation), serde_json, regex (expression reference extraction), tasker-grammar/tasker-sdk/tasker-mcp/tasker-ctl crates.

**Spec:** `docs/superpowers/specs/2026-03-20-tas-344-composition-explain-trace-design.md`

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/tasker-grammar/src/explain/mod.rs` | Module root: re-exports types and analyzer |
| `crates/tasker-grammar/src/explain/types.rs` | `ExplanationTrace`, `InvocationTrace`, `EnvelopeSnapshot`, `ExpressionReference`, `OutcomeSummary`, `SimulationInput` |
| `crates/tasker-grammar/src/explain/analyzer.rs` | `ExplainAnalyzer` with `analyze()` and `analyze_with_simulation()` |
| `crates/tasker-grammar/src/explain/tests.rs` | All analyzer + extract_references tests |

### Modified files

| File | Change |
|------|--------|
| `crates/tasker-grammar/src/lib.rs` | Add `pub mod explain;` and re-export key types |
| `crates/tasker-grammar/src/expression/mod.rs` | Add `extract_references()` method to `ExpressionEngine` |
| `crates/tasker-grammar/src/expression/tests.rs` | Add `extract_references` unit tests |
| `crates/tasker-sdk/src/grammar_query.rs` | Add `explain_composition()` function and `CompositionExplanation` type |
| `crates/tasker-mcp/src/tools/params.rs` | Add `CompositionExplainParams` struct |
| `crates/tasker-mcp/src/tools/developer.rs` | Add `composition_explain()` handler function |
| `crates/tasker-mcp/src/server.rs` | Register `composition_explain` tool (line ~458) |
| `crates/tasker-mcp/tests/mcp_protocol_test.rs` | Update tool count assertions (13→14, 36→37, 30→31) |
| `crates/tasker-ctl/src/commands/grammar.rs` | Add `Explain` variant to `CompositionCommands` and handler |

---

## Task 1: Expression Reference Extraction

Add `extract_references()` to `ExpressionEngine` — the foundation for the explain trace's expression path analysis.

**Files:**
- Modify: `crates/tasker-grammar/src/expression/mod.rs`
- Test: `crates/tasker-grammar/src/expression/tests.rs`

- [ ] **Step 1: Write failing tests for `extract_references`**

Add to `crates/tasker-grammar/src/expression/tests.rs`:

```rust
#[test]
fn extract_references_simple_context() {
    let engine = ExpressionEngine::with_defaults();
    let refs = engine.extract_references(".context.order_id").unwrap();
    assert_eq!(refs, vec![".context.order_id"]);
}

#[test]
fn extract_references_prev_field() {
    let engine = ExpressionEngine::with_defaults();
    let refs = engine.extract_references(".prev.total").unwrap();
    assert_eq!(refs, vec![".prev.total"]);
}

#[test]
fn extract_references_deps_nested() {
    let engine = ExpressionEngine::with_defaults();
    let refs = engine.extract_references(".deps.step_a.result").unwrap();
    assert_eq!(refs, vec![".deps.step_a.result"]);
}

#[test]
fn extract_references_step_metadata() {
    let engine = ExpressionEngine::with_defaults();
    let refs = engine.extract_references(".step.name").unwrap();
    assert_eq!(refs, vec![".step.name"]);
}

#[test]
fn extract_references_multiple_paths() {
    let engine = ExpressionEngine::with_defaults();
    let mut refs = engine
        .extract_references("{total: .prev.amount, id: .context.order_id}")
        .unwrap();
    refs.sort();
    assert_eq!(refs, vec![".context.order_id", ".prev.amount"]);
}

#[test]
fn extract_references_root_only() {
    let engine = ExpressionEngine::with_defaults();
    let refs = engine.extract_references(".context | keys").unwrap();
    assert_eq!(refs, vec![".context"]);
}

#[test]
fn extract_references_no_envelope_paths() {
    let engine = ExpressionEngine::with_defaults();
    let refs = engine.extract_references("42").unwrap();
    assert!(refs.is_empty());
}

#[test]
fn extract_references_deduplicates() {
    let engine = ExpressionEngine::with_defaults();
    let refs = engine
        .extract_references("{a: .prev.x, b: .prev.x}")
        .unwrap();
    assert_eq!(refs, vec![".prev.x"]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run --features test-messaging -p tasker-grammar -E 'test(extract_references)'`
Expected: FAIL — `extract_references` method not found.

- [ ] **Step 3: Implement `extract_references`**

Add to `crates/tasker-grammar/src/expression/mod.rs`, after the `validate_syntax` method (line 88):

```rust
/// Extract envelope field references from a jaq expression.
///
/// Scans the expression string for path patterns rooted at the four envelope
/// fields: `.context`, `.deps`, `.prev`, `.step`. Returns deduplicated paths
/// sorted alphabetically.
///
/// Uses regex-based extraction (jaq-core compiles to an opaque `Filter` type
/// with no walkable AST). Handles common patterns: `.context.field`,
/// `.deps.step_name.field`, `.prev.nested.path`. Dynamic patterns like
/// `.context | keys` are captured as the root reference (`.context`).
pub fn extract_references(&self, expression: &str) -> Result<Vec<String>, ExpressionError> {
    // First validate syntax so we don't extract from invalid expressions
    self.validate_syntax(expression)?;

    use std::collections::BTreeSet;

    let pattern = regex::Regex::new(r"\.(context|deps|prev|step)(\.[a-zA-Z_][a-zA-Z0-9_]*)*")
        .expect("static regex");

    let refs: BTreeSet<String> = pattern
        .find_iter(expression)
        .map(|m| m.as_str().to_owned())
        .collect();

    Ok(refs.into_iter().collect())
}
```

Add `regex` dependency to `crates/tasker-grammar/Cargo.toml` under `[dependencies]`:
```toml
regex = { workspace = true }
```
(The workspace root already defines `regex = "1.12"`.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run --features test-messaging -p tasker-grammar -E 'test(extract_references)'`
Expected: All 8 tests PASS.

- [ ] **Step 5: Run clippy on the grammar crate**

Run: `cargo clippy --all-targets --all-features -p tasker-grammar`
Expected: Zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/tasker-grammar/src/expression/mod.rs crates/tasker-grammar/src/expression/tests.rs crates/tasker-grammar/Cargo.toml
git commit -m "feat(TAS-344): add extract_references to ExpressionEngine

Regex-based extraction of envelope path references (.context, .deps,
.prev, .step) from jaq expressions. Foundation for composition explain
trace data flow analysis."
```

---

## Task 2: Explain Module Types

Define the core types for the explanation trace in `tasker-grammar`.

**Files:**
- Create: `crates/tasker-grammar/src/explain/mod.rs`
- Create: `crates/tasker-grammar/src/explain/types.rs`
- Modify: `crates/tasker-grammar/src/lib.rs`

- [ ] **Step 1: Create the explain module root**

Create `crates/tasker-grammar/src/explain/mod.rs`:

```rust
//! Composition explanation and data flow tracing.
//!
//! The [`ExplainAnalyzer`] produces an [`ExplanationTrace`] that visualizes how
//! data flows through a composition's invocation chain. Two modes:
//!
//! - **Static analysis**: traces structure, envelope field availability, expression
//!   references, output schemas, and checkpoint placement.
//! - **Simulated evaluation**: when [`SimulationInput`] is provided, evaluates jaq
//!   expressions against sample data and threads computed results through the chain.
//!
//! **Ticket**: TAS-344

mod analyzer;
mod types;

pub use analyzer::ExplainAnalyzer;
pub use types::{
    EnvelopeSnapshot, ExplanationTrace, ExpressionReference, InvocationTrace, OutcomeSummary,
    SimulationInput,
};

#[cfg(test)]
mod tests;
```

- [ ] **Step 2: Create the types file**

Create `crates/tasker-grammar/src/explain/types.rs`:

```rust
use std::collections::HashMap;

use serde_json::Value;

use crate::types::{GrammarCategoryKind, ValidationFinding};

/// Complete trace of data flow through a composition.
#[derive(Debug, Clone)]
pub struct ExplanationTrace {
    /// Composition name (if declared).
    pub name: Option<String>,
    /// Declared outcome description and output schema.
    pub outcome: OutcomeSummary,
    /// Per-invocation trace entries, in execution order.
    pub invocations: Vec<InvocationTrace>,
    /// Validation findings (errors/warnings) from the underlying validator.
    pub validation: Vec<ValidationFinding>,
    /// Whether simulation was performed (sample data provided).
    pub simulated: bool,
}

/// Summary of the declared outcome.
#[derive(Debug, Clone)]
pub struct OutcomeSummary {
    /// Human-readable description of what the composition achieves.
    pub description: String,
    /// JSON Schema for the composition's output.
    pub output_schema: Value,
}

/// Trace for a single capability invocation.
#[derive(Debug, Clone)]
pub struct InvocationTrace {
    /// Position in the invocation chain (0-based).
    pub index: usize,
    /// Capability name.
    pub capability: String,
    /// Grammar category.
    pub category: GrammarCategoryKind,
    /// Whether this is a checkpoint boundary.
    pub checkpoint: bool,
    /// Whether this capability is mutating.
    pub is_mutating: bool,
    /// Envelope fields available at this invocation.
    pub envelope_available: EnvelopeSnapshot,
    /// Jaq expressions found in config and which envelope paths they reference.
    pub expressions: Vec<ExpressionReference>,
    /// Declared output schema (if any — transforms declare this).
    pub output_schema: Option<Value>,
    /// Simulated output value (when sample data provided).
    pub simulated_output: Option<Value>,
    /// For side-effecting capabilities: whether a mock output was provided.
    pub mock_output_used: bool,
}

/// What's available in the envelope at a given point in the chain.
#[derive(Debug, Clone)]
pub struct EnvelopeSnapshot {
    /// Always true — task-level input.
    pub context: bool,
    /// Always true — dependency step results.
    pub deps: bool,
    /// Always true — step metadata.
    pub step: bool,
    /// Whether .prev is non-null at this point.
    pub has_prev: bool,
    /// Description of what .prev contains (e.g., "output of invocation 0 (transform)").
    pub prev_source: Option<String>,
    /// Schema of .prev if known (from prior invocation's output schema).
    pub prev_schema: Option<Value>,
}

/// A jaq expression found in an invocation's config.
#[derive(Debug, Clone)]
pub struct ExpressionReference {
    /// Config field path (e.g., "filter", "data.expression").
    pub field_path: String,
    /// The raw expression string.
    pub expression: String,
    /// Envelope paths referenced (e.g., [".context.order_id", ".prev.total"]).
    pub referenced_paths: Vec<String>,
    /// Simulated result value (when sample data provided).
    pub simulated_result: Option<Value>,
}

/// Sample data for simulated evaluation.
#[derive(Debug, Clone)]
pub struct SimulationInput {
    /// Sample task-level input — populates .context
    pub context: Value,
    /// Sample dependency results — populates .deps
    pub deps: Value,
    /// Sample step metadata — populates .step
    pub step: Value,
    /// Mock outputs for side-effecting invocations, keyed by invocation index.
    /// Used as .prev for the next invocation when the capability can't be
    /// evaluated purely (persist, acquire, emit).
    pub mock_outputs: HashMap<usize, Value>,
}
```

- [ ] **Step 3: Register the module in lib.rs**

In `crates/tasker-grammar/src/lib.rs`, add `pub mod explain;` after line 27 (`pub mod executor;`), and add re-exports after the existing re-export block (after line 53):

```rust
pub use explain::{
    EnvelopeSnapshot, ExplainAnalyzer, ExplanationTrace, ExpressionReference, InvocationTrace,
    OutcomeSummary, SimulationInput,
};
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check --all-features -p tasker-grammar`
Expected: Compiles (analyzer.rs and tests.rs will be empty stubs or `// TODO` for now — create minimal files so the module compiles).

Create stub `crates/tasker-grammar/src/explain/analyzer.rs`:
```rust
use crate::explain::types::{ExplanationTrace, SimulationInput};
use crate::types::CompositionSpec;
use crate::validation::CapabilityRegistry;
use crate::ExpressionEngine;

/// Analyzes a CompositionSpec to produce a data flow trace.
pub struct ExplainAnalyzer<'a> {
    registry: &'a dyn CapabilityRegistry,
    expression_engine: &'a ExpressionEngine,
}

impl std::fmt::Debug for ExplainAnalyzer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExplainAnalyzer")
            .field("expression_engine", &self.expression_engine)
            .finish_non_exhaustive()
    }
}

impl<'a> ExplainAnalyzer<'a> {
    /// Create a new analyzer with the given capability registry and expression engine.
    pub fn new(
        registry: &'a dyn CapabilityRegistry,
        expression_engine: &'a ExpressionEngine,
    ) -> Self {
        Self {
            registry,
            expression_engine,
        }
    }

    /// Produce a static analysis trace (no expression evaluation).
    pub fn analyze(&self, _spec: &CompositionSpec) -> ExplanationTrace {
        todo!("Task 3 implements this")
    }

    /// Produce a trace with simulated expression evaluation.
    pub fn analyze_with_simulation(
        &self,
        _spec: &CompositionSpec,
        _input: &SimulationInput,
    ) -> ExplanationTrace {
        todo!("Task 4 implements this")
    }
}
```

Create stub `crates/tasker-grammar/src/explain/tests.rs`:
```rust
// Tests added in Tasks 3 and 4.
```

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-grammar/src/explain/ crates/tasker-grammar/src/lib.rs
git commit -m "feat(TAS-344): add explain module types and analyzer stub

ExplanationTrace, InvocationTrace, EnvelopeSnapshot, ExpressionReference,
SimulationInput, OutcomeSummary types. ExplainAnalyzer struct with stub
methods. Module registered in tasker-grammar lib.rs."
```

---

## Task 3: ExplainAnalyzer Static Analysis

Implement `ExplainAnalyzer::analyze()` — the static trace mode with no expression evaluation.

**Files:**
- Modify: `crates/tasker-grammar/src/explain/analyzer.rs`
- Modify: `crates/tasker-grammar/src/explain/tests.rs`

**Reference files (read, don't modify):**
- `crates/tasker-grammar/src/validation/validator.rs` — `CompositionValidator`, `extract_expression()`, expression field mapping per category (lines 431-489)
- `crates/tasker-grammar/src/executor/mod.rs` — `build_envelope()` function (lines 400-428)
- `crates/tasker-grammar/src/types/composition.rs` — `CompositionSpec`, `CapabilityInvocation`

- [ ] **Step 1: Write failing tests for static analysis**

Add to `crates/tasker-grammar/src/explain/tests.rs`:

```rust
use std::collections::HashMap;
use serde_json::json;

use crate::explain::{ExplainAnalyzer, SimulationInput};
use crate::types::{
    CapabilityDeclaration, CapabilityInvocation, CompositionSpec, GrammarCategoryKind,
    MutationProfile, OutcomeDeclaration,
};
use crate::ExpressionEngine;

fn test_registry() -> HashMap<String, CapabilityDeclaration> {
    let mut reg = HashMap::new();
    reg.insert("transform".to_owned(), CapabilityDeclaration {
        name: "transform".to_owned(),
        grammar_category: GrammarCategoryKind::Transform,
        description: "Pure data transformation".to_owned(),
        config_schema: json!({
            "type": "object",
            "required": ["output", "filter"],
            "properties": {
                "output": {"type": "object"},
                "filter": {"type": "string"}
            }
        }),
        mutation_profile: MutationProfile::NonMutating,
        tags: vec![],
        version: "1.0.0".to_owned(),
    });
    reg.insert("persist".to_owned(), CapabilityDeclaration {
        name: "persist".to_owned(),
        grammar_category: GrammarCategoryKind::Persist,
        description: "Write data to a resource".to_owned(),
        config_schema: json!({
            "type": "object",
            "required": ["resource", "data"],
            "properties": {
                "resource": {"type": "object"},
                "data": {"type": "object"}
            }
        }),
        mutation_profile: MutationProfile::Mutating { supports_idempotency_key: true },
        tags: vec![],
        version: "1.0.0".to_owned(),
    });
    reg.insert("assert".to_owned(), CapabilityDeclaration {
        name: "assert".to_owned(),
        grammar_category: GrammarCategoryKind::Assert,
        description: "Boolean assertion gate".to_owned(),
        config_schema: json!({
            "type": "object",
            "required": ["filter"],
            "properties": {
                "filter": {"type": "string"},
                "error": {"type": "string"}
            }
        }),
        mutation_profile: MutationProfile::NonMutating,
        tags: vec![],
        version: "1.0.0".to_owned(),
    });
    reg
}

fn test_engine() -> ExpressionEngine {
    ExpressionEngine::with_defaults()
}

#[test]
fn analyze_single_transform() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Test outcome".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {"type": "object", "properties": {"result": {"type": "string"}}},
                "filter": "{result: .context.name}"
            }),
            checkpoint: false,
        }],
    };

    let trace = analyzer.analyze(&spec);
    assert!(!trace.simulated);
    assert_eq!(trace.name, Some("test".to_owned()));
    assert_eq!(trace.invocations.len(), 1);

    let inv = &trace.invocations[0];
    assert_eq!(inv.index, 0);
    assert_eq!(inv.capability, "transform");
    assert_eq!(inv.category, GrammarCategoryKind::Transform);
    assert!(!inv.checkpoint);
    assert!(!inv.is_mutating);
    assert!(!inv.envelope_available.has_prev);
    assert!(inv.envelope_available.prev_source.is_none());
    assert!(inv.output_schema.is_some());
    assert!(inv.simulated_output.is_none());

    // Should have one expression reference for the filter
    assert_eq!(inv.expressions.len(), 1);
    assert_eq!(inv.expressions[0].field_path, "config.filter");
    assert!(inv.expressions[0].referenced_paths.contains(&".context.name".to_owned()));
}

#[test]
fn analyze_multi_invocation_chain() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("chain".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Chained".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object", "properties": {"x": {"type": "number"}}},
                    "filter": "{x: .context.value}"
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object", "properties": {"doubled": {"type": "number"}}},
                    "filter": "{doubled: (.prev.x * 2)}"
                }),
                checkpoint: false,
            },
        ],
    };

    let trace = analyzer.analyze(&spec);
    assert_eq!(trace.invocations.len(), 2);

    // First invocation: no .prev
    assert!(!trace.invocations[0].envelope_available.has_prev);

    // Second invocation: .prev comes from invocation 0
    let inv1 = &trace.invocations[1];
    assert!(inv1.envelope_available.has_prev);
    assert!(inv1.envelope_available.prev_source.as_ref().unwrap().contains("invocation 0"));
    assert!(inv1.envelope_available.prev_schema.is_some());
    assert!(inv1.expressions[0].referenced_paths.contains(&".prev.x".to_owned()));
}

#[test]
fn analyze_checkpoint_and_mutating() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Persist test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": {"type": "postgres", "table": "orders"},
                "data": {"expression": ".prev.payload"}
            }),
            checkpoint: true,
        }],
    };

    let trace = analyzer.analyze(&spec);
    let inv = &trace.invocations[0];
    assert!(inv.checkpoint);
    assert!(inv.is_mutating);
    assert_eq!(inv.expressions.len(), 1);
    assert_eq!(inv.expressions[0].field_path, "config.data.expression");
}

#[test]
fn analyze_empty_composition() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Empty".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![],
    };

    let trace = analyzer.analyze(&spec);
    assert!(trace.invocations.is_empty());
    assert!(trace.validation.iter().any(|f| f.code == "EMPTY_COMPOSITION"));
}

#[test]
fn analyze_missing_capability() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Bad ref".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "nonexistent".to_owned(),
            config: json!({}),
            checkpoint: false,
        }],
    };

    let trace = analyzer.analyze(&spec);
    // Should still produce a trace entry (partial trace)
    assert_eq!(trace.invocations.len(), 1);
    assert!(trace.validation.iter().any(|f| f.code == "MISSING_CAPABILITY"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run --features test-messaging -p tasker-grammar -E 'test(analyze_)'`
Expected: FAIL — `analyze` returns `todo!()`.

- [ ] **Step 3: Implement `ExplainAnalyzer::analyze()`**

Replace the stub in `crates/tasker-grammar/src/explain/analyzer.rs` with the full implementation. Key logic:

1. Run `CompositionValidator::validate()`, capture findings.
2. For degenerate cases (empty, too many), return trace with empty invocations + findings.
3. Walk invocations, for each:
   - Resolve capability from registry (produce partial entry if missing).
   - Build `EnvelopeSnapshot` tracking `.prev` source and schema from prior invocation.
   - Extract expressions from category-specific config fields (reuse the field mapping from `validator.rs` lines 431-439).
   - For each expression, call `extract_references()`.
   - Extract output schema (transform: `config.output`).
   - Track output schema as `prev_schema` for the next invocation.

The analyzer uses the same `extract_expression()` helper pattern as the validator for extracting jaq expressions from flat strings vs ExpressionField objects.

Refer to:
- `crates/tasker-grammar/src/validation/validator.rs` lines 174-184 for `extract_expression()`
- `crates/tasker-grammar/src/validation/validator.rs` lines 431-439 for expression field mapping per category (top-level expression fields)
- `crates/tasker-grammar/src/validation/validator.rs` lines 464-489 for emit metadata expressions (nested under `config.metadata`: `correlation_id`, `idempotency_key`) — these must also be extracted
- `crates/tasker-grammar/src/validation/validator.rs` lines 616-631 for `extract_output_schema()`

**Important**: The `Validate` category has no expression fields (schema-based, not expression-based). Its `expressions` list should be empty.

**Error handling for `extract_references`**: If `extract_references` returns an error for a syntactically invalid expression, the analyzer should skip reference extraction for that expression (the syntax error is already captured by the validator's findings). Record an empty `referenced_paths` for that expression.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run --features test-messaging -p tasker-grammar -E 'test(analyze_)'`
Expected: All 5 tests PASS.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all-targets --all-features -p tasker-grammar`
Expected: Zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/tasker-grammar/src/explain/
git commit -m "feat(TAS-344): implement ExplainAnalyzer static analysis

Walks the invocation chain producing EnvelopeSnapshot, ExpressionReference,
and output schema tracking. Includes validation findings. Handles degenerate
cases (empty composition, missing capabilities) with partial traces."
```

---

## Task 4: ExplainAnalyzer Simulated Evaluation

Implement `ExplainAnalyzer::analyze_with_simulation()` — evaluating jaq expressions against sample data.

**Files:**
- Modify: `crates/tasker-grammar/src/explain/analyzer.rs`
- Modify: `crates/tasker-grammar/src/explain/tests.rs`

**Reference files:**
- `crates/tasker-grammar/src/executor/mod.rs` — `build_envelope()` (lines 400-428), execution loop pattern (lines 297-379)

- [ ] **Step 1: Write failing tests for simulation**

Add to `crates/tasker-grammar/src/explain/tests.rs`:

```rust
#[test]
fn analyze_with_simulation_threads_data() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: Some("sim_test".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Simulation".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object"},
                    "filter": "{doubled: (.context.value * 2)}"
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object"},
                    "filter": "{result: (.prev.doubled + 1)}"
                }),
                checkpoint: false,
            },
        ],
    };

    let input = SimulationInput {
        context: json!({"value": 21}),
        deps: json!({}),
        step: json!({"name": "test_step"}),
        mock_outputs: HashMap::new(),
    };

    let trace = analyzer.analyze_with_simulation(&spec, &input);
    assert!(trace.simulated);

    // First invocation: .context.value=21, doubled=42
    let inv0 = &trace.invocations[0];
    assert_eq!(inv0.simulated_output, Some(json!({"doubled": 42})));
    assert_eq!(inv0.expressions[0].simulated_result, Some(json!({"doubled": 42})));

    // Second invocation: .prev.doubled=42, result=43
    let inv1 = &trace.invocations[1];
    assert_eq!(inv1.simulated_output, Some(json!({"result": 43})));
}

#[test]
fn analyze_with_simulation_mock_outputs() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Mock test".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "persist".to_owned(),
                config: json!({
                    "resource": {"type": "postgres", "table": "orders"},
                    "data": {"expression": ".context.order"}
                }),
                checkpoint: true,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object"},
                    "filter": "{id: .prev.inserted_id}"
                }),
                checkpoint: false,
            },
        ],
    };

    let mut mock_outputs = HashMap::new();
    mock_outputs.insert(0, json!({"inserted_id": 42}));

    let input = SimulationInput {
        context: json!({"order": {"item": "widget"}}),
        deps: json!({}),
        step: json!({"name": "persist_step"}),
        mock_outputs,
    };

    let trace = analyzer.analyze_with_simulation(&spec, &input);

    // Persist invocation used mock output
    let inv0 = &trace.invocations[0];
    assert!(inv0.mock_output_used);
    assert_eq!(inv0.simulated_output, Some(json!({"inserted_id": 42})));

    // Transform reads .prev.inserted_id = 42
    let inv1 = &trace.invocations[1];
    assert!(!inv1.mock_output_used);
    assert_eq!(inv1.simulated_output, Some(json!({"id": 42})));
}

#[test]
fn analyze_with_simulation_missing_mock() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "No mock".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "persist".to_owned(),
            config: json!({
                "resource": {"type": "postgres", "table": "orders"},
                "data": {"expression": ".context.order"}
            }),
            checkpoint: true,
        }],
    };

    let input = SimulationInput {
        context: json!({"order": {"item": "widget"}}),
        deps: json!({}),
        step: json!({"name": "test"}),
        mock_outputs: HashMap::new(), // no mock for index 0
    };

    let trace = analyzer.analyze_with_simulation(&spec, &input);
    let inv = &trace.invocations[0];
    assert!(!inv.mock_output_used);
    // .prev becomes null — should have info finding about missing mock
    assert!(trace.validation.iter().any(|f| f.message.contains("mock")));
}

#[test]
fn analyze_with_simulation_assert_passthrough() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Assert passthrough".to_owned(),
            output_schema: json!({"type": "object"}),
        },
        invocations: vec![
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object"},
                    "filter": "{x: .context.value}"
                }),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "assert".to_owned(),
                config: json!({"filter": "(.prev.x > 0)", "error": "must be positive"}),
                checkpoint: false,
            },
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "output": {"type": "object"},
                    "filter": "{result: .prev.x}"
                }),
                checkpoint: false,
            },
        ],
    };

    let input = SimulationInput {
        context: json!({"value": 5}),
        deps: json!({}),
        step: json!({"name": "test"}),
        mock_outputs: HashMap::new(),
    };

    let trace = analyzer.analyze_with_simulation(&spec, &input);

    // Assert passes .prev through unchanged
    let assert_inv = &trace.invocations[1];
    assert_eq!(assert_inv.simulated_output, Some(json!({"x": 5})));

    // Next transform reads .prev.x which is still 5 from the first transform
    let final_inv = &trace.invocations[2];
    assert_eq!(final_inv.simulated_output, Some(json!({"result": 5})));
}

#[test]
fn analyze_with_simulation_expression_failure() {
    let registry = test_registry();
    let engine = test_engine();
    let analyzer = ExplainAnalyzer::new(&registry, &engine);

    let spec = CompositionSpec {
        name: None,
        outcome: OutcomeDeclaration {
            description: "Eval failure".to_owned(),
            output_schema: json!({}),
        },
        invocations: vec![CapabilityInvocation {
            capability: "transform".to_owned(),
            config: json!({
                "output": {"type": "object"},
                "filter": ".prev.missing_field | .nested"
            }),
            checkpoint: false,
        }],
    };

    let input = SimulationInput {
        context: json!({}),
        deps: json!({}),
        step: json!({"name": "test"}),
        mock_outputs: HashMap::new(),
    };

    let trace = analyzer.analyze_with_simulation(&spec, &input);
    // Should have a warning about the evaluation failure
    let inv = &trace.invocations[0];
    // simulated_output should be null or absent
    // trace should continue (not panic)
    assert!(trace.simulated);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run --features test-messaging -p tasker-grammar -E 'test(analyze_with_simulation)'`
Expected: FAIL — `analyze_with_simulation` returns `todo!()`.

- [ ] **Step 3: Implement `analyze_with_simulation()`**

In `crates/tasker-grammar/src/explain/analyzer.rs`, implement the simulation path. Key logic:

1. Start with the same static analysis pass.
2. Additionally, build an envelope at each step using the sample data + accumulated outputs. Include accumulated invocation outputs under `.deps.invocations.{idx}` (same as the real executor's `build_envelope` at `executor/mod.rs` lines 400-428) so that expressions referencing prior invocation outputs via `.deps.invocations.0.field` work correctly in simulation.
3. For transform: evaluate the `filter` expression against the envelope, use result as simulated output and next `.prev`.
4. For validate/assert: pass `.prev` through unchanged as simulated output.
5. For persist/acquire/emit: evaluate data/params/payload expressions to record `simulated_result` on the ExpressionReference, then check `mock_outputs` for the `.prev` value. If no mock, set `.prev` to `null` and add an info-level `ValidationFinding`.
6. On expression evaluation failure: record `null` as `simulated_result`, add a warning finding, continue.

Factor out shared logic between `analyze()` and `analyze_with_simulation()` into internal helpers to avoid duplication.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run --features test-messaging -p tasker-grammar -E 'test(analyze_)'`
Expected: All tests PASS (both static and simulation tests).

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all-targets --all-features -p tasker-grammar`
Expected: Zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/tasker-grammar/src/explain/
git commit -m "feat(TAS-344): implement simulated evaluation in ExplainAnalyzer

Evaluates jaq expressions against user-provided sample data, threading
computed values through the chain. Mock outputs for side-effecting
capabilities. Assert/validate pass .prev through unchanged. Graceful
handling of expression eval failures."
```

---

## Task 5: SDK Function

Add `explain_composition()` to `tasker_sdk::grammar_query` with serializable return types.

**Files:**
- Modify: `crates/tasker-sdk/src/grammar_query.rs`

**Reference:** Existing `validate_composition_yaml()` at lines 221-285 for the pattern.

- [ ] **Step 1: Write failing tests**

Add to the `#[cfg(test)] mod tests` block in `crates/tasker-sdk/src/grammar_query.rs`:

```rust
#[test]
fn explain_composition_static() {
    let yaml = r#"
name: test
outcome:
  description: Test outcome
  output_schema: {}
invocations:
  - capability: transform
    config:
      output:
        type: object
        properties:
          x:
            type: string
        required: [x]
      filter: "{x: .context.name}"
    checkpoint: false
"#;
    let explanation = explain_composition(yaml, None);
    assert!(!explanation.simulated);
    assert_eq!(explanation.invocations.len(), 1);
    assert_eq!(explanation.invocations[0].capability, "transform");
    assert_eq!(explanation.invocations[0].category, "Transform");
    assert!(!explanation.invocations[0].expressions.is_empty());
}

#[test]
fn explain_composition_with_simulation() {
    let yaml = r#"
name: sim
outcome:
  description: Simulation
  output_schema: {}
invocations:
  - capability: transform
    config:
      output: {type: object}
      filter: "{doubled: (.context.value * 2)}"
    checkpoint: false
"#;
    let sim = tasker_grammar::explain::SimulationInput {
        context: serde_json::json!({"value": 21}),
        deps: serde_json::json!({}),
        step: serde_json::json!({"name": "test"}),
        mock_outputs: std::collections::HashMap::new(),
    };
    let explanation = explain_composition(yaml, Some(sim));
    assert!(explanation.simulated);
    assert_eq!(
        explanation.invocations[0].simulated_output,
        Some(serde_json::json!({"doubled": 42}))
    );
}

#[test]
fn explain_composition_invalid_yaml() {
    let explanation = explain_composition("not: valid: yaml: [[[", None);
    assert!(explanation
        .findings
        .iter()
        .any(|f| f.code == "PARSE_ERROR"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run --features test-messaging -p tasker-sdk -E 'test(explain_composition)'`
Expected: FAIL — function not found.

- [ ] **Step 3: Add serializable types and `explain_composition()` function**

Add to `crates/tasker-sdk/src/grammar_query.rs`:

1. New return types: `CompositionExplanation`, `InvocationExplanation`, `EnvelopeSnapshotInfo`, `ExpressionReferenceInfo`, `OutcomeInfo` — all `#[derive(Debug, Serialize)]`. These mirror the grammar-layer types with `GrammarCategoryKind` → `String` and `ValidationFinding` → `CompositionFinding` conversions.

2. The `explain_composition()` function following the same pattern as `validate_composition_yaml()`:
   - Parse YAML/JSON (try YAML first, fall back to JSON)
   - On parse failure, return explanation with PARSE_ERROR finding
   - Construct `ExplainAnalyzer` with `standard_capability_registry()` and `ExpressionEngine::with_defaults()`
   - Call `analyze()` or `analyze_with_simulation()` based on whether `simulation` is `Some`
   - Map `ExplanationTrace` to `CompositionExplanation`

Re-export `SimulationInput` from tasker-grammar so MCP/CLI layers can construct it.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run --features test-messaging -p tasker-sdk -E 'test(explain_composition)'`
Expected: All 3 tests PASS.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --all-targets --all-features -p tasker-sdk`
Expected: Zero warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/tasker-sdk/src/grammar_query.rs
git commit -m "feat(TAS-344): add explain_composition to SDK grammar_query

Serializable CompositionExplanation type and explain_composition() function
wrapping ExplainAnalyzer. Supports static analysis and simulated evaluation.
Re-exports SimulationInput from tasker-grammar."
```

---

## Task 6: MCP Tool

Add the `composition_explain` Tier 1 offline tool to `tasker-mcp`.

**Files:**
- Modify: `crates/tasker-mcp/src/tools/params.rs` (after line 796)
- Modify: `crates/tasker-mcp/src/tools/developer.rs` (after line 368, imports at line ~18)
- Modify: `crates/tasker-mcp/src/server.rs` (after line 458)
- Modify: `crates/tasker-mcp/tests/mcp_protocol_test.rs` (update assertions)

**Reference:** Existing `composition_validate` pattern at `developer.rs:364-368`, `params.rs:788-796`, `server.rs:448-458`.

- [ ] **Step 1: Add parameter struct**

Add to `crates/tasker-mcp/src/tools/params.rs` after the `CompositionValidateParams` struct (after line 796):

```rust
// ── composition_explain ──

/// Parameters for the `composition_explain` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CompositionExplainParams {
    /// Composition spec as YAML or JSON string.
    #[schemars(
        description = "Composition spec as YAML or JSON string. Must include name, outcome (with output_schema), and invocations array."
    )]
    pub composition_yaml: String,

    /// Optional sample context data as JSON string for simulated evaluation.
    /// Populates .context in the composition envelope.
    #[schemars(description = "Sample task-level input data as JSON string. Populates .context in the envelope for simulated evaluation.")]
    #[serde(default)]
    pub sample_context: Option<String>,

    /// Optional sample dependency results as JSON string.
    /// Populates .deps in the composition envelope.
    #[schemars(description = "Sample dependency step results as JSON string. Populates .deps in the envelope.")]
    #[serde(default)]
    pub sample_deps: Option<String>,

    /// Optional sample step metadata as JSON string.
    /// Populates .step in the composition envelope.
    #[schemars(description = "Sample step metadata as JSON string. Populates .step in the envelope.")]
    #[serde(default)]
    pub sample_step: Option<String>,

    /// Optional mock outputs for side-effecting invocations as JSON string.
    /// Object keyed by invocation index: {"0": {"id": 123}, "2": {"status": "sent"}}
    #[schemars(description = "Mock outputs for side-effecting invocations as JSON object keyed by invocation index (e.g. {\"0\": {\"id\": 123}}). Used as .prev for subsequent invocations.")]
    #[serde(default)]
    pub mock_outputs: Option<String>,
}
```

- [ ] **Step 2: Add handler function**

Add to `crates/tasker-mcp/src/tools/developer.rs` after `composition_validate` (after line 368):

```rust
pub fn composition_explain(params: CompositionExplainParams) -> String {
    use std::collections::HashMap;
    use tasker_grammar::SimulationInput;

    // Build SimulationInput if any sample data provided
    let simulation = if params.sample_context.is_some()
        || params.sample_deps.is_some()
        || params.sample_step.is_some()
        || params.mock_outputs.is_some()
    {
        let context = params
            .sample_context
            .as_deref()
            .map(|s| serde_json::from_str(s).unwrap_or(serde_json::Value::Null))
            .unwrap_or(serde_json::Value::Null);
        let deps = params
            .sample_deps
            .as_deref()
            .map(|s| serde_json::from_str(s).unwrap_or(serde_json::Value::Null))
            .unwrap_or(serde_json::Value::Null);
        let step = params
            .sample_step
            .as_deref()
            .map(|s| serde_json::from_str(s).unwrap_or(serde_json::Value::Null))
            .unwrap_or(serde_json::Value::Null);
        let mock_outputs: HashMap<usize, serde_json::Value> = params
            .mock_outputs
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(s).ok())
            .map(|map| {
                map.into_iter()
                    .filter_map(|(k, v)| k.parse::<usize>().ok().map(|idx| (idx, v)))
                    .collect()
            })
            .unwrap_or_default();

        Some(SimulationInput {
            context,
            deps,
            step,
            mock_outputs,
        })
    } else {
        None
    };

    let explanation = grammar_query::explain_composition(&params.composition_yaml, simulation);
    serde_json::to_string_pretty(&explanation)
        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
}
```

Add `CompositionExplainParams` to the imports from `crate::tools::params`.

- [ ] **Step 3: Register the tool in server.rs**

Add after the `composition_validate` tool registration (after line 458 in `crates/tasker-mcp/src/server.rs`):

```rust
/// Explain data flow through a composition spec.
#[tool(
    name = "composition_explain",
    description = "Analyze and explain data flow through a CompositionSpec. Shows how data threads through invocations via the envelope (.context, .deps, .prev, .step), which jaq expressions reference which fields, checkpoint placement, and output schemas. Optionally evaluates expressions against sample data for simulated execution. Works offline."
)]
pub async fn composition_explain(
    &self,
    Parameters(params): Parameters<CompositionExplainParams>,
) -> String {
    developer::composition_explain(params)
}
```

Update the Tier 1 tools doc comment in server.rs to mention 14 tools and include `composition_explain`.

- [ ] **Step 4: Update test assertions**

In `crates/tasker-mcp/tests/mcp_protocol_test.rs`:
- Add `"composition_explain"` to the offline tools list (alphabetically before `"composition_validate"`).
- Update assertion: 13 → 14 Tier 1 tools.
- Update full tool count: 36 → 37.
- Update T1+profile+T2 count: 30 → 31.
- Update doc comment mentioning tool counts.

- [ ] **Step 5: Verify compilation and tests**

Run: `cargo check --all-features -p tasker-mcp`
Run: `cargo clippy --all-targets --all-features -p tasker-mcp`
Expected: Compiles, zero clippy warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/tasker-mcp/
git commit -m "feat(TAS-344): add composition_explain MCP tool

Tier 1 offline tool for data flow visualization. Accepts composition
YAML with optional sample data for simulated evaluation. Brings Tier 1
count from 13 to 14."
```

---

## Task 7: CLI Command

Add `tasker-ctl grammar composition explain` subcommand.

**Files:**
- Modify: `crates/tasker-ctl/src/commands/grammar.rs`

**Reference:** Existing `composition_validate` at grammar.rs lines 60-69 (enum variant) and lines 84-88 (dispatch) and the validate handler (lines ~270-365).

- [ ] **Step 1: Add Explain variant to CompositionCommands**

In `crates/tasker-ctl/src/commands/grammar.rs`, add the Explain variant after Validate (line 69):

```rust
/// Explain data flow through a composition spec file
Explain {
    /// Path to the composition file
    path: PathBuf,

    /// Explain only the named step's composition within a full task template
    #[arg(long)]
    step: Option<String>,

    /// Sample context data (inline JSON or @path/to/file.json)
    #[arg(long)]
    sample_context: Option<String>,

    /// Sample dependency results (inline JSON or @path/to/file.json)
    #[arg(long)]
    sample_deps: Option<String>,

    /// Sample step metadata (inline JSON or @path/to/file.json)
    #[arg(long)]
    sample_step: Option<String>,

    /// Mock outputs for side-effecting invocations (inline JSON or @path/to/file.json)
    #[arg(long)]
    mock_outputs: Option<String>,
},
```

- [ ] **Step 2: Add dispatch arm**

In the `handle_grammar_command` match (line 84-88), add the Explain arm:

```rust
CompositionCommands::Explain {
    path,
    step,
    sample_context,
    sample_deps,
    sample_step,
    mock_outputs,
} => composition_explain(
    &path,
    step.as_deref(),
    sample_context.as_deref(),
    sample_deps.as_deref(),
    sample_step.as_deref(),
    mock_outputs.as_deref(),
    format,
),
```

- [ ] **Step 3: Implement the handler function**

Add the `composition_explain` handler function. It should:

1. Read the file at `path`.
2. If `--step` is provided, extract the step's composition from the template (same logic as `composition_validate` handler at lines 270-324).
3. Parse `--sample-context`, `--sample-deps`, `--sample-step`, `--mock-outputs` — for each, if it starts with `@`, read the file; otherwise parse as inline JSON.
4. Build `SimulationInput` if any sample data is provided.
5. Call `tasker_sdk::grammar_query::explain_composition()`.
6. For JSON format: print the full `CompositionExplanation`.
7. For table format: print a summary showing the invocation chain with key details (index, capability, category, checkpoint, `.prev` source, expression count, and simulated values if present).

Helper for `@path` loading:

```rust
fn load_json_arg(value: &str) -> Result<serde_json::Value, tasker_client::ClientError> {
    let content = if let Some(path) = value.strip_prefix('@') {
        std::fs::read_to_string(path)
            .map_err(|e| tasker_client::ClientError::Internal(format!("Failed to read {path}: {e}")))?
    } else {
        value.to_owned()
    };
    serde_json::from_str(&content)
        .map_err(|e| tasker_client::ClientError::Internal(format!("Invalid JSON: {e}")))
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --all-features -p tasker-ctl`
Run: `cargo clippy --all-targets --all-features -p tasker-ctl`
Expected: Compiles, zero clippy warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-ctl/src/commands/grammar.rs
git commit -m "feat(TAS-344): add grammar composition explain CLI command

Supports --sample-context, --sample-deps, --sample-step, --mock-outputs
flags with inline JSON or @path/to/file.json syntax. Table and JSON
output formats."
```

---

## Task 8: Final Verification

Full workspace validation.

- [ ] **Step 1: Run all grammar tests**

Run: `cargo nextest run --features test-messaging -p tasker-grammar`
Expected: All tests pass.

- [ ] **Step 2: Run all SDK tests**

Run: `cargo nextest run --features test-messaging -p tasker-sdk`
Expected: All tests pass.

- [ ] **Step 3: Run workspace clippy**

Run: `cargo clippy --all-targets --all-features --workspace`
Expected: Zero warnings.

- [ ] **Step 4: Run workspace check**

Run: `cargo check --all-features --workspace`
Expected: Compiles.

- [ ] **Step 5: Verify MCP tests compile (they require services for full run)**

Run: `cargo nextest run --features test-messaging -p tasker-mcp --lib`
Expected: Library tests pass (protocol tests may need services).
