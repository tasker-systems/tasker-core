# TAS-208 Phase 2: tasker-contrib CI & Template Validation Tooling

## Context

Phase 1 (complete, pushed) populated tasker-contrib with 58 template files across 5 plugins. Phase 2 builds the tooling to validate these templates actually work — both locally via cargo-make and in CI via GitHub Actions.

**Strategy: bleeding-edge first.** We shallow-clone tasker-core and build tasker-ctl from source. This sidesteps the unsolved .dylib/.so distribution question while providing real validation. sccache (borrowed from tasker-core CI patterns) prevents build timeouts.

**Scope: Tier 1 template validation only.** Generate every template, syntax-check every output file. Tier 2 (running generated tests against FFI packages) and Tier 3 (example apps with services) are future work.

---

## Phase A: Plugin Discovery Config + Cleanup

### A1. Create `.tasker-cli.toml` at repo root

```toml
plugin-paths = ["."]
```

This enables `tasker-ctl` to discover all 5 plugins via Level 2 scanning (`*/tasker-cli-plugin/`).

### A2. Remove .keep files from populated template directories

Remove `.keep` files from populated template directories. Also remove unpopulated dirs entirely where we have `.keep` files but no planned work - YAGNI and we can always create a new directory.

---

## Phase B: Test Scripts

All scripts in `scripts/` — POSIX-compatible (macOS bash 3.2 safe: no `mapfile`, no `${var^^}`, no associative arrays).

### B1. `scripts/build-tasker-ctl.sh`

- Accepts `TASKER_CORE_PATH` (default: `../tasker-core`)
- Runs `SQLX_OFFLINE=true cargo build --package tasker-ctl` in that directory
- Exports `TASKER_CTL` pointing to the built binary
- Skips rebuild if binary is newer than `Cargo.lock`

### B2. `scripts/validate-plugins.sh`

- Iterates `*/tasker-cli-plugin/` directories
- Runs `$TASKER_CTL plugin validate <dir>` on each
- Reports pass/fail summary

### B3. `scripts/test-templates.sh` (core deliverable)

The main validation engine. Algorithm:

1. Requires `TASKER_CTL` env var
2. Creates temp working directory with `.tasker-cli.toml` pointing at repo root
3. For each plugin (`*/tasker-cli-plugin/`):
   - Extract plugin name and language from `tasker-plugin.toml`
   - Run `$TASKER_CTL plugin validate <dir>`
   - For each template (parsed from `[[templates]]` entries):
     - Determine test parameters from hardcoded map:
       | Template | Params |
       |----------|--------|
       | `step_handler` | `name=TestProcessor` |
       | `step_handler_api` | `name=FetchUser` |
       | `step_handler_decision` | `name=RouteOrder` |
       | `step_handler_batchable` | `name=ProcessBatch` |
       | `task_template` | `name=ProcessOrder`, `handler_callable=Handlers::ProcessOrderHandler` |
       | `docker_compose` | `name=myapp` |
       | `config` | _(none required)_ |
     - Run `$TASKER_CTL template generate <name> --plugin <plugin> --param ... --output <tmpdir>`
     - Verify output directory is non-empty
     - Syntax-check each generated file by extension:
       | Extension | Check |
       |-----------|-------|
       | `.rb` | `ruby -c` |
       | `.py` | `python3 -m py_compile` |
       | `.ts` | `bun build --no-bundle` |
       | `.rs` | `rustfmt --check` (format-as-syntax proxy) |
       | `.yaml`/`.yml` | `python3 -c "import yaml; yaml.safe_load(...)"` |
       | `.toml` | `python3 -c "import tomllib; ..."` |
4. Reports summary: N templates tested, N passed, N failed
5. Exits non-zero on any failure

Accepts `--plugin <name>` filter for running a single plugin's templates.

### B4. `scripts/cleanup-keep-files.sh`

- Finds `.keep` files under `*/tasker-cli-plugin/templates/`
- Removes only where sibling `.tera` or `template.toml` files exist
- Reports removals

---

## Phase C: cargo-make Tooling

