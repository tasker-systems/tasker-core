# Coverage Analysis: tasker-cli

**Current Coverage**: 8.82% line (223/2,527), 8.28% function (12/145)
**Target**: 30%
**Gap**: 21.18 percentage points (535 additional lines needed)

---

## Summary

The `tasker-cli` crate is the command-line interface binary for Tasker. It was extracted from `tasker-client` as a separate crate, taking with it all CLI command handlers, the `main.rs` entry point, and documentation generation templates.

The crate has 9 source files. Only 1 file has coverage: `docs/templates.rs` at 88.49% (223/252 lines), which contains Askama template rendering logic with comprehensive unit tests. All 8 other files -- the `main.rs` entry point and 7 command handler modules -- have 0% coverage. This is inherent to binary crate architecture: the command handlers are compiled into the binary and cannot be invoked from `cargo test` library tests.

The existing tests (37 total) come from two sources:
- **`tests/config_commands_test.rs`** (26 tests): Integration tests for ConfigMerger, config validation, documentation loading, and environment variable substitution. These test utility logic defined in the test file itself, not the binary command handlers.
- **`src/docs/templates.rs`** (11 tests): Unit tests for Askama template rendering (annotated config, reference docs, parameter explanations, section details).

**Key constraint**: As a binary crate, the command handler modules (`commands/*.rs`) cannot be unit tested in-place. Testing requires either (a) extracting testable logic into functions callable from tests, (b) subprocess-based testing with `assert_cmd`, or (c) moving reusable logic to `tasker-client` as library code.

---

## File Coverage (sorted by coverage ascending)

| File | Lines Covered | Lines Total | Line % | Function % |
|------|-------------:|------------:|-------:|----------:|
| `src/commands/auth.rs` | 0 | 258 | 0.00% | 0.00% |
| `src/commands/config.rs` | 0 | 850 | 0.00% | 0.00% |
| `src/commands/dlq.rs` | 0 | 150 | 0.00% | 0.00% |
| `src/commands/docs.rs` | 0 | 317 | 0.00% | 0.00% |
| `src/commands/system.rs` | 0 | 181 | 0.00% | 0.00% |
| `src/commands/task.rs` | 0 | 360 | 0.00% | 0.00% |
| `src/commands/worker.rs` | 0 | 122 | 0.00% | 0.00% |
| `src/main.rs` | 0 | 37 | 0.00% | 0.00% |
| `src/docs/templates.rs` | 223 | 252 | 88.49% | 100.00% |

---

## Gap Analysis by Priority

### High Priority

**1. `commands/config.rs` -- 0% (0/850 lines)**

The largest command module. Implements config generation, validation, explanation, source validation, usage analysis, dump, and show commands. Contains substantial business logic:
- `ConfigMerger`: Merges base + environment TOML configurations
- `ConfigValidator`: Validates generated configuration against JSON schema
- `ParameterDocumentation`: Loads and queries parameter documentation
- `extract_error_position()`: Parses TOML validation errors for user-friendly output
- Environment variable substitution logic

Much of the config_commands_test.rs integration test exercises `ConfigMerger` and related types, but because those types are defined inside the binary crate, the coverage is attributed to the test binary, not the source file.

**Testing strategy**:
- Extract `ConfigMerger`, `ConfigValidator`, and `ParameterDocumentation` into the library portion of the crate or into `tasker-client` so their coverage counts
- Alternatively, use `assert_cmd` subprocess tests for `tasker-cli config generate`, `config validate`, and `config explain` (these require no network)

**Estimated coverable lines**: ~400-500 lines (through logic extraction or subprocess tests)

---

**2. `commands/task.rs` -- 0% (0/360 lines)**

Task CRUD operations: create, get, list, cancel, steps, step, reset-step, resolve-step, complete-step, step-audit. These are thin wrappers that parse CLI arguments, construct API client calls, and format output. The logic complexity is in JSON parsing for task creation and step action construction.

**Testing strategy**:
- Subprocess tests for `tasker-cli task list` (requires running services) are lower value
- Extract input validation logic (JSON parsing, UUID validation) into testable functions
- The `format_task_response()` and similar formatting functions could be extracted and tested

**Estimated coverable lines**: ~100-150 lines (through extraction)

---

**3. `commands/docs.rs` -- 0% (0/317 lines)**

