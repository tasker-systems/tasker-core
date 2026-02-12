# Plan: tasker-client Coverage Improvement (26% → 60%)

## Strategy

Two-pronged approach:

1. **New unit tests** in tasker-client for error.rs, conversions.rs, common.rs (no infrastructure needed)
2. **Instrumentation of existing E2E tests** via `coverage-crate-integrated` to credit tasker-client for coverage it already earns through root tests
3. **Targeted new integration tests** in `tasker-client/tests/` only for API methods NOT already exercised by root E2E tests

## What Existing Root E2E Tests Already Cover

`tests/e2e/rust/` exercises these tasker-client methods (via REST):

- **OrchestrationApiClient**: `create_task`, `get_task`, `list_tasks`, `get_basic_health`, `readiness_probe`, `liveness_probe`, `get_detailed_health`, `get_config`, `get_prometheus_metrics`
- **WorkerApiClient**: `health_check`, `readiness_probe`, `liveness_probe`, `get_detailed_health`, `get_config`, `get_prometheus_metrics`, `get_worker_metrics`, `get_domain_event_stats`, `list_templates`

**NOT covered by existing tests** (needs new tests):

- **OrchestrationApiClient**: `cancel_task`, `list_task_steps`, `get_step`, `get_step_audit_history`, DLQ endpoints (list/get/stats/update), analytics endpoints (bottlenecks, staleness, investigation_queue, performance_metrics), `list_templates`, `get_template`
- **WorkerApiClient**: `get_template`, `validate_template`, `get_handler_registry`
- **gRPC clients**: Nothing (all root E2E uses REST)
- **transport.rs**: Unified client barely exercised

---

## Phase 1: Unit Tests (no infrastructure)

### 1A. `tasker-client/src/error.rs` — add `#[cfg(test)] mod tests`

- ~12 tests: constructor functions, `is_recoverable()` for every variant, `Display` output, `From` impls
- Estimated: 27 additional lines covered (10% → ~100%)

### 1B. `tasker-client/src/grpc_clients/conversions.rs` — expand `#[cfg(test)] mod tests`

- ~60-70 tests grouped by:
  - Timestamp conversions (proto_timestamp_to_datetime,_to_string variants)
  - JSON/Struct conversions (prost_value_to_json for all Kind variants)
  - State string conversions (all task/step state variants)
  - Task response conversions (proto_task_to_domain, create/get/list response, error paths)
  - Step response conversions (proto_step_to_domain, audit)
  - Health response conversions (orchestration + worker, missing-field error paths)
  - Template response conversions (orchestration + worker, with/without cache_stats)
  - DLQ conversions (status mapping, entry/stats/queue)
  - Analytics conversions (performance_metrics, bottleneck, staleness)
  - Config conversions (orchestration + worker, missing-field error paths)
- Estimated: ~700 additional lines covered (8.6% → ~70%)

### 1C. `tasker-client/src/grpc_clients/common.rs` — expand `#[cfg(test)] mod tests`

- ~8 tests: `AuthInterceptor::call()` for bearer/api-key/no-auth paths, additional `Status` → `ClientError` mappings (PermissionDenied, Cancelled, ResourceExhausted)
- Estimated: ~35 additional lines covered (59.5% → ~78%)

**Phase 1 projected result: ~42-44% coverage**

---

## Phase 2: Coverage Instrumentation (tooling)

### 2A. Extend `cargo-make/scripts/coverage-crate-integrated.sh`

Add support for additional test targets via `EXTRA_TEST_TARGETS` env var:

```bash
# After Step 2 (root integration tests), add:
# Step 2b: Run additional test targets if specified
for target in ${EXTRA_TEST_TARGETS:-}; do
    echo "  Running extra test target: ${target}..."
    cargo nextest run --all-features --test "${target}" || \
        echo "  Note: Some ${target} tests failed; collecting coverage."
done
```

### 2B. Add convenience task in `Makefile.toml`

```toml
[tasks.coverage-client-full]
description = "Run tasker-client coverage including E2E test attribution"
dependencies = ["coverage-tools-setup"]
env = { "CRATE_NAME" = "tasker-client", "EXTRA_TEST_TARGETS" = "e2e_tests" }
script_runner = "@shell"
script = ["${SCRIPTS_DIR}/coverage-crate-integrated.sh"]
```

