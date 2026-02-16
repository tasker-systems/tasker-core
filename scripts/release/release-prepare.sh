#!/usr/bin/env bash
# scripts/release/release-prepare.sh
#
# Prepare a release branch with version bumps, then open a PR to main.
#
# Usage:
#   ./scripts/release/release-prepare.sh [--dry-run] [--from TAG]
#
# --dry-run  Show what would happen without creating a branch or PR.
# --from TAG Override the base reference for change detection.
#
# Flow:
#   1. Pre-flight checks (clean tree, on main, up-to-date, gh available)
#   2. Detect changes and calculate versions (reuses existing scripts)
#   3. Display summary
#   4. Create release branch, bump versions, commit, push, open PR

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "${SCRIPT_DIR}/lib/common.sh"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
DRY_RUN=false
YES=false
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case $1 in
        --dry-run) DRY_RUN=true; shift ;;
        --yes|-y)  YES=true; shift ;;
        --from)    EXTRA_ARGS+=(--from "$2"); shift 2 ;;
        --from=*)  EXTRA_ARGS+=(--from "${1#*=}"); shift ;;
        *) die "Unknown argument: $1. Usage: $0 [--dry-run] [--yes] [--from TAG]" ;;
    esac
done

log_header "Tasker Release Preparation"

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
if [[ "$BRANCH" != "main" ]]; then
    if [[ "$DRY_RUN" == "true" ]]; then
        log_warn "On branch '$BRANCH', not 'main' (ignored in dry-run mode)"
    else
        die "Must be on main branch (currently on '$BRANCH')"
    fi
else
    log_info "On main branch"
fi

git fetch origin --quiet
LOCAL_SHA=$(git rev-parse HEAD)
REMOTE_SHA=$(git rev-parse origin/main 2>/dev/null || echo "unknown")
if [[ "$LOCAL_SHA" != "$REMOTE_SHA" ]]; then
    if [[ "$DRY_RUN" == "true" ]]; then
        log_warn "Local branch is not up-to-date with origin/main (ignored in dry-run mode)"
    else
        die "Local main is not up-to-date with origin/main. Run: git pull"
    fi
else
    log_info "main is up-to-date with origin"
fi

if ! command -v gh &>/dev/null; then
    die "gh CLI not found. Install: https://cli.github.com/"
fi
log_info "gh CLI available"

# ---------------------------------------------------------------------------
# Change detection + version calculation (single pass)
# ---------------------------------------------------------------------------
log_section "Detecting changes and calculating versions"

eval "$("${SCRIPT_DIR}/calculate-versions.sh" "${EXTRA_ARGS[@]+"${EXTRA_ARGS[@]}"}")"

log_info "Base ref: ${CHANGES_BASE_REF}"
log_info "FFI core changed: ${FFI_CORE_CHANGED}"
log_info "Server core changed: ${SERVER_CORE_CHANGED}"
log_info "Ruby changed: ${RUBY_CHANGED} (infra: ${RUBY_INFRA_CHANGED})"
log_info "Python changed: ${PYTHON_CHANGED} (infra: ${PYTHON_INFRA_CHANGED})"
log_info "TypeScript changed: ${TYPESCRIPT_CHANGED} (infra: ${TYPESCRIPT_INFRA_CHANGED})"

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
    CURRENT_VAR="CURRENT_${LANG_UPPER}_VERSION"
    CURRENT_VAL="${!CURRENT_VAR}"
    if [[ "$VERSION_VAL" != "unchanged" ]]; then
        REASON=""
        LANG_INFRA_VAR="${LANG_UPPER}_INFRA_CHANGED"
        if [[ "$FFI_CORE_CHANGED" == "true" ]]; then
            REASON=" (core changed)"
        elif [[ "${!LANG_INFRA_VAR}" == "true" ]]; then
            REASON=" (infra changed)"
        else
            REASON=" (binding changed)"
        fi
        printf "    %-14s %s -> %s%s\n" "${lang}:" "${CURRENT_VAL}" "${VERSION_VAL}" "${REASON}"
    else
        printf "    %-14s %s (unchanged)\n" "${lang}:" "${CURRENT_VAL}"
    fi
