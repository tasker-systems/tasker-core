#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Restore Ruby FFI extension from CI build
# =============================================================================
# Restores the Ruby FFI extension (tasker_rb.bundle/.so) from the
# ruby-extension artifact produced by build-workers.yml
#
# upload-artifact preserves directory structure, so files may be nested
# (e.g., artifacts/crates/tasker-rb/lib/tasker_core/tasker_rb.so).
# This script uses find to locate them regardless of nesting depth.
#
# Environment variables:
#   ARTIFACTS_DIR - Directory where artifacts were downloaded (default: artifacts/ruby)
#
# Usage:
#   ./ci-restore-ruby-extension.sh
#   ARTIFACTS_DIR=/path/to/artifacts ./ci-restore-ruby-extension.sh
# =============================================================================

ARTIFACTS_DIR="${ARTIFACTS_DIR:-artifacts/ruby}"

echo "Restoring Ruby extension from ${ARTIFACTS_DIR}..."

# Create target directory
mkdir -p crates/tasker-rb/lib/tasker_core

if [ -d "${ARTIFACTS_DIR}" ]; then
    restored=false

    # Look for .bundle (macOS) or .so (Linux) files at any depth
    for ext in bundle so; do
        while IFS= read -r file; do
            cp -f "$file" crates/tasker-rb/lib/tasker_core/
            echo "  Restored $(basename "$file")"
            restored=true
        done < <(find "${ARTIFACTS_DIR}" -name "*.${ext}" 2>/dev/null)
    done

    if [ "$restored" = true ]; then
        echo ""
        echo "Ruby extension in crates/tasker-rb/lib/tasker_core/:"
        ls -lh crates/tasker-rb/lib/tasker_core/ 2>/dev/null || true
    else
        echo "  Warning: No extension files found in artifacts"
        echo "  Ruby extension will need to be built from source"
    fi
else
    echo "  Warning: Artifacts directory not found: ${ARTIFACTS_DIR}"
    echo "  Ruby extension will need to be built from source"
    exit 0
fi

echo "Ruby extension restoration complete"
