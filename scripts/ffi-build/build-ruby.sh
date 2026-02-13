#!/usr/bin/env bash
# scripts/ffi-build/build-ruby.sh
# Builds the Ruby FFI extension (Magnus/rb_sys) for a target architecture.
#
# Usage:
#   ./scripts/ffi-build/build-ruby.sh [--target TARGET_TRIPLE]
#
# If --target is not specified, builds for the native architecture.
# Produces tasker_rb-<target>.{so,bundle} in artifacts/<target>/ruby/
#
# Note: Ruby extensions must be built on the target architecture (no cross-compilation).
# Docker provides the correct platform for Linux builds; macOS builds run natively.

set -euo pipefail

source "$(dirname "$0")/lib/common.sh"

parse_target_arg "$@"

log_header "Building Ruby FFI Extension"
log_info "Target: ${TARGET}"

# Ensure artifact output directory exists
DEST_DIR="$(ensure_artifact_dir "$TARGET" "ruby")"

# Verify Ruby and bundler are available
if ! command -v ruby &>/dev/null; then
    die "Ruby not found"
fi
if ! command -v bundle &>/dev/null; then
    die "Bundler not found"
fi

log_info "Ruby version: $(ruby --version)"
log_info "Bundler version: $(bundle --version)"

cd "${FFI_REPO_ROOT}/workers/ruby"

# Install Ruby dependencies if needed
if [[ ! -d "vendor/bundle" ]] && [[ ! -f "${BUNDLE_PATH:-/nonexistent}/.bundle/config" ]]; then
    log_info "Installing Ruby dependencies..."
    bundle install --jobs 4
fi

# Set rb_sys to use release profile
export RB_SYS_CARGO_PROFILE=release
export RB_SYS_CARGO_BUILD_ARGS="--locked"
export SQLX_OFFLINE=true

log_info "Running: bundle exec rake compile"
bundle exec rake compile

# Find the compiled extension
EXT="$(ruby_extension)"
LIB_NAME="tasker_rb.${EXT}"
DEST_NAME="tasker_rb-${TARGET}.${EXT}"

# rb_sys places the compiled extension in lib/tasker_core/
SRC_PATH="${FFI_REPO_ROOT}/workers/ruby/lib/tasker_core/${LIB_NAME}"

if [[ ! -f "$SRC_PATH" ]]; then
    # Also check the target directory for release builds
    RELEASE_DIR="$(cargo_release_dir "$TARGET")"
    ALT_SRC="${RELEASE_DIR}/${LIB_NAME}"
    if [[ -f "$ALT_SRC" ]]; then
        SRC_PATH="$ALT_SRC"
    else
        die "Compiled extension not found. Checked: ${SRC_PATH} and ${ALT_SRC:-none}"
    fi
fi

cp "$SRC_PATH" "${DEST_DIR}/${DEST_NAME}"
log_info "Copied: ${LIB_NAME} -> ${DEST_DIR}/${DEST_NAME}"

log_info "Ruby build complete: ${DEST_DIR}/${DEST_NAME}"
