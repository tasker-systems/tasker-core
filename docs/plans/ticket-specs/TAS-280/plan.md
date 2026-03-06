# TAS-280 Phase 1: `result_schema` + `tasker-ctl generate types`

## Context

Developers building step handlers have no type safety on dependency results — field name typos and shape mismatches only surface at runtime. TAS-294's DSL solved the wiring problem (`@depends_on`), but `order` is still an untyped dict/object/hash.

TAS-280 Phase 1 adds an optional `result_schema` (JSON Schema) field to `StepDefinition` in task template YAML, then implements `tasker-ctl generate types` to produce typed models from those schemas. The orchestrator never inspects `result_schema` — it's tooling metadata only.

---

## Commit 1: Pre-cleanup — Remove stale `task_handler` from YAML fixtures

**51 DSL fixture files** still have `task_handler:` blocks (removed from the Rust struct in a prior ticket but lingering in YAML). Remove the entire `task_handler:` block (callable + initialization) from:
- 15 files in `tests/fixtures/task_templates/python/`
- 19 files in `tests/fixtures/task_templates/ruby/`
- 17 files in `tests/fixtures/task_templates/typescript/`

Verify: `cargo make test-rust-unit` passes (serde already ignores the field, so this is a fixtures-only cleanup).

---

## Commit 2: Add `result_schema` to `StepDefinition`

### File: `tasker-shared/src/models/core/task_template/mod.rs`

Add after `batch_config` (line 541):
```rust
/// Optional JSON Schema describing the expected result payload for this step.
///
/// Tooling metadata only — the orchestrator never inspects or validates handler
/// results against this schema. Used by `tasker-ctl generate types` to produce
/// typed models (Pydantic BaseModel, Dry::Struct, TypeScript interfaces, Rust structs).
#[serde(default, skip_serializing_if = "Option::is_none")]
pub result_schema: Option<Value>,
```

### Update all StepDefinition struct literals — add `result_schema: None`

- `tasker-shared/src/types/base.rs` — test helper constructors
- `tasker-shared/src/config/orchestration/batch_processing.rs` — 3 construction sites
- `tasker-shared/src/events/registry.rs`
- `tasker-shared/src/events/worker_events.rs`
- `tasker-shared/src/models/core/task_template/mod.rs` — test construction sites
- `tasker-shared/src/models/core/task_template/event_validator.rs`

### Add serialization tests (same file, test module)

1. YAML with `result_schema` on a step → parses correctly, field populated
2. YAML without `result_schema` → parses correctly, field is `None` (backward compat)
3. Round-trip: parse → serialize → parse → assert equal
4. Nested objects and arrays in `result_schema`

### Add `result_schema` to API response type

**File:** `tasker-shared/src/types/api/templates.rs`

The API-facing `StepDefinition` struct (line 106) is a simplified projection used in template detail responses. Add:
```rust
/// Optional JSON Schema describing the expected result shape
#[cfg_attr(feature = "web-api", schema(value_type = Object))]
pub result_schema: Option<serde_json::Value>,
```

Update any code that constructs this API `StepDefinition` (likely in orchestration or worker template listing endpoints) to populate `result_schema` from the core `StepDefinition`.

Verify: `cargo build --all-features && cargo test --features test-messaging --lib -p tasker-shared`

---

## Commit 3: Create `codegen` module with schema extraction

### New files in `tasker-ctl/src/codegen/`

**`mod.rs`** — Public API:
```rust
pub enum TargetLanguage { Python, Ruby, TypeScript, Rust }

pub fn generate_types(
    template: &TaskTemplate,
    language: TargetLanguage,
) -> Result<String, CodegenError>
```
- Iterates `template.steps`, collects those with `result_schema`
- Calls `schema::extract_types()` for each
- Dispatches to language-specific generator
- Returns generated source code as a String

**`schema.rs`** — JSON Schema → intermediate representation:
```rust
pub struct TypeDef { pub name: String, pub fields: Vec<FieldDef> }
pub struct FieldDef { pub name: String, pub field_type: FieldType, pub required: bool, pub description: Option<String> }
pub enum FieldType { String, Integer, Number, Boolean, Array(Box<FieldType>), Nested(String), Any }

pub fn extract_types(step_name: &str, schema: &Value) -> Result<Vec<TypeDef>, CodegenError>
```
- Root type named `{StepName}Result` (PascalCase)
- Nested objects named `{StepName}{PropertyName}` (PascalCase) to avoid collisions
- Returns types in dependency order (leaves first)
- Unsupported constructs (`$ref`, `allOf`, `oneOf`) → `FieldType::Any` with warning comment

