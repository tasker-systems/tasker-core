# TAS-362: Rename FFI Worker Crates Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rename FFI worker crates from `crates/workers/{lang}` to `crates/tasker-{rb,py,ts,example-rs}`, eliminating the `crates/workers/` directory entirely.

**Architecture:** Four directory moves with path dependency updates. Only the Rust worker needs a package name change (`tasker-worker-rust` → `tasker-example-rs`). Moving from depth-3 (`crates/workers/{lang}`) to depth-2 (`crates/tasker-{name}`) simplifies all relative paths by one level.

**Tech Stack:** Cargo workspace, cargo-make, GitHub Actions, Docker, shell scripts

**Branch:** `jcoletaylor/tas-361-restructure-workspace-move-crates-scripts-and-config-into` (continuing from TAS-361)

---

## Pre-flight: Path Depth Reference

After the rename, all worker crates are direct children of `crates/` (depth 2 from root):

| Direction | From worker crate | Example |
|-----------|-------------------|---------|
| To root | `../../` | `../../target`, `../../.env` |
| To sibling crate | `../tasker-shared` | `../tasker-worker` |
| To tools | `../../tools/cargo-make/scripts/` | Service scripts, coverage |
| Ruby ext to sibling | `../../../tasker-shared` | From `ext/tasker_core/` |
| Ruby ext to root | `../../../../` | For tasker-core root package |

**Important:** The worker Makefile.toml files currently have `../../cargo-make/scripts/` paths that are doubly broken from TAS-361 (workers moved deeper + cargo-make moved to tools/). The TAS-362 rename fixes the depth but the `tools/` prefix must be added: `../../cargo-make/scripts/` → `../../tools/cargo-make/scripts/`.

---

## Task 1: Commit Outstanding TAS-361 Changes

**Files:** 3 modified, unstaged files from TAS-361

**Step 1: Stage and commit the leftover changes**

```bash
git add crates/tasker-ctl/tests/codegen_integration_test.rs \
       crates/tasker-ctl/tests/handler_codegen_integration_test.rs \
       crates/tasker-shared/src/config/tasker.rs
git commit -m "fix(TAS-361): test path resolution for workspace restructuring"
```

---

## Task 2: Move Worker Directories

**Step 1: Move all four worker directories**

```bash
git mv crates/workers/ruby crates/tasker-rb
git mv crates/workers/python crates/tasker-py
git mv crates/workers/typescript crates/tasker-ts
git mv crates/workers/rust crates/tasker-example-rs
```

**Step 2: Remove the now-empty `crates/workers/` directory**

```bash
# Remove the workers CLAUDE.md (gitignored, won't be tracked)
rm -f crates/workers/CLAUDE.md
# git will auto-remove the empty directory, but verify:
ls crates/workers/ 2>/dev/null && echo "STILL EXISTS" || echo "REMOVED"
```

**Step 3: Verify moves**

```bash
ls -d crates/tasker-rb crates/tasker-py crates/tasker-ts crates/tasker-example-rs
# Expected: all four listed
ls crates/workers/ 2>/dev/null
# Expected: error (directory gone)
```

---

## Task 3: Update Root Cargo.toml Workspace Members

**File:** `Cargo.toml` (root)

**Step 1: Update workspace members**

Change:
```toml
"crates/workers/python",          # TAS-72: PyO3 Python worker
"crates/workers/ruby/ext/tasker_core",
"crates/workers/rust",
"crates/workers/typescript",      # TAS-100: TypeScript FFI worker (napi-rs)
```

To:
```toml
"crates/tasker-py",               # TAS-72: PyO3 Python worker
"crates/tasker-rb/ext/tasker_core",
"crates/tasker-example-rs",
"crates/tasker-ts",               # TAS-100: TypeScript FFI worker (napi-rs)
```

---

## Task 4: Update Worker Cargo.toml Path Dependencies

### 4a: Python (`crates/tasker-py/Cargo.toml`)

