#!/usr/bin/env bash
# scripts/release/update-versions.sh
#
# Update all version files across the Tasker workspace.
#
# Usage:
#   ./scripts/release/update-versions.sh --core 0.1.9 [--ruby 0.1.9.0] \
#       [--python 0.1.9.0] [--typescript 0.1.9.0] [--dry-run]
#
# --core is always required. Language binding versions are optional and only
# needed when those bindings are being published.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "${SCRIPT_DIR}/lib/common.sh"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
CORE_VERSION=""
RUBY_VERSION=""
PYTHON_VERSION=""
TS_VERSION=""
_DRY_RUN=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --core)        CORE_VERSION="$2"; shift 2 ;;
        --core=*)      CORE_VERSION="${1#*=}"; shift ;;
        --ruby)        RUBY_VERSION="$2"; shift 2 ;;
        --ruby=*)      RUBY_VERSION="${1#*=}"; shift ;;
        --python)      PYTHON_VERSION="$2"; shift 2 ;;
        --python=*)    PYTHON_VERSION="${1#*=}"; shift ;;
        --typescript)  TS_VERSION="$2"; shift 2 ;;
        --typescript=*) TS_VERSION="${1#*=}"; shift ;;
        --dry-run)     _DRY_RUN=true; shift ;;
        *) die "Unknown argument: $1" ;;
    esac
done

if [[ -z "$CORE_VERSION" ]]; then
    die "Usage: $0 --core VERSION [--ruby VER] [--python VER] [--typescript VER] [--dry-run]"
fi

# ---------------------------------------------------------------------------
# Core version files
# ---------------------------------------------------------------------------
log_section "Core Rust crates"

update_version_file "$CORE_VERSION"
update_cargo_version "Cargo.toml" "$CORE_VERSION"

for crate in tasker-pgmq tasker-shared tasker-client tasker-ctl tasker-orchestration tasker-worker tasker-tooling tasker-mcp; do
    update_cargo_version "${crate}/Cargo.toml" "$CORE_VERSION"
done

# Update inter-crate dependency version fields (if they exist)
update_workspace_dep_versions "$CORE_VERSION"

# ---------------------------------------------------------------------------
# FFI Rust crate Cargo.toml versions (not published to crates.io, but
# the version field should track core for consistency)
# ---------------------------------------------------------------------------
log_section "FFI Rust crates (version tracking)"

for ffi_crate in workers/ruby/ext/tasker_core workers/python workers/typescript; do
    update_cargo_version "${ffi_crate}/Cargo.toml" "$CORE_VERSION"
done

# ---------------------------------------------------------------------------
# Ruby binding
# ---------------------------------------------------------------------------
if [[ -n "$RUBY_VERSION" ]]; then
    log_section "Ruby binding"
    update_ruby_version "$RUBY_VERSION"
fi

# ---------------------------------------------------------------------------
# Ruby ext Cargo.toml dependency pins (tasker-shared, tasker-worker, tasker-core)
# These use explicit crates.io versions instead of workspace deps so the
# source gem can build outside the workspace.
# ---------------------------------------------------------------------------
log_section "Ruby ext Cargo.toml dependency pins"
update_ruby_cargo_dep_pins "$CORE_VERSION"

# ---------------------------------------------------------------------------
# Python binding
# ---------------------------------------------------------------------------
if [[ -n "$PYTHON_VERSION" ]]; then
    log_section "Python binding"
    update_python_version "$PYTHON_VERSION"
fi

# ---------------------------------------------------------------------------
# TypeScript binding
# ---------------------------------------------------------------------------
if [[ -n "$TS_VERSION" ]]; then
    log_section "TypeScript binding"
    update_typescript_version "$TS_VERSION"
fi
