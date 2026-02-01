#!/bin/bash
# =============================================================================
# coverage-e2e.sh
# =============================================================================
# Run Rust E2E tests against instrumented service binaries to capture coverage
# of code paths only exercised through the full HTTP/gRPC service stack.
#
# Only starts orchestration + rust worker (not FFI workers). FFI workers
# (Python, Ruby, TypeScript) load the tasker-worker dylib into their own
# runtime's process space, so LLVM coverage instrumentation is not feasible.
# Tests are limited to tests/e2e/rust/ accordingly.
#
# Service startup follows the same patterns as services-start-all.sh and
# service-start.sh (PID files in .pids/, logs in .logs/, health checks),
# but runs pre-built instrumented binaries instead of `cargo run`.
#
# Generates per-crate E2E coverage reports for tasker-orchestration,
# tasker-worker-rust, and tasker-worker since both binaries are always running
# during the tests. The rust-worker binary links tasker-worker as a library,
# so its execution covers tasker-worker code paths that are only reachable
# through the full service stack (bootstrap, lifecycle, web/gRPC handlers).
#
# Usage:
#   cargo make coverage-e2e
#
# Prerequisites:
#   - PostgreSQL + RabbitMQ running (via docker-compose)
#   - cargo-llvm-cov installed
#   - Ports 8080/8081 available (stop existing services first)
# =============================================================================

set -euo pipefail

PROJECT_ROOT="$(pwd)"
SCRIPTS_DIR="${PROJECT_ROOT}/cargo-make/scripts"
PID_DIR="${PROJECT_ROOT}/.pids"
LOG_DIR="${PROJECT_ROOT}/.logs"

# Both binaries are always instrumented and running during E2E tests.
# The rust-worker binary links tasker-worker as a library dependency, so
# its execution also covers tasker-worker code paths.
E2E_CRATES=("tasker-orchestration" "tasker-worker-rust" "tasker-worker")

# ---- Environment setup (mirrors run-orchestration.sh pattern) ----

# TAS-78: Preserve database URLs if already set (for split-database mode)
_SAVED_DATABASE_URL="${DATABASE_URL:-}"
_SAVED_PGMQ_DATABASE_URL="${PGMQ_DATABASE_URL:-}"

# Source root .env
if [[ -f ".env" ]]; then
    set -a
    source ".env"
    set +a
fi

# Restore preserved URLs (split-db mode takes precedence)
[[ -n "$_SAVED_DATABASE_URL" ]] && export DATABASE_URL="$_SAVED_DATABASE_URL"
[[ -n "$_SAVED_PGMQ_DATABASE_URL" ]] && export PGMQ_DATABASE_URL="$_SAVED_PGMQ_DATABASE_URL"

export TASKER_ENV="${TASKER_ENV:-test}"
export TASKER_CONFIG_PATH="${TASKER_CONFIG_PATH:-${PROJECT_ROOT}/config/tasker/generated/complete-test.toml}"

mkdir -p coverage-reports/rust "$PID_DIR" "$LOG_DIR"

# ---- Cleanup trap (mirrors service-stop.sh pattern) ----

# Use "cov-" prefix to avoid colliding with normal service PID files.
COV_ORCH_NAME="cov-orchestration"
COV_WORKER_NAME="cov-rust-worker"

stop_service() {
    local name="$1"
    local pid_file="${PID_DIR}/${name}.pid"
    if [ ! -f "$pid_file" ]; then return 0; fi

    local pid
    pid=$(cat "$pid_file")
    if ! kill -0 "$pid" 2>/dev/null; then
        rm -f "$pid_file"
        return 0
    fi

    echo "  Stopping ${name} (PID: ${pid})..."
    kill -TERM "$pid" 2>/dev/null || true

    # Wait for graceful shutdown (allows profraw flush)
    local counter=0
    while kill -0 "$pid" 2>/dev/null && [ $counter -lt 10 ]; do
        sleep 1
        counter=$((counter + 1))
    done

    if kill -0 "$pid" 2>/dev/null; then
        echo "  Force killing ${name}..."
        kill -9 "$pid" 2>/dev/null || true
        sleep 1
    fi
    rm -f "$pid_file"
}

cleanup() {
    echo ""
    echo "Cleaning up instrumented services..."
    stop_service "$COV_ORCH_NAME"
    stop_service "$COV_WORKER_NAME"
    echo "Cleanup complete."
}
trap cleanup EXIT

