# Skill: Code Coverage Tooling

## When to Use

Use this skill when running coverage analysis, interpreting coverage reports, working with coverage thresholds, or understanding the cross-language coverage normalization pipeline.

## Quick Commands

```bash
# Single Rust crate
CRATE_NAME=tasker-shared cargo make coverage-crate

# All languages
cargo make coverage-all          # cova

# Aggregate report + threshold check
cargo make coverage-report
cargo make coverage-check        # covc - exits 1 on failure
```

## Coverage Tasks

### Rust Coverage

| Task | Alias | Description |
|------|-------|-------------|
| `coverage` | `cov` | Workspace-wide Rust coverage (JSON + HTML) |
| `coverage-crate` | - | Single crate (`CRATE_NAME` env var) |
| `coverage-crate-integrated` | `ccint` | Crate + root integration tests (multi-run profraw) |
| `coverage-e2e` | `cove` | E2E tests with instrumented service binaries (dual-backend) |
| `coverage-e2e-pgmq` | - | E2E coverage with PGMQ backend only |
| `coverage-e2e-rabbitmq` | - | E2E coverage with RabbitMQ backend only |
| `coverage-foundational` | - | `tasker-shared` + `tasker-pgmq` |
| `coverage-core` | - | `tasker-orchestration` + `tasker-worker` + `tasker-client` + `tasker-cli` |

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
| `coverage-check` | `covc` | Check thresholds (exit 1 on failure) |
| `coverage-clean` | - | Remove all coverage artifacts |

## Coverage Architecture

```
cargo make coverage-*
        |
   +-----------+-----------+
   |           |           |
Rust       Python/Ruby/TS  Aggregate
(llvm-cov + (language-     (cross-crate
 nextest)   native tools)   reporting)
   |           |           |
   v           v           v
*-raw.json  raw output   per-crate JSON
   |           |           |
normalize-*.py (uv run)    |
   |                       |
   v                       v
*-coverage.json    aggregate-coverage.json
(per-crate)        (all crates)
        |
 check-thresholds.py
```

### Coverage Tools Per Language

| Language | Coverage Tool | Normalizer Script |
|----------|--------------|------------------|
| Rust | `cargo-llvm-cov` + `nextest` | `normalize-rust.py` |
| Python | `pytest-cov` | `normalize-python.py` |
| Ruby | `SimpleCov` + `simplecov-json` | `normalize-ruby.py` |
| TypeScript | `bun --coverage` (LCOV) | `normalize-typescript.py` |

## Thresholds

Defined in `coverage-thresholds.json` at project root:

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

`cargo make coverage-check` exits with code 1 if any crate is below its threshold.

## Coverage Modes: When to Use Which

| Scenario | Task | Notes |
|----------|------|-------|
| Quick per-crate check | `coverage-crate` | Fastest, crate tests only |
| Include integration test paths | `coverage-crate-integrated` | Adds integration test coverage |
| Rust service stack coverage | `coverage-e2e` | Dual-backend, 6 crates |
| Full picture | `coverage-all` + `coverage-report` | Runs everything, merges results |

## Reading Reports

### Per-Crate Reports

Output at `coverage-reports/{language}/<crate>-coverage.json`:

```bash
# See 10 worst-covered files
jq '.files[:10][] | "\(.line_coverage_percent)% \(.path)"' \
  coverage-reports/rust/tasker-shared-coverage.json

# See uncovered files (0%)
jq '.files[] | select(.line_coverage_percent == 0) | "\(.lines_total) lines  \(.path)"' \
  coverage-reports/rust/tasker-shared-coverage.json

# HTML reports for visual browsing
open coverage-reports/rust/html/index.html
```

### Aggregate Report

After `cargo make coverage-report`, find `coverage-reports/aggregate-coverage.json` with cross-crate summaries, threshold pass/fail status, worst-covered files, and uncovered files.

## Normalizer Scripts

Managed as a uv-backed Python project at `cargo-make/scripts/coverage/`. All invoked via `uv run --project cargo-make/scripts/coverage`.

Key scripts:
- `normalize-rust.py` -- llvm-cov JSON to standard schema (with `rustfilt` demangling)
- `normalize-python.py` -- pytest-cov JSON to standard schema
- `normalize-ruby.py` -- SimpleCov JSON to standard schema
- `normalize-typescript.py` -- LCOV to standard schema
- `aggregate.py` -- Combine all per-crate reports
- `check-thresholds.py` -- Enforce thresholds with exit codes

## Design Decisions

- **nextest for coverage**: Prevents env var leakage between auth/non-auth tests (per-test process isolation)
- **Normalize to JSON**: Enables cross-language aggregation, threshold enforcement, `jq` gap analysis
- **Filter to crate source**: Prevents noise from workspace dependency files
- **Separate collection from enforcement**: Worker tools (SimpleCov, pytest-cov) collect data successfully; thresholds enforced uniformly via `cargo make coverage-check`

## External Dependencies

| Tool | Purpose |
|------|---------|
| `cargo-llvm-cov` | Rust coverage instrumentation |
| `cargo-nextest` | Per-test process isolation |
| `rustfilt` | Rust symbol demangling |
| `uv` | Python project management for normalizers |

Install via: `cargo make coverage-tools-setup`

## References

- Full documentation: `docs/development/coverage-tooling.md`