### `Makefile.toml` at repo root

```
[config]
skip_core_tasks = true

[env]
TASKER_CORE_PATH = { default = "../tasker-core" }
SCRIPTS_DIR = "scripts"

Tasks:
  build-ctl          Build tasker-ctl from tasker-core source
  validate      (v)  Validate all plugin manifests (depends: build-ctl)
  test-templates (tt) Tier 1: generate + syntax check (depends: build-ctl)
  test-all      (ta) Run all validation (depends: validate, test-templates)
  cleanup-keep       Remove .keep from populated template dirs
```

---

## Phase D: Composite GitHub Actions

Adapted from tasker-core's `.github/actions/` but stripped of DB/auth/messaging concerns.

### D1. `.github/actions/setup-sccache/action.yml`

- `mozilla-actions/sccache-action@v0.0.9`
- Sets `RUSTC_WRAPPER=sccache`, `SCCACHE_GHA_ENABLED=true`, `CARGO_TERM_COLOR=always`

### D2. `.github/actions/setup-rust-cache/action.yml`

- `actions/cache@v5` for `~/.cargo/` + `target/`
- Input: `cache-prefix` (default: `"contrib"`), `working-directory` (default: `"tasker-core"`)
- Key: `{os}-cargo-{prefix}-{hash(Cargo.lock)}`

### D3. `.github/actions/build-tasker-core/action.yml`

The key composite action:
1. Shallow clone: `git clone --depth 1 --branch <ref> https://github.com/tasker-systems/tasker-core.git tasker-core`
2. `dtolnay/rust-toolchain@stable`
3. Install protobuf compiler (required for tonic-build)
4. Uses D1 (sccache) + D2 (rust-cache)
5. `SQLX_OFFLINE=true cargo build --package tasker-ctl --bin tasker-ctl`
6. Outputs `tasker-ctl-path` and uploads binary as artifact

Input: `tasker-core-ref` (default: `"main"`)

---

## Phase E: CI Workflow Rewrite

### E1. `.github/workflows/ci.yml` (rewrite)

The existing ci.yml tests non-existent contrib packages. Rewrite for template validation.

```
jobs:
  changes:
    # dorny/paths-filter@v3
    # Filters: templates (*/tasker-cli-plugin/**), docs, scripts, actions

  build-tasker-ctl:
    if: needs.changes.outputs.templates == 'true'
    # Uses .github/actions/build-tasker-core
    # Uploads tasker-ctl binary as artifact
    timeout-minutes: 20

  validate-plugins:
    needs: build-tasker-ctl
    # Downloads tasker-ctl artifact
    # Runs scripts/validate-plugins.sh against all 5 plugins

  test-ruby-templates:
    needs: build-tasker-ctl
    if: needs.changes.outputs.rails-templates == 'true'
    # ruby/setup-ruby@v1 (for ruby -c syntax check)
    # Downloads tasker-ctl, generates rails templates, syntax checks .rb files

  test-python-templates:
    needs: build-tasker-ctl
    if: needs.changes.outputs.python-templates == 'true'
    # actions/setup-python@v6 (for py_compile)
    # Generates python templates, syntax checks .py files

  test-typescript-templates:
    needs: build-tasker-ctl
    if: needs.changes.outputs.typescript-templates == 'true'
    # oven-sh/setup-bun@v2 (for bun build syntax check)
    # Generates typescript templates, syntax checks .ts files

  test-rust-templates:
    needs: build-tasker-ctl
    if: needs.changes.outputs.rust-templates == 'true'
    # dtolnay/rust-toolchain@stable (for rustfmt)
    # Generates rust templates, syntax checks .rs files

  test-ops-templates:
    needs: build-tasker-ctl
    if: needs.changes.outputs.ops-templates == 'true'
    # Python for yaml.safe_load / tomllib validation
    # Generates ops templates, validates YAML + TOML output

  docs:
    if: needs.changes.outputs.docs == 'true'
    # Keep existing markdown link checker

  ci-summary:
    needs: [validate-plugins, test-ruby-templates, test-python-templates,
            test-typescript-templates, test-rust-templates, test-ops-templates, docs]
    if: always()
    # Aggregate results into summary table
```