# ---- Port conflict check ----

for port in 8080 8081; do
    if lsof -i ":${port}" -sTCP:LISTEN >/dev/null 2>&1; then
        echo "Error: Port ${port} is already in use."
        echo "  Stop existing services first: cargo make services-stop"
        exit 1
    fi
done

# =============================================================================
# Step 1: Ensure consistent .env files
# =============================================================================
echo "Step 1/7: Ensuring consistent .env files..."
cargo make setup-env
cargo make setup-env-orchestration
cargo make setup-env-rust-worker

# Re-source root .env after regeneration (preserving split-db URLs)
_SAVED_DATABASE_URL="${DATABASE_URL:-}"
_SAVED_PGMQ_DATABASE_URL="${PGMQ_DATABASE_URL:-}"
if [[ -f ".env" ]]; then
    set -a
    source ".env"
    set +a
fi
[[ -n "$_SAVED_DATABASE_URL" ]] && export DATABASE_URL="$_SAVED_DATABASE_URL"
[[ -n "$_SAVED_PGMQ_DATABASE_URL" ]] && export PGMQ_DATABASE_URL="$_SAVED_PGMQ_DATABASE_URL"

# =============================================================================
# Step 2: Setup coverage instrumentation
# =============================================================================
echo "Step 2/7: Setting up coverage instrumentation..."

cargo llvm-cov clean --workspace
eval "$(cargo llvm-cov show-env --export-prefix)"

# --all-features enables tokio-console which requires tokio_unstable cfg.
# show-env overwrites RUSTFLAGS, so append the cfg here.
export RUSTFLAGS="${RUSTFLAGS} --cfg tokio_unstable"

# show-env sets CARGO_LLVM_COV_TARGET_DIR to control where builds go
LLVM_COV_TARGET_DIR="${CARGO_LLVM_COV_TARGET_DIR:-${CARGO_TARGET_DIR:-target}}"
PROFRAW_DIR="${LLVM_COV_TARGET_DIR}"

# =============================================================================
# Step 3: Build instrumented binaries
# =============================================================================
echo "Step 3/7: Building instrumented service binaries..."
cargo build --all-features -p tasker-orchestration --bin tasker-server
cargo build --all-features -p tasker-worker-rust --bin rust-worker

ORCH_BIN="${LLVM_COV_TARGET_DIR}/debug/tasker-server"
WORKER_BIN="${LLVM_COV_TARGET_DIR}/debug/rust-worker"

for bin_name in "$ORCH_BIN" "$WORKER_BIN"; do
    if [ ! -f "$bin_name" ]; then
        echo "Error: Instrumented binary not found at: ${bin_name}"
        exit 1
    fi
done

# =============================================================================
# Step 4: Start instrumented services
# =============================================================================
# Follows service-start.sh patterns: nohup, PID files, log files,
# duplicate-instance prevention. Uses pre-built instrumented binaries
# instead of `cargo run` so the LLVM instrumentation is preserved.
echo "Step 4/7: Starting instrumented services..."

# ---- Orchestration (port 8080, mirrors run-orchestration.sh) ----
if [ -f "${PID_DIR}/${COV_ORCH_NAME}.pid" ]; then
    old_pid=$(cat "${PID_DIR}/${COV_ORCH_NAME}.pid")
    if kill -0 "$old_pid" 2>/dev/null; then
        echo "Error: ${COV_ORCH_NAME} already running (PID: ${old_pid})"
        exit 1
    fi
    rm -f "${PID_DIR}/${COV_ORCH_NAME}.pid"
fi

export PORT=8080
LLVM_PROFILE_FILE="${PROFRAW_DIR}/orch-%p-%m.profraw" \
    nohup "$ORCH_BIN" > "${LOG_DIR}/${COV_ORCH_NAME}.log" 2>&1 &
ORCH_PID=$!
echo "$ORCH_PID" > "${PID_DIR}/${COV_ORCH_NAME}.pid"

sleep 2
if kill -0 "$ORCH_PID" 2>/dev/null; then
    echo "  Orchestration started (PID: ${ORCH_PID})"
else
    echo "Error: Orchestration failed to start"
    echo "  Check logs: tail -50 ${LOG_DIR}/${COV_ORCH_NAME}.log"
    exit 1
fi