done

# ---------------------------------------------------------------------------
# Dry-run: exit here
# ---------------------------------------------------------------------------
if [[ "$DRY_RUN" == "true" ]]; then
    echo ""
    log_info "DRY RUN -- no branch created, no PR opened"
    exit 0
fi

# ---------------------------------------------------------------------------
# Confirm
# ---------------------------------------------------------------------------
if [[ "$YES" != "true" ]]; then
    echo ""
    confirm "Create release branch and prepare PR?"
fi

# ---------------------------------------------------------------------------
# Build update-versions arguments
# ---------------------------------------------------------------------------
UPDATE_ARGS="--core ${NEXT_CORE_VERSION}"
[[ "${NEXT_RUBY_VERSION}" != "unchanged" ]] && UPDATE_ARGS+=" --ruby ${NEXT_RUBY_VERSION}"
[[ "${NEXT_PYTHON_VERSION}" != "unchanged" ]] && UPDATE_ARGS+=" --python ${NEXT_PYTHON_VERSION}"
[[ "${NEXT_TYPESCRIPT_VERSION}" != "unchanged" ]] && UPDATE_ARGS+=" --typescript ${NEXT_TYPESCRIPT_VERSION}"

# ---------------------------------------------------------------------------
# Create release branch
# ---------------------------------------------------------------------------
if [[ "$CORE_CHANGED" == "true" ]]; then
    RELEASE_BRANCH="release/v${NEXT_CORE_VERSION}"
else
    # FFI-only release: build branch name from changed packages
    FFI_SUFFIX=""
    [[ "${NEXT_RUBY_VERSION}" != "unchanged" ]] && FFI_SUFFIX+="-rb-v${NEXT_RUBY_VERSION}"
    [[ "${NEXT_PYTHON_VERSION}" != "unchanged" ]] && FFI_SUFFIX+="-py-v${NEXT_PYTHON_VERSION}"
    [[ "${NEXT_TYPESCRIPT_VERSION}" != "unchanged" ]] && FFI_SUFFIX+="-ts-v${NEXT_TYPESCRIPT_VERSION}"
    RELEASE_BRANCH="release/ffi${FFI_SUFFIX}"
fi
log_section "Creating branch: ${RELEASE_BRANCH}"

git checkout -b "$RELEASE_BRANCH"

# ---------------------------------------------------------------------------
# Bump versions
# ---------------------------------------------------------------------------
log_section "Bumping versions"

# shellcheck disable=SC2086
"${SCRIPT_DIR}/update-versions.sh" ${UPDATE_ARGS}

# ---------------------------------------------------------------------------
# Refresh lockfiles (version bumps may have changed gemspec/pyproject/package.json)
# ---------------------------------------------------------------------------
log_section "Refreshing lockfiles"

if command -v bundle &>/dev/null && [[ -f "${REPO_ROOT}/workers/ruby/Gemfile.lock" ]]; then
    (cd "${REPO_ROOT}/workers/ruby" && bundle lock --update)
    log_info "Updated Gemfile.lock"
else
    log_warn "bundle not found or Gemfile.lock missing, skipping Ruby lockfile"
fi

if command -v uv &>/dev/null && [[ -f "${REPO_ROOT}/workers/python/uv.lock" ]]; then
    (cd "${REPO_ROOT}/workers/python" && uv lock)
    log_info "Updated uv.lock"
else
    log_warn "uv not found or uv.lock missing, skipping Python lockfile"
fi

if command -v bun &>/dev/null && [[ -f "${REPO_ROOT}/workers/typescript/bun.lock" ]]; then
    (cd "${REPO_ROOT}/workers/typescript" && bun install)
    log_info "Updated bun.lock"
else
    log_warn "bun not found or bun.lock missing, skipping TypeScript lockfile"
fi