Change `../../tasker-shared` → `../tasker-shared` and `../../tasker-worker` → `../tasker-worker`:

```toml
tasker-shared = { path = "../tasker-shared" }
tasker-worker = { path = "../tasker-worker" }
```

### 4b: TypeScript (`crates/tasker-ts/Cargo.toml`)

Same changes as Python:

```toml
tasker-shared = { path = "../tasker-shared" }
tasker-worker = { path = "../tasker-worker" }
```

### 4c: Rust Example (`crates/tasker-example-rs/Cargo.toml`)

**Package name change** plus path updates:

```toml
[package]
name = "tasker-example-rs"
description = "Example Rust worker with sample step handlers for tasker-worker"

[lib]
name = "tasker_example_rs"

[dependencies]
tasker-core = { package = "tasker-core", path = "../../" }
tasker-shared = { path = "../tasker-shared" }
tasker-worker = { path = "../tasker-worker" }

[dev-dependencies]
tasker-orchestration = { path = "../tasker-orchestration" }
```

### 4d: Ruby Extension (`crates/tasker-rb/ext/tasker_core/Cargo.toml`)

Path depth decreases by one level (was 4 up, now 3 up for siblings; was 5 up, now 4 up for root):

```toml
[dependencies]
tasker-shared = { path = "../../../tasker-shared", version = "=0.1.6" }
tasker-worker = { path = "../../../tasker-worker", version = "=0.1.6" }

[dev-dependencies]
tasker-core = { package = "tasker-core", path = "../../../../", version = "=0.1.6" }
```

---

## Task 5: Update Worker Makefile.toml Files

### 5a: Rust Example (`crates/tasker-example-rs/Makefile.toml`)

Update `extend` path and `CRATE_NAME`:

```toml
extend = "../../tools/cargo-make/base-tasks.toml"

[env]
CRATE_NAME = "tasker-example-rs"
```

Update coverage task: replace all `tasker-worker-rust` with `tasker-example-rs` in the coverage script (lines 87-98).

### 5b: Ruby (`crates/tasker-rb/Makefile.toml`)

Replace all `../../cargo-make/scripts/` with `../../tools/cargo-make/scripts/` (lines 208, 209, 328, 356, 362).

The `../../target`, `../../coverage-reports/`, `../../.logs/` paths are now correct at depth-2 (no change needed).

### 5c: Python (`crates/tasker-py/Makefile.toml`)

Replace all `../../cargo-make/scripts/` with `../../tools/cargo-make/scripts/` (lines 161, 162, 317, 344, 351).

### 5d: TypeScript (`crates/tasker-ts/Makefile.toml`)

Replace all `../../cargo-make/scripts/` with `../../tools/cargo-make/scripts/` (lines 167, 168, 318, 343, 350).

---

## Task 6: Smoke Test — Cargo Build

**Step 1: Verify workspace resolves**

Run: `cargo check --all-features`
Expected: Compiles successfully

**Step 2: Commit**

```bash
git add -A
git commit -m "refactor(TAS-362): rename FFI worker crates to match published package names

Moves:
- crates/workers/ruby → crates/tasker-rb
- crates/workers/python → crates/tasker-py
- crates/workers/typescript → crates/tasker-ts
- crates/workers/rust → crates/tasker-example-rs

Renames tasker-worker-rust package to tasker-example-rs.
Eliminates crates/workers/ directory."
```

---

## Task 7: Update Root Makefile.toml

**File:** `Makefile.toml`

**Step 1: Update all `cwd` directives for worker tasks**

Find and replace these patterns throughout the file:

| Old | New |
|-----|-----|
| `cwd = "crates/workers/python"` | `cwd = "crates/tasker-py"` |
| `cwd = "crates/workers/ruby"` | `cwd = "crates/tasker-rb"` |
| `cwd = "crates/workers/typescript"` | `cwd = "crates/tasker-ts"` |
| `cwd = "crates/workers/rust"` | `cwd = "crates/tasker-example-rs"` |

**Step 2: Update all `tasker-worker-rust` package references**

