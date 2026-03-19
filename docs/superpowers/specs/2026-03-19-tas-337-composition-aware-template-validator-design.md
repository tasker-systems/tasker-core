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

### D2: `StepDefinition` gains `composition: Option<CompositionSpec>`

Added to `tasker-shared::models::core::task_template::StepDefinition` with `#[serde(default)]` so existing YAML templates parse without changes. When present, the step's behavior is defined by the composition rather than the handler callable.

**Rationale**: Without this field, the template validator can't discover compositions. Adding it now (rather than waiting for Lane 3A's StepContext rename) unblocks the full template-level integration with minimal blast radius — it's an optional field on a serde struct.

### D3: `tasker-sdk` depends on `tasker-grammar`

New dependency: `tasker-grammar = { path = "../tasker-grammar" }`. The grammar crate is lightweight (no I/O, no infrastructure deps), so this doesn't inflate the SDK's dependency tree meaningfully.

**Rationale**: The SDK is the tooling layer. It must understand grammar constructs to validate them. The dependency direction is correct: `tasker-sdk → tasker-grammar → (nothing)`.

### D4: Standard vocabulary as a first-class function

A new public function `tasker_grammar::standard_capability_registry()` returns a `HashMap<String, CapabilityDeclaration>` with the 6 built-in capabilities (transform, validate, assert, persist, acquire, emit). Currently each test file builds its own `make_registry()` — this centralizes the canonical vocabulary.

**Rationale**: The template validator needs a default registry. Duplicating capability declarations across test files and SDK code is fragile. A single canonical source ensures vocabulary consistency.

### D5: Registry as parameter with default

The template validator's `validate()` signature stays unchanged for backward compatibility. A new `validate_with_registry()` accepts an `&dyn CapabilityRegistry` for extensibility. The existing `validate()` uses `standard_capability_registry()` as the default.

**Rationale**: Out-of-the-box users get the standard 6-capability vocabulary. Users building custom binaries with additional capabilities can pass their own registry. No breaking change to existing callers.

---

## Architecture

### Dependency graph (new edges in bold)

```
tasker-mcp ──→ tasker-sdk ──→ tasker-shared
tasker-ctl ──→ tasker-sdk ──→ tasker-grammar (NEW)
                              tasker-shared ──→ tasker-grammar (NEW, for CompositionSpec on StepDefinition)
```

Note: `tasker-shared` gains a dependency on `tasker-grammar` for the `CompositionSpec` type on `StepDefinition`. This is the lightest-touch approach — grammar is a pure-data crate with no I/O deps.

**Alternative considered**: Define a parallel `CompositionSpec` in `tasker-shared` and convert. Rejected — duplicating a complex type tree across crates creates a maintenance burden and ensures the types will drift.

### Module structure

```
crates/tasker-sdk/src/
├── composition_validator/
│   ├── mod.rs           # Public API: validate_composition(), translate findings
│   └── tests.rs         # Unit tests with composition-bearing templates
├── template_validator/
│   └── mod.rs           # Extended: calls composition_validator for steps with compositions
└── lib.rs               # Adds pub mod composition_validator
```

### Data flow

```
TaskTemplate YAML
    │
    ▼ parse
TaskTemplate { steps: [StepDefinition { composition: Some(spec), ... }, ...] }
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
         │   for each step with composition: Some(spec)
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

### `tasker_grammar::standard_capability_registry`

```rust
/// Returns the standard 6-capability vocabulary (transform, validate, assert,
/// persist, acquire, emit) as a `HashMap` implementing `CapabilityRegistry`.
pub fn standard_capability_registry() -> HashMap<String, CapabilityDeclaration>
```

### `tasker_sdk::composition_validator`

```rust
/// Validate a standalone CompositionSpec against a capability registry.
///
/// Returns SDK-level ValidationFindings (no step context).
pub fn validate_composition(
    spec: &CompositionSpec,
    registry: &dyn CapabilityRegistry,
) -> Vec<ValidationFinding>

/// Validate a composition in the context of a template step.
///
/// Runs CompositionValidator checks plus template-level checks
/// (result_schema compatibility, callable convention).
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
| `COMPOSITION_INVALID` | Error | CompositionValidator reports errors (detail in message) |
| `COMPOSITION_WARNING` | Warning | CompositionValidator reports warnings (detail in message) |
| `COMPOSITION_RESULT_SCHEMA_MISMATCH` | Error | Step's result_schema incompatible with composition outcome |
| `COMPOSITION_CALLABLE_CONVENTION` | Warning | Step has composition but callable doesn't start with `grammar:` |
| `COMPOSITION_PARSE_ERROR` | Error | CompositionSpec failed to deserialize (malformed YAML) |

Grammar-level findings are translated into SDK findings with the code prefix `COMPOSITION_` and the step name attached. Each grammar finding becomes one SDK finding — no aggregation or loss of detail.

---

## Changes by Crate

### `tasker-grammar`

1. **New function**: `standard_capability_registry()` in a new `vocabulary` module (or in `fixtures.rs` if colocating with existing fixture helpers)
2. **No other changes** — `CompositionValidator`, `CapabilityRegistry`, `ValidationResult` already exist

### `tasker-shared`

1. **`StepDefinition`**: Add `pub composition: Option<CompositionSpec>` with `#[serde(default)]`
2. **`Cargo.toml`**: Add `tasker-grammar` dependency (path only, since grammar has `publish = false`)

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

### Grammar unit tests (`cargo test -p tasker-grammar --lib`)

11. **Standard vocabulary** — `standard_capability_registry()` returns all 6 capabilities with correct schemas

### Integration

12. **Existing template validator tests pass unchanged** — no regressions
13. **Workflow fixtures** — the 3 grammar workflow fixtures (ecommerce, payment, customer_onboarding) validate successfully when embedded in template YAML

---

## Out of Scope

- **TAS-338** (expression syntax validation) — parallel ticket, extends validation from a different angle
- **TAS-339** (input mapping validation) — parallel ticket
- **MCP/CLI tool updates** — existing `template_validate` path picks up composition awareness automatically
- **`grammar:` callable enforcement** — warning only, not an error gate
- **Lane 3A StepContext rename** — independent work, not blocked by or blocking this ticket
- **Runtime execution** — this is design-time validation only
