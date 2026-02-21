# Research & Removal Plan: Vestigial `task_handler` Concept

## Research Findings

### Verdict: CONFIRMED VESTIGIAL

The `task_handler` concept is confirmed to be a legacy artifact. While the field exists
throughout the codebase (179 occurrences across 80+ files), **no code path resolves
`task_handler.callable` to an actual callable and invokes it**. Only step handlers go
through the `StepHandlerResolver` → `ResolverChain` → invocation pipeline.

### Evidence Summary

#### 1. No Invocation Path Exists

The system has a full resolution pipeline for **step handlers**:
- `StepHandlerResolver` trait → `ResolverChain` → `ResolvedHandler` → `MethodDispatchWrapper`
- Step handlers are dispatched via `HandlerDispatchService` and `FfiDispatchChannel`

**No equivalent pipeline exists for task handlers.** The `task_handler.callable` string
from YAML is parsed, stored, and passed around — but never resolved to executable code.

#### 2. `TaskHandlerRegistry` Is Misnamed

Despite its name, `TaskHandlerRegistry` (`tasker-shared/src/registry/task_handler_registry.rs`)
is a **template registry/loader**, not a handler invocation engine. It:
- Loads YAML templates from disk
- Persists them to the `named_tasks` table
- Retrieves `HandlerMetadata` on demand (with optional distributed cache)
- **Never invokes any handler**

#### 3. `HandlerMetadata.handler_class` Is Informational Only

The `handler_class` field on `HandlerMetadata` defaults to `"TaskerCore::TaskHandler::Base"`
(a Ruby class name literal). It's extracted from the `named_tasks.configuration` JSONB
column but is never used for actual class resolution or instantiation in the Rust system.

Relevant code (`task_handler_registry.rs:667-714`):
```rust
let handler_class = named_task.configuration.as_ref().and_then(|config| {
    config.get("handler_class").and_then(|v| {
        v.as_str()
            .map(|s| s.to_string())
            .or(Some("TaskerCore::TaskHandler::Base".to_string()))
    })
});
// ...
handler_class: handler_class.unwrap_or("TaskerCore::TaskHandler::Base".to_string()),
```

#### 4. `all_callables()` Only Used in Tests

`TaskTemplate::all_callables()` collects both `task_handler.callable` and step handler
callables, but is only called in unit tests (`test_all_callables_extraction`), never in
production code.

#### 5. Ruby `TaskHandler::Base` Exists but Is Not Resolved via YAML

Ruby has `TaskerCore::TaskHandler::Base` (`workers/ruby/lib/tasker_core/task_handler/base.rb`)
with `handle(task_uuid)` and `initialize_task(task_request)` methods. However:
- `TaskerCore::Handlers::Tasks.handle(task_uuid)` always does `Base.new.handle(task_uuid)`
  — it never resolves a specific callable from YAML
- Ruby example handlers (`DiamondWorkflowHandler`, etc.) inherit from `Base` but their
  class names in YAML `task_handler.callable` are used only for **discovery** (template_discovery.rb),
  not invocation

#### 6. Database Schema Has No `task_handler` Column

The `tasker_named_tasks` table stores a `configuration JSONB` column. The `handler_class`
value lives inside this JSON blob as one key among many. There is no dedicated column.

#### 7. Template Discovery Uses It for Import, Not Invocation

Both `workers/ruby/lib/tasker_core/template_discovery.rb` and
`workers/python/python/tasker_core/template_discovery.py` extract `task_handler.callable`
from YAML — but only to discover which classes/modules to import/require, not to invoke them.

---

## Scope of References

### Category Breakdown

| Category | Files | Occurrences | Notes |
|----------|-------|-------------|-------|
| YAML fixtures (tests/fixtures/) | ~76 | ~76 | `task_handler:` key in template YAML |
| Config templates (config/tasks/) | 11 | ~22 | `task_handler:` + environment overrides |
| Worker config YAML | ~17 | ~17 | Ruby/Rust worker-specific templates |
| Rust source (TaskTemplate struct) | 1 | 15 | Field definition, tests, env override logic |
| Rust source (TaskHandlerRegistry) | 22 | 81 | Type name references |
| Rust source (task_handler_registry field) | 20 | 51 | Field access on SystemContext, services |
| Ruby source | 7 | ~12 | Types, handlers, discovery, base class |
| Python source | 2 | ~18 | Template discovery + tests |
| Documentation (*.md) | ~12 | ~15 | Various docs and ticket specs |
| Tests (Rust) | ~10 | ~20 | Integration tests, service tests |

### Two Distinct Removal Concerns

**Concern A: The `task_handler` field (primary target)**
- The `task_handler: Option<HandlerDefinition>` on `TaskTemplate`
- The `task_handler: Option<HandlerOverride>` on `EnvironmentOverride`
- The `task_handler:` key in all YAML templates
- `all_callables()` including task_handler
- Ruby/Python template discovery extracting task_handler.callable
- Ruby `TaskHandler::Base` class and inheriting handlers

**Concern B: The `TaskHandlerRegistry` naming (secondary, larger scope)**
- The type is really a "template registry" — renaming to `TaskTemplateRegistry` or
  `TemplateRegistry` would be more accurate
- 81 type name occurrences + 51 field name occurrences across 22+ files
- Renaming `SystemContext.task_handler_registry` to `template_registry`
- This is a bigger rename that may warrant its own ticket

---

## Step-wise Removal Plan

### Phase 1: Remove `task_handler` Field from TaskTemplate (Rust)

