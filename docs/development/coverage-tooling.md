# Code Coverage Tooling

Unified coverage tooling for the tasker-core workspace. Generates per-crate coverage reports across Rust, Python, Ruby, and TypeScript with normalized JSON output, cross-language aggregation, and threshold enforcement.

---

## Quick Start

```bash
# Ensure coverage tools are installed (rustfilt, uv project)
cargo make coverage-tools-setup

# Single crate
CRATE_NAME=tasker-shared cargo make coverage-crate

# All languages
cargo make coverage-all

# Aggregate report + threshold check
cargo make coverage-report
cargo make coverage-check
```

---

## Command Reference

### Rust Coverage

| Task | Alias | Description |
|------|-------|-------------|
| `coverage` | `cov` | Workspace-wide Rust coverage (JSON + HTML) |
| `coverage-crate` | - | Single crate (`CRATE_NAME` env var) |
| `coverage-crate-integrated` | `ccint` | Single crate + root integration tests (multi-run profraw) |
| `coverage-e2e` | `cove` | E2E tests with instrumented service binaries (dual-backend) |
| `coverage-e2e-pgmq` | - | E2E coverage with PGMQ backend only |
| `coverage-e2e-rabbitmq` | - | E2E coverage with RabbitMQ backend only |
| `coverage-foundational` | - | `tasker-shared` + `tasker-pgmq` |
| `coverage-core` | - | `tasker-orchestration` + `tasker-worker` + `tasker-client` + `tasker-cli` |
| `coverage-rust-worker` | `covr` | Rust worker crate |

### Language Worker Coverage

| Task | Alias | Description |
|------|-------|-------------|
| `coverage-python` | `covp` | Python worker (`pytest-cov`) |
| `coverage-ruby` | `covrb` | Ruby worker (`SimpleCov`) |
| `coverage-typescript` | `covts` | TypeScript worker (`bun --coverage`) |

### Aggregate and Reporting

| Task | Alias | Description |
|------|-------|-------------|
| `coverage-all` | `cova` | Run all languages |
| `coverage-report` | - | Generate aggregate JSON report |
| `coverage-check` | `covc` | Check thresholds (exit code 1 on failure) |
| `coverage-clean` | - | Remove all coverage artifacts |
| `coverage-tools-setup` | - | Install `rustfilt` and sync Python project |

---

## Architecture

```
                     cargo make coverage-*
                            |
                +-----------+-----------+
                |           |           |
         Rust Coverage  Python/Ruby/TS  Aggregate
         (llvm-cov +    (language-      (cross-crate
          nextest)       native tools)   reporting)
                |           |           |
                v           v           v
         *-raw.json    raw output   per-crate JSON
                |           |           |
                +-----+-----+          |
                      |                 |
               normalize-*.py           |
               (uv run)                 |
                      |                 |
                      v                 v
               *-coverage.json    aggregate-coverage.json
               (per-crate)        (all crates)
                      |                 |
                      +--------+--------+
                               |
                        check-thresholds.py
```

### Data Flow

1. **Coverage collection** - Language-specific tools produce raw output
2. **Normalization** - Python scripts convert to a standardized JSON schema
3. **Aggregation** - Cross-crate report combining all languages (merges unit + E2E per crate)
4. **Thresholds** - Pass/fail enforcement from `coverage-thresholds.json`
5. **Markdown report** - Human-readable `COVERAGE.md` at repo root

### Coverage Tools Per Language

| Language | Coverage Tool | Normalizer |
|----------|--------------|------------|
| Rust | `cargo-llvm-cov` + `nextest` | `normalize-rust.py` |
| Python | `pytest-cov` | `normalize-python.py` |
| Ruby | `SimpleCov` + `simplecov-json` | `normalize-ruby.py` |
| TypeScript | `bun --coverage` (LCOV) | `normalize-typescript.py` |

---

## File Layout

