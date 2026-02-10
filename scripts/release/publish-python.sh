#!/usr/bin/env bash
# scripts/release/publish-python.sh
#
# Build wheel via maturin and publish to PyPI.
#
# Usage:
#   ./scripts/release/publish-python.sh VERSION [--dry-run] [--on-duplicate=skip|warn|fail]
#
# No credential check required — PyPI uses OIDC trusted publishing in CI
# (automatic with GitHub Actions `environment: pypi` + `id-token: write`).
# For local testing, falls back to ~/.pypirc or MATURIN_PYPI_TOKEN.
#
# Initial release: Linux x86_64 wheel only (multi-platform matrix is future work).
# Requires SQLX_OFFLINE=true + protobuf for Rust compilation.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "${SCRIPT_DIR}/lib/common.sh"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
VERSION="${1:-}"
shift || true

DRY_RUN=false
ON_DUPLICATE="warn"

while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)          DRY_RUN=true; shift ;;
        --on-duplicate=*)   ON_DUPLICATE="${1#*=}"; shift ;;
        *)                  die "Unknown argument: $1" ;;
    esac
done

[[ -z "$VERSION" ]] && die "Usage: $0 VERSION [--dry-run] [--on-duplicate=skip|warn|fail]"

PYPI_PACKAGE="tasker-py"

log_header "Publish Python Package (PyPI)"
log_info "Package: ${PYPI_PACKAGE}"
log_info "Version: ${VERSION}"
log_info "Dry run: ${DRY_RUN}"
log_info "On duplicate: ${ON_DUPLICATE}"

# No credential check — PyPI uses OIDC in CI.
# Local publishes use ~/.pypirc or MATURIN_PYPI_TOKEN.

# ---------------------------------------------------------------------------
# Work around maturin sdist README conflict
# ---------------------------------------------------------------------------
# maturin's sdist builder includes the workspace root README.md (from the root
# Cargo.toml) AND the local workers/python/README.md — both resolve to
# "README.md" in the tarball, causing a duplicate file error.
# Fix: remove the root README before building. Each CI job gets a fresh
# checkout, so this doesn't affect other jobs.
if [[ -f "${REPO_ROOT}/README.md" && -f "${REPO_ROOT}/workers/python/README.md" ]]; then
    rm "${REPO_ROOT}/README.md"
    log_info "Removed root README.md to avoid maturin sdist conflict"
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
log_section "Building wheel"
cd "${REPO_ROOT}/workers/python"

uv run maturin build --release

# ---------------------------------------------------------------------------
# Publish
# ---------------------------------------------------------------------------
if [[ "$DRY_RUN" == "true" ]]; then
    log_info "[dry-run] Would publish ${PYPI_PACKAGE}==${VERSION}"
    log_info "[dry-run] Built wheels:"
    ls -la "${REPO_ROOT}/target/wheels/"tasker_*.whl 2>/dev/null || log_warn "No wheel files found in target/wheels/"
else
    if pypi_exists_on_registry "$PYPI_PACKAGE" "$VERSION"; then
        handle_duplicate "$ON_DUPLICATE" "$PYPI_PACKAGE" "$VERSION" "PyPI"
    else
        log_info "Publishing ${PYPI_PACKAGE}==${VERSION}..."
        uv run maturin publish
    fi
fi

log_section "Done"
if [[ "$DRY_RUN" == "true" ]]; then
    log_info "Dry run complete — package was not published"
else
    log_info "Package published successfully"
fi
