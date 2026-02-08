#!/usr/bin/env bash
# =============================================================================
# CI Sanity Check - Validate CI/CD infrastructure integrity
# =============================================================================
#
# Validates shell scripts, GitHub workflows, and file references across the
# tasker-core monorepo. Runs 6 validation passes:
#
#   1. Tool availability (shellcheck, actionlint)
#   2. Script executability (all .sh files)
#   3. Shebang consistency (bash shebangs)
#   4. shellcheck (lint all shell scripts)
#   5. actionlint (lint GitHub Actions workflows)
#   6. Script reference integrity (Makefile.toml + workflow references)
#
# Usage:
#   cargo make ci-sanity-check
#   cargo make csc
#
# Compatible with bash 3.2+ (macOS default).
#
# =============================================================================

set -euo pipefail

# cargo-make sets cwd to the project root, so relative paths work directly.
# If run standalone, navigate to repo root.
if [[ -f "Makefile.toml" ]]; then
    REPO_ROOT="$(pwd)"
else
    REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
    cd "$REPO_ROOT"
fi

EXIT_CODE=0
PASS_COUNT=0
FAIL_COUNT=0

# -----------------------------------------------------------------------------
# Helpers
# -----------------------------------------------------------------------------

pass_header() {
    echo ""
    echo "================================================================"
    echo "  Pass $1: $2"
    echo "================================================================"
    echo ""
}

pass_ok() {
    echo "  PASS: $1"
    PASS_COUNT=$((PASS_COUNT + 1))
}

pass_fail() {
    echo "  FAIL: $1"
    FAIL_COUNT=$((FAIL_COUNT + 1))
    EXIT_CODE=1
}

# Collect all .sh files from known directories into a temp file (one per line).
# Using a temp file avoids bash 4+ mapfile dependency.
SCRIPT_LIST=$(mktemp)
trap 'rm -f "$SCRIPT_LIST"' EXIT

collect_scripts() {
    local dirs="scripts cargo-make/scripts .github/scripts bin docker/scripts"
    for dir in $dirs; do
        if [[ -d "$dir" ]]; then
            find "$dir" -name '*.sh' -type f 2>/dev/null
        fi
    done | sort > "$SCRIPT_LIST"
}

# Count lines in a file
count_lines() {
    wc -l < "$1" | tr -d ' '
}

# -----------------------------------------------------------------------------
# Pass 1: Tool Availability
# -----------------------------------------------------------------------------

pass_header 1 "Tool Availability"

TOOLS_OK=true

if command -v shellcheck &>/dev/null; then
    echo "  shellcheck: $(shellcheck --version 2>&1 | grep 'version:' | head -1)"
else
    echo "  shellcheck: NOT FOUND"
    echo ""
    echo "  Install with: brew install shellcheck"
    TOOLS_OK=false
fi

if command -v actionlint &>/dev/null; then
    echo "  actionlint: $(actionlint --version 2>&1 | head -1)"
else
    echo "  actionlint: NOT FOUND"
    echo ""
    echo "  Install with: brew install actionlint"
    TOOLS_OK=false
fi

if $TOOLS_OK; then
    pass_ok "All required tools are installed"
else
    pass_fail "Missing required tools (see above)"
    echo ""
    echo "  Cannot continue without required tools. Exiting."
    exit 1
fi

# Collect scripts once for reuse across passes
collect_scripts
SCRIPT_COUNT=$(count_lines "$SCRIPT_LIST")
echo ""
echo "  Found $SCRIPT_COUNT shell scripts to validate"

# -----------------------------------------------------------------------------
# Pass 2: Script Executability
# -----------------------------------------------------------------------------

pass_header 2 "Script Executability"

NON_EXEC_COUNT=0
NON_EXEC_LIST=""
while IFS= read -r script; do
    if [[ ! -x "$script" ]]; then
        NON_EXEC_COUNT=$((NON_EXEC_COUNT + 1))
        NON_EXEC_LIST="${NON_EXEC_LIST}    - ${script}"$'\n'
    fi
done < "$SCRIPT_LIST"