```
tasker-core/
+-- COVERAGE.md                            # Auto-generated markdown report
+-- Makefile.toml                          # Root coverage tasks
+-- coverage-thresholds.json               # Per-crate threshold config
+-- cargo-make/
|   +-- scripts/
|       +-- coverage-crate-integrated.sh   # Crate + integration test coverage
|       +-- coverage-e2e.sh               # E2E coverage with instrumented binaries
|       +-- coverage/
|           +-- pyproject.toml             # uv Python project definition
|           +-- normalize-rust.py          # llvm-cov JSON -> standard schema
|           +-- normalize-python.py        # pytest-cov JSON -> standard schema
|           +-- normalize-ruby.py          # SimpleCov JSON -> standard schema
|           +-- normalize-typescript.py    # LCOV -> standard schema
|           +-- aggregate.py              # Combine all per-crate reports
|           +-- check-thresholds.py       # Enforce thresholds, exit code
+-- workers/
|   +-- rust/Makefile.toml                # Rust worker coverage task
|   +-- ruby/Makefile.toml                # Ruby worker coverage task
|   +-- ruby/.simplecov                   # SimpleCov configuration
|   +-- python/Makefile.toml              # Python worker coverage task
|   +-- typescript/Makefile.toml          # TypeScript worker coverage task
+-- coverage-reports/                      # Generated (gitignored)
    +-- rust/
    |   +-- <crate>-raw.json              # Raw llvm-cov output
    |   +-- <crate>-coverage.json         # Normalized report
    |   +-- html/                         # HTML browsable report
    +-- python/
    +-- ruby/
    +-- typescript/
    +-- aggregate-coverage.json           # JSON aggregate (source for COVERAGE.md)
```

---

## Reading Reports

### Per-Crate Reports

After running a crate's coverage, find its normalized JSON at:

```
coverage-reports/{language}/<crate>-coverage.json
```

The report is structured for actionability:

- **`summary`** - Total lines/functions covered and percentages
- **`files[]`** - Per-file breakdown, sorted worst-first
- **`uncovered_functions[]`** - Demangled Rust function names with no test coverage

**Finding your biggest gaps:**

```bash
# See the 10 worst-covered files
jq '.files[:10][] | "\(.line_coverage_percent)% \(.path)"' \
  coverage-reports/rust/tasker-shared-coverage.json

# See uncovered files (0% coverage)
jq '.files[] | select(.line_coverage_percent == 0) | "\(.lines_total) lines  \(.path)"' \
  coverage-reports/rust/tasker-shared-coverage.json

# See uncovered functions in a specific file
jq '.uncovered_functions[] | select(.file | contains("config/web.rs")) | .name' \
  coverage-reports/rust/tasker-shared-coverage.json
```

### Aggregate Report

After `cargo make coverage-report`:

```
coverage-reports/aggregate-coverage.json
```

Contains cross-crate summaries with threshold pass/fail status, plus:

- **`lowest_coverage_files[]`** - Worst 30 files across all crates
- **`uncovered_files[]`** - All files at 0%, sorted by size (biggest gaps first)

### HTML Reports

For visual browsing of line-by-line Rust coverage:

```bash
cargo make coverage    # Generates HTML at coverage-reports/rust/html/
open coverage-reports/rust/html/index.html
```

---

## Normalized JSON Schema

### Per-Crate Report

Every normalizer produces this schema. Rust and TypeScript reports include
function-level detail; Python and Ruby include line-level detail where available.

```json
{
  "meta": {
    "timestamp": "2026-01-29T18:36:55.034134+00:00",
    "crate": "tasker-shared",
    "language": "rust",
    "tool": "cargo-llvm-cov",
    "git_commit": "bab744a8d3f1",
    "git_branch": "main"
  },
  "summary": {
    "lines_covered": 17527,
    "lines_total": 31333,
    "line_coverage_percent": 55.94,
    "functions_covered": 2196,
    "functions_total": 4404,
    "function_coverage_percent": 49.86
  },
  "files_tested": 113,
  "files_total": 143,
  "files": [
    {
      "path": "tasker-shared/src/config/web.rs",
      "lines_covered": 0,
      "lines_total": 44,
      "line_coverage_percent": 0.0,
      "functions_covered": 0,
      "functions_total": 8,
      "function_coverage_percent": 0.0
    }
  ],
  "uncovered_functions": [
    {
      "name": "<tasker_shared::cache::provider::CacheBackend>::delete",
      "file": "tasker-shared/src/cache/provider.rs"
    }
  ]
}
```

