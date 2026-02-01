#!/bin/bash
# =============================================================================
# coverage-e2e.sh
# =============================================================================
# Run Rust E2E tests against instrumented service binaries to capture coverage
# of code paths only exercised through the full HTTP/gRPC service stack.
#
# Supports dual-backend mode: runs E2E tests against PGMQ and RabbitMQ
# backends sequentially, accumulating LLVM profraw coverage data from both
# passes. LLVM profraw files use %p-%m patterns (PID-based), so running
# services twice with different backends produces distinct profraw files
# that `cargo llvm-cov report` merges automatically.
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
# Generates per-crate E2E coverage reports for 6 Rust crates. Both binaries
# link tasker-shared, tasker-pgmq, and tasker-client as library dependencies,
# so their execution covers code paths only reachable through the full service
# stack.
#
# Usage:
#   cargo make coverage-e2e                   # Both backends (default)
#   cargo make coverage-e2e-pgmq              # PGMQ backend only
#   cargo make coverage-e2e-rabbitmq          # RabbitMQ backend only
#   COV_E2E_BACKEND=pgmq cargo make coverage-e2e  # Env var selection
#
# Prerequisites:
#   - At least one messaging backend available (PostgreSQL for PGMQ, RabbitMQ)
#   - cargo-llvm-cov installed
#   - Ports 8080/8081 available (stop existing services first)
# =============================================================================

set -euo pipefail

PROJECT_ROOT="$(pwd)"
SCRIPTS_DIR="${PROJECT_ROOT}/cargo-make/scripts"
PID_DIR="${PROJECT_ROOT}/.pids"
LOG_DIR="${PROJECT_ROOT}/.logs"

# All non-FFI Rust crates that get E2E coverage reports.
# Both binaries are always instrumented and running during E2E tests.
# The rust-worker binary links tasker-worker, tasker-shared, tasker-pgmq,
# and tasker-client as library dependencies, so execution covers their
# code paths too.
E2E_CRATES=("tasker-orchestration" "tasker-worker-rust" "tasker-worker" "tasker-shared" "tasker-pgmq" "tasker-client")

# =============================================================================
# Backend Selection
# =============================================================================
# COV_E2E_BACKEND env var or --backend= flag selects a single backend.
# When unset, both backends are tested sequentially.

REQUESTED_BACKEND="${COV_E2E_BACKEND:-}"
for arg in "$@"; do
    case "$arg" in
        --backend=*) REQUESTED_BACKEND="${arg#--backend=}" ;;
    esac
done

if [[ -n "$REQUESTED_BACKEND" ]]; then
    BACKENDS=("$REQUESTED_BACKEND")
else
    BACKENDS=("pgmq" "rabbitmq")
fi

# Track which backends actually ran
BACKENDS_RAN=()
BACKENDS_SKIPPED=()

# =============================================================================
# Infrastructure Probes
# =============================================================================

check_backend_available() {
    local backend="$1"
    case "$backend" in
        pgmq)
            pg_isready -h localhost -p "${PGPORT:-5432}" > /dev/null 2>&1
            ;;
        rabbitmq)
            nc -z localhost 5672 2>/dev/null || lsof -i :5672 -sTCP:LISTEN >/dev/null 2>&1
            ;;
        *)
            echo "  Warning: Unknown backend '${backend}', skipping."
            return 1
            ;;
    esac
}

# =============================================================================
# Environment Setup (mirrors run-orchestration.sh pattern)
# =============================================================================

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

# =============================================================================
# Service Lifecycle Functions
# =============================================================================

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

stop_all_services() {
    stop_service "$COV_ORCH_NAME"
    stop_service "$COV_WORKER_NAME"
}

cleanup() {
    echo ""
    echo "Cleaning up instrumented services..."
    stop_all_services
    echo "Cleanup complete."
}
trap cleanup EXIT

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
    return 1
}

# =============================================================================
# Start instrumented services for a given backend
# =============================================================================
# Returns 0 on success, 1 on failure (caller should skip this backend).

