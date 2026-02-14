#!/usr/bin/env bash
# scripts/release/calculate-versions.sh
#
# Calculate next version numbers for all components.
#
# Usage:
#   ./scripts/release/calculate-versions.sh [--from TAG]
#
# Reads: VERSION file, git tags, output from detect-changes.sh
#
# Output:
#   KEY=VALUE pairs suitable for eval:
#     CURRENT_CORE_VERSION=0.1.0
#     NEXT_CORE_VERSION=0.1.1
#     NEXT_RUBY_VERSION=0.1.1|unchanged
#     NEXT_PYTHON_VERSION=0.1.1|unchanged
#     NEXT_TYPESCRIPT_VERSION=0.1.1|unchanged
#   Plus all variables from detect-changes.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "${SCRIPT_DIR}/lib/common.sh"

# ---------------------------------------------------------------------------
# Forward arguments to detect-changes (e.g., --from TAG)
# ---------------------------------------------------------------------------
eval "$("${SCRIPT_DIR}/detect-changes.sh" "$@")"

# ---------------------------------------------------------------------------
# Read current core version
# ---------------------------------------------------------------------------
VERSION_FILE="${REPO_ROOT}/VERSION"
if [[ ! -f "$VERSION_FILE" ]]; then
    die "VERSION file not found at ${VERSION_FILE}"
fi

CURRENT_CORE_VERSION=$(cat "$VERSION_FILE" | tr -d '[:space:]')
echo "CURRENT_CORE_VERSION=${CURRENT_CORE_VERSION}"

# ---------------------------------------------------------------------------
# Calculate next core version
# ---------------------------------------------------------------------------
if [[ "$CORE_CHANGED" == "true" ]]; then
    NEXT_CORE_VERSION=$(bump_patch "$CURRENT_CORE_VERSION")
else
    NEXT_CORE_VERSION="$CURRENT_CORE_VERSION"
fi
echo "NEXT_CORE_VERSION=${NEXT_CORE_VERSION}"

# ---------------------------------------------------------------------------
# Read current per-language versions from source files
# ---------------------------------------------------------------------------
eval "$("${SCRIPT_DIR}/read-versions.sh" 2>/dev/null)"

CURRENT_RUBY_VERSION="${RUBY_VERSION:-$CURRENT_CORE_VERSION}"
CURRENT_PYTHON_VERSION="${PYTHON_VERSION:-$CURRENT_CORE_VERSION}"
CURRENT_TYPESCRIPT_VERSION="${TYPESCRIPT_VERSION:-$CURRENT_CORE_VERSION}"

echo "CURRENT_RUBY_VERSION=${CURRENT_RUBY_VERSION}"
echo "CURRENT_PYTHON_VERSION=${CURRENT_PYTHON_VERSION}"
echo "CURRENT_TYPESCRIPT_VERSION=${CURRENT_TYPESCRIPT_VERSION}"

# ---------------------------------------------------------------------------
# Calculate FFI binding versions (independent per-language versioning)
#
# Each FFI package versions independently:
#   - FFI core changed: max(bump_patch(current_lang), next_core) — never go backwards
#   - Language binding or infra changed: bump_patch(current_lang)
#   - Nothing changed: unchanged
# ---------------------------------------------------------------------------
for lang in ruby python typescript; do
    LANG_UPPER=$(echo "$lang" | tr '[:lower:]' '[:upper:]')

    LANG_CHANGED_VAR="${LANG_UPPER}_CHANGED"
    LANG_CHANGED="${!LANG_CHANGED_VAR}"

    LANG_INFRA_VAR="${LANG_UPPER}_INFRA_CHANGED"
    LANG_INFRA="${!LANG_INFRA_VAR}"

    CURRENT_LANG_VAR="CURRENT_${LANG_UPPER}_VERSION"
    CURRENT_LANG="${!CURRENT_LANG_VAR}"

    if [[ "$FFI_CORE_CHANGED" == "true" ]]; then
        # FFI core changed — bump language version but never go below next core
        BUMPED=$(bump_patch "$CURRENT_LANG")
        if semver_ge "$BUMPED" "$NEXT_CORE_VERSION"; then
            echo "NEXT_${LANG_UPPER}_VERSION=${BUMPED}"
        else
            echo "NEXT_${LANG_UPPER}_VERSION=${NEXT_CORE_VERSION}"
        fi
    elif [[ "$LANG_CHANGED" == "true" || "$LANG_INFRA" == "true" ]]; then
        # Only this language's binding or build infra changed
        echo "NEXT_${LANG_UPPER}_VERSION=$(bump_patch "$CURRENT_LANG")"
    else
        echo "NEXT_${LANG_UPPER}_VERSION=unchanged"
    fi
done

# ---------------------------------------------------------------------------
# Re-emit detect-changes variables so callers get everything in one eval
# ---------------------------------------------------------------------------
echo "CHANGES_BASE_REF=${CHANGES_BASE_REF}"
echo "FFI_CORE_CHANGED=${FFI_CORE_CHANGED}"
echo "SERVER_CORE_CHANGED=${SERVER_CORE_CHANGED}"
echo "CORE_CHANGED=${CORE_CHANGED}"
echo "RUBY_CHANGED=${RUBY_CHANGED}"
echo "PYTHON_CHANGED=${PYTHON_CHANGED}"
echo "TYPESCRIPT_CHANGED=${TYPESCRIPT_CHANGED}"
echo "RUBY_INFRA_CHANGED=${RUBY_INFRA_CHANGED}"
echo "PYTHON_INFRA_CHANGED=${PYTHON_INFRA_CHANGED}"
echo "TYPESCRIPT_INFRA_CHANGED=${TYPESCRIPT_INFRA_CHANGED}"
