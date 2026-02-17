#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Restore TypeScript artifacts from CI build
# =============================================================================
# Restores the TypeScript dist folder and napi-rs FFI library from the
# typescript-artifacts artifact produced by build-workers.yml
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
    # Restore napi-rs FFI library to target/debug
    # napi-rs produces libtasker_ts.{so,dylib} which is loaded via require() or env var
    mkdir -p target/debug

    for lib in libtasker_ts.so libtasker_ts.dylib; do
        # Try flat structure first
        if [ -f "${ARTIFACTS_DIR}/${lib}" ]; then
            cp -f "${ARTIFACTS_DIR}/${lib}" target/debug/
            echo "  Restored ${lib} (flat)"
        # Try nested structure
        elif [ -f "${ARTIFACTS_DIR}/target/debug/${lib}" ]; then
            cp -f "${ARTIFACTS_DIR}/target/debug/${lib}" target/debug/
            echo "  Restored ${lib} (nested)"
        fi
    done

    # Create .node copy — require() only recognizes .node extension as native modules.
    # Bun doesn't follow symlinks for .node files, so we hard-copy instead.
    if [ -f target/debug/libtasker_ts.so ]; then
        cp -f target/debug/libtasker_ts.so target/debug/tasker_ts.node
        echo "  Created tasker_ts.node from libtasker_ts.so"
    elif [ -f target/debug/libtasker_ts.dylib ]; then
        cp -f target/debug/libtasker_ts.dylib target/debug/tasker_ts.node
        echo "  Created tasker_ts.node from libtasker_ts.dylib"
    fi

    # Verify FFI library and set environment variable
    if [ -f target/debug/tasker_ts.node ]; then
        echo ""
        echo "FFI module in target/debug/:"
        ls -lh target/debug/tasker_ts.node target/debug/libtasker_ts.* 2>/dev/null || true
        TASKER_FFI_MODULE_PATH="$(pwd)/target/debug/tasker_ts.node"
        export TASKER_FFI_MODULE_PATH
        echo "TASKER_FFI_MODULE_PATH=$TASKER_FFI_MODULE_PATH"
    else
        echo "  Warning: FFI library not found in artifacts"
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
