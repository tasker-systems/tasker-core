#!/usr/bin/env bash
# scripts/release/release.sh
#
# Tasker release orchestrator.
#
# Usage:
#   ./scripts/release/release.sh [--dry-run] [--from TAG]
#
# --dry-run  Show what would happen without modifying any files or creating tags.
# --from TAG Override the base reference for change detection (default: last release tag).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "${SCRIPT_DIR}/lib/common.sh"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
DRY_RUN=false
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run) DRY_RUN=true; shift ;;
        --from)    EXTRA_ARGS+=(--from "$2"); shift 2 ;;
        --from=*)  EXTRA_ARGS+=(--from "${1#*=}"); shift ;;
        *) die "Unknown argument: $1. Usage: $0 [--dry-run] [--from TAG]" ;;
    esac
done

log_header "Tasker Release Manager"

# ---------------------------------------------------------------------------
# Pre-flight checks
# ---------------------------------------------------------------------------
log_section "Pre-flight checks"

if ! git diff-index --quiet HEAD -- 2>/dev/null; then
    if [[ "$DRY_RUN" == "true" ]]; then
        log_warn "Uncommitted changes detected (ignored in dry-run mode)"
    else
        die "Uncommitted changes detected. Commit or stash first."
    fi
else
    log_info "Working tree is clean"
fi

BRANCH=$(git branch --show-current)
log_info "Current branch: $BRANCH"

if [[ "$BRANCH" != "main" && "$DRY_RUN" == "false" ]]; then
    log_warn "On branch '$BRANCH', not 'main'"
    confirm "Continue anyway?"
fi

# ---------------------------------------------------------------------------
# Change detection + version calculation (single pass)
# ---------------------------------------------------------------------------
log_section "Detecting changes and calculating versions"

eval "$("${SCRIPT_DIR}/calculate-versions.sh" "${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"}")"

log_info "Base ref: ${CHANGES_BASE_REF}"
log_info "FFI core changed: ${FFI_CORE_CHANGED}"
log_info "Server core changed: ${SERVER_CORE_CHANGED}"
log_info "Ruby changed: ${RUBY_CHANGED}"
log_info "Python changed: ${PYTHON_CHANGED}"
log_info "TypeScript changed: ${TYPESCRIPT_CHANGED}"

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
log_section "Release Summary"

echo ""
echo "  Core version:       ${CURRENT_CORE_VERSION} -> ${NEXT_CORE_VERSION}"
echo ""
echo "  Rust crates to publish:"
if [[ "$CORE_CHANGED" == "true" ]]; then
    echo "    Phase 1: tasker-pgmq"
    echo "    Phase 2: tasker-shared"
    echo "    Phase 3: tasker-client, tasker-orchestration"
    echo "    Phase 4: tasker-worker, tasker-ctl"
else
    echo "    (none -- no core changes detected)"
fi

echo ""
echo "  FFI bindings to publish:"
for lang in ruby python typescript; do
    LANG_UPPER=$(echo "$lang" | tr '[:lower:]' '[:upper:]')
    VERSION_VAR="NEXT_${LANG_UPPER}_VERSION"
    VERSION_VAL="${!VERSION_VAR}"
    if [[ "$VERSION_VAL" != "unchanged" ]]; then
        REASON=""
        if [[ "$FFI_CORE_CHANGED" == "true" ]]; then
            REASON=" (core changed, reset to .0)"
        else
            REASON=" (binding-only change)"
        fi
        printf "    %-14s %s%s\n" "${lang}:" "${VERSION_VAL}" "${REASON}"
    else
        printf "    %-14s %s\n" "${lang}:" "(unchanged)"
    fi
done

# ---------------------------------------------------------------------------
# Dry-run: show file changes and exit
# ---------------------------------------------------------------------------
if [[ "$DRY_RUN" == "true" ]]; then
    echo ""
    log_info "DRY RUN -- no files modified, no tags created"

    log_section "Files that would be modified"

    UPDATE_ARGS="--core ${NEXT_CORE_VERSION} --dry-run"
    [[ "${NEXT_RUBY_VERSION}" != "unchanged" ]] && UPDATE_ARGS+=" --ruby ${NEXT_RUBY_VERSION}"
    [[ "${NEXT_PYTHON_VERSION}" != "unchanged" ]] && UPDATE_ARGS+=" --python ${NEXT_PYTHON_VERSION}"
    [[ "${NEXT_TYPESCRIPT_VERSION}" != "unchanged" ]] && UPDATE_ARGS+=" --typescript ${NEXT_TYPESCRIPT_VERSION}"

    # shellcheck disable=SC2086
    "${SCRIPT_DIR}/update-versions.sh" ${UPDATE_ARGS}

    echo ""
    log_info "End of dry run. No changes were made."
    exit 0
fi

# ---------------------------------------------------------------------------
# Apply version changes
# ---------------------------------------------------------------------------
log_section "Updating version files"

UPDATE_ARGS="--core ${NEXT_CORE_VERSION}"
[[ "${NEXT_RUBY_VERSION}" != "unchanged" ]] && UPDATE_ARGS+=" --ruby ${NEXT_RUBY_VERSION}"
[[ "${NEXT_PYTHON_VERSION}" != "unchanged" ]] && UPDATE_ARGS+=" --python ${NEXT_PYTHON_VERSION}"
[[ "${NEXT_TYPESCRIPT_VERSION}" != "unchanged" ]] && UPDATE_ARGS+=" --typescript ${NEXT_TYPESCRIPT_VERSION}"

# shellcheck disable=SC2086
"${SCRIPT_DIR}/update-versions.sh" ${UPDATE_ARGS}

# ---------------------------------------------------------------------------
# Create release tag
# ---------------------------------------------------------------------------
TAG="release-$(date +%Y%m%d-%H%M)"
git tag "$TAG"

log_section "Ready"
echo ""
echo "  Tag created: ${TAG}"
echo "  Core version: ${NEXT_CORE_VERSION}"
echo ""
echo "  To trigger CI release:"
echo "    git push origin ${TAG}"
echo ""
echo "  To abort:"
echo "    git tag -d ${TAG}"
echo ""
