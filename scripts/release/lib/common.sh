#!/usr/bin/env bash
# scripts/release/lib/common.sh
# Shared functions for Tasker release tooling.
#
# Source this from other release scripts:
#   source "$(dirname "$0")/lib/common.sh"
#
# Expects callers to set DRY_RUN=true|false before calling file-update functions.

set -euo pipefail

# Resolve repo root relative to this file (lib/ -> release/ -> scripts/ -> repo root)
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------
log_info()    { echo "  [info] $*"; }
log_warn()    { echo "  [warn] $*" >&2; }
log_error()   { echo "  [error] $*" >&2; }
log_header()  { echo ""; echo "== $* =="; echo ""; }
log_section() { echo ""; echo "-- $* --"; }

die() { log_error "$*"; exit 1; }

confirm() {
    read -p "  $1 (y/N) " -n 1 -r
    echo
    [[ $REPLY =~ ^[Yy]$ ]] || exit 1
}

# ---------------------------------------------------------------------------
# Portable sed -i (GNU vs BSD/macOS)
# ---------------------------------------------------------------------------
# macOS sed requires `sed -i ''` while GNU sed uses `sed -i`.
# This wrapper handles the difference transparently.
sed_i() {
    if sed --version 2>/dev/null | grep -q 'GNU'; then
        sed -i "$@"
    else
        sed -i '' "$@"
    fi
}

# ---------------------------------------------------------------------------
# Version arithmetic
# ---------------------------------------------------------------------------

# Bump the patch component: 0.1.8 -> 0.1.9
bump_patch() {
    local version="$1"
    local major minor patch
    IFS='.' read -r major minor patch <<< "$version"
    echo "${major}.${minor}.$((patch + 1))"
}

# Parse binding version's patch component: 0.1.8.2 -> 2
binding_patch() {
    local version="$1"
    echo "${version##*.}"
}

# ---------------------------------------------------------------------------
# File update helpers
#
# All functions respect the DRY_RUN variable from the caller's scope.
# ---------------------------------------------------------------------------

update_version_file() {
    local version="$1"
    local file="${REPO_ROOT}/VERSION"
    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Would update VERSION -> $version"
    else
        echo "$version" > "$file"
        log_info "Updated VERSION -> $version"
    fi
}

