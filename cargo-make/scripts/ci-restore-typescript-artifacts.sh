#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Restore TypeScript artifacts from CI build
# =============================================================================
# Restores the TypeScript dist folder and napi-rs .node file from the
# typescript-artifacts artifact produced by build-workers.yml.
#
# The napi CLI (`napi build --platform`) places .node files directly in the
# package root (workers/typescript/). FfiLayer auto-discovers them there.
#
# Environment variables:
#   ARTIFACTS_DIR - Directory where artifacts were downloaded (default: artifacts/typescript)
#
# Usage:
#   ./ci-restore-typescript-artifacts.sh
#   ARTIFACTS_DIR=/path/to/artifacts ./ci-restore-typescript-artifacts.sh
# =============================================================================

ARTIFACTS_DIR="${ARTIFACTS_DIR:-artifacts/typescript}"

echo "Restoring TypeScript artifacts from ${ARTIFACTS_DIR}..."

if [ -d "${ARTIFACTS_DIR}" ]; then
    # Restore napi-rs .node file to workers/typescript/ (package root)
    # napi CLI names them: tasker_ts.<platform>.node (e.g., tasker_ts.linux-x64-gnu.node)
    mkdir -p workers/typescript

    NODE_FILES_FOUND=0
    for node_file in "${ARTIFACTS_DIR}"/tasker_ts.*.node; do
        if [ -f "$node_file" ]; then
            cp -f "$node_file" workers/typescript/
            echo "  Restored $(basename "$node_file")"
            NODE_FILES_FOUND=$((NODE_FILES_FOUND + 1))
        fi
    done

    # Also check nested structure (artifact may preserve directory structure)
    for node_file in "${ARTIFACTS_DIR}"/workers/typescript/tasker_ts.*.node; do
        if [ -f "$node_file" ]; then
            cp -f "$node_file" workers/typescript/
            echo "  Restored $(basename "$node_file") (nested)"
            NODE_FILES_FOUND=$((NODE_FILES_FOUND + 1))
        fi
    done

    if [ "$NODE_FILES_FOUND" -eq 0 ]; then
        echo "  Warning: No .node files found in artifacts"
    else
        echo ""
        echo "FFI modules in workers/typescript/:"
        ls -lh workers/typescript/tasker_ts.*.node 2>/dev/null || true
    fi

    # Restore TypeScript dist folder
    mkdir -p workers/typescript/dist

    if [ -d "${ARTIFACTS_DIR}/dist" ]; then
        cp -r "${ARTIFACTS_DIR}/dist/"* workers/typescript/dist/ 2>/dev/null || true
        echo "  Restored TypeScript dist (flat)"
    elif [ -d "${ARTIFACTS_DIR}/workers/typescript/dist" ]; then
        cp -r "${ARTIFACTS_DIR}/workers/typescript/dist/"* workers/typescript/dist/ 2>/dev/null || true
        echo "  Restored TypeScript dist (nested)"
    else
        echo "  Note: No dist folder found - will be built fresh"
    fi
else
    echo "  Warning: Artifacts directory not found: ${ARTIFACTS_DIR}"
    echo "  TypeScript artifacts will need to be built from source"
    exit 0
fi

echo "TypeScript artifacts restoration complete"
