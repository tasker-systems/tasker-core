# TAS-337: Composition-Aware Template Validator in tasker-sdk

**Ticket**: TAS-337 (Lane 3C — Validation Tooling)
**Crates touched**: `tasker-grammar`, `tasker-shared`, `tasker-sdk`
**Branch**: `jcoletaylor/tas-337-composition-aware-template-validator-in-tasker-sdk`

---

## Problem

The existing `tasker-sdk::template_validator` validates task templates for structural correctness (DAG integrity, handler callables, result schemas) but has no awareness of composition blocks. As composition-defined "virtual handler" steps become first-class, the template validator must check that embedded compositions are well-formed before deployment.

The `CompositionValidator` in `tasker-grammar` already validates `CompositionSpec` structures in isolation. This ticket bridges that validator into the template validation pipeline so that composition errors surface alongside structural errors in a single `ValidationReport`.

A secondary problem: `tasker-sdk` and `tasker-grammar` each define their own `Severity` and `ValidationFinding` types with identical semantics, creating a mapping burden that will only grow.

---

## Design Decisions

### D1: Unified Severity type

`tasker-grammar::types::Severity` becomes the single source of truth. `tasker-sdk::template_validator` re-exports it instead of defining its own. The SDK's `ValidationFinding` struct retains its `step: Option<String>` context field but uses grammar's `Severity`.

**Rationale**: The two enums have identical variants (`Error`, `Warning`, `Info`). Maintaining two creates a lossy-mapping risk for zero benefit. Pre-1.0, the only consumers (`tasker-mcp`, `tasker-ctl`) are in-workspace — migration is trivial.

### D2: `StepDefinition` gains `composition: Option<serde_json::Value>`

Added to `tasker-shared::models::core::task_template::StepDefinition` (the models struct, not the API-layer `types::api::templates::StepDefinition`) with `#[serde(default)]` so existing YAML templates parse without changes. The value is an opaque JSON blob at the `tasker-shared` level — deserialization to `CompositionSpec` happens at the validation boundary in `tasker-sdk`.

**Rationale**: `tasker-shared` is the foundational crate depended on by everything. Making it depend on `tasker-grammar` would pull jaq-core, jaq-std, jaq-json, jsonschema, etc. into every crate in the workspace. Using `Option<serde_json::Value>` keeps `tasker-shared` free of grammar dependencies while still capturing the YAML structure for downstream validation. The typed interpretation (`CompositionSpec`) is applied at the `tasker-sdk` boundary where `tasker-grammar` is already a dependency.

**Alternative rejected**: `Option<CompositionSpec>` directly on `StepDefinition` — would require `tasker-shared → tasker-grammar`, contradicting grammar's design principle of being a leaf crate with no dependency on `tasker-shared` (and vice versa).

### D3: `tasker-sdk` depends on `tasker-grammar`

New dependency: `tasker-grammar = { path = "../tasker-grammar" }`. The grammar crate has no I/O deps but does bring jaq-core and jsonschema — acceptable for a tooling crate.

**Rationale**: The SDK is the tooling layer. It must understand grammar constructs to validate them. The dependency direction is correct: `tasker-sdk → tasker-grammar → (nothing)`.

### D4: Standard vocabulary as a first-class function

A new public function in a `tasker_grammar::vocabulary` module: `standard_capability_registry()` returns a `HashMap<String, CapabilityDeclaration>` with the 6 built-in capabilities (transform, validate, assert, persist, acquire, emit). Currently each test file builds its own `make_registry()` — this centralizes the canonical vocabulary.

**Rationale**: The template validator needs a default registry. Duplicating capability declarations across test files and SDK code is fragile. A single canonical source ensures vocabulary consistency.

### D5: Registry as parameter with default

The template validator's `validate()` signature stays unchanged for backward compatibility. A new `validate_with_registry()` accepts an `&dyn CapabilityRegistry` for extensibility. The existing `validate()` uses `standard_capability_registry()` as the default.

`validate()` constructs a `HashMap` and `ExpressionEngine` on each call. This is cheap for single-template validation but `validate_with_registry()` is preferred for batch validation of multiple templates to amortize construction.

**Rationale**: Out-of-the-box users get the standard 6-capability vocabulary. Users building custom binaries with additional capabilities can pass their own registry. No breaking change to existing callers.

### D6: ExpressionEngine constructed internally

`CompositionValidator::new()` requires both `&dyn CapabilityRegistry` and `&ExpressionEngine`. The `composition_validator` module constructs an `ExpressionEngine` with default config internally — callers don't need to manage it. The engine is stateless and cheap to construct.