**Field notes:**

| Field | Description |
|-------|-------------|
| `files[]` | Sorted by `line_coverage_percent` ascending (worst first). Filtered to the target crate's `src/` directory only. |
| `uncovered_functions[]` | Functions with zero execution count. Demangled via `rustfilt`, deduplicated across generic monomorphizations. Scoped to the target crate. |
| `summary.lines_*` | Recalculated from filtered crate files (accurate even with multi-run profraw accumulation). |
| `files_tested` / `files_total` | Counts of crate-scoped files with/without coverage. |

**Language-specific fields:**

| Language | Extra `files[]` fields | `uncovered_functions[]` |
|----------|----------------------|------------------------|
| Rust | `functions_covered`, `functions_total`, `function_coverage_percent` | Yes (demangled) |
| Python | `missing_lines`, `excluded_lines` | No |
| Ruby | (none) | No |
| TypeScript | `functions_covered`, `functions_total`, `function_coverage_percent` | Yes (summary counts from FNF/FNH) |

### Aggregate Report

```json
{
  "meta": {
    "timestamp": "...",
    "git_commit": "...",
    "git_branch": "...",
    "report_count": 10
  },
  "summary": {
    "total_lines_covered": 48797,
    "total_lines": 184353,
    "overall_line_coverage_percent": 26.47,
    "crates_passing": 1,
    "crates_failing": 8
  },
  "crates": {
    "tasker-shared": {
      "language": "rust",
      "lines_covered": 17527,
      "lines_total": 31333,
      "line_coverage_percent": 55.94,
      "threshold": 70,
      "passes_threshold": false,
      "source_file": "coverage-reports/rust/tasker-shared-coverage.json"
    }
  },
  "lowest_coverage_files": [],
  "uncovered_files": []
}
```

---

## Thresholds

Thresholds are defined in `coverage-thresholds.json` at the project root:

```json
{
  "rust": {
    "tasker-shared": 70,
    "tasker-pgmq": 65,
    "tasker-orchestration": 55,
    "tasker-worker": 55,
    "tasker-client": 60,
    "tasker-cli": 30
  },
  "python": { "tasker-core-py": 80 },
  "ruby": { "tasker-worker-rb": 70 },
  "typescript": { "tasker-worker-ts": 60 }
}
```

These represent target line coverage percentages. `cargo make coverage-check` exits
with code 1 if any crate is below its threshold.

---

## Normalizer Scripts

The normalizer and aggregation scripts are managed as a uv-backed Python project
at `cargo-make/scripts/coverage/`. All cargo-make tasks invoke them via
`uv run --project cargo-make/scripts/coverage` to ensure a consistent environment.

### `normalize-rust.py`

Converts `cargo-llvm-cov` JSON export to the normalized schema.

```
Usage: normalize-rust.py <input_json> <output_json> --crate <crate_name>
```

Key behaviors:
- Filters out external dependency files (`index.crates.io-*`, `.cargo/registry/`, `target/`)
- Scopes files and functions to the target crate's `src/` directory
- Batch-demangles all Rust symbols through `rustfilt` in a single subprocess
- Deduplicates generic monomorphizations (many mangled names -> one demangled name)
- Special crate mapping: `tasker-worker-rust` -> `workers/rust/src/`
- Pass `--crate workspace` for unfiltered workspace-wide output

### `normalize-python.py`

Converts `pytest-cov` JSON output to the normalized schema.

```
Usage: normalize-python.py <input_json> <output_json>
```

Per-file detail includes `missing_lines` and `excluded_lines` counts.
Function-level tracking is not available from pytest-cov.

