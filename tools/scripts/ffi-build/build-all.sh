#!/usr/bin/env bash
# scripts/ffi-build/build-all.sh
# Orchestrator for building all FFI libraries for a target architecture.
#
# Usage:
#   ./scripts/ffi-build/build-all.sh [--target TARGET_TRIPLE] [--language python|typescript|ruby]
#
# If --target is not specified, builds for the native architecture.
# If --language is specified, builds only that language.
# Builds sequentially to maximize sccache hit rates (shared rlibs).

set -euo pipefail

source "$(dirname "$0")/lib/common.sh"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
TARGET=""
LANGUAGE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --target)
            TARGET="$2"
            shift 2
            ;;
        --language)
            LANGUAGE="$2"
            shift 2
            ;;
        *)
            die "Unknown argument: $1. Usage: build-all.sh [--target T] [--language python|typescript|ruby]"
            ;;
    esac
done

if [[ -z "$TARGET" ]]; then
    TARGET="$(detect_arch)"
    log_info "No --target specified, using native: ${TARGET}"
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
log_header "FFI Cross-Architecture Build"
log_info "Target:    ${TARGET}"
log_info "Language:  ${LANGUAGE:-all}"
log_info "Artifacts: ${ARTIFACTS_DIR}"

# Configure sccache once for all builds
configure_sccache

# Setup cargo environment
setup_cargo_env "$TARGET"

FAILED=0

build_language() {
    local lang="$1"
    local script="${SCRIPT_DIR}/build-${lang}.sh"

    if [[ ! -x "$script" ]]; then
        die "Build script not found or not executable: ${script}"
    fi

    log_section "Building ${lang} FFI library"
    if "$script" --target "$TARGET"; then
        log_info "${lang} build succeeded"
    else
        log_error "${lang} build FAILED"
        FAILED=$((FAILED + 1))
    fi
}

if [[ -n "$LANGUAGE" ]]; then
    case "$LANGUAGE" in
        python|typescript|ruby)
            build_language "$LANGUAGE"
            ;;
        *)
            die "Unknown language: ${LANGUAGE}. Must be python, typescript, or ruby."
            ;;
    esac
else
    # Build all three sequentially for maximum sccache reuse.
    # The first build compiles ~709 shared rlibs into sccache.
    # Subsequent builds get near-100% cache hits, compiling only
    # 4-6 language-specific crates.
    build_language "python"
    build_language "typescript"
    build_language "ruby"
fi

# Show sccache statistics
show_sccache_stats

# Verify artifacts
log_section "Verifying artifacts"
VERIFY_ARGS=(--target "$TARGET")
if [[ -n "$LANGUAGE" ]]; then
    VERIFY_ARGS+=(--language "$LANGUAGE")
fi

if "${SCRIPT_DIR}/verify-artifacts.sh" "${VERIFY_ARGS[@]}"; then
    log_info "Artifact verification passed"
else
    log_error "Artifact verification FAILED"
    FAILED=$((FAILED + 1))
fi

# Summary
log_header "Build Summary"
log_info "Target: ${TARGET}"
if [[ "$FAILED" -gt 0 ]]; then
    die "${FAILED} build(s) failed"
fi
log_info "All builds completed successfully"
