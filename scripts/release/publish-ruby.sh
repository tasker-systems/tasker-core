#!/usr/bin/env bash
# scripts/release/publish-ruby.sh
#
# Build native gem and publish to RubyGems.
#
# Usage:
#   ./scripts/release/publish-ruby.sh VERSION [--dry-run] [--on-duplicate=skip|warn|fail]
#
# The gem ships source — users compile the native extension at install time.
# Requires GEM_HOST_API_KEY (skipped in dry-run mode).
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

GEM_NAME="tasker-rb"

log_header "Publish Ruby Gem (RubyGems)"
log_info "Package: ${GEM_NAME}"
log_info "Version: ${VERSION}"
log_info "Dry run: ${DRY_RUN}"
log_info "On duplicate: ${ON_DUPLICATE}"

# ---------------------------------------------------------------------------
# Pre-flight: verify credentials
# ---------------------------------------------------------------------------
# In GitHub Actions, OIDC trusted publishing handles auth via rubygems/release-gem.
# GEM_HOST_API_KEY is only required for local/manual publishing.
if [[ "$DRY_RUN" != "true" && -z "${GITHUB_ACTIONS:-}" ]]; then
    require_env "GEM_HOST_API_KEY" "RubyGems publishing"
fi

# ---------------------------------------------------------------------------
# Build
# ---------------------------------------------------------------------------
log_section "Building native extension"
cd "${REPO_ROOT}/workers/ruby"

bundle exec rake compile

log_section "Building gem"
gem build "${GEM_NAME}.gemspec"

# ---------------------------------------------------------------------------
# Publish
# ---------------------------------------------------------------------------
GEM_FILE="${GEM_NAME}-${VERSION}.gem"

if [[ ! -f "$GEM_FILE" ]]; then
    die "Expected gem file not found: ${GEM_FILE}"
fi

if [[ "$DRY_RUN" == "true" ]]; then
    log_info "[dry-run] Would publish ${GEM_FILE}"
    log_info "[dry-run] Gem contents:"
    gem specification "$GEM_FILE" | head -20
else
    if gem_exists_on_registry "$GEM_NAME" "$VERSION"; then
        handle_duplicate "$ON_DUPLICATE" "$GEM_NAME" "$VERSION" "RubyGems"
    else
        log_info "Publishing ${GEM_FILE}..."
        gem push "$GEM_FILE"
    fi
fi

log_section "Done"
if [[ "$DRY_RUN" == "true" ]]; then
    log_info "Dry run complete — gem was not published"
else
    log_info "Gem published successfully"
fi