### `normalize-ruby.py`

Converts SimpleCov `.resultset.json` to the normalized schema.

```
Usage: normalize-ruby.py <input_json> <output_json>
```

Handles both old-format (array) and new-format (dict with `lines` key)
SimpleCov output. Strips workspace path prefixes from file paths.

### `normalize-typescript.py`

Parses LCOV format coverage data from Bun test runner.

```
Usage: normalize-typescript.py <coverage_dir> <output_json>
```

Extracts per-file data from `DA:` (line) records and function summary counts
from `FNF:`/`FNH:` records. Bun does not emit individual `FN:`/`FNDA:` records,
so per-function uncovered names are not available.

### `aggregate.py`

Combines all per-crate normalized reports into a single aggregate.

```
Usage: aggregate.py --output <path> [--reports-dir <dir>]
```

Discovers reports in `coverage-reports/{rust,python,ruby,typescript}/`.
Applies thresholds from `coverage-thresholds.json`. Surfaces the worst-covered
files and all uncovered files in the aggregate output.

### `check-thresholds.py`

Enforces coverage thresholds. Exits 0 if all pass, 1 if any fail.

```
Usage: check-thresholds.py [--aggregate <path>] [--reports-dir <dir>]
```

Prefers the aggregate report if available; falls back to individual reports.

---

## External Dependencies

| Tool | Install | Purpose |
|------|---------|---------|
| `cargo-llvm-cov` | `cargo install cargo-llvm-cov` | Rust code coverage instrumentation |
| `cargo-nextest` | `cargo install cargo-nextest` | Test runner with per-test process isolation |
| `rustfilt` | `cargo install rustfilt` | Rust symbol demangling |
| `uv` | `brew install uv` (in Brewfile) | Python project management |
| `pytest-cov` | Via `uv` in workers/python | Python coverage |
| `simplecov` / `simplecov-json` | Via Bundler in workers/ruby | Ruby coverage |
| `bun` | `brew install bun` (in Brewfile) | TypeScript test runner with LCOV coverage |

The `coverage-tools-setup` task auto-installs `rustfilt` and syncs the uv project.

---

## Adding Python Dependencies

If a normalizer script needs a new Python package:

1. Add it to `cargo-make/scripts/coverage/pyproject.toml` under `dependencies`
2. Run `uv sync --project cargo-make/scripts/coverage`
3. Commit `pyproject.toml` and `uv.lock`

---

## Integrated and E2E Coverage

Standard `coverage-crate` only runs tests defined in the target crate's package.
Root-level tests in `tests/integration/` and `tests/e2e/` exercise significant
orchestration code paths (actors, state machines, listeners, batch processing)
but aren't captured by per-crate coverage.

### Integrated Coverage (`coverage-crate-integrated`)

Captures coverage from both crate-internal tests and root-level integration tests
using multi-run profraw accumulation:

```bash
CRATE_NAME=tasker-orchestration cargo make coverage-crate-integrated
# or: CRATE_NAME=tasker-orchestration cargo make ccint
```

**How it works:**

1. Run crate's own tests (`--no-report` -- collect profraw, don't generate report yet)
2. Run root integration tests (`--no-clean --no-report` -- accumulate more profraw data)
3. Generate combined report from all accumulated profraw files
4. Normalize with `normalize-rust.py` (summary recalculated from filtered crate files)

Integration tests use `LifecycleTestManager` and `#[sqlx::test]` to execute
orchestration code **in-process** via direct function calls. This is why multi-run
profraw accumulation works -- the test binary calls into the same instrumented
crate code.

**Prerequisites:** PostgreSQL running (integration tests need a database).

**Output:** Same filenames as `coverage-crate` (`<crate>-raw.json`, `<crate>-coverage.json`),
so threshold enforcement works unchanged.

### E2E Coverage (`coverage-e2e`)

Captures coverage from **Rust E2E tests** (`tests/e2e/rust/`) that communicate
with services over HTTP/gRPC. Requires instrumenting the **service binaries**
themselves since the test binary only exercises the API client.