Documentation generation commands: reference, annotated, section, coverage, explain, index. Template rendering is already well-tested in `docs/templates.rs` (88.49%). The uncovered code is the CLI dispatch layer that loads data and invokes templates.

**Testing strategy**:
- Subprocess tests for `tasker-cli docs reference` and `tasker-cli docs index` (no network required)
- These commands produce deterministic output from the config directory structure

**Estimated coverable lines**: ~100-150 lines (through subprocess tests)

---

### Medium Priority

**4. `commands/auth.rs` -- 0% (0/258 lines)**

Auth commands: generate-keys (RSA key pair generation), generate-token (JWT creation), show-permissions, validate-token. Contains cryptographic operations (RSA key generation, JWT signing) that have real correctness requirements.

**Testing strategy**:
- `generate-keys` and `show-permissions` can be tested via subprocess (no network)
- RSA key generation and JWT signing logic could be extracted into testable functions
- `validate-token` requires a valid token, testable with a generated fixture

**Estimated coverable lines**: ~80-120 lines

---

**5. `commands/system.rs` -- 0% (0/181 lines)**

Health check and system info commands. Thin wrappers around API client calls with output formatting. These require a running service to produce meaningful output.

**Testing strategy**: Lower priority. Health check formatting could be extracted, but the value is limited since the logic is in `tasker-client`.

**Estimated coverable lines**: ~30-50 lines (through extraction)

---

### Lower Priority

**6. `commands/dlq.rs` -- 0% (0/150 lines)**

DLQ list/get/update/stats commands. Thin wrappers. DLQ status string parsing is the main testable logic.

**Estimated coverable lines**: ~30-50 lines

---

**7. `commands/worker.rs` -- 0% (0/122 lines)**

Worker list/status/health commands. Thin wrappers with minimal logic.

**Estimated coverable lines**: ~20-30 lines

---

**8. `main.rs` -- 0% (0/37 lines)**

CLI entry point with Clap argument definitions and command dispatch. This is boilerplate -- low testing value.

**Estimated coverable lines**: ~0 lines (dispatch code is not meaningfully testable)

---

## Recommended Test Plan

### Phase 1: Logic Extraction (~30% target)

**Focus**: Extract testable logic from binary command handlers into functions that can be called from tests within the crate. This is the most effective approach because it makes the logic available to standard `cargo test` without subprocess overhead.

1. **Extract config command logic** -- Move `ConfigMerger`, `ConfigValidator`, `ParameterDocumentation` into a `lib.rs` module or `tasker-client` (~400 lines)
2. **Extract auth utilities** -- RSA key generation wrapper, permissions listing (~80 lines)
3. **Extract input validation** -- JSON parsing, UUID validation, DLQ status parsing (~50 lines)

**Estimated result**: ~753 lines covered / 2,527 total = **~29.8%** -- approaches 30% threshold

### Phase 2: Subprocess Tests (stretch goal ~40%)

**Focus**: Use `assert_cmd` for end-to-end CLI testing of commands that require no network.

4. **Config commands** -- `config generate`, `config validate`, `config explain` with test fixtures
5. **Docs commands** -- `docs reference`, `docs index`
6. **Auth commands** -- `generate-keys`, `show-permissions`

**Estimated result**: ~900+ lines covered / 2,527 total = **~35%+**

---

## Estimated Impact

| Phase | New Lines Covered | Cumulative Lines | Cumulative Coverage | Key Files Improved |
|-------|------------------:|-----------------:|--------------------:|-------------------|
| Current | -- | 223 | 8.82% | docs/templates.rs at 88% |
| Phase 1 | ~530 | ~753 | ~29.8% | config.rs, auth.rs, task.rs |
| Phase 2 | ~150+ | ~900+ | ~35%+ | config.rs, docs.rs, auth.rs |

**Assessment**: The 30% threshold is achievable primarily through logic extraction (Phase 1). The key insight is that `commands/config.rs` alone has 850 instrumented lines -- extracting its reusable logic (ConfigMerger, validation, documentation) provides the largest coverage gain. Phase 2 subprocess tests add incremental coverage but are not required to meet the threshold.

**Binary crate coverage ceiling**: Command handler dispatch code, output formatting, and Clap boilerplate will always be at 0% unless subprocess testing is implemented. A realistic ceiling for this crate without subprocess tests is ~35-40% (covering all extractable logic). With subprocess tests, ~50-60% is possible but requires more infrastructure.
