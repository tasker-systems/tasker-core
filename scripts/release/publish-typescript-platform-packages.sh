#!/usr/bin/env bash
# scripts/release/publish-typescript-platform-packages.sh
#
# Publish TypeScript platform-specific npm packages containing pre-built
# native libraries.
#
# Usage:
#   ./scripts/release/publish-typescript-platform-packages.sh VERSION ARTIFACTS_DIR \
#       [--dry-run] [--on-duplicate=skip|warn|fail]
#
# Each platform package contains a single native library file. npm's os/cpu
# constraints ensure users only download the package matching their platform.
#
# Must be published BEFORE the main @tasker-systems/tasker package so that
# optionalDependencies can resolve during npm install.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "${SCRIPT_DIR}/lib/common.sh"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
VERSION="${1:-}"
shift || true
ARTIFACTS_DIR="${1:-}"
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

[[ -z "$VERSION" ]] && die "Usage: $0 VERSION ARTIFACTS_DIR [--dry-run] [--on-duplicate=skip|warn|fail]"
[[ -z "$ARTIFACTS_DIR" ]] && die "Usage: $0 VERSION ARTIFACTS_DIR [--dry-run] [--on-duplicate=skip|warn|fail]"
[[ ! -d "$ARTIFACTS_DIR" ]] && die "Artifacts directory not found: $ARTIFACTS_DIR"

TS_NPM_DIR="${REPO_ROOT}/workers/typescript/npm"

log_header "Publish TypeScript Platform Packages (npm)"
log_info "Version: ${VERSION}"
log_info "Artifacts: ${ARTIFACTS_DIR}"
log_info "Dry run: ${DRY_RUN}"

# ---------------------------------------------------------------------------
# Platform mapping: Rust triple -> npm package dir + library filename
# ---------------------------------------------------------------------------
# Format: "triple|package_dir|artifact_name|library_name"
PLATFORMS=(
    "x86_64-unknown-linux-gnu|tasker-linux-x64|libtasker_ts-x86_64-unknown-linux-gnu.so|libtasker_ts.so"
    "aarch64-unknown-linux-gnu|tasker-linux-arm64|libtasker_ts-aarch64-unknown-linux-gnu.so|libtasker_ts.so"
    "aarch64-apple-darwin|tasker-darwin-arm64|libtasker_ts-aarch64-apple-darwin.dylib|libtasker_ts.dylib"
)

PUBLISHED=0
SKIPPED=0

for entry in "${PLATFORMS[@]}"; do
    IFS='|' read -r triple pkg_dir artifact_name lib_name <<< "$entry"

    NPM_PKG_DIR="${TS_NPM_DIR}/${pkg_dir}"
    PKG_NAME="@tasker-systems/${pkg_dir}"
    ARTIFACT="${ARTIFACTS_DIR}/typescript/${artifact_name}"

    log_section "Platform: ${pkg_dir}"

    if [[ ! -f "$ARTIFACT" ]]; then
        log_warn "No artifact for ${triple}: ${ARTIFACT} — skipping"
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    if [[ ! -d "$NPM_PKG_DIR" ]]; then
        log_warn "Package directory not found: ${NPM_PKG_DIR} — skipping"
        SKIPPED=$((SKIPPED + 1))
        continue
    fi

    # Copy native library into package directory
    cp "$ARTIFACT" "${NPM_PKG_DIR}/${lib_name}"
    log_info "Copied: ${artifact_name} -> ${NPM_PKG_DIR}/${lib_name}"

    # Update version in package.json
    local_pkg_json="${NPM_PKG_DIR}/package.json"
    local line_num
    line_num=$(grep -n -m1 '"version"' "$local_pkg_json" | cut -d: -f1)
    if [[ -n "$line_num" ]]; then
        sed_i "${line_num}s/\"version\": \"[^\"]*\"/\"version\": \"${VERSION}\"/" "$local_pkg_json"
    fi
    log_info "Set version to ${VERSION} in ${local_pkg_json}"

    # Publish
    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "[dry-run] Would publish ${PKG_NAME}@${VERSION}"
        (cd "$NPM_PKG_DIR" && npm pack --dry-run)
    else
        if npm_exists_on_registry "$PKG_NAME" "$VERSION"; then
            handle_duplicate "$ON_DUPLICATE" "$PKG_NAME" "$VERSION" "npm"
        else
            log_info "Publishing ${PKG_NAME}@${VERSION}..."
            (cd "$NPM_PKG_DIR" && npm publish --provenance --access public)
        fi
    fi

    PUBLISHED=$((PUBLISHED + 1))
done

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
log_section "Summary"
log_info "Published: ${PUBLISHED} platform packages"
if [[ "$SKIPPED" -gt 0 ]]; then
    log_warn "Skipped: ${SKIPPED} platforms (missing artifacts)"
fi