By default, runs E2E tests against **both messaging backends** (PGMQ then
RabbitMQ) in sequence, accumulating LLVM profraw coverage data from both passes.
This exercises both the PGMQ code paths (LISTEN/NOTIFY, queue operations) and
the RabbitMQ AMQP paths, plus the shared orchestration/worker code under both
configurations.

Only starts orchestration + rust worker. FFI workers (Python, Ruby, TypeScript)
load the `tasker-worker` dylib into their own runtime's process space, so LLVM
coverage instrumentation is not feasible for them.

```bash
cargo make coverage-e2e              # Both backends (default)
cargo make coverage-e2e-pgmq        # PGMQ backend only
cargo make coverage-e2e-rabbitmq    # RabbitMQ backend only
# or: cargo make cove
```

No `CRATE_NAME` needed -- the script always instruments both `tasker-server` and
`rust-worker`, runs the same Rust E2E tests per backend, and generates per-crate
reports for 6 crates: `tasker-orchestration`, `tasker-worker-rust`,
`tasker-worker`, `tasker-shared`, `tasker-pgmq`, and `tasker-client`. Both
binaries link these crates as library dependencies, so their execution covers
code paths only reachable through the full service stack.

**How it works:**

1. Run `setup-env` for root, orchestration, and rust-worker to ensure consistent `.env` files
2. `cargo llvm-cov show-env` provides instrumentation environment variables (appends `--cfg tokio_unstable` for `tokio-console` feature)
3. Build instrumented `tasker-server` and `rust-worker` binaries **once**
4. For each backend (PGMQ, RabbitMQ):
   a. Probe infrastructure availability (skip with warning if unavailable)
   b. Export `TASKER_MESSAGING_BACKEND=$backend`
   c. Start services, wait for `/health` endpoints
   d. Run Rust E2E tests (`-E 'test(~e2e::rust::)'`), uninstrumented
   e. SIGTERM services (triggers profraw flush)
5. Generate one combined raw report from all accumulated profraw files, normalize per-crate

**Profraw accumulation:** LLVM profraw files use `%p-%m` patterns (PID + module hash).
Each service process gets a unique PID, so running services twice with different
backends produces distinct profraw files. `cargo llvm-cov report` merges all of
them automatically. One build, two test passes, one combined report.

**Backend selection:** The `COV_E2E_BACKEND` environment variable or `--backend=`
flag restricts to a single backend. The convenience tasks `coverage-e2e-pgmq` and
`coverage-e2e-rabbitmq` set this automatically. When neither is set, both backends
are tested. If a backend's infrastructure isn't available, it is skipped with a
warning. At least one backend must succeed.

Service startup mirrors `services-start-all.sh` and `service-start.sh` patterns:
PID files in `.pids/`, logs in `.logs/`, duplicate-instance prevention, and
graceful SIGTERM shutdown with SIGKILL fallback. Uses `cov-` prefix for PID/log
files to avoid colliding with normal service instances.

**Prerequisites:**
- At least one messaging backend available (PostgreSQL for PGMQ, or RabbitMQ)
- `cargo-llvm-cov` installed
- Ports 8080/8081 available (stop existing services first)

**Output:**
- `coverage-reports/rust/e2e-raw.json` -- single raw report from all backend passes
- `coverage-reports/rust/tasker-orchestration-e2e-coverage.json` -- orchestration coverage
- `coverage-reports/rust/tasker-worker-rust-e2e-coverage.json` -- rust worker coverage
- `coverage-reports/rust/tasker-worker-e2e-coverage.json` -- tasker-worker library coverage
- `coverage-reports/rust/tasker-shared-e2e-coverage.json` -- shared library coverage
- `coverage-reports/rust/tasker-pgmq-e2e-coverage.json` -- PGMQ wrapper coverage
- `coverage-reports/rust/tasker-client-e2e-coverage.json` -- client library coverage