### Unit tests for `schema.rs`
- Flat schema with string/number/integer/boolean
- Required vs optional fields
- Nested object properties → multiple TypeDefs
- Array with typed items
- Empty/missing properties → empty TypeDef
- Description propagation

---

## Commit 4: Language generators

### Four new files, one per language:

**`codegen/python.rs`** — Pydantic `BaseModel`:
```python
from pydantic import BaseModel
from typing import Optional

class ValidateOrderResult(BaseModel):
    validated: bool
    order_total: float
    item_count: int
```

**`codegen/ruby.rs`** — `Dry::Struct`:
```ruby
require "dry-struct"
module Types; include Dry.Types(); end

class ValidateOrderResult < Dry::Struct
  attribute :validated, Types::Strict::Bool
  attribute :order_total, Types::Strict::Float
end
```

**`codegen/typescript.rs`** — interfaces:
```typescript
export interface ValidateOrderResult {
  validated: boolean;
  order_total: number;
}
```

**`codegen/rust_gen.rs`** — `#[derive(Serialize, Deserialize)]` structs:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateOrderResult {
    pub validated: bool,
    pub order_total: f64,
}
```

### Implementation approach: Askama compile-time templates

Use **Askama** (already configured in tasker-ctl via `askama.toml`, templates in `tasker-ctl/templates/`). This is consistent with how `tasker-ctl docs` and `tasker-ctl init` already generate output — compile-time verified, type-safe, no runtime template parsing.

**Template files** (in `tasker-ctl/templates/codegen/`):
- `python_models.py` — Pydantic BaseModel output
- `ruby_structs.rb` — Dry::Struct output
- `typescript_interfaces.ts` — TypeScript interface output
- `rust_structs.rs` — Rust struct output

**Template structs** (in `tasker-ctl/src/codegen/`):
Each language module defines a `#[derive(Template)]` struct that receives `&[TypeDef]` and the template name, then renders via `.render()`. The templates use Askama's loops, conditionals, and method calls — same patterns as `config-reference.md` and `annotated-config.toml`.

**Askama escaper config:** Add the language file extensions to `askama.toml` with `Text` escaping (same as TOML — no HTML entity encoding):
```toml
[[escaper]]
path = "::askama::filters::Text"
extensions = ["toml", "py", "rb", "ts", "rs"]
```

**Custom Askama filters:** Register `snake_case` and `pascal_case` filters (using `heck`) for use inside templates. Askama supports custom filter modules.

**Why Askama over `write!`/`writeln!`:** Askama is already the established pattern in tasker-ctl for structured output generation. It gives compile-time template verification, keeps template logic separate from Rust business logic, and is more readable for contributors familiar with the codebase.

### Snapshot-style unit tests per language
- Flat types, nested types, optional fields, arrays
- Empty input (no steps with result_schema) → header-only output
- Multiple steps → all types in one output

---

## Commit 5: CLI command + integration

### New file: `tasker-ctl/src/commands/generate.rs`

### Modified: `tasker-ctl/src/main.rs`

Add new top-level command:
```rust
/// Generate typed code from task template schemas (TAS-280)
#[command(subcommand)]
Generate(GenerateCommands),
```

```rust
pub(crate) enum GenerateCommands {
    /// Generate typed result models from step result_schema definitions
    Types {
        /// Path to task template YAML file
        #[arg(short, long)]
        template: PathBuf,
        /// Target language: python, ruby, typescript, rust
        #[arg(short, long)]
        language: String,
        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Specific step (default: all steps with result_schema)
        #[arg(short, long)]
        step: Option<String>,
    },
}
```

### Modified: `tasker-ctl/src/commands/mod.rs`
- Add `pub(crate) mod generate;` and `pub(crate) use generate::handle_generate_command;`

### Handler flow (`commands/generate.rs`):
1. Read + parse YAML → `TaskTemplate` (using existing `serde_yaml` deserialization)
2. Parse `--language` → `TargetLanguage`
3. Optionally filter to `--step`
4. Call `codegen::generate_types(&template, language)`
5. Write to `--output` or stdout

This command is purely local — no API client, no server connection.

### Modified: main.rs dispatch (line ~793):
```rust
Commands::Generate(gen_cmd) => handle_generate_command(gen_cmd).await,
```

---

## Commit 6: Test fixture with `result_schema` + integration test

### New fixture: `tests/fixtures/task_templates/codegen_test_template.yaml`

