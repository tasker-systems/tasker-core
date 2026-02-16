#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Start orchestration server for CI client API tests
# =============================================================================
# Restores the pre-built tasker-server binary from artifacts (or builds from
# source as fallback), starts it in the background, and waits for health check.
#
# Environment variables:
#   ARTIFACTS_DIR      - Directory with core artifacts (default: artifacts/core)
#   DATABASE_URL       - PostgreSQL connection string (required)
#   REDIS_URL          - Redis connection string (required)
#   TASKER_ENV         - Environment name (default: test)
#   TASKER_TEMPLATE_PATH - Path to task template fixtures (required)
#   HEALTH_TIMEOUT     - Max seconds to wait for health (default: 120)
#   GITHUB_ENV         - GitHub Actions env file (auto-set in CI)
#
# Outputs (via GITHUB_ENV):
#   ORCHESTRATION_PID  - PID of the background server process
#
# Usage:
#   ./ci-start-orchestration-server.sh
#   ARTIFACTS_DIR=/path/to/artifacts ./ci-start-orchestration-server.sh
# =============================================================================

ARTIFACTS_DIR="${ARTIFACTS_DIR:-artifacts/core}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-120}"
HEALTH_INTERVAL=2
MAX_RETRIES=$((HEALTH_TIMEOUT / HEALTH_INTERVAL))

# ---- Step 1: Restore binary from artifacts ----
if [ -f "${ARTIFACTS_DIR}/tasker-server" ]; then
    mkdir -p target/debug
    cp -f "${ARTIFACTS_DIR}/tasker-server" target/debug/tasker-server
    chmod +x target/debug/tasker-server
    echo "✅ Restored pre-built tasker-server binary"
fi

# ---- Step 2: Start the server ----
echo "🚀 Starting orchestration server for client API tests..."

if [ -x "target/debug/tasker-server" ]; then
    echo "Using pre-built tasker-server binary"
    ./target/debug/tasker-server &
else
    echo "Pre-built binary not found, building from source..."
    cargo build -p tasker-orchestration --bin tasker-server
    ./target/debug/tasker-server &
fi
ORCH_PID=$!

# Export PID for cleanup in later steps
if [ -n "${GITHUB_ENV:-}" ]; then
    echo "ORCHESTRATION_PID=$ORCH_PID" >> "$GITHUB_ENV"
fi

# ---- Step 3: Wait for health check ----
for i in $(seq 1 "$MAX_RETRIES"); do
    if curl -sf http://localhost:8080/health > /dev/null 2>&1; then
        echo "✅ Orchestration server healthy (attempt $i)"
        exit 0
    fi
    if [ "$i" -eq "$MAX_RETRIES" ]; then
        echo "❌ Orchestration server failed to start after ${HEALTH_TIMEOUT}s"
        kill "$ORCH_PID" 2>/dev/null || true
        exit 1
    fi
    echo "⏳ Waiting for orchestration server... (attempt $i/$MAX_RETRIES)"
    sleep "$HEALTH_INTERVAL"
done