# ---- Rust Worker (port 8081, mirrors workers/rust run task) ----
# Source worker-specific .env for TASKER_TEMPLATE_PATH, WORKER_ID, etc.
if [ -f "workers/rust/.env" ]; then
    set -a
    source "workers/rust/.env"
    set +a
fi

if [ -f "${PID_DIR}/${COV_WORKER_NAME}.pid" ]; then
    old_pid=$(cat "${PID_DIR}/${COV_WORKER_NAME}.pid")
    if kill -0 "$old_pid" 2>/dev/null; then
        echo "Error: ${COV_WORKER_NAME} already running (PID: ${old_pid})"
        exit 1
    fi
    rm -f "${PID_DIR}/${COV_WORKER_NAME}.pid"
fi

export PORT=8081
LLVM_PROFILE_FILE="${PROFRAW_DIR}/worker-%p-%m.profraw" \
    nohup "$WORKER_BIN" > "${LOG_DIR}/${COV_WORKER_NAME}.log" 2>&1 &
WORKER_PID=$!
echo "$WORKER_PID" > "${PID_DIR}/${COV_WORKER_NAME}.pid"

sleep 2
if kill -0 "$WORKER_PID" 2>/dev/null; then
    echo "  Rust worker started (PID: ${WORKER_PID})"
else
    echo "Error: Rust worker failed to start"
    echo "  Check logs: tail -50 ${LOG_DIR}/${COV_WORKER_NAME}.log"
    exit 1
fi

# =============================================================================
# Step 5: Wait for health checks
# =============================================================================
echo "Step 5/7: Waiting for services to become healthy..."

health_check() {
    local url="$1"
    local name="$2"
    local max_attempts=30
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        if curl -sf "$url" > /dev/null 2>&1; then
            echo "  ${name} is healthy"
            return 0
        fi
        attempt=$((attempt + 1))
        sleep 1
    done
    echo "  Error: ${name} failed health check after ${max_attempts}s"
    echo "  Check logs: tail -50 ${LOG_DIR}/cov-${name}.log"
    return 1
}

health_check "http://localhost:8080/health" "orchestration"
health_check "http://localhost:8081/health" "rust-worker"

# =============================================================================
# Step 6: Run Rust E2E tests (uninstrumented)
# =============================================================================
echo "Step 6/7: Running Rust E2E tests (50 tests in e2e::rust::)..."

# Run in a subshell with instrumentation env cleared so the test binary
# itself isn't instrumented (we only want coverage from service binaries).
# Filter to e2e::rust:: tests only -- FFI worker tests need Python/Ruby/TS
# workers which can't be instrumented for Rust coverage.
(
    unset RUSTFLAGS CARGO_LLVM_COV LLVM_PROFILE_FILE CARGO_TARGET_DIR \
          CARGO_LLVM_COV_TARGET_DIR CARGO_LLVM_COV_BUILD_DIR CARGO_LLVM_COV_SHOW_ENV
    cargo nextest run --test e2e_tests --features test-services \
        -E 'test(~e2e::rust::)'
) || echo "  Warning: Some E2E tests failed. Collecting coverage from what ran."

# =============================================================================
# Step 7: Stop services and generate report
# =============================================================================
echo "Step 7/7: Stopping services and generating report..."

# Stop services gracefully (SIGTERM triggers profraw flush)
stop_service "$COV_ORCH_NAME"
stop_service "$COV_WORKER_NAME"

# Disable the cleanup trap since we already stopped services
trap - EXIT

# Instrumentation env vars are still set in the main shell; generate report
cargo llvm-cov report --json --output-path "coverage-reports/rust/e2e-raw.json"

# Normalize once per crate (both binaries contributed to the same profraw data)
for crate in "${E2E_CRATES[@]}"; do
    echo "  Normalizing ${crate}..."
    uv run --project cargo-make/scripts/coverage python3 cargo-make/scripts/coverage/normalize-rust.py \
        "coverage-reports/rust/e2e-raw.json" \
        "coverage-reports/rust/${crate}-e2e-coverage.json" \
        --crate "${crate}"
done

echo ""
echo "E2E coverage complete"
echo "   Raw: coverage-reports/rust/e2e-raw.json"
for crate in "${E2E_CRATES[@]}"; do
    echo "   ${crate}: coverage-reports/rust/${crate}-e2e-coverage.json"
done