# ---------------------------------------------------------------------------
# Sanity check: verify workspace compiles
# ---------------------------------------------------------------------------
log_section "Sanity check (cargo check)"

SQLX_OFFLINE=true cargo check --all-features

# ---------------------------------------------------------------------------
# Commit
# ---------------------------------------------------------------------------
log_section "Committing changes"

git add -u

if [[ "$CORE_CHANGED" == "true" ]]; then
    COMMIT_MSG="chore(release): prepare v${NEXT_CORE_VERSION}"
else
    # FFI-only: list changed packages in commit message
    FFI_PARTS=""
    [[ "${NEXT_RUBY_VERSION}" != "unchanged" ]] && FFI_PARTS+="Ruby v${NEXT_RUBY_VERSION}, "
    [[ "${NEXT_PYTHON_VERSION}" != "unchanged" ]] && FFI_PARTS+="Python v${NEXT_PYTHON_VERSION}, "
    [[ "${NEXT_TYPESCRIPT_VERSION}" != "unchanged" ]] && FFI_PARTS+="TypeScript v${NEXT_TYPESCRIPT_VERSION}, "
    FFI_PARTS="${FFI_PARTS%, }"  # trim trailing comma
    COMMIT_MSG="chore(release): prepare FFI packages - ${FFI_PARTS}"
fi
git commit -m "$COMMIT_MSG"

# ---------------------------------------------------------------------------
# Push + PR
# ---------------------------------------------------------------------------
log_section "Pushing and creating PR"

git push -u origin "$RELEASE_BRANCH"

# Build PR title and body
if [[ "$CORE_CHANGED" == "true" ]]; then
    PR_TITLE="chore(release): prepare v${NEXT_CORE_VERSION}"
    PR_BODY="## Release v${NEXT_CORE_VERSION}"$'\n\n'
else
    PR_TITLE="chore(release): prepare FFI packages - ${FFI_PARTS}"
    PR_BODY="## FFI Package Release"$'\n\n'
fi

PR_BODY+="Prepared by \`cargo make release-prepare\`."$'\n\n'
PR_BODY+="### Version Changes"$'\n\n'
PR_BODY+="| Component | Current | Next |"$'\n'
PR_BODY+="|-----------|---------|------|"$'\n'

if [[ "$CORE_CHANGED" == "true" ]]; then
    PR_BODY+="| Rust crates | ${CURRENT_CORE_VERSION} | ${NEXT_CORE_VERSION} |"$'\n'
fi
if [[ "${NEXT_RUBY_VERSION}" != "unchanged" ]]; then
    PR_BODY+="| Ruby (tasker-rb) | ${CURRENT_RUBY_VERSION} | ${NEXT_RUBY_VERSION} |"$'\n'
fi
if [[ "${NEXT_PYTHON_VERSION}" != "unchanged" ]]; then
    PR_BODY+="| Python (tasker-py) | ${CURRENT_PYTHON_VERSION} | ${NEXT_PYTHON_VERSION} |"$'\n'
fi
if [[ "${NEXT_TYPESCRIPT_VERSION}" != "unchanged" ]]; then
    PR_BODY+="| TypeScript (@tasker-systems/tasker) | ${CURRENT_TYPESCRIPT_VERSION} | ${NEXT_TYPESCRIPT_VERSION} |"$'\n'
fi

PR_BODY+=$'\n'"### Post-Merge"$'\n\n'
PR_BODY+="After merging, trigger the release workflow:"$'\n'
PR_BODY+="\`\`\`"$'\n'
PR_BODY+="gh workflow run release.yml --ref main -f dry_run=false"$'\n'
PR_BODY+="\`\`\`"$'\n'

gh pr create \
    --title "$PR_TITLE" \
    --body "$PR_BODY" \
    --base main \
    --head "$RELEASE_BRANCH"

log_section "Done"
echo ""
echo "  Release branch: ${RELEASE_BRANCH}"
echo "  PR created -- merge to main, then trigger the release workflow."
echo ""
