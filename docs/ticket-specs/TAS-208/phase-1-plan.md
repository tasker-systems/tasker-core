# TAS-208: Populate tasker-contrib with Starter Templates

## Context

The CLI plugin system was delivered in TAS-207 (PR #212) — plugin discovery, Tera template engine, and `plugin`/`template` commands are all working in tasker-ctl. But there's no content to discover yet.

tasker-contrib already exists as a sibling repo (`../tasker-contrib`) with the right directory structure and empty template directories. It has `tasker-plugin.toml` files but they use an **outdated schema** that predates the actual parser. All template directories are empty.

**Two repos involved:**
- **tasker-contrib** (primary work): fix manifests, create all template content
- **tasker-core** (this branch): integration tests, any parser fixes

---

## Phase 1: Fix Plugin Manifests in tasker-contrib

Existing manifests use wrong schema (`languages` plural, `[templates]` table). Must rewrite to match `PluginManifest` struct in `tasker-ctl/src/plugins/manifest.rs`:

**Correct format:**
```toml
[plugin]
name = "tasker-contrib-rails"
version = "0.1.0"
description = "Ruby on Rails templates for Tasker CLI"
language = "ruby"
framework = "rails"

[[templates]]
name = "step_handler"
path = "templates/step_handler"
description = "Generate a step handler class with RSpec test"
```

**Files to rewrite (4):**
- `rails/tasker-cli-plugin/tasker-plugin.toml`
- `python/tasker-cli-plugin/tasker-plugin.toml`
- `typescript/tasker-cli-plugin/tasker-plugin.toml`
- `rust/tasker-cli-plugin/tasker-plugin.toml`

**File to create (1):**
- `ops/tasker-cli-plugin/tasker-plugin.toml` — infrastructure plugin (docker-compose, config)

---

## Phase 2: Step Handler Templates (All 4 Languages)

Each directory gets `template.toml` + `.tera` files using the **actual published FFI package API**.

### Ruby (`rails/tasker-cli-plugin/templates/step_handler/`)
- **handler.rb.tera**: Class `{{ name | pascal_case }}Handler < TaskerCore::StepHandler::Base` with `call(context)`, `success(result:)`, `failure(message:)`
- **handler_spec.rb.tera**: RSpec test with describe/context/it

### Python (`python/tasker-cli-plugin/templates/step_handler/`)
- **handler.py.tera**: Class `{{ name | pascal_case }}Handler(StepHandler)` from `tasker_core.step_handler` with `call(self, context)`, `self.success()`, `self.failure()`
- **test_handler.py.tera**: pytest test

### TypeScript (`typescript/tasker-cli-plugin/templates/step_handler/`)
- **handler.ts.tera**: Class `{{ name | pascal_case }}Handler extends StepHandler` from `@tasker-systems/tasker` with `async call(context)`, `this.success()`, `this.failure()`
- **handler.test.ts.tera**: vitest test

### Rust (`rust/tasker-cli-plugin/templates/step_handler/`)
- **handler.rs.tera**: Struct implementing `RustStepHandler` trait with `async fn call()`, `success_result()`, `error_result()`
- **handler_test.rs.tera**: `#[cfg(test)]` module

**Parameters for all:** `name` (required), `module_name` (optional with language-appropriate default)

---

## Phase 3: Task Definition Templates (All 4 Languages)

YAML task template with step DAG. Format is language-agnostic — only `handler.callable` differs.

### All languages (`{lang}/tasker-cli-plugin/templates/task_template/`)
- **template.toml**: params `name` (required), `namespace` (default: "default"), `handler_callable` (required)
- **task.yaml.tera**: YAML with name, namespace, version, steps array, retry config, input_schema

---

## Phase 4: Specialized Handler Variants (Ruby, Python, TypeScript)

Build on the base step_handler with domain-specific patterns:

### step_handler_api
- HTTP client setup, request/response handling, error mapping
- Shows external API call pattern within a step

### step_handler_decision
- Returns decision outcomes routing to different downstream steps
- Paired with decision_routes in task template

### step_handler_batchable
- Implements `Batchable` mixin/interface
- Shows `create_cursor_ranges`, `batch_analyzer_success` pattern

Rust excluded from variants (fewer specialized patterns in the Rust worker).

---

## Phase 5: Infrastructure Plugin

### `ops/tasker-cli-plugin/` (new)

**docker_compose template:**
- params: `name` (required), `messaging` (default: "pgmq"), `cache` (default: "none")
- `docker-compose.yml.tera`: Tera conditionals for optional services
  - `tasker-postgres` — always (ghcr.io/tasker-systems/tasker-postgres:latest)
  - `tasker-orchestration` — always (ghcr.io/tasker-systems/tasker-orchestration:latest)
  - `rabbitmq` — if `messaging == "rabbitmq"`
  - `dragonfly` — if `cache == "dragonfly"`

**config template:**
- params: `messaging` (default: "pgmq"), `environment` (default: "development")
- `common.toml.tera`: database pool, circuit breakers, messaging backend selection
- `orchestration.toml.tera`: event systems, actors config
- `worker.toml.tera`: event systems, step processing config

---

## Phase 6: Integration Tests in tasker-core

**`tasker-ctl/tests/plugin_integration_test.rs`** (new):
- Create temp directories with realistic plugin structure (mirroring tasker-contrib layout)
- Test manifest parsing with correct schema
- Test `template generate` produces expected output files with correct content
- Test 3-level discovery with `*/tasker-cli-plugin/` layout
- Self-contained (no dependency on tasker-contrib being on disk)

---

## Execution Order

1. **Phase 1** — fix manifests (everything depends on this)
2. **Phase 2** — step_handler templates (highest value, proves pipeline)
3. **Phase 3** — task_template (complements step_handler)
4. **Phase 5** — infrastructure plugin (docker-compose + config)
5. **Phase 4** — specialized handler variants (builds on phase 2)
6. **Phase 6** — integration tests in tasker-core

---

## File Summary

### tasker-contrib (~50 files)

**Manifests (5):** 4 rewrites + 1 new (ops)

**Step handler templates (12):** template.toml + handler.tera + test.tera × 4 languages

**Task definition templates (8):** template.toml + task.yaml.tera × 4 languages

**Specialized handler templates (~18):** 3 variants × 3 languages × (template.toml + handler.tera)

**Infrastructure templates (7):** docker_compose (template.toml + yml.tera) + config (template.toml + 3 toml.tera)

### tasker-core (this branch)

**New:** `tasker-ctl/tests/plugin_integration_test.rs`
**Possible:** parser fixes discovered during template creation

---

## Verification

1. In tasker-contrib directory:
   ```bash
   # Create .tasker-cli.toml pointing at tasker-contrib
   echo 'plugin-paths = ["."]' > /tmp/.tasker-cli.toml

   # From tasker-contrib root:
   tasker-ctl plugin list
   tasker-ctl template list
   tasker-ctl template list --language ruby
   tasker-ctl template info step_handler --plugin tasker-contrib-rails
   tasker-ctl template generate step_handler --language ruby \
     --param name=ProcessPayment --output /tmp/test-output
   ```
2. Verify generated handler has correct class name, imports, test structure
3. `cargo test --all-features -p tasker-ctl` passes (including new integration tests)
4. `cargo clippy --all-targets --all-features -p tasker-ctl` zero warnings
