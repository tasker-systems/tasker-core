#!/usr/bin/env bash
set -euo pipefail

# =============================================================================
# Display test summary for framework & client CI runs
# =============================================================================
# Parses test result files and prints a human-readable summary.
# Knows the result file naming conventions for each language.
#
# Usage:
#   ./ci-display-test-summary.sh <language>
#
# Arguments:
#   language - One of: python, ruby, typescript
#
# Environment variables:
#   TARGET_DIR - Directory containing result files (default: target)
# =============================================================================

LANG="${1:?Usage: ci-display-test-summary.sh <python|ruby|typescript>}"
TARGET_DIR="${TARGET_DIR:-target}"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

# Print test count from a JUnit XML file
summarize_xml() {
    local label="$1" file="$2"
    echo ""
    echo "${label}:"
    if [ -f "$file" ]; then
        local test_count
        test_count=$(grep -c '<testcase' "$file" || true)
        local fail_count
        fail_count=$(grep -c '<failure' "$file" || true)
        if [ "$fail_count" -gt 0 ]; then
            echo "  Tests: ${test_count} (${fail_count} failed)"
        else
            echo "  Tests: ${test_count}"
        fi
    else
        echo "  ⚠️ No results file found"
    fi
}

# Print tail of a text result file
summarize_txt() {
    local label="$1" file="$2"
    echo ""
    echo "${label}:"
    if [ -f "$file" ]; then
        tail -5 "$file"
    else
        echo "  ⚠️ No results file found"
    fi
}

# Print pass count from a text result file (for FFI tests)
summarize_pass_count() {
    local label="$1" file="$2"
    local count
    count=$(grep -c 'pass' "$file" 2>/dev/null || echo "skipped/failed")
    echo "  ${label}: ${count}"
}

# ---------------------------------------------------------------------------
# Language-specific summaries
# ---------------------------------------------------------------------------

case "$LANG" in
    python)
        echo "📊 Python Framework & Client Test Summary"
        echo "==========================================="
        summarize_xml "Framework Tests" "${TARGET_DIR}/python-framework-results.xml"
        summarize_xml "Client API Tests" "${TARGET_DIR}/python-client-results.xml"
        ;;

    ruby)
        echo "📊 Ruby Framework & Client Test Summary"
        echo "========================================="
        summarize_xml "Framework Tests" "${TARGET_DIR}/ruby-framework-results.xml"
        summarize_xml "Client API Tests" "${TARGET_DIR}/ruby-client-results.xml"
        ;;

    typescript)
        echo "📊 TypeScript Framework & Client Test Summary"
        echo "==============================================="
        summarize_txt "Unit Tests" "${TARGET_DIR}/typescript-unit-results.txt"
        echo ""
        echo "FFI Integration Tests:"
        summarize_pass_count "Node.js" "${TARGET_DIR}/typescript-ffi-node-results.txt"
        summarize_pass_count "Deno   " "${TARGET_DIR}/typescript-ffi-deno-results.txt"
        summarize_txt "Client API Tests" "${TARGET_DIR}/typescript-client-results.txt"
        ;;

    *)
        echo "❌ Unknown language: ${LANG}"
        echo "Usage: ci-display-test-summary.sh <python|ruby|typescript>"
        exit 1
        ;;
esac

echo ""
LANG_TITLE="$(echo "$LANG" | cut -c1 | tr '[:lower:]' '[:upper:]')$(echo "$LANG" | cut -c2-)"
echo "✅ ${LANG_TITLE} framework & client tests completed"
