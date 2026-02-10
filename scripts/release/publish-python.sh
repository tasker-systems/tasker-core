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
# Build
# ---------------------------------------------------------------------------
log_section "Building wheel"
cd "${REPO_ROOT}/workers/python"

# Build wheel only (no sdist). maturin's sdist builder hits a README conflict:
# it includes both the workspace root README.md (via root Cargo.toml path dep)
# and workers/python/README.md — both map to "README.md" in the tarball.
# Wheel-only publishing avoids this entirely. sdist can be added later when
# maturin supports workspace-aware sdist deduplication.
# --manylinux auto: detect and apply the correct manylinux platform tag.
# Without this, `maturin build` produces a plain `linux_x86_64` wheel
# that PyPI rejects (only manylinux-tagged wheels are accepted).
uv run maturin build --release --manylinux auto

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
        log_info "Publishing ${PYPI_PACKAGE}==${VERSION} (wheel only)..."
        uv run maturin upload "${REPO_ROOT}/target/wheels/tasker_py-${VERSION}"*.whl
    fi
fi

log_section "Done"
if [[ "$DRY_RUN" == "true" ]]; then
    log_info "Dry run complete — package was not published"
else
    log_info "Package published successfully"
fi
