# TAS-280: Implementation Context

*Read the [Linear ticket](https://linear.app/tasker-systems/issue/TAS-280) first. This document provides codebase navigation, architectural context, and implementation guidance that the ticket assumes but doesn't spell out.*

---

## Where Things Live

### StepDefinition — where `result_schema` goes

**File:** `tasker-shared/src/models/core/task_template/mod.rs`

`StepDefinition` starts at ~line 465. This is the struct that gets the new `result_schema: Option<serde_json::Value>` field. Follow the existing patterns exactly:

- Use `#[serde(default, skip_serializing_if = "Option::is_none")]` — same as other optional fields
- Use `#[builder(default)]` for the derive_builder integration
- `Option<Value>` (where `Value = serde_json::Value`) — same type as `input_schema` on `TaskTemplate`

The existing `input_schema` field on `TaskTemplate` (~line 445) is the precedent. It's template-level input validation. `result_schema` is step-level output description. Same type, different scope, different enforcement model (`input_schema` is validated at task submission; `result_schema` is never enforced by orchestration — it's tooling metadata only).

Adjacent structs you'll touch or reference:
- `HandlerDefinition` (~line 127) — `callable`, `method`, `resolver`, `initialization`
- `TaskTemplate` (~line 404) — the parent struct; has `input_schema` and `steps: Vec<StepDefinition>`
- `BatchConfiguration` (~line 525) — example of an optional config struct on StepDefinition

The mod.rs file has extensive round-trip serialization tests starting around line 2900. Add tests for `result_schema` parsing there.

### tasker-ctl — where the new commands go

**File:** `tasker-ctl/src/commands/template.rs`

The existing `TemplateCommands` enum (defined in `tasker-ctl/src/main.rs` or the CLI definition module) dispatches through `handle_template_command()`. Today it has `List`, `Info`, and `Generate` subcommands.

**Important distinction:** The existing `template generate` command generates project scaffolds from plugin templates (Tera-based, discovered via `tasker-plugin.toml` manifests). The new TAS-280 commands (`generate types`, `generate handler`) are a **different flow** — they read task template YAML files directly and produce typed code from `result_schema` definitions. These are not plugin templates; they are schema-driven code generation.

The template engine (`tasker-ctl/src/template_engine/mod.rs`) uses Tera and has custom filters (`snake_case`, `pascal_case`, `camel_case`, `kebab_case`). You will likely want Tera templates for the generated code (Python models, Ruby structs, TypeScript interfaces), but the input is a parsed `TaskTemplate`, not plugin template parameters.

### Worker DSL — the consumer of generated types

**Python** (`workers/python/python/tasker_core/step_handler/functional.py`):
- `@depends_on` decorator (~line 354) already supports typed dependencies: `@depends_on(order=("validate_order", ValidateOrderResult))`
- Uses Pydantic `model_construct(**raw_dict)` for deserialization
- `@inputs` decorator (~line 400) supports `@inputs(MyPydanticModel)` for typed input injection
- **This is the most mature typed DSL** — generated Python code should target this exact pattern

**TypeScript** (`workers/typescript/src/handler/functional.ts`):
- `defineHandler()` factory with `depends: Record<string, string>` — string-based mapping today
- TypeScript doesn't need serde-style deserialization modeling — interfaces and type annotations (`: MyCoolInterface`, `: Promise<MyCoolInterface>`) are the natural TypeScript approach
- The goal for generated TypeScript is to produce **interfaces** that developers attach to their handler parameters and return types, catching incompatibilities at build-and-lint time through the TypeScript compiler — not runtime deserialization
- Only pursue runtime serde if there's a clear, obvious, TypeScript-natural way of making it more intentional; don't force a pattern from another language

**Ruby** (`workers/ruby/lib/tasker_core/step_handler/functional.rb`):
- Full functional DSL with `step_handler`, `decision_handler`, `batch_analyzer`, `batch_worker`, and `api_handler` block-based patterns
- **Already supports typed `depends_on`**: `depends_on: { order: ['validate_order', ValidateOrderResult] }` — the tuple syntax deserializes raw dependency results into `Dry::Struct` instances via `model_cls.new(**symbolized.slice(*known))`
- `inputs:` supports both symbol keys (`inputs: [:payment_info]`) and model classes for typed input injection
- `tasker-contrib/examples/rails-app/app/handlers/` has many examples of the DSL in use
- Tera templates already exist for Ruby handler generation

### Test fixtures — existing template YAML examples

`tests/fixtures/task_templates/` contains YAML templates organized by language subdirectory. These are the canonical examples of template structure. Look at these to understand the full range of what templates express today before adding `result_schema` support.

---

## Pre-Cleanup: `task_handler` and Registry Rename

Before starting TAS-280 implementation, do this cleanup to put the codebase on firm ground:

### 1. Remove `task_handler` from YAML fixture files

The `task_handler` field was already removed from the `TaskTemplate` struct in a prior ticket, but it still lingers in YAML test fixtures:

```yaml
# Still present in tests/fixtures/task_templates/ — remove these blocks
task_handler:
  callable: TestScenarios_dsl.SuccessOnlyHandler
  initialization: {}
```

Scope: Remove `task_handler` blocks from all YAML files in `tests/fixtures/task_templates/`. Since the struct no longer has this field, serde should already be ignoring it during deserialization — but the fixtures should reflect the current schema.

### 2. Rename `TaskTemplateRegistry` → `TaskTemplateRegistry`

The `TaskTemplateRegistry` in `tasker-shared/src/registry/task_template_registry.rs` is the runtime registry that discovers and caches task templates. The name is a holdover — it manages templates, not handlers. Rename to `TaskTemplateRegistry` for clarity:

- `tasker-shared/src/registry/task_template_registry.rs` → `task_template_registry.rs`
- `TaskTemplateRegistry` struct → `TaskTemplateRegistry`
- `task_template_registry` field on `SystemContext` → `task_template_registry`
- Update all references across the workspace (re-exports in `registry/mod.rs`, usage in `system_context.rs`, etc.)

This is a mechanical rename with no behavior change. Do it as a separate commit before TAS-280 work begins so the diff is clean.

---

## Phasing Guidance

### Phase 1: Schema in template + model generation (start here)

1. Add `result_schema: Option<Value>` to `StepDefinition` in tasker-shared
2. Add serialization round-trip tests
3. Implement `tasker-ctl generate types` — reads a task template YAML, produces typed models:
   - Python: Pydantic `BaseModel` classes
   - Ruby: `Dry::Struct` classes
   - TypeScript: interfaces
   - Rust: `#[derive(Debug, Clone, Serialize, Deserialize)]` structs
4. Schema is stored but ignored by orchestration runtime — no changes to actors, result processing, or step execution

### Phase 2: Typed handler generation

1. Implement `tasker-ctl generate handler` — produces DSL handler scaffold with typed `depends_on`
2. For Python: generates `@depends_on(order=("validate_order", ValidateOrderResult))` — typed tuple syntax already works in the DSL
3. For Ruby: generates `depends_on: { order: ['validate_order', ValidateOrderResult] }` — typed tuple syntax already works in the functional DSL (`functional.rb`). Tera templates for Ruby handler generation already exist
4. For TypeScript: generates `defineHandler()` with generated interfaces as type annotations on handler parameters and return types — the goal is build-and-lint-time catching via the TypeScript compiler, not runtime deserialization
5. Generate test scaffolds with typed assertions on result shapes

### Phase 3: Validation tooling

1. `tasker-ctl template check` — lint-time validation
2. Schema compatibility checking between connected steps (step A's `result_schema` output matches what step B's handler expects)
3. **Phase 3 explicitly excludes runtime enforcement** — the orchestrator never rejects handler results based on schema

---

## What Phase 1 Explicitly Excludes

- **Cross-step schema compatibility checking.** That's Phase 3. Phase 1 parses and stores `result_schema`, generates types from it, and trusts the developer to keep schemas consistent. The typed `depends_on` patterns make inconsistencies more visible (type errors in handler code) but the tooling doesn't enforce it yet.
- **Runtime schema validation.** The orchestrator never inspects, validates, or rejects handler results based on `result_schema`. This is a tooling-only field. Misconfigured schemas must never cause orchestration errors.
- **Changes to existing handler registration or resolution.** The resolver chain, `TaskTemplateRegistry`, handler dispatch — none of this changes.

---

## Test Strategy

- **`result_schema` parsing:** Unit tests in `tasker-shared` — round-trip serialization, optional field handling, backwards compatibility (templates without `result_schema` continue to parse). Add to the existing test block starting at ~line 2900 in `task_template/mod.rs`.
- **Code generation:** Unit tests in `tasker-ctl` — generate types from a fixture template with `result_schema`, compare output against expected snapshots. Test each language target.
- **Generated code correctness:** The generated code itself is tested by the developer in their project, not within tasker-core. But the generated code should be syntactically correct — validate by running the relevant language's parser/compiler on the output in CI if practical.
- **Feature gate:** New tests should use `--features test-messaging` at minimum (for DB access if testing template parsing from YAML → DB round trips). Pure code generation tests may not need any feature flags.

---

## Vision Context

`result_schema` is the first data contract in Tasker. In the longer-term generative workflow vision (see `tasker-book/src/vision/`), these schemas become the foundation for action grammar data contracts, capability schema derivation, and LLM-assisted workflow planning. For TAS-280, the scope is strictly tooling — but design decisions here carry forward:

- **Schema format:** JSON Schema, consistent with existing `input_schema`. Don't invent a new schema language.
- **Where schemas live:** On `StepDefinition`, not in a separate registry. Keep it close to the step.
- **How schemas are accessed:** Through the parsed `TaskTemplate` struct. No new storage, no new APIs.
- **Orchestration stays agnostic:** This is the most important design boundary. The orchestrator must never inspect `result_schema` at runtime. Tooling reads it; the runtime ignores it.

---

## JSON Schema → Language Type Mapping Reference

For the code generation phase, here's the type mapping the generator needs:

| JSON Schema type | Python (Pydantic) | Ruby (Dry::Struct) | TypeScript | Rust |
|---|---|---|---|---|
| `string` | `str` | `Types::Strict::String` | `string` | `String` |
| `number` | `float` | `Types::Strict::Float` | `number` | `f64` |
| `integer` | `int` | `Types::Strict::Integer` | `number` | `i64` |
| `boolean` | `bool` | `Types::Strict::Bool` | `boolean` | `bool` |
| `object` (with properties) | nested `BaseModel` | nested `Dry::Struct` | nested `interface` | nested `struct` |
| `array` (with items) | `list[ItemType]` | `Types::Strict::Array.of(ItemType)` | `ItemType[]` | `Vec<ItemType>` |
| `null` | `None` | `Types::Strict::Nil` | `null` | `()` |
| optional (not in `required`) | `Optional[T] = None` | `attribute :x, Types::Strict::T.optional` | `x?: T` | `Option<T>` |

Nested objects generate nested types. Array item types generate the corresponding collection. The `required` array determines which fields are optional in the generated type.
