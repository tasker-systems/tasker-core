#!/usr/bin/env bash
# scripts/ffi-build/lib/common.sh
# Shared functions for FFI cross-architecture build tooling.
#
# Source this from other ffi-build scripts:
#   source "$(dirname "$0")/lib/common.sh"
#
# Designed for bash 3.2+ compatibility (macOS default).

set -euo pipefail

# Resolve repo root relative to this file (lib/ -> ffi-build/ -> scripts/ -> repo root)
FFI_REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

# Default artifacts directory (overridable via ARTIFACTS_DIR env var)
ARTIFACTS_DIR="${ARTIFACTS_DIR:-${FFI_REPO_ROOT}/artifacts}"

# ---------------------------------------------------------------------------
# Logging (matches scripts/release/lib/common.sh style)
# ---------------------------------------------------------------------------
log_info()    { echo "  [info] $*"; }
log_warn()    { echo "  [warn] $*" >&2; }
log_error()   { echo "  [error] $*" >&2; }
log_header()  { echo ""; echo "== $* =="; echo ""; }
log_section() { echo ""; echo "-- $* --"; }

die() { log_error "$*"; exit 1; }

# ---------------------------------------------------------------------------
# Architecture detection
# ---------------------------------------------------------------------------

# Returns a Rust target triple based on the current platform.
# Can be overridden by passing --target <triple> to build scripts.
detect_arch() {
    local machine os
    machine="$(uname -m)"
    os="$(uname -s)"

    case "${os}" in
        Linux)
            case "${machine}" in
                x86_64)  echo "x86_64-unknown-linux-gnu" ;;
                aarch64) echo "aarch64-unknown-linux-gnu" ;;
                arm64)   echo "aarch64-unknown-linux-gnu" ;;
                *)       die "Unsupported Linux architecture: ${machine}" ;;
            esac
            ;;
        Darwin)
            case "${machine}" in
                x86_64)  echo "x86_64-apple-darwin" ;;
                arm64)   echo "aarch64-apple-darwin" ;;
                aarch64) echo "aarch64-apple-darwin" ;;
                *)       die "Unsupported macOS architecture: ${machine}" ;;
            esac
            ;;
        *)
            die "Unsupported OS: ${os}"
            ;;
    esac
}

# Returns the shared library extension for the current OS
lib_extension() {
    case "$(uname -s)" in
        Darwin) echo "dylib" ;;
        Linux)  echo "so" ;;
        *)      die "Unsupported OS for lib extension: $(uname -s)" ;;
    esac
}

# Returns the Ruby extension extension for the current OS
ruby_extension() {
    case "$(uname -s)" in
        Darwin) echo "bundle" ;;
        Linux)  echo "so" ;;
        *)      die "Unsupported OS for ruby extension: $(uname -s)" ;;
    esac
}

# ---------------------------------------------------------------------------
# Artifact directory management
# ---------------------------------------------------------------------------

# Returns the artifact directory for a given target and language.
# Usage: artifact_dir <target_triple> <language>
artifact_dir() {
    local target="$1" language="$2"
    echo "${ARTIFACTS_DIR}/${target}/${language}"
}

# Creates the artifact directory for a given target and language.
# Usage: ensure_artifact_dir <target_triple> <language>
ensure_artifact_dir() {
    local target="$1" language="$2"
    local dir
    dir="$(artifact_dir "$target" "$language")"
    mkdir -p "$dir"
    echo "$dir"
}

# ---------------------------------------------------------------------------
# sccache configuration
# ---------------------------------------------------------------------------

# Configures sccache as the Rust compiler wrapper if available.
# Sets RUSTC_WRAPPER and logs cache configuration.
configure_sccache() {
    if command -v sccache &>/dev/null; then
        export RUSTC_WRAPPER=sccache
        log_info "sccache enabled ($(sccache --version 2>/dev/null || echo 'unknown version'))"

        # Log cache configuration
        if [[ -n "${SCCACHE_DIR:-}" ]]; then
            log_info "sccache dir: ${SCCACHE_DIR}"
        fi
        if [[ "${SCCACHE_GHA_ENABLED:-}" == "true" ]]; then
            log_info "sccache GHA backend enabled"
        fi

        # Zero out stats for this build session
        sccache --zero-stats 2>/dev/null || true
    else
        log_warn "sccache not found, builds will not be cached"
    fi
}

# Displays sccache statistics after a build.
show_sccache_stats() {
    if command -v sccache &>/dev/null && [[ -n "${RUSTC_WRAPPER:-}" ]]; then
        log_section "sccache Statistics"
        sccache --show-stats 2>/dev/null || true
    fi
}

# ---------------------------------------------------------------------------
# Cargo environment setup
# ---------------------------------------------------------------------------

# Sets environment variables for Rust compilation.
# Usage: setup_cargo_env [target_triple]
setup_cargo_env() {
    local target="${1:-}"

    # Always build with offline SQLx (cached queries)
    export SQLX_OFFLINE=true

    # Set target for cross-compilation if specified
    if [[ -n "$target" ]]; then
        export CARGO_BUILD_TARGET="$target"
        log_info "CARGO_BUILD_TARGET=${target}"
    fi

    log_info "SQLX_OFFLINE=true"
}

# Returns the cargo target directory for a given target triple.
# When CARGO_BUILD_TARGET is set, output goes to target/<triple>/release/
# Otherwise it goes to target/release/
cargo_release_dir() {
    local target="${1:-}"
    if [[ -n "$target" ]]; then
        echo "${FFI_REPO_ROOT}/target/${target}/release"
    else
        echo "${FFI_REPO_ROOT}/target/release"
    fi
}

# ---------------------------------------------------------------------------
# Argument parsing helper
# ---------------------------------------------------------------------------

# Parses --target <triple> from script arguments.
# Sets TARGET variable. Defaults to native arch if not specified.
# Usage: parse_target_arg "$@"
parse_target_arg() {
    TARGET=""
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --target)
                TARGET="$2"
                shift 2
                ;;
            *)
                shift
                ;;
        esac
    done

    if [[ -z "$TARGET" ]]; then
        TARGET="$(detect_arch)"
        log_info "No --target specified, using native: ${TARGET}"
    else
        log_info "Target: ${TARGET}"
    fi
}