# Update the top-level `version = "..."` in a Cargo.toml.
# Only touches the first occurrence (the [package] version).
update_cargo_version() {
    local file="$1" version="$2"

    # Resolve relative to repo root if not absolute
    [[ "$file" != /* ]] && file="${REPO_ROOT}/${file}"

    if [[ ! -f "$file" ]]; then
        log_warn "File not found: $file"
        return
    fi

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        local current
        current=$(grep -m1 '^version = ' "$file" | sed 's/version = "\(.*\)"/\1/')
        log_info "Would update $file version: $current -> $version"
    else
        # Use sed to replace only the first `version = "..."` line
        sed_i '0,/^version = ".*"/{s/^version = ".*"/version = "'"$version"'"/}' "$file"
        log_info "Updated $file -> $version"
    fi
}

# Update inter-crate dependency version fields in all workspace Cargo.toml files.
#
# Two patterns exist in the codebase:
#   1. Workspace-level:  tasker-pgmq = { path = "tasker-pgmq" }
#   2. Crate-level:      tasker-shared = { path = "../tasker-shared" }
#      or:               tasker-shared = { package = "tasker-shared", path = "..." }
#
# For publishing, these need version fields:
#   tasker-pgmq = { path = "tasker-pgmq", version = "=0.1.0" }
#
# This function handles both adding missing version fields and updating existing ones.
update_workspace_dep_versions() {
    local version="$1"

    # List of crate names that are part of the publishable workspace
    local -a WORKSPACE_CRATES=(
        tasker-pgmq
        tasker-shared
        tasker-client
        tasker-cli
        tasker-orchestration
        tasker-worker
    )

    # Find all Cargo.toml files in the workspace
    local -a toml_files
    mapfile -t toml_files < <(find "$REPO_ROOT" -name Cargo.toml -not -path '*/target/*' -not -path '*/.cargo/*')

    local changes_found=false

    for toml_file in "${toml_files[@]}"; do
        for crate in "${WORKSPACE_CRATES[@]}"; do
            # Skip self-references (a crate doesn't depend on itself)
            local _crate_dir
            _crate_dir=$(basename "$(dirname "$toml_file")")

            # Match lines that reference this crate with a path but have a version field
            # Pattern: tasker-pgmq = { ... version = "..." ... }
            if grep -q "^${crate} = {.*path = .*version = " "$toml_file" 2>/dev/null; then
                changes_found=true
                if [[ "${DRY_RUN:-false}" == "true" ]]; then
                    log_info "Would update dep $crate version in $toml_file -> =$version"
                else
                    sed_i "s/\(${crate} = {.*\)version = \"=[^\"]*\"/\1version = \"=${version}\"/" "$toml_file"
                fi
            fi
        done
    done

    if [[ "$changes_found" == "false" ]]; then
        log_info "No inter-crate version fields found (version fields are a Phase 2 prerequisite)"
    fi
}

update_ruby_version() {
    local binding_version="$1" core_version="$2"
    local file="${REPO_ROOT}/workers/ruby/lib/tasker_core/version.rb"

    if [[ ! -f "$file" ]]; then
        log_warn "Ruby version file not found: $file"
        return
    fi

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        log_info "Would update $file -> VERSION='$binding_version', RUST_CORE_VERSION='$core_version'"
    else
        sed_i "s/\(  VERSION = '\)[^']*'/\1${binding_version}'/" "$file"
        sed_i "s/\(  Version = '\)[^']*'/\1${binding_version}'/" "$file"
        sed_i "s/\(  RUST_CORE_VERSION = '\)[^']*'/\1${core_version}'/" "$file"
        log_info "Updated Ruby version -> $binding_version (core: $core_version)"
    fi
}

update_python_version() {
    local version="$1"
    local file="${REPO_ROOT}/workers/python/pyproject.toml"

    if [[ ! -f "$file" ]]; then
        log_warn "Python pyproject.toml not found: $file"
        return
    fi

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        local current
        current=$(grep -m1 '^version = ' "$file" | sed 's/version = "\(.*\)"/\1/')
        log_info "Would update $file version: $current -> $version"
    else
        sed_i '0,/^version = ".*"/{s/^version = ".*"/version = "'"$version"'"/}' "$file"
        log_info "Updated Python version -> $version"
    fi
}

update_typescript_version() {
    local version="$1"
    local file="${REPO_ROOT}/workers/typescript/package.json"

    if [[ ! -f "$file" ]]; then
        log_warn "TypeScript package.json not found: $file"
        return
    fi

    if [[ "${DRY_RUN:-false}" == "true" ]]; then
        local current
        current=$(grep -m1 '"version"' "$file" | sed 's/.*"version": "\([^"]*\)".*/\1/')
        log_info "Would update $file version: $current -> $version"
    else
        # Replace only the first "version" field (the package version, not a dependency version)
        sed_i '0,/"version": "[^"]*"/{s/"version": "[^"]*"/"version": "'"$version"'"/}' "$file"
        log_info "Updated TypeScript version -> $version"
    fi
}

# ---------------------------------------------------------------------------
# Credential verification
# ---------------------------------------------------------------------------
require_env() {
    local var_name="$1" purpose="$2"
    if [[ -z "${!var_name:-}" ]]; then
        die "Missing $var_name (required for $purpose)"
    fi
    log_info "Verified $var_name is set ($purpose)"
}

# ---------------------------------------------------------------------------
# Registry duplicate detection
# ---------------------------------------------------------------------------
crate_exists_on_registry() {
    local crate="$1" version="$2"
    local url="https://crates.io/api/v1/crates/${crate}/${version}"
    curl -sf "$url" > /dev/null 2>&1
}

gem_exists_on_registry() {
    local gem="$1" version="$2"
    local url="https://rubygems.org/api/v1/versions/${gem}.json"
    curl -sf "$url" 2>/dev/null | grep -q "\"number\":\"${version}\""
}

pypi_exists_on_registry() {
    local package="$1" version="$2"
    local url="https://pypi.org/pypi/${package}/${version}/json"
    curl -sf "$url" > /dev/null 2>&1
}

npm_exists_on_registry() {
    local package="$1" version="$2"
    npm view "${package}@${version}" version > /dev/null 2>&1
}

handle_duplicate() {
    local mode="$1" package="$2" version="$3" registry="$4"
    case "$mode" in
        skip)
            log_info "$package@$version already on $registry, skipping"
            ;;
        warn)
            log_warn "$package@$version already on $registry, skipping"
            ;;
        fail)
            die "$package@$version already exists on $registry (--on-duplicate=fail)"
            ;;
        *)
            die "Unknown --on-duplicate mode: $mode (expected skip|warn|fail)"
            ;;
    esac
}
