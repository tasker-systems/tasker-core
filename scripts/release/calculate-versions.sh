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
#     NEXT_RUBY_VERSION=0.1.1.0|unchanged
#     NEXT_PYTHON_VERSION=0.1.1.0|unchanged
#     NEXT_TYPESCRIPT_VERSION=0.1.1.0|unchanged
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
# Calculate FFI binding versions
#
# Logic:
#   - If FFI-facing core changed: reset binding patch to .0 (new core version)
#   - If only the binding changed: increment the .P patch level
#   - If nothing changed: "unchanged"
# ---------------------------------------------------------------------------
for lang in ruby python typescript; do
    LANG_UPPER=$(echo "$lang" | tr '[:lower:]' '[:upper:]')

    # Read the change flag for this language
    LANG_CHANGED_VAR="${LANG_UPPER}_CHANGED"
    LANG_CHANGED="${!LANG_CHANGED_VAR}"

    if [[ "$FFI_CORE_CHANGED" == "true" ]]; then
        # Core changed: all bindings reset to .0 on the new core version
        echo "NEXT_${LANG_UPPER}_VERSION=${NEXT_CORE_VERSION}.0"

    elif [[ "$LANG_CHANGED" == "true" ]]; then
        # Binding-only change: increment the language-specific patch level
        LAST_LANG_TAG=$(git tag -l "${lang}-v*" --sort=-version:refname 2>/dev/null | head -n1 || true)

        if [[ -n "$LAST_LANG_TAG" ]] && [[ "$LAST_LANG_TAG" =~ ${lang}-v([0-9]+\.[0-9]+\.[0-9]+)\.([0-9]+) ]]; then
            LAST_PATCH="${BASH_REMATCH[2]}"
            NEXT_PATCH=$(( LAST_PATCH + 1 ))
            echo "NEXT_${LANG_UPPER}_VERSION=${CURRENT_CORE_VERSION}.${NEXT_PATCH}"
        else
            # No prior language tag — this is the first release for this binding
            echo "NEXT_${LANG_UPPER}_VERSION=${CURRENT_CORE_VERSION}.0"
        fi

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