**Files to modify:**
1. `tasker-shared/src/models/core/task_template/mod.rs`
   - Remove `pub task_handler: Option<HandlerDefinition>` from `TaskTemplate` struct (line 66)
   - Remove `pub task_handler: Option<HandlerOverride>` from `EnvironmentOverride` struct (line 729)
   - Remove task_handler override logic from `resolve_for_environment()` (lines 884-891)
   - Remove task_handler from `all_callables()` (lines 924-926)
   - Remove/update all inline tests that reference task_handler

2. `tasker-client/src/grpc_clients/conversions.rs`
   - Remove `task_handler: None` from TaskTemplate construction (line 848)

### Phase 1b: Remove `task_handler` from JSON Schema

**File to modify:**
1. `schemas/task-template.json`
   - Remove `task_handler` property from root schema (references `HandlerDefinition`)
   - Remove `task_handler` property from `EnvironmentOverride` definition

### Phase 2: Remove `task_handler` from YAML Templates

**~104 YAML files across:**
- `tests/fixtures/task_templates/{python,ruby,rust,typescript}/*.yaml`
- `config/tasks/**/*.yaml`
- `workers/rust/config/tasks/**/*.yaml`
- `workers/ruby/spec/fixtures/templates/*.yaml`
- `workers/ruby/spec/handlers/examples/**/config/*.yaml`

Each file needs the `task_handler:` block (and any `environments.*.task_handler:` blocks) removed.

### Phase 3: Remove `task_handler` from Ruby Worker

**Files to modify:**
1. `workers/ruby/lib/tasker_core/task_handler/base.rb` — **Delete entirely**
2. `workers/ruby/lib/tasker_core/handlers.rb` — Remove `require_relative 'task_handler/base'`,
   the `Tasks` module, and `Base = TaskHandler` alias
3. `workers/ruby/lib/tasker_core/types/task_template.rb`
   - Remove `attribute :task_handler` from `TaskTemplate` class (line 159)
   - Remove `attribute :task_handler` from `EnvironmentOverride` class (line 142)
   - Remove task_handler from `all_callables` method (line 177)
   - Remove task_handler override logic from `resolve_for_environment` (lines 200-203)
4. `workers/ruby/lib/tasker_core/template_discovery.rb`
   - Remove task_handler extraction from `extract_handlers_from_template` (lines 97-100)
5. `workers/ruby/spec/handlers/examples/*/handlers/*_handler.rb`
   - All handlers inheriting from `TaskerCore::TaskHandler::Base` need to be updated
   - These are test fixtures — decide whether to remove them or change their base class

### Phase 4: Remove `task_handler` from Python Worker

**Files to modify:**
1. `workers/python/python/tasker_core/template_discovery.py`
   - Remove task_handler.callable extraction (lines 286-291)
2. `workers/python/tests/test_template_discovery.py`
   - Remove/update tests referencing task_handler (11 occurrences across multiple test methods)

### Phase 5: Remove `task_handler` from Rust Integration Tests

**Files to modify:**
1. `tasker-orchestration/tests/config_integration_test.rs` — Remove task_handler assertions
2. `tests/basics/task_template_loading_integration_test.rs` — Update as needed
3. `tasker-orchestration/tests/services/viable_step_discovery_tests.rs` — Update as needed

### Phase 6: Clean Up `HandlerMetadata.handler_class`

1. `tasker-shared/src/types/base.rs` — Evaluate whether `handler_class` on `HandlerMetadata`
   is still needed or should be removed/renamed
2. `tasker-shared/src/registry/task_handler_registry.rs` — Remove the
   `"TaskerCore::TaskHandler::Base"` default string and handler_class extraction logic
3. `tasker-shared/src/messaging/execution_types.rs` — `handler_class` here refers to
   **step handler** class, not task handler — leave as-is

### Phase 7: Update Documentation

**Files to update:**
- `docs/workers/ruby.md`
- `docs/guides/conditional-workflows.md`
- `docs/guides/batch-processing.md`
- `docs/architecture/actors.md`
- `docs/security/alpha-audit-report.md`
- `docs/ticket-specs/TAS-{63,71,93,133,156,157,168}/*.md`
- `workers/ruby/spec/handlers/examples/blog_examples/README.md`
- `workers/rust/README.md`

### Phase 8 (Separate Ticket): Rename `TaskHandlerRegistry` → `TemplateRegistry`

This is a large mechanical rename affecting 22+ Rust source files and should be its own PR:
- Rename `TaskHandlerRegistry` → `TemplateRegistry` (or `TaskTemplateRegistry`)
- Rename `task_handler_registry` field → `template_registry` on `SystemContext` and all services
- Rename file `task_handler_registry.rs` → `template_registry.rs`
- Update module declarations in `registry/mod.rs`
- This is ~132 occurrences across ~22 files

### Phase 9: Proto/gRPC Cleanup

- Verify no `.proto` files reference `task_handler` (confirmed: none found)
- Update gRPC conversion code if TaskTemplate proto changes

---

## Risk Assessment

- **Low risk**: Removing `task_handler` from YAML, Python, docs — purely cosmetic
- **Medium risk**: Removing from TaskTemplate struct — touches serialization/deserialization;
  YAML files with `task_handler:` keys will need `#[serde(default)]` removed or the field
  handling updated so old YAML still parses (or all YAML files updated simultaneously)
- **Medium risk**: Removing Ruby `TaskHandler::Base` — need to verify no runtime code path
  depends on it (the `Handlers::Tasks.handle` method needs a replacement or removal)
- **Low risk**: TaskHandlerRegistry rename — purely mechanical, no semantic change

## Recommended Approach

1. Start with Phases 1-2 together (Rust struct + YAML) as they must be atomic
2. Phase 3 (Ruby) can be its own PR
3. Phase 4 (Python) can be its own PR
4. Phases 5-7 can be combined into one cleanup PR
5. Phase 8 (rename) should be its own ticket/PR
