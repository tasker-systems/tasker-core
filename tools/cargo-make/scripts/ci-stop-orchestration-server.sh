#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Stop orchestration server started by ci-start-orchestration-server.sh
# =============================================================================
# Gracefully stops the background orchestration server using the PID stored
# in ORCHESTRATION_PID.
#
# Environment variables:
#   ORCHESTRATION_PID - PID of the server process (set by start script)
#
# Usage:
#   ./ci-stop-orchestration-server.sh
# =============================================================================

if [ -n "${ORCHESTRATION_PID:-}" ]; then
    echo "🛑 Stopping orchestration server (PID: $ORCHESTRATION_PID)..."
    kill "$ORCHESTRATION_PID" 2>/dev/null || true
    wait "$ORCHESTRATION_PID" 2>/dev/null || true
    echo "✅ Orchestration server stopped"
else
    echo "⚠️ No ORCHESTRATION_PID set, nothing to stop"
fi
