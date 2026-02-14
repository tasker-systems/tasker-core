#!/usr/bin/env bash
# scripts/release/publish-typescript.sh
#
# Build TypeScript package and publish to npm.
#
# Usage:
#   ./scripts/release/publish-typescript.sh VERSION [--dry-run] [--on-duplicate=skip|warn|fail]
#
# No credential check required — npm uses OIDC trusted publishing in CI
# (automatic with GitHub Actions `environment: npm` + `id-token: write`).
#
# Ships the TS SDK layer only — no Rust cdylib bundled in the npm package
# (users obtain the native library separately).
# No Rust toolchain needed for this step.
#
# --provenance enables OIDC-based provenance attestation.
# --tag alpha is used for pre-release versions.

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

NPM_PACKAGE="@tasker-systems/tasker"

log_header "Publish TypeScript Package (npm)"
log_info "Package: ${NPM_PACKAGE}"
log_info "Version: ${VERSION}"
log_info "Dry run: ${DRY_RUN}"
log_info "On duplicate: ${ON_DUPLICATE}"

# No credential check — npm uses OIDC in CI.

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
log_section "Building TypeScript package"
cd "${REPO_ROOT}/workers/typescript"

bun install
bun run build

# ---------------------------------------------------------------------------
# Determine npm tag
# ---------------------------------------------------------------------------
NPM_TAG="latest"
# if [[ "$VERSION" == *"-"* || "$VERSION" == 0.* ]]; then
#     NPM_TAG="alpha"
# fi

# ---------------------------------------------------------------------------
# Publish
# ---------------------------------------------------------------------------
if [[ "$DRY_RUN" == "true" ]]; then
    log_info "[dry-run] Would publish ${NPM_PACKAGE}@${VERSION} with tag '${NPM_TAG}'"
    npm pack --dry-run
else
    if npm_exists_on_registry "$NPM_PACKAGE" "$VERSION"; then
        handle_duplicate "$ON_DUPLICATE" "$NPM_PACKAGE" "$VERSION" "npm"
    else
        log_info "Publishing ${NPM_PACKAGE}@${VERSION} (tag: ${NPM_TAG})..."
        npm publish --provenance --access public --tag "$NPM_TAG"
    fi
fi

log_section "Done"
if [[ "$DRY_RUN" == "true" ]]; then
    log_info "Dry run complete — package was not published"
else
    log_info "Package published successfully"
fi