This runs: (1) tasker-client's own tests, (2) root integration tests, (3) root E2E tests — all instrumented, with report filtered to tasker-client source files.

---

## Phase 3: Targeted Integration Tests (test-services)

Only for methods NOT already covered by root E2E tests.

### 3A. Cargo.toml changes

Add to `tasker-client/Cargo.toml`:

```toml
[features]
default = ["grpc"]
grpc = ["tonic", "prost-types", "tasker-shared/grpc-api"]
test-services = []  # Integration tests requiring running services
```

### 3B. `tasker-client/tests/common/mod.rs` — shared helpers

~80 lines. URL helpers from env vars (same pattern as `tests/common/integration_test_manager.rs`), client creation helpers, `create_task_request()` with `_test_run_id`.

### 3C. `tasker-client/tests/orchestration_api_gaps_test.rs`

Feature gate: `#![cfg(feature = "test-services")]`

Tests for orchestration methods NOT covered by root E2E:

- `test_cancel_task` — create task then cancel
- `test_list_task_steps` — create task, list its steps
- `test_get_step` — get individual step by UUID
- `test_get_step_audit_history` — step audit after task completes
- `test_list_dlq_entries` — DLQ listing (may be empty)
- `test_get_dlq_stats` — DLQ stats
- `test_get_investigation_queue` — investigation queue
- `test_get_staleness_monitoring` — staleness monitoring
- `test_get_performance_metrics` — with and without time range
- `test_get_bottlenecks` — bottleneck analysis
- `test_list_templates` — orchestration-side templates
- `test_get_template` — specific template detail

~12 tests, covering remaining orchestration_client.rs and transport.rs gaps.

### 3D. `tasker-client/tests/grpc_integration_test.rs`

Feature gate: `#![cfg(all(feature = "test-services", feature = "grpc"))]`

Exercises gRPC client code (currently 3-10% coverage). Tests orchestration + worker gRPC methods:

- Health checks (orchestration + worker)
- Task lifecycle (create, get, list, cancel)
- Step operations
- Template operations
- Config endpoint

Also exercises `conversions.rs` indirectly (real proto data round-trips).

~20 tests covering `grpc_clients/orchestration_grpc_client.rs`, `grpc_clients/worker_grpc_client.rs`, and significant additional `conversions.rs` lines.

---

## Files Modified/Created

| File | Action | Phase |
|------|--------|-------|
| `tasker-client/Cargo.toml` | Add `test-services` feature | 3A |
| `tasker-client/src/error.rs` | Add `#[cfg(test)] mod tests` block | 1A |
| `tasker-client/src/grpc_clients/conversions.rs` | Expand tests | 1B |
| `tasker-client/src/grpc_clients/common.rs` | Expand tests | 1C |
| `cargo-make/scripts/coverage-crate-integrated.sh` | Add `EXTRA_TEST_TARGETS` support | 2A |
| `Makefile.toml` | Add `coverage-client-full` task | 2B |
| `tasker-client/tests/common/mod.rs` | Create shared test helpers | 3B |
| `tasker-client/tests/orchestration_api_gaps_test.rs` | Create gap-filling integration tests | 3C |
| `tasker-client/tests/grpc_integration_test.rs` | Create gRPC transport tests | 3D |

---

## Verification

After each phase:

```bash
# Phase 1: Unit tests only
cargo test --package tasker-client --all-features

# Phase 2: Run full instrumented coverage (requires services)
cargo make coverage-client-full

# Phase 3: Run integration tests specifically
cargo test --package tasker-client --features test-services

# Final coverage check
CRATE_NAME=tasker-client cargo make coverage-crate
```

## Projected Coverage

| Phase | Lines Covered | Coverage % | Notes |
|-------|--------------|-----------|-------|
| Current | 1,185 | 26.05% | 67 unit tests |
| After Phase 1 | ~1,947 | ~42.8% | +762 from unit tests |
| After Phase 2 | ~2,500+ | ~55%+ | E2E attribution (estimate depends on call frequency) |
| After Phase 3 | ~3,100+ | ~68%+ | Gap-filling + gRPC tests |