No PostgreSQL services needed anywhere — tasker-ctl is a pure CLI tool for template operations.

### E2. `.github/workflows/bleeding-edge.yml` (rewrite)

Simplified from existing. Tests templates against latest tasker-core main.

```
triggers:
  - repository_dispatch: tasker-core-updated (from tasker-core CI)
  - workflow_dispatch (manual, with tasker-core-ref input)
  - schedule: '0 4 * * *' (nightly)

jobs:
  build-tasker-ctl:
    # Build from $TASKER_CORE_REF (dispatch SHA or main)
    # Uses .github/actions/build-tasker-core with ref input

  validate-and-test:
    needs: build-tasker-ctl
    # Install all language toolchains (Ruby, Python, Bun, Rust)
    # Run full scripts/test-templates.sh (all plugins, all templates)
    # Single job since bleeding-edge runs less frequently

  report:
    needs: validate-and-test
    if: failure()
    # Create/update GitHub issue with label 'bleeding-edge-failure'
```

### E3. `.github/workflows/upstream-check.yml` (minor update)

- Update package names: `@tasker-systems/tasker` (npm), `tasker_core` (PyPI), `tasker-core-rb` (RubyGems)
- Update `.github/upstream-versions.json` to `0.1.1`

---

## Phase F: Documentation Updates

- `.github/CI-ARCHITECTURE.md` — rewrite to reflect template validation architecture
- Root `CLAUDE.md` or `CONTRIBUTING.md` — add cargo-make commands for local development

---

## Execution Order

1. **Phase A** — `.tasker-cli.toml` + cleanup .keep files (unblocks everything)
2. **Phase B** — Test scripts (core deliverables, can test locally immediately)
3. **Phase C** — Makefile.toml (wraps Phase B for ergonomic local use)
4. **Phase D** — Composite GitHub Actions (needed by CI workflows)
5. **Phase E** — CI workflow rewrites (depends on B + D)
6. **Phase F** — Documentation updates

---

## File Summary

### New files (13):
| File | Purpose |
|------|---------|
| `.tasker-cli.toml` | Plugin discovery config |
| `scripts/build-tasker-ctl.sh` | Local tasker-ctl build helper |
| `scripts/validate-plugins.sh` | Plugin manifest validation |
| `scripts/test-templates.sh` | Template generation + syntax validation |
| `scripts/cleanup-keep-files.sh` | Remove obsolete .keep files |
| `Makefile.toml` | cargo-make task definitions |
| `.github/actions/setup-sccache/action.yml` | sccache composite action |
| `.github/actions/setup-rust-cache/action.yml` | Cargo cache composite action |
| `.github/actions/build-tasker-core/action.yml` | tasker-ctl build composite action |

### Rewritten files (2):
| File | Change |
|------|--------|
| `.github/workflows/ci.yml` | Package testing -> template validation |
| `.github/workflows/bleeding-edge.yml` | Simplify to tasker-ctl-only build + template tests |

### Updated files (2-3):
| File | Change |
|------|--------|
| `.github/workflows/upstream-check.yml` | Fix package names, update versions |
| `.github/CI-ARCHITECTURE.md` | Reflect new workflow structure |

### Deleted files (~19):
| Pattern | Count |
|---------|-------|
| `.keep` in populated `*/tasker-cli-plugin/templates/*/` dirs | ~19 |

---

## Verification

### Local
```bash
cd tasker-contrib

# Install cargo-make if needed
cargo install cargo-make

# Build tasker-ctl from sibling tasker-core repo
cargo make build-ctl

# Validate all plugin manifests
cargo make validate

# Run full template generation + syntax checks
cargo make test-templates
```

### CI
- Push branch, open PR — ci.yml triggers template validation jobs
- Manual dispatch of bleeding-edge.yml to test against tasker-core main
- All template generation + syntax checks pass across all 5 plugins (19 templates)