if [[ $NON_EXEC_COUNT -eq 0 ]]; then
    pass_ok "All $SCRIPT_COUNT scripts have executable bit set"
else
    pass_fail "${NON_EXEC_COUNT} script(s) missing executable bit:"
    echo "$NON_EXEC_LIST"
    echo "  Fix with: chmod +x <files>"
fi

# -----------------------------------------------------------------------------
# Pass 3: Shebang Consistency
# -----------------------------------------------------------------------------

pass_header 3 "Shebang Consistency"

BAD_SHEBANG_COUNT=0
BAD_SHEBANG_LIST=""
while IFS= read -r script; do
    first_line=$(head -1 "$script" 2>/dev/null || echo "")
    case "$first_line" in
        "#!/usr/bin/env bash"|"#!/bin/bash")
            # Acceptable shebangs
            ;;
        *)
            BAD_SHEBANG_COUNT=$((BAD_SHEBANG_COUNT + 1))
            BAD_SHEBANG_LIST="${BAD_SHEBANG_LIST}    - ${script} (${first_line})"$'\n'
            ;;
    esac
done < "$SCRIPT_LIST"

if [[ $BAD_SHEBANG_COUNT -eq 0 ]]; then
    pass_ok "All scripts use valid bash shebangs"
else
    pass_fail "${BAD_SHEBANG_COUNT} script(s) with non-standard shebang:"
    echo "$BAD_SHEBANG_LIST"
    echo "  Expected: #!/usr/bin/env bash (preferred) or #!/bin/bash"
fi

# -----------------------------------------------------------------------------
# Pass 4: shellcheck
# -----------------------------------------------------------------------------

pass_header 4 "shellcheck"

if [[ $SCRIPT_COUNT -eq 0 ]]; then
    pass_ok "No scripts to check"
else
    echo "  Checking $SCRIPT_COUNT scripts with shellcheck..."
    echo ""

    # Build args array from file list (xargs to handle large lists)
    # Uses -S warning (errors + warnings), excludes SC1091, enforces bash dialect
    if xargs shellcheck -S warning -s bash -e SC1091 < "$SCRIPT_LIST"; then
        pass_ok "All scripts pass shellcheck"
    else
        pass_fail "shellcheck found issues (see above)"
    fi
fi

# -----------------------------------------------------------------------------
# Pass 5: actionlint
# -----------------------------------------------------------------------------

pass_header 5 "actionlint"

WORKFLOW_DIR=".github/workflows"
WORKFLOW_COUNT=0

if [[ -d "$WORKFLOW_DIR" ]]; then
    WORKFLOW_LIST=$(mktemp)
    find "$WORKFLOW_DIR" \( -name '*.yml' -o -name '*.yaml' \) -type f | sort > "$WORKFLOW_LIST"
    WORKFLOW_COUNT=$(count_lines "$WORKFLOW_LIST")

    if [[ $WORKFLOW_COUNT -eq 0 ]]; then
        pass_ok "No workflow files found"
    else
        echo "  Checking $WORKFLOW_COUNT workflow(s)..."
        echo ""

        # Ignore shellcheck info/style-level issues (SC*:info, SC*:style) reported
        # by actionlint's embedded shellcheck. These are typically SC2086:info about
        # quoting ${{ }} expressions, which are safe in GitHub Actions context.
        if xargs actionlint \
            -ignore 'SC[0-9]+:info:' \
            -ignore 'SC[0-9]+:style:' \
            < "$WORKFLOW_LIST"; then
            pass_ok "All workflows pass actionlint"
        else
            pass_fail "actionlint found issues (see above)"
        fi
    fi
    rm -f "$WORKFLOW_LIST"
else
    echo "  No .github/workflows directory found, skipping"
    pass_ok "N/A (no workflows)"
fi

# -----------------------------------------------------------------------------
# Pass 6: Script Reference Integrity
# -----------------------------------------------------------------------------

pass_header 6 "Script Reference Integrity"

BROKEN_REF_COUNT=0
BROKEN_REF_LIST=""

# 6a: Parse Makefile.toml files for script = { file = "..." } references
echo "  Checking Makefile.toml script references..."