**Rationale**: ExpressionEngine is an implementation detail of composition validation. Exposing it in the SDK API would leak grammar internals for no user benefit.

### D7: Grammar finding translation preserves invocation context in messages

Grammar's `ValidationFinding` includes `invocation_index: Option<usize>` to identify which capability invocation caused the finding. The SDK's `ValidationFinding` has `step: Option<String>` but no `invocation_index`. When translating grammar findings to SDK findings, the invocation index is encoded into the message string (e.g., `"invocation[2]: unknown capability 'foo'"`).

**Rationale**: SDK consumers (MCP tools, CLI) present findings as human-readable messages. Encoding the invocation index in the message preserves full detail without complicating the struct. Step-level granularity in the struct is sufficient for programmatic filtering (which step has problems), while the message carries the invocation-level detail for human diagnosis.

---

## Architecture

### Dependency graph (new edges marked)

```
tasker-mcp ──→ tasker-sdk ──→ tasker-shared
tasker-ctl ──→ tasker-sdk ──→ tasker-grammar (NEW)
```

`tasker-shared` does NOT depend on `tasker-grammar`. The `composition` field on `StepDefinition` is `Option<serde_json::Value>` — no grammar types cross the shared boundary.

### Module structure

```
crates/tasker-grammar/src/
├── vocabulary.rs          # NEW: standard_capability_registry()
└── ...

crates/tasker-sdk/src/
├── composition_validator/
│   ├── mod.rs             # Public API: validate_composition(), translate findings
│   └── tests.rs           # Unit tests with composition-bearing templates
├── template_validator/
│   └── mod.rs             # Extended: calls composition_validator for steps with compositions
└── lib.rs                 # Adds pub mod composition_validator
```

### Data flow

```
TaskTemplate YAML
    │
    ▼ parse (serde_yaml)
TaskTemplate { steps: [StepDefinition { composition: Some(json_value), ... }, ...] }
    │
    ▼ validate() or validate_with_registry()
    │
    ├─── structural checks (existing) ──→ Vec<ValidationFinding>
    │    ├── duplicate step names
    │    ├── dependency references
    │    ├── handler callables
    │    ├── namespace length
    │    ├── result schemas
    │    ├── orphan steps
    │    └── cycle detection
    │
    └─── composition checks (NEW) ──→ Vec<ValidationFinding>
         │   for each step with composition: Some(json_value)
         │
         ├── deserialize json_value → CompositionSpec
         │   (failure → COMPOSITION_PARSE_ERROR)
         │
         ├── delegate to CompositionValidator::validate(spec, registry)
         │   ├── capability existence
         │   ├── config schema validation
         │   ├── contract chaining
         │   ├── checkpoint coverage
         │   ├── expression syntax
         │   ├── output schema presence
         │   └── outcome convergence
         │
         ├── result_schema compatibility (NEW)
         │   step.result_schema vs composition.outcome.output_schema
         │
         └── callable convention check (NEW, warning)
             composition present but callable doesn't start with "grammar:"
    │
    ▼ merge
ValidationReport { valid, findings, step_count, has_cycles }
```

---

## Public API

### `tasker_grammar::vocabulary`

```rust
/// Returns the standard 6-capability vocabulary (transform, validate, assert,
/// persist, acquire, emit) as a `HashMap` implementing `CapabilityRegistry`.
pub fn standard_capability_registry() -> HashMap<String, CapabilityDeclaration>
```

### `tasker_sdk::composition_validator`

```rust
/// Validate a standalone CompositionSpec against a capability registry.
///
/// Constructs an ExpressionEngine with default config internally.
/// Returns SDK-level ValidationFindings (no step context).
pub fn validate_composition(
    spec: &CompositionSpec,
    registry: &dyn CapabilityRegistry,
) -> Vec<ValidationFinding>

/// Validate a composition in the context of a template step.
///
/// Deserializes the step's `composition` field (Option<serde_json::Value>)
/// to CompositionSpec, then runs CompositionValidator checks plus
/// template-level checks (result_schema compatibility, callable convention).
/// All findings are tagged with the step name.
pub fn validate_step_composition(
    step: &StepDefinition,
    registry: &dyn CapabilityRegistry,
) -> Vec<ValidationFinding>
```

### `tasker_sdk::template_validator` (extended)

```rust
/// Validate a task template (existing signature, unchanged).
/// Now also checks composition blocks using the standard vocabulary.
pub fn validate(template: &TaskTemplate) -> ValidationReport

/// Validate a task template with a custom capability registry.
pub fn validate_with_registry(
    template: &TaskTemplate,
    registry: &dyn CapabilityRegistry,
) -> ValidationReport
```

### `tasker_sdk::template_validator::ValidationFinding` (modified)

