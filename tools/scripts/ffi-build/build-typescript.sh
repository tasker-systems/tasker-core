#!/usr/bin/env bash
# scripts/ffi-build/build-typescript.sh
# Builds the TypeScript napi-rs FFI extension for a target architecture.
#
# Usage:
#   ./scripts/ffi-build/build-typescript.sh [--target TARGET_TRIPLE]
#
# If --target is not specified, builds for the native architecture.
# Produces tasker_ts-<target>.node in artifacts/<target>/typescript/

set -euo pipefail

source "$(dirname "$0")/lib/common.sh"

parse_target_arg "$@"

log_header "Building TypeScript napi-rs FFI Extension"
log_info "Target: ${TARGET}"

# Ensure artifact output directory exists
DEST_DIR="$(ensure_artifact_dir "$TARGET" "typescript")"

cd "${FFI_REPO_ROOT}"

BUILD_ARGS=(
    build
    -p tasker-ts
    --release
    --locked
)

# Only pass --target for cross-compilation
NATIVE_TARGET="$(detect_arch)"
if [[ "$TARGET" != "$NATIVE_TARGET" ]]; then
    BUILD_ARGS+=(--target "$TARGET")
fi

log_info "Running: cargo ${BUILD_ARGS[*]}"
cargo "${BUILD_ARGS[@]}"

# Determine output paths — napi-rs still produces a cdylib with standard naming
RELEASE_DIR="$(cargo_release_dir "$TARGET")"
EXT="$(lib_extension)"
LIB_NAME="libtasker_ts.${EXT}"

SRC_PATH="${RELEASE_DIR}/${LIB_NAME}"
if [[ ! -f "$SRC_PATH" ]]; then
    # When building for native target without --target flag, check target/release/
    SRC_PATH="${FFI_REPO_ROOT}/target/release/${LIB_NAME}"
fi

if [[ ! -f "$SRC_PATH" ]]; then
    die "Built library not found. Checked: ${RELEASE_DIR}/${LIB_NAME} and target/release/${LIB_NAME}"
fi

# Copy as .node file using napi-rs naming convention for release/publishing
DEST_NAME="tasker_ts-${TARGET}.node"
cp "$SRC_PATH" "${DEST_DIR}/${DEST_NAME}"
log_info "Copied: ${LIB_NAME} -> ${DEST_DIR}/${DEST_NAME}"

log_info "TypeScript build complete: ${DEST_DIR}/${DEST_NAME}"