start_services() {
    local backend="$1"

    echo "  Starting instrumented services (backend: ${backend})..."

    export TASKER_MESSAGING_BACKEND="$backend"

    # ---- Port conflict check ----
    for port in 8080 8081; do
        if lsof -i ":${port}" -sTCP:LISTEN >/dev/null 2>&1; then
            echo "  Error: Port ${port} is already in use."
            echo "    Stop existing services first: cargo make services-stop"
            return 1
        fi
    done

    # ---- Orchestration (port 8080) ----
    if [ -f "${PID_DIR}/${COV_ORCH_NAME}.pid" ]; then
        local old_pid
        old_pid=$(cat "${PID_DIR}/${COV_ORCH_NAME}.pid")
        if kill -0 "$old_pid" 2>/dev/null; then
            echo "  Error: ${COV_ORCH_NAME} already running (PID: ${old_pid})"
            return 1
        fi
        rm -f "${PID_DIR}/${COV_ORCH_NAME}.pid"
    fi

    export PORT=8080
    LLVM_PROFILE_FILE="${PROFRAW_DIR}/orch-%p-%m.profraw" \
        nohup "$ORCH_BIN" > "${LOG_DIR}/${COV_ORCH_NAME}.log" 2>&1 &
    ORCH_PID=$!
    echo "$ORCH_PID" > "${PID_DIR}/${COV_ORCH_NAME}.pid"

    sleep 2
    if ! kill -0 "$ORCH_PID" 2>/dev/null; then
        echo "  Error: Orchestration failed to start"
        echo "    Check logs: tail -50 ${LOG_DIR}/${COV_ORCH_NAME}.log"
        return 1
    fi
    echo "  Orchestration started (PID: ${ORCH_PID})"

    # ---- Rust Worker (port 8081) ----
    if [ -f "workers/rust/.env" ]; then
        set -a
        source "workers/rust/.env"
        set +a
    fi

    if [ -f "${PID_DIR}/${COV_WORKER_NAME}.pid" ]; then
        local old_pid
        old_pid=$(cat "${PID_DIR}/${COV_WORKER_NAME}.pid")
        if kill -0 "$old_pid" 2>/dev/null; then
            echo "  Error: ${COV_WORKER_NAME} already running (PID: ${old_pid})"
            stop_service "$COV_ORCH_NAME"
            return 1
        fi
        rm -f "${PID_DIR}/${COV_WORKER_NAME}.pid"
    fi

    export PORT=8081
    LLVM_PROFILE_FILE="${PROFRAW_DIR}/worker-%p-%m.profraw" \
        nohup "$WORKER_BIN" > "${LOG_DIR}/${COV_WORKER_NAME}.log" 2>&1 &
    WORKER_PID=$!
    echo "$WORKER_PID" > "${PID_DIR}/${COV_WORKER_NAME}.pid"

    sleep 2
    if ! kill -0 "$WORKER_PID" 2>/dev/null; then
        echo "  Error: Rust worker failed to start"
        echo "    Check logs: tail -50 ${LOG_DIR}/${COV_WORKER_NAME}.log"
        stop_service "$COV_ORCH_NAME"
        return 1
    fi
    echo "  Rust worker started (PID: ${WORKER_PID})"

    # ---- Health checks ----
    if ! health_check "http://localhost:8080/health" "orchestration"; then
        stop_all_services
        return 1
    fi
    if ! health_check "http://localhost:8081/health" "rust-worker"; then
        stop_all_services
        return 1
    fi

    return 0
}

# =============================================================================
# Run E2E tests for a single backend
# =============================================================================
# Starts services, runs tests, stops services. Profraw files accumulate
# across backend passes because each process gets a unique PID-based filename.