```rust
use tasker_grammar::Severity;  // re-exported, no longer defined locally

#[derive(Debug, Clone, Serialize)]
pub struct ValidationFinding {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub step: Option<String>,
}
```

---

## Validation Codes (new)

| Code | Severity | Condition |
|------|----------|-----------|
| `COMPOSITION_PARSE_ERROR` | Error | Step's `composition` value failed to deserialize to `CompositionSpec` |
| `COMPOSITION_INVALID` | Error | CompositionValidator reports errors (invocation index encoded in message) |
| `COMPOSITION_WARNING` | Warning | CompositionValidator reports warnings (invocation index encoded in message) |
| `COMPOSITION_RESULT_SCHEMA_MISMATCH` | Error | Step's `result_schema` incompatible with composition's `outcome.output_schema` |
| `COMPOSITION_CALLABLE_CONVENTION` | Warning | Step has composition but callable doesn't start with `grammar:` |

Grammar-level findings are translated 1:1 into SDK findings with the step name attached and invocation index encoded in the message. No aggregation or loss of detail.

---

## Changes by Crate

### `tasker-grammar`

1. **New module**: `vocabulary` with `standard_capability_registry()` function
2. **`lib.rs`**: Export the new module
3. **No other changes** — `CompositionValidator`, `CapabilityRegistry`, `ValidationResult` already exist

### `tasker-shared`

1. **`StepDefinition`** (models version at `models::core::task_template::StepDefinition`): Add `pub composition: Option<serde_json::Value>` with `#[serde(default)]`
2. The API-layer `types::api::templates::StepDefinition` is a separate struct and is NOT modified by this ticket
3. **No new crate dependencies** — `serde_json::Value` is already available

### `tasker-sdk`

1. **`Cargo.toml`**: Add `tasker-grammar` dependency
2. **`template_validator/mod.rs`**:
   - Remove local `Severity` enum, import from `tasker_grammar`
   - Add `validate_with_registry()` function
   - Refactor `validate()` to call `validate_with_registry()` with default registry
   - Add composition validation pass in the pipeline
3. **New module**: `composition_validator/` with public API described above
4. **`lib.rs`**: Add `pub mod composition_validator`

### `tasker-mcp` / `tasker-ctl`

No changes needed. They call `tasker_sdk::template_validator::validate()` which now includes composition checks automatically. The `Severity` type path changes but both crates import it through the SDK re-export.

---

## Test Plan

### Unit tests (`cargo test -p tasker-sdk --lib`)

1. **Valid composition in template** — template with a composition step validates cleanly
2. **Unknown capability** — composition references `nonexistent_cap`, produces `COMPOSITION_INVALID` error
3. **Missing checkpoint** — mutating capability without checkpoint marker, produces `COMPOSITION_INVALID` error
4. **Invalid expression** — malformed jaq filter, produces `COMPOSITION_INVALID` error
5. **Result schema mismatch** — step result_schema incompatible with composition outcome, produces `COMPOSITION_RESULT_SCHEMA_MISMATCH`
6. **Callable convention warning** — composition present but callable is `my_handler`, produces `COMPOSITION_CALLABLE_CONVENTION` warning
7. **Mixed template** — template with both composition and non-composition steps, each validated appropriately
8. **Backward compatibility** — existing test templates (no composition field) continue to validate identically
9. **Custom registry** — `validate_with_registry()` with extended vocabulary validates custom capabilities
10. **Standalone composition validation** — `validate_composition()` works without template context
11. **Malformed composition** — step has `composition:` with invalid structure, produces `COMPOSITION_PARSE_ERROR`

### Grammar unit tests (`cargo test -p tasker-grammar --lib`)

12. **Standard vocabulary** — `standard_capability_registry()` returns all 6 capabilities with correct config schemas
13. **Vocabulary completeness** — all 6 capabilities present, names match, grammar categories correct

### Integration

14. **Existing template validator tests pass unchanged** — no regressions
15. **Workflow fixtures** — the 3 grammar workflow fixtures (ecommerce, payment, customer_onboarding) embedded as inline YAML in template test data validate successfully through the full pipeline

---

## Out of Scope

- **TAS-338** (expression syntax validation) — parallel ticket, extends validation from a different angle
- **TAS-339** (input mapping validation) — parallel ticket
- **MCP/CLI tool updates** — existing `template_validate` path picks up composition awareness automatically
- **`grammar:` callable enforcement** — warning only, not an error gate
- **Lane 3A StepContext rename** — independent work, not blocked by or blocking this ticket
- **Runtime execution** — this is design-time validation only
- **API-layer StepDefinition** — only the models-layer `StepDefinition` is modified