### Aggregate Merging

`coverage-e2e` is included in `coverage-all`. The aggregate report (`coverage-report`)
automatically merges multiple reports for the same crate. When both
`tasker-orchestration-coverage.json` (unit tests) and
`tasker-orchestration-e2e-coverage.json` (E2E tests) exist, the aggregate takes
the highest coverage per file path and recalculates the summary. This gives a
conservative lower bound on the true combined coverage.

**Note on line count differences:** E2E and unit test reports may show different
`lines_total` for the same file because they compile in different contexts. Unit
tests compile the crate directly (including `#[cfg(test)]` blocks), while E2E
tests instrument the production binary (which excludes test-only code). The
aggregate takes the entry with the highest `lines_covered`, which may use either
line total. For `tasker-worker` in particular, this captures critical paths
through `worker/core.rs`, `bootstrap.rs`, and the web/gRPC handler stack that
are only exercised through the full service lifecycle.

### Which Coverage Mode to Use

| Scenario | Task | Notes |
|----------|------|-------|
| Quick per-crate check | `coverage-crate` | Fastest, crate tests only |
| Include integration test paths | `coverage-crate-integrated` | Adds ~30s for integration tests |
| Rust service stack coverage | `coverage-e2e` | Dual-backend, captures bootstrap + handlers for 6 crates |
| Single backend only | `coverage-e2e-pgmq` / `coverage-e2e-rabbitmq` | Faster, one backend pass |
| Full picture | `coverage-all` + `coverage-report` | Runs everything, merges per-crate results |

---

## Design Decisions

### Why nextest for coverage?

`cargo test` runs all tests in a single process. Auth integration tests use
`std::env::set_var("TASKER_CONFIG_PATH", ...)` to point at auth-enabled config,
which pollutes subsequent non-auth tests in the same process. `cargo nextest`
gives each test its own process, preventing env var leakage.

### Why normalize to JSON instead of using HTML directly?

HTML reports are great for humans but can't be diffed, aggregated, or enforced
in CI. The normalized JSON enables:
- Cross-language aggregation (Rust + Python + Ruby + TypeScript in one report)
- Threshold enforcement with exit codes
- File-level gap analysis via `jq`
- Trend tracking over time (compare JSON snapshots)

### Why filter to crate source in per-crate reports?

`cargo-llvm-cov --package X` runs X's tests but instruments all compiled code,
including workspace dependencies. Without filtering, the `tasker-shared` report
would show `tasker-pgmq` files (a dependency) with 0% coverage, creating noise.
Filtering to `<crate>/src/` focuses each report on code the crate owns.

### Why batch-demangle with rustfilt?

The raw llvm-cov JSON contains mangled Rust symbols like
`_RNvMs0_NtNtCs7cxDW5Sx6BT_13tasker_shared5cache8providerNtB5_13CacheProvider3new`.
These are unreadable. `rustfilt` demangles them to
`<tasker_shared::cache::provider::CacheProvider>::new`. Batch-piping all names
through a single subprocess call is efficient regardless of count.

Generic functions produce multiple mangled symbols (one per monomorphization)
that all demangle to the same name. Deduplication collapses these, reducing
the `tasker-shared` uncovered functions list from ~170k raw symbols to ~3.2k
unique demangled entries.

### Why separate data collection from threshold enforcement?

Worker coverage tools (SimpleCov, pytest-cov) have their own built-in threshold
mechanisms. These are useful for local development but cause data collection to
fail in the coverage pipeline before normalization can run. The pipeline uses
environment flags (`COVERAGE_COLLECT_ONLY` for Ruby, `--cov-fail-under=0` for
Python) to always collect data successfully, then enforces thresholds uniformly
via `cargo make coverage-check` across all languages.

---

## Related Documentation

- [Build Tooling and Task Runner System](./tooling.md) - cargo-make task runner overview
- [TAS-63 Coverage Gap Analysis](../ticket-specs/TAS-63/ticket.md) - Prioritized coverage gaps and closure plan