Replace `tasker-worker-rust` with `tasker-example-rs` in:
- `cargo build --package` commands
- `cargo run -p` commands
- Any `--features tokio-console` build lines

Approximate lines: 1129, 1237, 1268-1279, 1559, 1676, 1696.

---

## Task 8: Update tools/cargo-make/workspace-config.toml

**File:** `tools/cargo-make/workspace-config.toml`

**Step 1: Update WORKER_PATHS**

Change:
```toml
WORKER_PATHS = "crates/workers/python,crates/workers/ruby,crates/workers/typescript,crates/workers/rust"
```

To:
```toml
WORKER_PATHS = "crates/tasker-py,crates/tasker-rb,crates/tasker-ts,crates/tasker-example-rs"
```

---

## Task 9: Update Shell Scripts in tools/

All scripts use `crates/workers/{lang}` paths that need updating. Apply these replacements:

| Old path | New path |
|----------|----------|
| `crates/workers/ruby` | `crates/tasker-rb` |
| `crates/workers/python` | `crates/tasker-py` |
| `crates/workers/typescript` | `crates/tasker-ts` |
| `crates/workers/rust` | `crates/tasker-example-rs` |

**Files to update (all under `tools/`):**

1. `tools/cargo-make/scripts/setup-env.sh` — .env path mappings (lines 84, 87, 90, 93)
2. `tools/cargo-make/scripts/setup-workers.sh` — worker setup dirs (lines 10, 13, 16)
3. `tools/cargo-make/scripts/sqlx-prepare.sh` — SQLx preparation paths (lines 16-19)
4. `tools/cargo-make/scripts/services-health-check.sh` — health check .env paths (lines 36-39)
5. `tools/cargo-make/scripts/multi-deploy/start-cluster.sh` — worker dirs (lines 68, 74, 80)
6. `tools/cargo-make/scripts/clean-workers.sh` — cleanup dirs (lines 11, 18, 26)
7. `tools/cargo-make/scripts/check-serde-contract.sh` — framework file paths (lines 36, 38-40)
8. `tools/cargo-make/scripts/ci-restore-ruby-extension.sh` — extension dir (line 23)
9. `tools/cargo-make/scripts/ci-restore-typescript-artifacts.sh` — artifact paths (lines 26-43, 52-64)
10. `tools/cargo-make/scripts/coverage-e2e.sh` — .env sourcing (lines 237-239)
11. `tools/scripts/code_check.sh` — worker project paths (lines 40-42, 52-53, 368, 495, 604)
12. `tools/scripts/ffi-build/build-ruby.sh` — ruby dir (lines 37, 59)
13. `tools/scripts/ffi-build/build-python.sh` — python dir (line 31)
14. `tools/scripts/ffi-build/build-typescript.sh` — `-p tasker-ts` (already correct package name)
15. `tools/scripts/release/read-versions.sh` — version file paths (lines 35, 47, 59)
16. `tools/scripts/release/update-versions.sh` — FFI crate paths (line 67)
17. `tools/scripts/release/release-prepare.sh` — lock file paths (lines 197-212)
18. `tools/scripts/release/detect-changes.sh` — change detection patterns (lines 94, 99, 104)
19. `tools/scripts/release/lib/common.sh` — version update paths (lines 176, 193, 216, 242)
20. `tools/scripts/release/build-ruby-gems.sh` — gem build dir (lines 12, 34)
21. `tools/scripts/release/publish-ruby.sh` — worker dir (lines 69, 71)
22. `tools/scripts/release/publish-python.sh` — worker dir (lines 54, 68)
23. `tools/scripts/release/publish-typescript.sh` — worker dir (line 57)
24. `tools/scripts/clean_project.sh` — cleanup (line 241)

Also update `tasker-worker-rust` → `tasker-example-rs` in any scripts that reference the package name.

---

## Task 10: Smoke Test — Build System

Run: `cargo make check-rust`
Expected: PASS

Run: `cargo make build`
Expected: Builds all workspace members