A purpose-built template with `result_schema` on multiple steps, covering:
- Flat types (string, number, integer, boolean)
- Nested objects
- Arrays with typed items
- Optional vs required fields
- A step without `result_schema` (should be skipped)

### Integration tests in `tasker-ctl`
- Parse fixture → generate types for each language → verify output contains expected type names/fields
- Test `--step` flag filters to single step
- Test error: file not found
- Test warning: no steps have `result_schema`

---

## Files Summary

### New files (12):
| File | Purpose |
|------|---------|
| `tasker-ctl/src/codegen/mod.rs` | Module root, `TargetLanguage`, `generate_types()` |
| `tasker-ctl/src/codegen/schema.rs` | JSON Schema → `TypeDef` extraction |
| `tasker-ctl/src/codegen/python.rs` | Python Pydantic `#[derive(Template)]` + rendering |
| `tasker-ctl/src/codegen/ruby.rs` | Ruby Dry::Struct `#[derive(Template)]` + rendering |
| `tasker-ctl/src/codegen/typescript.rs` | TypeScript interface `#[derive(Template)]` + rendering |
| `tasker-ctl/src/codegen/rust_gen.rs` | Rust struct `#[derive(Template)]` + rendering |
| `tasker-ctl/templates/codegen/python_models.py` | Askama template for Python output |
| `tasker-ctl/templates/codegen/ruby_structs.rb` | Askama template for Ruby output |
| `tasker-ctl/templates/codegen/typescript_interfaces.ts` | Askama template for TypeScript output |
| `tasker-ctl/templates/codegen/rust_structs.rs` | Askama template for Rust output |
| `tasker-ctl/src/commands/generate.rs` | CLI command handler |
| `tests/fixtures/task_templates/codegen_test_template.yaml` | Test fixture |

### Modified files (~13):
| File | Change |
|------|--------|
| `tasker-shared/src/models/core/task_template/mod.rs` | Add `result_schema` field + tests |
| `tasker-shared/src/types/api/templates.rs` | Add `result_schema` to API `StepDefinition` |
| `tasker-shared/src/types/base.rs` | Add `result_schema: None` |
| `tasker-shared/src/config/orchestration/batch_processing.rs` | Add `result_schema: None` (3 sites) |
| `tasker-shared/src/events/registry.rs` | Add `result_schema: None` |
| `tasker-shared/src/events/worker_events.rs` | Add `result_schema: None` |
| `tasker-shared/src/models/core/task_template/event_validator.rs` | Add `result_schema: None` |
| `tasker-ctl/askama.toml` | Add `py`, `rb`, `ts`, `rs` to Text escaper |
| `tasker-ctl/src/main.rs` | Add `GenerateCommands` + `Commands::Generate` |
| `tasker-ctl/src/commands/mod.rs` | Add generate module + re-export |
| 51 YAML fixtures | Remove `task_handler` blocks |

---

## Verification

1. `cargo build --all-features` — workspace compiles
2. `cargo clippy --all-targets --all-features` — zero warnings
3. `cargo test --features test-messaging --lib -p tasker-shared` — result_schema tests pass
4. `cargo test --lib -p tasker-ctl` — codegen + CLI tests pass
5. Manual: `cargo run -p tasker-ctl -- generate types --template tests/fixtures/task_templates/codegen_test_template.yaml --language python` produces valid output
6. `cargo make test-rust-unit` — full unit test suite passes

---

## Design Decisions

1. **New top-level `Generate` command** (not under `Template`): The existing `template generate` is plugin-scaffold generation. Schema-driven codegen is a different flow. `tasker-ctl generate types` / `tasker-ctl generate handler` (Phase 2) reads naturally.

2. **Askama compile-time templates** (not Tera, not `write!`/`writeln!`): Consistent with existing tasker-ctl patterns (`docs`, `init`). Compile-time verification catches template errors at build time. Templates live in `tasker-ctl/templates/codegen/` alongside the existing `tasker-ctl/templates/` files. Custom `snake_case`/`pascal_case` Askama filters via `heck`.

3. **One output file per template**: All step result types for a template go in a single file per language. Keeps imports simple and matches how developers will consume them.

4. **Nested type naming**: `{StepName}{PropertyName}` in PascalCase to avoid collisions across steps. No deduplication in Phase 1.

5. **Graceful handling of unsupported JSON Schema features**: `$ref`, `allOf`, `oneOf`, `enum` → `FieldType::Any` (maps to `Any`/`object`/`serde_json::Value` in generated code) with a comment noting the unsupported construct.
