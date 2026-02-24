#!/usr/bin/env bash
# cargo-make/scripts/validate-codegen.sh
#
# Validates that tasker-ctl code generation produces syntactically valid output
# for all supported languages (Python, Ruby, TypeScript, Rust).
#
# Usage:
#   validate-codegen.sh [OPTIONS]
#
# Options:
#   --binary PATH    Path to tasker-ctl binary (default: target/debug/tasker-ctl)
#   --fixture PATH   Path to test template YAML (default: tests/fixtures/task_templates/codegen_test_template.yaml)
#   --verbose        Print commands as they run
#
# Bash 3.2 compatible (no ${var^^}, no mapfile, no associative arrays).

set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
BINARY="target/debug/tasker-ctl"
FIXTURE="tests/fixtures/task_templates/codegen_test_template.yaml"
VERBOSE=false

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
while [ $# -gt 0 ]; do
    case $1 in
        --binary)  BINARY="$2"; shift 2 ;;
        --binary=*) BINARY="${1#*=}"; shift ;;
        --fixture) FIXTURE="$2"; shift 2 ;;
        --fixture=*) FIXTURE="${1#*=}"; shift ;;
        --verbose) VERBOSE=true; shift ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
debug() {
    if [ "$VERBOSE" = "true" ]; then
        echo "[validate-codegen] $*" >&2
    fi
}

PASS=0
FAIL=0
SKIP=0

check_tool() {
    local tool="$1"
    if command -v "$tool" >/dev/null 2>&1; then
        return 0
    else
        debug "Tool not found: $tool (skipping checks that require it)"
        return 1
    fi
}

record_pass() {
    local label="$1"
    echo "  PASS: ${label}"
    PASS=$((PASS + 1))
}

record_fail() {
    local label="$1"
    local detail="${2:-}"
    echo "  FAIL: ${label}"
    if [ -n "$detail" ]; then
        echo "        ${detail}"
    fi
    FAIL=$((FAIL + 1))
}

record_skip() {
    local label="$1"
    local reason="${2:-tool not found}"
    echo "  SKIP: ${label} (${reason})"
    SKIP=$((SKIP + 1))
}

# ---------------------------------------------------------------------------
# Validate prerequisites
# ---------------------------------------------------------------------------
if [ ! -x "$BINARY" ]; then
    echo "ERROR: tasker-ctl binary not found or not executable: ${BINARY}" >&2
    exit 1
fi

if [ ! -f "$FIXTURE" ]; then
    echo "ERROR: Fixture file not found: ${FIXTURE}" >&2
    exit 1
fi

debug "Binary: ${BINARY}"
debug "Fixture: ${FIXTURE}"

# ---------------------------------------------------------------------------
# Create temp directory with cleanup
# ---------------------------------------------------------------------------
TMPDIR_CODEGEN="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_CODEGEN"' EXIT

debug "Temp dir: ${TMPDIR_CODEGEN}"

# ---------------------------------------------------------------------------
# Detect available tools
# ---------------------------------------------------------------------------
HAS_PYTHON=false
HAS_RUBY=false
HAS_TSC=false
HAS_BUN=false
HAS_RUSTFMT=false

if check_tool python3; then HAS_PYTHON=true; fi
if check_tool ruby; then HAS_RUBY=true; fi
if check_tool rustfmt; then HAS_RUSTFMT=true; fi

# TypeScript: prefer tsc directly, fall back to bun x tsc
if check_tool tsc; then
    HAS_TSC=true
elif check_tool bun; then
    HAS_BUN=true
fi

debug "Tools: python3=${HAS_PYTHON} ruby=${HAS_RUBY} tsc=${HAS_TSC} bun=${HAS_BUN} rustfmt=${HAS_RUSTFMT}"

# ---------------------------------------------------------------------------
# Generate and validate for each language
# ---------------------------------------------------------------------------
echo "Validating codegen output syntax..."
echo ""

validate_python() {
    local kind="$1"
    local file="$2"
    local label="python ${kind}"

    if [ "$HAS_PYTHON" = "false" ]; then
        record_skip "$label"
        return
    fi

    if python3 -c "import ast, sys; ast.parse(open(sys.argv[1]).read())" "$file" 2>/dev/null; then
        record_pass "$label"
    else
        record_fail "$label" "$(python3 -c "import ast, sys; ast.parse(open(sys.argv[1]).read())" "$file" 2>&1 || true)"
    fi
}