**Commit:**

```bash
git add -A
git commit -m "refactor(TAS-362): update build system for renamed worker crates"
```

---

## Task 11: Update CI Workflows

**Files under `.github/workflows/`:**

### 11a: build-workers.yml
- Update `working-directory` values from `crates/workers/` to `crates/tasker-`
- Update `cargo build --package tasker-worker-rust` → `--package tasker-example-rs`
- Update artifact paths

### 11b: test-ruby-framework.yml
- `crates/workers/ruby` → `crates/tasker-rb` (lines 46, 60)

### 11c: test-typescript-framework.yml
- `crates/workers/typescript` → `crates/tasker-ts` (lines 47, 114)

### 11d: test-python-framework.yml
- `crates/workers/python` → `crates/tasker-py` (lines 46, 136)

### 11e: test-integration.yml
- Worker setup/compilation references (lines 247, 321, 337, 358)
- `cargo build --package tasker-worker-rust` → `--package tasker-example-rs`

### 11f: build-ffi-libraries.yml
- Ruby worker working directory (line 166)

### 11g: release.yml
- TypeScript artifact paths (lines 548-556): `crates/workers/typescript/` → `crates/tasker-ts/`

### 11h: .github/workflows/README.md
- Update all path references (12+ occurrences)

### 11i: .github/scripts/release/create-github-release.sh
- References to `tasker-rb`, `tasker-py` (already correct names, but verify path references)

---

## Task 12: Update Docker Files

**Files under `docker/`:**

Apply path replacements in all `COPY` directives and volume mounts:

| Old | New |
|-----|-----|
| `crates/workers/ruby` | `crates/tasker-rb` |
| `crates/workers/python` | `crates/tasker-py` |
| `crates/workers/typescript` | `crates/tasker-ts` |
| `crates/workers/rust` | `crates/tasker-example-rs` |

**Dockerfiles to update:**
1. `docker/ruby-worker.prod.Dockerfile` (lines 55-60)
2. `docker/ruby-worker.test.Dockerfile` (lines 63-68)
3. `docker/rust-worker.prod.Dockerfile` (lines 44, 53, 82, 92)
4. `docker/rust-worker.test.Dockerfile` (lines 67, 76, 112, 122)
5. `docker/python-worker.prod.Dockerfile` (lines 63-68)
6. `docker/python-worker.test.Dockerfile` (lines 74-79)
7. `docker/typescript-worker.prod.Dockerfile` (lines 55-60)
8. `docker/typescript-worker.test.Dockerfile` (lines 64-69)
9. `docker/orchestration.prod.Dockerfile` (lines 53-56, 91-94)
10. `docker/orchestration.test.Dockerfile` (lines 85-88, 128-131)
11. `docker/ffi-builder.Dockerfile` (lines 118-123)

**Docker Compose files:**
12. `docker/docker-compose.test-full.yml` (lines 298, 365, 433)
13. `docker/docker-compose.ci.yml` (line 185)
14. `docker/docker-compose.prod.yml` (line 122)

Also update any `tasker-worker-rust` → `tasker-example-rs` in Dockerfiles (binary build targets).

**Commit:**

```bash
git add -A
git commit -m "refactor(TAS-362): update CI workflows and Docker for renamed worker crates"
```

---

## Task 13: Update Documentation

### 13a: CLAUDE.md (root project)

**File:** `CLAUDE.md`

Update workspace structure table (lines ~99-101):
```
- crates/tasker-py             # Python FFI bindings (maturin/pyo3)
- crates/tasker-rb             # Ruby FFI bindings (magnus)
- crates/tasker-example-rs     # Example Rust worker implementation
- crates/tasker-ts             # TypeScript FFI bindings (Bun/Node/Deno)
```

Update Ruby integration section (line ~205):
```bash
cd crates/tasker-rb
```

Update troubleshooting (line ~424):
```bash
cd crates/tasker-rb && rake clean && rake compile
```

### 13b: Parent CLAUDE.md (`tasker-systems/CLAUDE.md`)