run_e2e_for_backend() {
    local backend="$1"

    echo ""
    echo "======================================================================="
    echo " E2E Coverage Pass: ${backend}"
    echo "======================================================================="

    # Check infrastructure availability
    if ! check_backend_available "$backend"; then
        echo "  Warning: ${backend} infrastructure not available, skipping."
        BACKENDS_SKIPPED+=("$backend")
        return 0
    fi

    # Start services with this backend
    if ! start_services "$backend"; then
        echo "  Warning: Failed to start services for ${backend}, skipping."
        BACKENDS_SKIPPED+=("$backend")
        return 0
    fi

    # Run E2E tests (uninstrumented test binary)
    # TASKER_COVERAGE_MODE signals to test timeouts that services are running
    # under instrumented debug builds, so they apply a 4x multiplier.
    # Limit to 4 parallel tests (-j 4) to avoid overwhelming the instrumented
    # services -- mirrors the CI profile's test-threads=4 setting.
    echo "  Running Rust E2E tests (backend: ${backend})..."
    (
        unset RUSTFLAGS CARGO_LLVM_COV LLVM_PROFILE_FILE CARGO_TARGET_DIR \
              CARGO_LLVM_COV_TARGET_DIR CARGO_LLVM_COV_BUILD_DIR CARGO_LLVM_COV_SHOW_ENV
        export TASKER_COVERAGE_MODE=1
        cargo nextest run --test e2e_tests --features test-services \
            -j 4 -E 'test(~e2e::rust::)'
    ) || echo "  Warning: Some E2E tests failed for ${backend}. Collecting coverage from what ran."

    # Stop services (SIGTERM triggers profraw flush)
    echo "  Stopping services for ${backend}..."
    stop_all_services

    BACKENDS_RAN+=("$backend")
    echo "  ${backend} pass complete."
}

# =============================================================================
# Step 1: Ensure consistent .env files
# =============================================================================
echo "Step 1/5: Ensuring consistent .env files..."
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
echo "Step 2/5: Setting up coverage instrumentation..."

cargo llvm-cov clean --workspace
eval "$(cargo llvm-cov show-env --export-prefix)"

# --all-features enables tokio-console which requires tokio_unstable cfg.
# show-env overwrites RUSTFLAGS, so append the cfg here.
export RUSTFLAGS="${RUSTFLAGS} --cfg tokio_unstable"

# show-env sets CARGO_LLVM_COV_TARGET_DIR to control where builds go
LLVM_COV_TARGET_DIR="${CARGO_LLVM_COV_TARGET_DIR:-${CARGO_TARGET_DIR:-target}}"
PROFRAW_DIR="${LLVM_COV_TARGET_DIR}"

# =============================================================================
# Step 3: Build instrumented binaries (once for all backends)
# =============================================================================
echo "Step 3/5: Building instrumented service binaries..."
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
# Step 4: Run E2E tests for each backend
# =============================================================================
echo "Step 4/5: Running E2E tests per backend..."
echo "  Backends to test: ${BACKENDS[*]}"

for backend in "${BACKENDS[@]}"; do
    run_e2e_for_backend "$backend"
done

# Check that at least one backend ran
if [ ${#BACKENDS_RAN[@]} -eq 0 ]; then
    echo ""
    echo "Error: No backends were available. At least one is required."
    echo "  Backends requested: ${BACKENDS[*]}"
    echo "  Ensure PostgreSQL (for PGMQ) or RabbitMQ is running."
    exit 1
fi

# =============================================================================
# Step 5: Generate combined report and normalize per crate
# =============================================================================
echo "Step 5/5: Generating combined report from all backend passes..."

# Disable the cleanup trap since we already stopped services
trap - EXIT

# Instrumentation env vars are still set in the main shell; generate report
cargo llvm-cov report --json --output-path "coverage-reports/rust/e2e-raw.json"

# Normalize once per crate (all backend passes contributed to the profraw data)
for crate in "${E2E_CRATES[@]}"; do
    echo "  Normalizing ${crate}..."
    uv run --project cargo-make/scripts/coverage python3 cargo-make/scripts/coverage/normalize-rust.py \
        "coverage-reports/rust/e2e-raw.json" \
        "coverage-reports/rust/${crate}-e2e-coverage.json" \
        --crate "${crate}"
done

# =============================================================================
# Summary
# =============================================================================
echo ""
echo "E2E coverage complete"
echo "  Backends tested: ${BACKENDS_RAN[*]}"
if [ ${#BACKENDS_SKIPPED[@]} -gt 0 ]; then
    echo "  Backends skipped: ${BACKENDS_SKIPPED[*]}"
fi
echo "  Raw: coverage-reports/rust/e2e-raw.json"
for crate in "${E2E_CRATES[@]}"; do
    echo "  ${crate}: coverage-reports/rust/${crate}-e2e-coverage.json"
done