validate_ruby() {
    local kind="$1"
    local file="$2"
    local label="ruby ${kind}"

    if [ "$HAS_RUBY" = "false" ]; then
        record_skip "$label"
        return
    fi

    local output
    if output="$(ruby -c "$file" 2>&1)"; then
        record_pass "$label"
    else
        record_fail "$label" "$output"
    fi
}

validate_typescript() {
    local kind="$1"
    local file="$2"
    local label="typescript ${kind}"

    if [ "$HAS_TSC" = "false" ] && [ "$HAS_BUN" = "false" ]; then
        record_skip "$label"
        return
    fi

    # Build tsc command
    local tsc_cmd
    if [ "$HAS_TSC" = "true" ]; then
        tsc_cmd="tsc"
    else
        tsc_cmd="bun x tsc"
    fi

    local check_file="$file"
    if [ "$kind" = "handler" ]; then
        # Handler imports external modules (tasker-worker) that aren't installed.
        # Strip import lines and add stub declarations for imported symbols
        # to validate syntax without requiring the actual module.
        check_file="${file%.ts}_check.ts"
        {
            echo "declare function defineHandler(...args: any[]): any;"
            echo "declare function getDependencyResult(...args: any[]): any;"
            sed '/^import /d' "$file"
        } > "$check_file"
    fi

    local output
    if output="$(${tsc_cmd} --noEmit --skipLibCheck "$check_file" 2>&1)"; then
        record_pass "$label"
    else
        record_fail "$label" "$output"
    fi
}

validate_rust() {
    local kind="$1"
    local file="$2"
    local label="rust ${kind}"

    if [ "$HAS_RUSTFMT" = "false" ]; then
        record_skip "$label"
        return
    fi

    # rustfmt succeeds only on syntactically valid Rust.
    # Format in-place (not --check) so formatting style differences don't cause failures.
    local output
    if output="$(rustfmt --edition 2021 "$file" 2>&1)"; then
        record_pass "$label"
    else
        record_fail "$label" "$output"
    fi
}

# --- Generate and validate each language ---
for lang in python ruby typescript rust; do
    debug "Generating ${lang} types..."
    types_file="${TMPDIR_CODEGEN}/${lang}_types"
    handler_file="${TMPDIR_CODEGEN}/${lang}_handler"

    # Set appropriate file extensions
    case "$lang" in
        python)     types_file="${types_file}.py";  handler_file="${handler_file}.py" ;;
        ruby)       types_file="${types_file}.rb";  handler_file="${handler_file}.rb" ;;
        typescript) types_file="${types_file}.ts";  handler_file="${handler_file}.ts" ;;
        rust)       types_file="${types_file}.rs";  handler_file="${handler_file}.rs" ;;
    esac

    # Generate types
    debug "  ${BINARY} generate types --language ${lang} --template ${FIXTURE} --output ${types_file}"
    if ! "${BINARY}" generate types --language "$lang" --template "$FIXTURE" --output "$types_file" 2>/dev/null; then
        record_fail "${lang} types" "tasker-ctl generate types failed"
        record_fail "${lang} handler" "skipped (types generation failed)"
        continue
    fi

    # Generate handler
    debug "  ${BINARY} generate handler --language ${lang} --template ${FIXTURE} --output ${handler_file}"
    if ! "${BINARY}" generate handler --language "$lang" --template "$FIXTURE" --output "$handler_file" 2>/dev/null; then
        # Validate types even if handler generation fails
        "validate_${lang}" types "$types_file"
        record_fail "${lang} handler" "tasker-ctl generate handler failed"
        continue
    fi

    # Validate both
    "validate_${lang}" types "$types_file"
    "validate_${lang}" handler "$handler_file"
done

# ---------------------------------------------------------------------------
# Report
# ---------------------------------------------------------------------------
echo ""
TOTAL=$((PASS + FAIL + SKIP))
echo "Results: ${PASS} passed, ${FAIL} failed, ${SKIP} skipped (total: ${TOTAL})"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