If it references worker paths, update accordingly.

### 13c: Claude Skills

Update these skill files under `.claude/skills/`:

1. `architecture-fundamentals.md` — crate structure (lines 24-27)
2. `cross-language-development.md` — worker paths table (lines 13-16, 62, 78, 87)
3. `ruby-development.md` — ruby path (lines 5, 19, 203)
4. `python-development.md` — python path (lines 5, 20)
5. `typescript-development.md` — typescript path (lines 5, 20)
6. `versioning-and-releases.md` — version file paths (lines 39-45, 90, 117)

### 13d: Documentation files in docs/

Update worker path references in:
1. `docs/workers/ruby.md`
2. `docs/workers/python.md`
3. `docs/workers/typescript.md`
4. `docs/workers/rust.md`
5. `docs/reference/ffi-boundary-types.md`
6. `docs/reference/ffi-telemetry-pattern.md`
7. `docs/development/best-practices-python.md`
8. `docs/development/best-practices-ruby.md`
9. `docs/development/best-practices-typescript.md`
10. `docs/testing/decision-point-e2e-tests.md`

### 13e: README.md

Update worker package references (lines 110-113, 127).

### 13f: Memory file

**File:** `~/.claude/projects/-Users-petetaylor-projects-tasker-systems-tasker-core/memory/MEMORY.md`

Update the "Workspace Structure" section:
- Remove `crates/workers/{ruby,python,rust,typescript}` references
- Add `crates/tasker-{rb,py,ts,example-rs}` as direct children of `crates/`
- Update TAS-362 status
- Note: `tasker-rs` name reserved for future action-grammar worker

**Commit:**

```bash
git add -A
git commit -m "docs(TAS-362): update documentation for renamed worker crates"
```

---

## Task 14: Final Verification

**Step 1: Cargo workspace check**

Run: `cargo check --all-features`
Expected: PASS

**Step 2: Clippy**

Run: `cargo clippy --all-targets --all-features --workspace`
Expected: Zero warnings

**Step 3: No-infra tests**

Run: `cargo make test-no-infra`
Expected: PASS

**Step 4: Verify `crates/workers/` is gone**

```bash
test -d crates/workers && echo "FAIL: workers/ still exists" || echo "PASS: workers/ eliminated"
```

**Step 5: Verify all crates are direct children**

```bash
ls -d crates/tasker-*/
# Expected: tasker-client, tasker-ctl, tasker-example-rs, tasker-grammar,
#           tasker-mcp, tasker-orchestration, tasker-pgmq, tasker-py,
#           tasker-rb, tasker-sdk, tasker-shared, tasker-ts, tasker-worker
```

**Step 6: Verify no stale references**

```bash
grep -r "crates/workers/" --include="*.toml" --include="*.yml" --include="*.sh" --include="*.md" --include="*.rs" . | grep -v ".git/" | grep -v "target/"
# Expected: zero results (or only in this plan document)
```

**Step 7: Cargo publish dry-run (if feasible)**

```bash
cd crates/tasker-py && cargo publish --dry-run 2>&1 | tail -5
cd crates/tasker-rb/ext/tasker_core && cargo publish --dry-run 2>&1 | tail -5
```

---

## Execution Notes

- **No public-facing breaking changes**: PyPI (`tasker-py`), RubyGems (`tasker-rb`), npm (`@tasker-systems/tasker`) names are unchanged.
- **Only one crate name changes**: `tasker-worker-rust` → `tasker-example-rs`. All others already match published names.
- **`tasker-rs` name reserved**: Left unoccupied for the future action-grammar virtual handler worker.
- **Path depth fix bonus**: Moving from depth-3 to depth-2 fixes several `../../` paths that were broken by TAS-361 (workers moved deeper without updating all relative paths).
- **Worker Makefile.toml scripts**: The `../../cargo-make/scripts/` paths need `tools/` prefix added since TAS-361 moved cargo-make to `tools/cargo-make/`.