while IFS= read -r makefile; do
    makefile_dir=$(dirname "$makefile")

    # Extract file = "..." patterns from script = { file = "..." }
    while IFS= read -r ref; do
        # Strip quotes and whitespace
        ref=$(echo "$ref" | sed 's/.*file *= *"//; s/".*//')

        # Skip empty
        [[ -z "$ref" ]] && continue

        # Resolve ${SCRIPTS_DIR} to cargo-make/scripts (relative to repo root)
        resolved=$(echo "$ref" | sed 's|\${SCRIPTS_DIR}|cargo-make/scripts|g')

        # If path doesn't start with / or cargo-make, it's relative to the makefile dir
        if [[ "$resolved" != /* ]] && [[ "$resolved" != cargo-make/* ]]; then
            resolved="$makefile_dir/$resolved"
        fi

        # Normalize path (remove leading ./)
        resolved=$(echo "$resolved" | sed 's|^\./||')

        if [[ ! -f "$resolved" ]]; then
            BROKEN_REF_COUNT=$((BROKEN_REF_COUNT + 1))
            BROKEN_REF_LIST="${BROKEN_REF_LIST}    - ${makefile}: ${ref} -> ${resolved} (NOT FOUND)"$'\n'
        elif [[ ! -x "$resolved" ]]; then
            BROKEN_REF_COUNT=$((BROKEN_REF_COUNT + 1))
            BROKEN_REF_LIST="${BROKEN_REF_LIST}    - ${makefile}: ${ref} -> ${resolved} (not executable)"$'\n'
        fi
    done < <(grep -E 'file\s*=\s*"' "$makefile" 2>/dev/null || true)
done < <(find . -name 'Makefile.toml' -not -path '*/target/*' 2>/dev/null | sort)

# 6b: Parse workflow files for script references (run: blocks with script paths)
echo "  Checking workflow script references..."

if [[ -d "$WORKFLOW_DIR" ]]; then
    while IFS= read -r workflow; do
        # Look for .sh file references that look like actual script invocations
        # (must contain a / to distinguish paths from substrings like "github.sha")
        while IFS= read -r script_path; do
            [[ -z "$script_path" ]] && continue

            # Skip variable references like ${{ ... }} or ${...}
            case "$script_path" in
                *'${'*|*'$('*) continue ;;
            esac

            # Normalize path: remove leading ./ and collapse relative ../ prefixes
            script_path=$(echo "$script_path" | sed 's|^\./||')

            # For workflow references with ../../ prefixes (scripts run from subdirs),
            # also check if the path resolves when stripped of leading ../
            normalized_path=$(echo "$script_path" | sed 's|\.\./||g')

            if [[ ! -f "$script_path" ]] && [[ ! -f "$normalized_path" ]]; then
                BROKEN_REF_COUNT=$((BROKEN_REF_COUNT + 1))
                BROKEN_REF_LIST="${BROKEN_REF_LIST}    - ${workflow}: ${script_path} (NOT FOUND)"$'\n'
            fi
        done < <(grep -oE '[a-zA-Z0-9_./-]+/[a-zA-Z0-9_./-]+\.sh' "$workflow" 2>/dev/null | sort -u || true)
    done < <(find "$WORKFLOW_DIR" \( -name '*.yml' -o -name '*.yaml' \) -type f 2>/dev/null | sort)
fi

if [[ $BROKEN_REF_COUNT -eq 0 ]]; then
    pass_ok "All script references are valid"
else
    pass_fail "${BROKEN_REF_COUNT} broken reference(s):"
    echo "$BROKEN_REF_LIST"
fi

# -----------------------------------------------------------------------------
# Summary
# -----------------------------------------------------------------------------

echo ""
echo "================================================================"
echo "  Summary"
echo "================================================================"
echo ""
echo "  Passed: $PASS_COUNT"
echo "  Failed: $FAIL_COUNT"
echo "  Scripts checked: $SCRIPT_COUNT"
echo "  Workflows checked: ${WORKFLOW_COUNT}"
echo ""

if [[ $EXIT_CODE -eq 0 ]]; then
    echo "  All checks passed."
else
    echo "  Some checks failed. See details above."
fi

exit $EXIT_CODE
