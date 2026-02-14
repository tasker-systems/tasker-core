#!/usr/bin/env bash
# scripts/release/publish-ruby.sh
#
# Build and publish Ruby gem(s) to RubyGems.
#
# Usage:
#   ./scripts/release/publish-ruby.sh VERSION [--dry-run] [--on-duplicate=skip|warn|fail] \
#       [--artifacts-dir DIR]
#
# When --artifacts-dir is provided, builds platform gems from pre-compiled FFI
# artifacts (3 platform gems + 1 source gem). Otherwise builds just the source gem.
#
# Requires GEM_HOST_API_KEY for local publishing (skipped in dry-run and CI).

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
ARTIFACTS_DIR=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run)          DRY_RUN=true; shift ;;
        --on-duplicate=*)   ON_DUPLICATE="${1#*=}"; shift ;;
        --artifacts-dir)    ARTIFACTS_DIR="$2"; shift 2 ;;
        --artifacts-dir=*)  ARTIFACTS_DIR="${1#*=}"; shift ;;
        *)                  die "Unknown argument: $1" ;;
    esac
done

[[ -z "$VERSION" ]] && die "Usage: $0 VERSION [--dry-run] [--on-duplicate=skip|warn|fail] [--artifacts-dir DIR]"

GEM_NAME="tasker-rb"

log_header "Publish Ruby Gem (RubyGems)"
log_info "Package: ${GEM_NAME}"
log_info "Version: ${VERSION}"
log_info "Dry run: ${DRY_RUN}"
log_info "On duplicate: ${ON_DUPLICATE}"
log_info "Artifacts dir: ${ARTIFACTS_DIR:-none (source-only)}"

# ---------------------------------------------------------------------------
# Pre-flight: verify credentials
# ---------------------------------------------------------------------------
if [[ "$DRY_RUN" != "true" && -z "${GITHUB_ACTIONS:-}" ]]; then
    require_env "GEM_HOST_API_KEY" "RubyGems publishing"
fi

# ---------------------------------------------------------------------------
# Build gems
# ---------------------------------------------------------------------------
if [[ -n "$ARTIFACTS_DIR" ]]; then
    # Build platform gems + source gem from pre-built artifacts
    log_section "Building platform gems from pre-built artifacts"
    "${SCRIPT_DIR}/build-ruby-gems.sh" "$VERSION" "$ARTIFACTS_DIR"
    GEM_DIR="${REPO_ROOT}/gem-output"
else
    # Source gem only (requires Rust toolchain)
    log_section "Building source gem (no pre-built artifacts)"
    cd "${REPO_ROOT}/workers/ruby"
    gem build "${GEM_NAME}.gemspec"
    GEM_DIR="${REPO_ROOT}/workers/ruby"
fi

# ---------------------------------------------------------------------------
# Publish
# ---------------------------------------------------------------------------
log_section "Publishing gems"

publish_gem() {
    local gem_file="$1"
    local gem_basename
    gem_basename="$(basename "$gem_file")"

    if [[ "$DRY_RUN" == "true" ]]; then
        log_info "[dry-run] Would publish ${gem_basename}"
        gem specification "$gem_file" | head -20
        return
    fi

    if gem_exists_on_registry "$GEM_NAME" "$VERSION"; then
        handle_duplicate "$ON_DUPLICATE" "$GEM_NAME" "$VERSION" "RubyGems"
        return
    fi

    log_info "Publishing ${gem_basename}..."
    gem push "$gem_file"
}

# Publish platform gems first (if they exist)
for gem_file in "${GEM_DIR}"/${GEM_NAME}-${VERSION}-*.gem; do
    [[ -f "$gem_file" ]] && publish_gem "$gem_file"
done

# Publish source gem
SOURCE_GEM="${GEM_DIR}/${GEM_NAME}-${VERSION}.gem"
if [[ -f "$SOURCE_GEM" ]]; then
    publish_gem "$SOURCE_GEM"
fi

log_section "Done"
if [[ "$DRY_RUN" == "true" ]]; then
    log_info "Dry run complete — gems were not published"
else
    log_info "Gems published successfully"
fi
