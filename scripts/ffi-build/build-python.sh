#!/usr/bin/env bash
# scripts/ffi-build/build-python.sh
# Builds the Python FFI extension (PyO3/maturin) for a target architecture.
#
# Usage:
#   ./scripts/ffi-build/build-python.sh [--target TARGET_TRIPLE]
#
# If --target is not specified, builds for the native architecture.
# Produces a wheel in artifacts/<target>/python/

set -euo pipefail

source "$(dirname "$0")/lib/common.sh"

parse_target_arg "$@"

log_header "Building Python FFI Extension"
log_info "Target: ${TARGET}"

# Ensure artifact output directory exists
DEST_DIR="$(ensure_artifact_dir "$TARGET" "python")"

# Verify maturin is available
if ! command -v maturin &>/dev/null; then
    die "maturin not found. Install with: pip install maturin"
fi

log_info "maturin version: $(maturin --version)"

# Build the wheel
cd "${FFI_REPO_ROOT}/workers/python"

BUILD_ARGS=(
    build
    --release
    --locked
)

# Only pass --target for cross-compilation (not when building for native arch)
NATIVE_TARGET="$(detect_arch)"
if [[ "$TARGET" != "$NATIVE_TARGET" ]]; then
    BUILD_ARGS+=(--target "$TARGET")
fi

# On Linux, let maturin auto-detect the manylinux compatibility level.
# Debian bookworm has glibc 2.36, so it targets manylinux_2_34+.
# Do NOT force --manylinux 2_17 — bookworm's glibc is too new for that tag.

log_info "Running: maturin ${BUILD_ARGS[*]}"
maturin "${BUILD_ARGS[@]}"

# Copy wheel(s) to artifact directory
WHEEL_DIR="${FFI_REPO_ROOT}/target/wheels"
if [[ ! -d "$WHEEL_DIR" ]]; then
    die "Wheel directory not found: ${WHEEL_DIR}"
fi

# Find the most recently built wheel
WHEEL_COUNT=0
for whl in "${WHEEL_DIR}"/tasker_py-*.whl; do
    if [[ -f "$whl" ]]; then
        cp "$whl" "$DEST_DIR/"
        log_info "Copied: $(basename "$whl") -> ${DEST_DIR}/"
        WHEEL_COUNT=$((WHEEL_COUNT + 1))
    fi
done

if [[ "$WHEEL_COUNT" -eq 0 ]]; then
    die "No wheel files found in ${WHEEL_DIR}"
fi

log_info "Python build complete: ${WHEEL_COUNT} wheel(s) in ${DEST_DIR}"
