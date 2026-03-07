# TAS-361: Workspace Restructuring Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Move all crates under `crates/` and all build tooling under `tools/` in a single atomic commit.

**Architecture:** Directory restructuring with path updates across Cargo.toml, Makefile.toml, shell scripts, CI workflows, Dockerfiles, and documentation. No functional code changes.

**Tech Stack:** Cargo workspaces, cargo-make, GitHub Actions, Docker, shell scripts

**Important context:** `--package tasker-shared` style references do NOT need updating — Cargo resolves package names via workspace members, not filesystem paths. Only filesystem path references (`path = "..."`, `cwd = "..."`, `cd workers/...`, `extend = "..."`) need updating.

---

### Task 1: Create directory structure and move crates

**Files:**
- Create: `crates/`, `crates/workers/`, `tools/`

**Step 1: Create target directories**

```bash
mkdir -p crates/workers tools
```

**Step 2: Move all crate directories into crates/**

```bash
# Top-level crates
mv tasker-shared crates/
mv tasker-orchestration crates/
mv tasker-worker crates/
mv tasker-client crates/
mv tasker-ctl crates/
mv tasker-sdk crates/
mv tasker-mcp crates/
mv tasker-pgmq crates/
mv tasker-grammar crates/

# Workers (move contents, not the workers/ dir itself since we created crates/workers/)
mv workers/ruby crates/workers/
mv workers/python crates/workers/
mv workers/rust crates/workers/
mv workers/typescript crates/workers/
rmdir workers
```

**Step 3: Move build tooling into tools/**

```bash
mv cargo-make tools/
mv bin tools/
mv scripts tools/
```

**Step 4: Verify moves**

```bash
ls crates/
# Expected: tasker-shared tasker-orchestration tasker-worker tasker-client tasker-ctl tasker-sdk tasker-mcp tasker-pgmq tasker-grammar workers

ls crates/workers/
# Expected: ruby python rust typescript

ls tools/
# Expected: cargo-make bin scripts
```

**Do NOT commit yet** — nothing compiles until path updates are done.

---

### Task 2: Update root Cargo.toml

**Files:**
- Modify: `Cargo.toml` (workspace root)

**Step 1: Update workspace members**

Change the `[workspace] members` array. All entries gain `crates/` prefix:

```toml
[workspace]
members = [
  ".",  # Current crate temporarily remains during migration
  "crates/tasker-pgmq",
  "crates/tasker-client",
  "crates/tasker-ctl",      # TAS-188: CLI binary (split from tasker-client)
  "crates/tasker-sdk",      # TAS-304: Shared SDK (codegen, template parsing, schema inspection, operational tooling)
  "crates/tasker-mcp",      # TAS-304: MCP server scaffold
  "crates/tasker-orchestration",
  "crates/tasker-shared",
  "crates/tasker-worker",
  "crates/tasker-grammar",  # TAS-321: Action grammar expression engine
  "crates/workers/python",  # TAS-72: PyO3 Python worker
  "crates/workers/ruby/ext/tasker_core",
  "crates/workers/rust",
  "crates/workers/typescript",  # TAS-100: TypeScript FFI worker (napi-rs)
  # "crates/workers/wasm",      # Future
]
```

**Step 2: Update workspace dependency paths**

```toml
# These two workspace deps have path references:
tasker-pgmq = { path = "crates/tasker-pgmq", version = "=0.1.6" }
tasker-sdk = { path = "crates/tasker-sdk", version = "=0.1.6" }
```

**Step 3: Update dev-dependency paths**

```toml
[dev-dependencies]
tasker-client = { path = "crates/tasker-client" }
tasker-mcp = { path = "crates/tasker-mcp" }
tasker-orchestration = { path = "crates/tasker-orchestration" }
tasker-shared = { path = "crates/tasker-shared" }
tasker-worker = { path = "crates/tasker-worker" }
```

**Step 4: Update feature references**

The feature references like `tasker-orchestration/test-utils` use package names, not paths — these do NOT need changing.

Similarly, `tasker-client/grpc` uses the package name. No change needed.

---

### Task 3: Update inter-crate Cargo.toml path dependencies

**Files:**
- Modify: `crates/tasker-client/Cargo.toml`
- Modify: `crates/tasker-mcp/Cargo.toml`
- Modify: `crates/tasker-ctl/Cargo.toml`
- Modify: `crates/tasker-orchestration/Cargo.toml`
- Modify: `crates/tasker-shared/Cargo.toml`
- Modify: `crates/tasker-worker/Cargo.toml`
- Modify: `crates/tasker-sdk/Cargo.toml`
- Modify: `crates/workers/python/Cargo.toml`
- Modify: `crates/workers/typescript/Cargo.toml`
- Modify: `crates/workers/rust/Cargo.toml`
- Modify: `crates/workers/ruby/ext/tasker_core/Cargo.toml`

**Key insight:** Sibling crate references (`path = "../tasker-shared"`) remain unchanged because all crates moved together. Only references to the workspace ROOT need updating (one more `../` since crates are now one level deeper).

**Step 1: Update crates that reference the root package (tasker-core)**

These crates have `path = "../"` pointing to root — change to `path = "../../"`:

In `crates/tasker-orchestration/Cargo.toml`:
```toml
# Was: tasker-core = { package = "tasker-core", path = "../" }
tasker-core = { package = "tasker-core", path = "../../" }
```

In `crates/tasker-shared/Cargo.toml`:
```toml
# Was: tasker-core = { package = "tasker-core", path = "../" }
tasker-core = { package = "tasker-core", path = "../../" }
```

In `crates/tasker-worker/Cargo.toml`:
```toml
# Was: tasker-core = { package = "tasker-core", path = "../" }
tasker-core = { package = "tasker-core", path = "../../" }
```

**Step 2: Update workers that reference root package**

In `crates/workers/rust/Cargo.toml`:
```toml
# Was: tasker-core = { package = "tasker-core", path = "../../" }
tasker-core = { package = "tasker-core", path = "../../../" }
```

In `crates/workers/ruby/ext/tasker_core/Cargo.toml`:
```toml
# Was: tasker-core = { package = "tasker-core", path = "../../../../" }
tasker-core = { package = "tasker-core", path = "../../../../../" }
```

**Step 3: Verify sibling references are still correct**

These should NOT change (verify they still resolve):
- `crates/tasker-client/Cargo.toml`: `path = "../tasker-shared"` — still correct (sibling under crates/)
- `crates/tasker-mcp/Cargo.toml`: `path = "../tasker-sdk"`, `"../tasker-shared"`, `"../tasker-client"` — still correct
- `crates/tasker-ctl/Cargo.toml`: `path = "../tasker-client"`, `"../tasker-shared"`, `"../tasker-sdk"` — still correct
- `crates/tasker-orchestration/Cargo.toml`: `path = "../tasker-pgmq"`, `"../tasker-shared"` — still correct
- `crates/tasker-worker/Cargo.toml`: `path = "../tasker-pgmq"`, `"../tasker-client"`, `"../tasker-shared"` — still correct
- `crates/tasker-sdk/Cargo.toml`: `path = "../tasker-shared"`, `"../tasker-client"` — still correct
- `crates/workers/python/Cargo.toml`: `path = "../../tasker-shared"`, `"../../tasker-worker"` — still correct (workers/python → ../../ → crates/)
- `crates/workers/typescript/Cargo.toml`: `path = "../../tasker-shared"`, `"../../tasker-worker"` — still correct
- `crates/workers/rust/Cargo.toml`: `path = "../../tasker-shared"`, `"../../tasker-worker"`, `"../../tasker-orchestration"` — still correct
- `crates/workers/ruby/ext/tasker_core/Cargo.toml`: `path = "../../../../tasker-shared"`, `"../../../../tasker-worker"` — still correct (ext/tasker_core → ../../../../ → crates/)

---

### Task 4: Update tasker-shared symlinks and build.rs

**Files:**
- Modify: `crates/tasker-shared/proto` (symlink)
- Modify: `crates/tasker-shared/migrations` (symlink)
- Modify: `crates/tasker-shared/build.rs`

**Step 1: Recreate symlinks with correct depth**

```bash
cd crates/tasker-shared
rm proto migrations
ln -s ../../proto proto
ln -s ../../migrations migrations
cd ../..
```

**Step 2: Verify symlinks resolve**

```bash
ls -la crates/tasker-shared/proto
# Should show: proto -> ../../proto

ls crates/tasker-shared/proto/tasker/v1/
# Should list .proto files

ls crates/tasker-shared/migrations/
# Should list migration files
```

**Step 3: Update build.rs proto detection**

In `crates/tasker-shared/build.rs`, the workspace proto fallback uses `parent()` to go up one level. Now we need to go up two levels (crates/tasker-shared → crates → workspace root):

```rust
// Find proto directory: check crate-local first (published crate),
// then workspace root (development builds)
let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
let local_proto = manifest_dir.join("proto");
let workspace_proto = manifest_dir
    .parent()
    .and_then(|p| p.parent())  // Two levels up: tasker-shared -> crates -> root
    .map(|p| p.join("proto"))
    .unwrap_or_default();
```

Change line 27-29 from:
```rust
let workspace_proto = manifest_dir
    .parent()
    .map(|p| p.join("proto"))
    .unwrap_or_default();
```

To:
```rust
let workspace_proto = manifest_dir
    .parent()
    .and_then(|p| p.parent())
    .map(|p| p.join("proto"))
    .unwrap_or_default();
```

---

### Task 5: Smoke test — `cargo check`

**Step 1: Run cargo check**

```bash
cargo check --all-features 2>&1
```

Expected: Should compile successfully. This validates:
- All workspace member paths resolve
- All inter-crate path dependencies resolve
- Symlinks work for proto/migrations
- build.rs finds proto files

If this fails, fix path issues before continuing. This is the critical gate — everything after this is non-Cargo path updates.

---

### Task 6: Update all crate Makefile.toml extend paths

**Files:**
- Modify: `crates/tasker-shared/Makefile.toml`
- Modify: `crates/tasker-orchestration/Makefile.toml`
- Modify: `crates/tasker-worker/Makefile.toml`
- Modify: `crates/tasker-client/Makefile.toml`
- Modify: `crates/tasker-ctl/Makefile.toml`
- Modify: `crates/tasker-sdk/Makefile.toml`
- Modify: `crates/tasker-mcp/Makefile.toml`
- Modify: `crates/tasker-pgmq/Makefile.toml`
- Modify: `crates/tasker-grammar/Makefile.toml`
- Modify: `crates/workers/rust/Makefile.toml`

**Step 1: Update top-level crate Makefile.toml files**

All crates directly under `crates/` change from `../cargo-make/` to `../../tools/cargo-make/`:

```toml
# Was: extend = "../cargo-make/base-tasks.toml"
extend = "../../tools/cargo-make/base-tasks.toml"
```

Apply to: tasker-shared, tasker-orchestration, tasker-worker, tasker-client, tasker-ctl, tasker-sdk, tasker-mcp, tasker-pgmq, tasker-grammar (9 files).

**Step 2: Update workers/rust Makefile.toml**

```toml
# Was: extend = "../../cargo-make/base-tasks.toml"
extend = "../../../tools/cargo-make/base-tasks.toml"
```

**Note:** Workers under `crates/workers/{lang}/` — Python, Ruby, TypeScript workers do NOT have a Makefile.toml that extends base-tasks (they use language-specific tooling). Only `workers/rust/Makefile.toml` extends base-tasks. Verify this is the case before proceeding; if any FFI worker Makefile.toml files also extend base-tasks, update those too.

Actually — the Ruby, Python, and TypeScript workers DO have Makefile.toml files that extend base-tasks. Check each one:

- `crates/workers/ruby/Makefile.toml` — likely does NOT use cargo-make extend (Ruby uses Rake)
- `crates/workers/python/Makefile.toml` — check if it extends base-tasks
- `crates/workers/typescript/Makefile.toml` — check if it extends base-tasks

Based on the earlier grep results, the Python, Ruby, and TypeScript worker Makefile.toml files were found at `workers/python/Makefile.toml`, `workers/ruby/Makefile.toml`, `workers/typescript/Makefile.toml` but were NOT in the `extend = "..."` grep results for base-tasks. They likely have their own task definitions. Verify and update if needed.

**Correction:** The grep results DID NOT show these workers in the extend results — only `workers/rust/Makefile.toml` appeared. The Ruby, Python, and TypeScript Makefile.toml files exist but don't extend base-tasks. Confirm during implementation.

---

### Task 7: Update root Makefile.toml

**Files:**
- Modify: `Makefile.toml`

This is the largest single file to update. Changes fall into categories:

**Step 1: Update extend path**

```toml
# Line 29 — was: extend = "./cargo-make/main.toml"
extend = "./tools/cargo-make/main.toml"
```

**Step 2: Update SCRIPTS_DIR**

```toml
# Line 46 — was: SCRIPTS_DIR = "cargo-make/scripts"
SCRIPTS_DIR = "tools/cargo-make/scripts"
```

This fixes all `${SCRIPTS_DIR}/...` references automatically (50+ task definitions).

**Step 3: Update hardcoded `source` commands**

Search for `source "cargo-make/scripts/` and replace with `source "tools/cargo-make/scripts/`:

Lines to update (approximate, verify during implementation):
- ~530: `source "cargo-make/scripts/split-db-env.sh"` → `source "tools/cargo-make/scripts/split-db-env.sh"`
- ~561: same pattern
- ~592: same pattern
- ~882: `source "cargo-make/scripts/docker-env.sh"` → `source "tools/cargo-make/scripts/docker-env.sh"`
- ~900: same pattern
- ~909: same pattern

**Step 4: Update hardcoded `python3 cargo-make/scripts/` commands**

Search for `cargo-make/scripts/` (without `${SCRIPTS_DIR}`) and replace with `tools/cargo-make/scripts/`:

- ~1837: `python3 cargo-make/scripts/profiling/...`
- ~2053, 2075: `bash cargo-make/scripts/bench-...`
- ~2205, 2271, 2349, 2382, 2457, 2473, 2547: coverage script paths

**Step 5: Update worker `cwd` references**

Search for `cwd = "workers/` and replace with `cwd = "crates/workers/`:

- ~509-593: Worker build tasks
- ~608-620: More worker references
- ~630-679: Language-specific build dependencies
- ~1237-1339: Service start/stop tasks
- ~2394-2424: Coverage tasks

**Step 6: Update worker path references in inline scripts**

Search for `workers/` in script blocks (not just cwd) and update to `crates/workers/`:

- Coverage cleanup paths (~2490-2493): `workers/ruby/coverage/` → `crates/workers/ruby/coverage/`
- Any `cd workers/` commands in inline scripts

**Step 7: Update crate `cwd` references**

Search for patterns like `cwd = "tasker-` and update to `cwd = "crates/tasker-`:

These may appear in tasks that change into a crate directory.

---

### Task 8: Update cargo-make config files

**Files:**
- Modify: `tools/cargo-make/workspace-config.toml`
- Modify: `tools/cargo-make/main.toml`
- Modify: `tools/cargo-make/base-tasks.toml` (comments only)

**Step 1: Update workspace-config.toml**

```toml
# Line 11 — was: SCRIPTS_DIR = "${WORKSPACE_ROOT}/cargo-make/scripts"
SCRIPTS_DIR = "${WORKSPACE_ROOT}/tools/cargo-make/scripts"

# Line 18 — was: WORKER_PATHS = "workers/python,workers/ruby,workers/typescript,workers/rust"
WORKER_PATHS = "crates/workers/python,crates/workers/ruby,crates/workers/typescript,crates/workers/rust"
```

**Step 2: Update main.toml**

```toml
# Line 30 — was: SCRIPTS_DIR = "cargo-make/scripts"
SCRIPTS_DIR = "tools/cargo-make/scripts"
```

The `extend = "./base-tasks.toml"` on line 11 is relative within cargo-make/ itself — this does NOT change since base-tasks.toml moved with it.

**Step 3: Update base-tasks.toml comments**

Update the documentation comments (lines 5-6) to reflect new paths:
```toml
# For crates: extend = { path = "../../tools/cargo-make/base-tasks.toml" }
# For workers: extend = { path = "../../../tools/cargo-make/base-tasks.toml" }
```

---

### Task 9: Update shell script path traversals

**Files:**
- Modify: `tools/cargo-make/scripts/setup-env.sh`
- Modify: `tools/cargo-make/scripts/test-web.sh`
- Modify: `tools/cargo-make/scripts/clean-workers.sh`
- Modify: `tools/cargo-make/scripts/ci-sanity-check.sh`
- Modify: `tools/cargo-make/scripts/sqlx-prepare.sh`
- Modify: `tools/cargo-make/scripts/services-health-check.sh`
- Modify: `tools/cargo-make/scripts/setup-workers.sh`
- Modify: `tools/cargo-make/scripts/ci-restore-typescript-artifacts.sh`
- Modify: `tools/cargo-make/scripts/ci-restore-ruby-extension.sh`
- Modify: `tools/cargo-make/scripts/migrate-db-split.sh`
- Modify: `tools/cargo-make/scripts/coverage-e2e.sh`
- Modify: `tools/cargo-make/scripts/services-stop-all.sh`
- Modify: `tools/cargo-make/scripts/build-rust.sh`
- Modify: `tools/cargo-make/scripts/run-orchestration.sh`
- Modify: `tools/cargo-make/scripts/multi-deploy/start-cluster.sh`
- Modify: `tools/cargo-make/scripts/claude-web/setup-common.sh`
- Modify: `tools/scripts/code_check.sh`
- Modify: `tools/scripts/ffi-build/lib/common.sh`
- Modify: `tools/scripts/ffi-build/build-ruby.sh`
- Modify: `tools/scripts/ffi-build/build-python.sh`
- Modify: `tools/scripts/release/lib/common.sh`
- Modify: `tools/bin/setup-dev.sh`
- Modify: `tools/bin/setup-claude-web.sh`

There are two categories of fixes:

**Category A: SCRIPT_DIR-based root calculation (depth change)**

Scripts that calculate workspace root via `cd "$SCRIPT_DIR/../.." && pwd` now need one more level because they moved from `cargo-make/scripts/` (2 levels deep) to `tools/cargo-make/scripts/` (3 levels deep).

Change `"$SCRIPT_DIR/../.."` to `"$SCRIPT_DIR/../../.."` in:
- `tools/cargo-make/scripts/setup-env.sh` (line ~30)
- `tools/cargo-make/scripts/test-web.sh` (line ~28-30)
- `tools/cargo-make/scripts/clean-workers.sh` (line ~5)
- `tools/cargo-make/scripts/ci-sanity-check.sh` (line ~31)
- `tools/cargo-make/scripts/multi-deploy/start-cluster.sh` (line ~35): was `"../../.."` → now `"../../../.."`

For scripts under `tools/scripts/` (moved from `scripts/`), they go from 1 level to 2 levels deep:
- `tools/scripts/code_check.sh` (line ~25-26): `"$SCRIPT_DIR/.."` → `"$SCRIPT_DIR/../.."`
- `tools/scripts/ffi-build/lib/common.sh` (line ~13): `"../../.."` → `"../../../.."`
- `tools/scripts/release/lib/common.sh` (line ~13): `"../../.."` → `"../../../.."`

For scripts under `tools/bin/` (moved from `bin/`), they go from 1 level to 2 levels deep:
- `tools/bin/setup-claude-web.sh`: update PROJECT_DIR calculation
- `tools/bin/setup-dev.sh`: update PROJECT_ROOT calculation

**Category B: Hardcoded `cargo-make/scripts` in PROJECT_ROOT-relative paths**

Scripts that construct paths like `${PROJECT_ROOT}/cargo-make/scripts` need updating:
- `tools/cargo-make/scripts/test-web.sh` (~line 30): `CLAUDE_WEB_DIR="${PROJECT_DIR}/cargo-make/scripts/claude-web"` → `"${PROJECT_DIR}/tools/cargo-make/scripts/claude-web"`
- `tools/cargo-make/scripts/coverage-e2e.sh` (~line 43): `_SCRIPTS_DIR="${PROJECT_ROOT}/cargo-make/scripts"` → `"${PROJECT_ROOT}/tools/cargo-make/scripts"`
- `tools/cargo-make/scripts/services-stop-all.sh` (~line 14): same pattern
- `tools/cargo-make/scripts/run-orchestration.sh` (~line 22): same pattern

Scripts that use `$(pwd)/cargo-make/scripts` (assumes pwd is workspace root):
- `tools/cargo-make/scripts/migrate-db-split.sh` (~line 21): `"$(pwd)/cargo-make/scripts"` → `"$(pwd)/tools/cargo-make/scripts"`
- `tools/cargo-make/scripts/build-rust.sh` (~line 12): same pattern

**Category C: Hardcoded worker/crate paths relative to PROJECT_ROOT**

Scripts that reference `workers/` relative to PROJECT_ROOT need `crates/workers/`:
- `tools/cargo-make/scripts/clean-workers.sh`: `cd "$WORKSPACE_ROOT/workers/python"` → `cd "$WORKSPACE_ROOT/crates/workers/python"` (and ruby, typescript)
- `tools/cargo-make/scripts/sqlx-prepare.sh`: array of crate paths needs `crates/` prefix for all entries
- `tools/cargo-make/scripts/setup-workers.sh`: `cd "$WORKSPACE_ROOT/workers/python"` → `crates/workers/python` etc.
- `tools/cargo-make/scripts/services-health-check.sh`: worker `.env` paths
- `tools/cargo-make/scripts/ci-restore-typescript-artifacts.sh`: `workers/typescript/` → `crates/workers/typescript/`
- `tools/cargo-make/scripts/ci-restore-ruby-extension.sh`: `workers/ruby/` → `crates/workers/ruby/`
- `tools/bin/setup-dev.sh`: `cd workers/python` → `cd crates/workers/python` etc.
- `tools/scripts/code_check.sh`: worker directory references
- `tools/scripts/ffi-build/build-ruby.sh`: `workers/ruby` → `crates/workers/ruby`
- `tools/scripts/ffi-build/build-python.sh`: `workers/python` → `crates/workers/python`
- `tools/scripts/release/*.sh`: Multiple files with `${REPO_ROOT}/workers/{lang}/` → `${REPO_ROOT}/crates/workers/{lang}/`

Also update crate directory references where they appear relative to PROJECT_ROOT:
- `tools/cargo-make/scripts/sqlx-prepare.sh`: `"tasker-shared"` → `"crates/tasker-shared"` etc. for all crate entries in the array

**Implementation approach:** For each script file, read it, identify all three categories of path references, and fix them. The key principle: anything that resolves relative to the workspace root and references a moved directory needs updating.

---

### Task 10: Update CI workflow files

**Files:**
- Modify: `.github/workflows/test-integration.yml`
- Modify: `.github/workflows/build-workers.yml`
- Modify: `.github/workflows/test-ruby-framework.yml`
- Modify: `.github/workflows/test-python-framework.yml`
- Modify: `.github/workflows/test-typescript-framework.yml`
- Modify: `.github/workflows/validate-codegen.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `.github/workflows/release-mcp.yml`
- Modify: `.github/workflows/build-cli-binaries.yml`

**Step 1: Update worker directory references**

Search and replace `workers/` with `crates/workers/` in all workflow files for:
- `working-directory:` values
- `cd workers/` commands
- Artifact paths (`workers/ruby/lib/`, `workers/typescript/dist/`, etc.)

**Step 2: Update build tooling references**

Search and replace `cargo-make/scripts/` with `tools/cargo-make/scripts/` for:
- CI script invocations (`cargo-make/scripts/ci-restore-*.sh`, `ci-start-*.sh`, `ci-stop-*.sh`, `ci-display-*.sh`)
- Validation scripts (`cargo-make/scripts/validate-codegen.sh`)

**Step 3: Update scripts/ references**

Search and replace `./scripts/` with `./tools/scripts/` for:
- Release scripts (`./scripts/release/read-versions.sh`, `./scripts/release/detect-changes.sh`, etc.)

**Step 4: Update bin/ references**

Search and replace `./bin/` or `bin/` with `./tools/bin/` or `tools/bin/` where used.

**Step 5: Update Dockerfile references**

In `release-mcp.yml`:
```yaml
# Was: file: tasker-mcp/Dockerfile
file: crates/tasker-mcp/Dockerfile
```

Verify: does `crates/tasker-mcp/Dockerfile` exist? If not, check if this references a Docker file elsewhere.

**Note:** `--package tasker-shared` style references do NOT change — Cargo resolves by package name.

---

### Task 11: Update Docker files

**Files:**
- Modify: `docker/docker-compose.ci.yml`
- Modify: `docker/docker-compose.prod.yml`
- Modify: `docker/docker-compose.test-full.yml`
- Modify: Docker build files in `docker/build/` (all Dockerfiles)

**Step 1: Update compose file volume mounts**

Volume mounts reference paths relative to the compose file's parent (which is `docker/`):

```yaml
# Was: ../workers/ruby/spec/handlers:/app/ruby_worker/spec/handlers:ro
# Now: ../crates/workers/ruby/spec/handlers:/app/ruby_worker/spec/handlers:ro
```

Apply same pattern for all `../workers/` references in compose files.

**Step 2: Update Dockerfiles**

Dockerfiles in `docker/build/` use COPY commands relative to the Docker build context (typically workspace root). Update:
- `workers/rust/` → `crates/workers/rust/`
- `workers/ruby/` → `crates/workers/ruby/`
- `workers/python/` → `crates/workers/python/`
- `workers/typescript/` → `crates/workers/typescript/`
- `tasker-shared/` → `crates/tasker-shared/` (if referenced)
- `tasker-orchestration/` → `crates/tasker-orchestration/` (if referenced)
- Other crate references similarly

Also update references to:
- `scripts/ffi-build/` → `tools/scripts/ffi-build/`
- `cargo-make/` → `tools/cargo-make/` (if any)
- `docker/scripts/create-workspace-stubs.sh` — this stays under `docker/`, no change

**Step 3: Verify docker-compose.test.yml**

Check `docker/docker-compose.test.yml` for worker path references and update similarly.

---

### Task 12: Update documentation

**Files:**
- Modify: `CLAUDE.md`
- Modify: `AGENTS.md`
- Modify: `.claude/skills/cargo-make-tooling.md`
- Modify: `.claude/skills/deployment-and-infrastructure.md`
- Modify: `.claude/skills/coverage-tooling.md`
- Modify: `.claude/skills/cross-language-development.md`
- Modify: `.claude/skills/project-navigation.md`
- Modify: `.claude/skills/testing-infrastructure.md`
- Modify: `.claude/skills/database-and-sqlx.md`
- Modify: `CONTRIBUTING.md`
- Modify: `crates/tasker-orchestration/AGENTS.md`
- Modify: `crates/tasker-worker/AGENTS.md`
- Modify: `crates/workers/typescript/AGENTS.md`
- Modify: `docs/development/tooling.md`
- Modify: `docs/development/coverage-tooling.md`
- Modify: Any other docs referencing moved paths

**Step 1: Update CLAUDE.md**

The root CLAUDE.md has extensive path references. Key updates:
- Workspace structure section: all crate paths gain `crates/` prefix
- `cargo-make/` references → `tools/cargo-make/`
- `bin/` references → `tools/bin/`
- `scripts/` references → `tools/scripts/`
- `workers/` references → `crates/workers/`
- `cd workers/ruby` → `cd crates/workers/ruby`
- `SCRIPTS_DIR` documentation

**Step 2: Update AGENTS.md**

Same pattern as CLAUDE.md — update all path references.

**Step 3: Update skill files**

Each skill file that references crate paths, cargo-make paths, or worker paths needs updating. Focus on the ones identified by the research agents:
- `cargo-make-tooling.md`: `cargo-make/scripts/` → `tools/cargo-make/scripts/`
- `deployment-and-infrastructure.md`: same
- `coverage-tooling.md`: same
- `cross-language-development.md`: `workers/` → `crates/workers/`
- `project-navigation.md`: crate path references

**Step 4: Update CONTRIBUTING.md**

```bash
# Was: bin/setup-dev.sh
tools/bin/setup-dev.sh
```

**Step 5: Update other docs**

Search `docs/` for references to moved paths and update. Key files:
- `docs/development/tooling.md`
- `docs/development/coverage-tooling.md`

**Step 6: Update parent-level CLAUDE.md files**

The `tasker-systems/CLAUDE.md` references tasker-core internal paths. Update:
- Workspace structure showing crate names
- Any `cd workers/ruby` type commands

---

### Task 13: Verification — cargo check and tests

**Step 1: Full cargo check**

```bash
cargo check --all-features
```

Expected: Clean compilation.

**Step 2: cargo clippy**

```bash
cargo clippy --all-targets --all-features --workspace
```

Expected: Zero warnings.

**Step 3: cargo fmt check**

```bash
cargo fmt -- --check
```

Expected: No formatting issues.

**Step 4: Run no-infra tests**

```bash
cargo make test-no-infra
```

Expected: All tests pass. This validates cargo-make paths, SCRIPTS_DIR, and test fixture paths.

**Step 5: Run unit tests (if DB available)**

```bash
cargo make test-rust-unit
```

Expected: All tests pass.

---

### Task 14: Regenerate .sqlx cache

**Files:**
- Modify: `.sqlx/` directory contents

**Step 1: Regenerate SQLx prepared queries**

```bash
DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test \
cargo sqlx prepare --workspace -- --all-targets --all-features
```

**Step 2: Stage the regenerated cache**

```bash
git add .sqlx/
```

---

### Task 15: Verify publishing (dry-run)

**Step 1: Verify tasker-shared can be packaged**

```bash
cd crates/tasker-shared
cargo publish --dry-run
cd ../..
```

Expected: Package builds successfully. This validates that symlinks for `proto` and `migrations` resolve correctly in the packaged crate.

---

### Task 16: Commit

**Step 1: Stage all changes**

```bash
git add -A
```

**Step 2: Review staged changes**

```bash
git status
git diff --cached --stat
```

Verify: No unexpected files, no missing files, no stale paths in committed content.

**Step 3: Commit atomically**

```bash
git commit -m "refactor(TAS-361): restructure workspace — crates/ and tools/ directories

Move all workspace crates under crates/ and build tooling under tools/
to match idiomatic Rust workspace conventions.

- 9 crates moved to crates/tasker-{name}/
- 4 FFI workers moved to crates/workers/{lang}/
- cargo-make, bin, scripts moved to tools/
- Updated all path dependencies, symlinks, build.rs
- Updated Makefile.toml extend paths and SCRIPTS_DIR
- Updated CI workflows, Dockerfiles, documentation
- Regenerated .sqlx prepared query cache
- proto/, migrations/, tests/, config/, docker/ stay at root

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task Dependency Graph

```
Task 1 (move dirs)
  → Task 2 (root Cargo.toml)
  → Task 3 (inter-crate Cargo.toml)
  → Task 4 (symlinks + build.rs)
    → Task 5 (smoke test: cargo check) ← GATE
      → Task 6 (crate Makefile.toml)
      → Task 7 (root Makefile.toml)
      → Task 8 (cargo-make configs)
      → Task 9 (shell scripts)
      → Task 10 (CI workflows)
      → Task 11 (Docker files)
      → Task 12 (documentation)
        → Task 13 (verification) ← GATE
          → Task 14 (.sqlx cache)
          → Task 15 (publish dry-run)
            → Task 16 (commit)
```

Tasks 6-12 are independent of each other and can be parallelized.
Tasks 2-4 are independent of each other and can be parallelized after Task 1.
