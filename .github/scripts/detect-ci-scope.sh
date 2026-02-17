#!/usr/bin/env bash
# .github/scripts/detect-ci-scope.sh
#
# Determine which CI jobs need to run based on changed files.
#
# Usage:
#   .github/scripts/detect-ci-scope.sh [OPTIONS]
#
# Options:
#   --github-output   Write outputs to $GITHUB_OUTPUT (for Actions)
#   --stdin           Read file list from stdin instead of git diff
#   --base REF        Override base ref for git diff
#   --verbose         Print debug info to stderr
#
# Output:
#   KEY=VALUE pairs to stdout (eval-safe), and optionally to $GITHUB_OUTPUT.
#
# Bash 3.2 compatible (no ${var^^}, no mapfile, no associative arrays).

set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
USE_GITHUB_OUTPUT=false
USE_STDIN=false
BASE_REF_OVERRIDE=""
VERBOSE=false

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
while [ $# -gt 0 ]; do
    case $1 in
        --github-output) USE_GITHUB_OUTPUT=true; shift ;;
        --stdin)         USE_STDIN=true; shift ;;
        --base)          BASE_REF_OVERRIDE="$2"; shift 2 ;;
        --base=*)        BASE_REF_OVERRIDE="${1#*=}"; shift ;;
        --verbose)       VERBOSE=true; shift ;;
        *)               echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
debug() {
    if [ "$VERBOSE" = "true" ]; then
        echo "[detect-ci-scope] $*" >&2
    fi
}

# ---------------------------------------------------------------------------
# Get changed files
# ---------------------------------------------------------------------------
if [ "$USE_STDIN" = "true" ]; then
    CHANGED_FILES="$(cat)"
else
    # Determine base ref
    if [ -n "$BASE_REF_OVERRIDE" ]; then
        BASE_REF="$BASE_REF_OVERRIDE"
    elif [ -n "${GITHUB_BASE_REF:-}" ]; then
        # PR context
        BASE_REF="$(git merge-base "origin/${GITHUB_BASE_REF}" HEAD 2>/dev/null || echo "origin/${GITHUB_BASE_REF}")"
    elif [ "${GITHUB_EVENT_NAME:-}" = "push" ] && [ "${GITHUB_REF:-}" = "refs/heads/main" ]; then
        # Push to main
        BASE_REF="HEAD~1"
    else
        # Local dev — compare against main
        BASE_REF="$(git merge-base origin/main HEAD 2>/dev/null || echo "origin/main")"
    fi

    debug "Base ref: ${BASE_REF}"
    CHANGED_FILES="$(git diff "${BASE_REF}" HEAD --name-only 2>/dev/null || true)"
fi

if [ -z "$CHANGED_FILES" ]; then
    debug "No changed files detected — defaulting to full CI"
    # When no changes detected, run everything (safety fallback)
    CHANGED_FILES="__force_full_ci__"
fi

debug "Changed files:"
if [ "$VERBOSE" = "true" ]; then
    echo "$CHANGED_FILES" | while IFS= read -r f; do
        echo "  $f" >&2
    done
fi

# ---------------------------------------------------------------------------
# Path group detection
# ---------------------------------------------------------------------------
# Helper: check if any changed file matches a pattern.
changes_match() {
    local pattern="$1"
    echo "$CHANGED_FILES" | grep -qE "$pattern"
}

# Helper: check if any non-doc changed file matches a pattern.
# Used for code directories where .md files don't affect builds.
code_changes_match() {
    local pattern="$1"
    echo "$CHANGED_FILES" | grep -vE '\.(md|txt|adoc)$' | grep -qE "$pattern"
}

HAS_DOCS=false
HAS_CI_TOOLING=false
HAS_CONFIG=false
HAS_PROTO=false
HAS_MIGRATIONS=false
HAS_SQLX_CACHE=false
HAS_DOCKER=false
HAS_FFI_CORE=false
HAS_SERVER_CORE=false
HAS_ROOT_RUST=false
HAS_RUBY_WORKER=false
HAS_PYTHON_WORKER=false
HAS_TS_WORKER=false
HAS_RUST_WORKER=false
HAS_SELF=false
HAS_OTHER=false

# Docs: markdown/text files NOT under src/ directories
if changes_match '\.(md|txt|adoc)$'; then
    HAS_DOCS=true
fi

# CI tooling
if changes_match '^\.github/|^cargo-make/|^Makefile\.toml$'; then
    HAS_CI_TOOLING=true
fi

# Config changes (broad impact)
if changes_match '^config/|^Cargo\.(toml|lock)$|^\.env'; then
    HAS_CONFIG=true
fi

# Proto definitions (affects all code generation)
if changes_match '^proto/'; then
    HAS_PROTO=true
fi

# Database migrations
if changes_match '^migrations/'; then
    HAS_MIGRATIONS=true
fi

# SQLx query cache
if changes_match '^\.sqlx/'; then
    HAS_SQLX_CACHE=true
fi

# Docker infrastructure
if changes_match '^docker/'; then
    HAS_DOCKER=true
fi

# FFI-facing core crates (changes cascade to all workers)
# Only non-doc files matter — a README.md or CLAUDE.md in a crate dir doesn't affect builds.
if code_changes_match '^(tasker-shared|tasker-worker|tasker-pgmq)/'; then
    HAS_FFI_CORE=true
fi

# Server/client core crates
if code_changes_match '^(tasker-orchestration|tasker-client|tasker-ctl)/'; then
    HAS_SERVER_CORE=true
fi

# Root-level Rust source
if code_changes_match '^(src/|tests/|benches/|build\.rs)'; then
    HAS_ROOT_RUST=true
fi

# Worker directories
if code_changes_match '^workers/ruby/'; then
    HAS_RUBY_WORKER=true
fi

if code_changes_match '^workers/python/'; then
    HAS_PYTHON_WORKER=true
fi

if code_changes_match '^workers/typescript/'; then
    HAS_TS_WORKER=true
fi

if code_changes_match '^workers/rust/'; then
    HAS_RUST_WORKER=true
fi

# Self-referential (this detection script itself changed)
if changes_match '^\.github/scripts/detect-ci-scope'; then
    HAS_SELF=true
fi

# Check for files that don't match any known docs-only pattern
# This catches source files with .rs, .rb, .py, .ts, .toml, .yml, etc.
NON_DOC_FILES="$(echo "$CHANGED_FILES" | grep -vE '\.(md|txt|adoc)$' || true)"
if [ -n "$NON_DOC_FILES" ]; then
    HAS_OTHER=true
fi

debug "Path groups: docs=$HAS_DOCS ci_tooling=$HAS_CI_TOOLING config=$HAS_CONFIG"
debug "  proto=$HAS_PROTO migrations=$HAS_MIGRATIONS sqlx_cache=$HAS_SQLX_CACHE docker=$HAS_DOCKER"
debug "  ffi_core=$HAS_FFI_CORE server_core=$HAS_SERVER_CORE root_rust=$HAS_ROOT_RUST"
debug "  ruby=$HAS_RUBY_WORKER python=$HAS_PYTHON_WORKER ts=$HAS_TS_WORKER rust=$HAS_RUST_WORKER"
debug "  self=$HAS_SELF other=$HAS_OTHER"

# ---------------------------------------------------------------------------
# Escalation logic
# ---------------------------------------------------------------------------

# docs_only: ONLY doc files changed, nothing else
DOCS_ONLY=false
if [ "$HAS_DOCS" = "true" ] && [ "$HAS_OTHER" = "false" ]; then
    DOCS_ONLY=true
fi

# ci_tooling_only: only CI/build tooling changed (may include docs too)
CI_TOOLING_ONLY=false
if [ "$HAS_CI_TOOLING" = "true" ] && [ "$DOCS_ONLY" = "false" ]; then
    # Check if ALL non-doc files are CI tooling
    NON_DOC_NON_CI="$(echo "$CHANGED_FILES" | grep -vE '\.(md|txt|adoc)$' | grep -vE '^\.github/|^cargo-make/|^Makefile\.toml$' || true)"
    # Also verify no code-affecting path groups are set (safety net)
    if [ -z "$NON_DOC_NON_CI" ] && \
       [ "$HAS_FFI_CORE" = "false" ] && [ "$HAS_SERVER_CORE" = "false" ] && \
       [ "$HAS_ROOT_RUST" = "false" ] && [ "$HAS_CONFIG" = "false" ] && \
       [ "$HAS_PROTO" = "false" ] && [ "$HAS_MIGRATIONS" = "false" ] && \
       [ "$HAS_SQLX_CACHE" = "false" ] && [ "$HAS_DOCKER" = "false" ] && \
       [ "$HAS_RUBY_WORKER" = "false" ] && [ "$HAS_PYTHON_WORKER" = "false" ] && \
       [ "$HAS_TS_WORKER" = "false" ] && [ "$HAS_RUST_WORKER" = "false" ]; then
        CI_TOOLING_ONLY=true
    fi
fi

# full_ci: changes that force complete pipeline
FULL_CI=false
if [ "$HAS_SELF" = "true" ] || [ "$HAS_PROTO" = "true" ] || \
   [ "$HAS_MIGRATIONS" = "true" ] || [ "$HAS_SQLX_CACHE" = "true" ] || \
   [ "$HAS_DOCKER" = "true" ]; then
    FULL_CI=true
fi

# Aggregate flags
ANY_RUST="false"
if [ "$HAS_FFI_CORE" = "true" ] || [ "$HAS_SERVER_CORE" = "true" ] || [ "$HAS_ROOT_RUST" = "true" ]; then
    ANY_RUST="true"
fi

ANY_WORKER="false"
if [ "$HAS_RUBY_WORKER" = "true" ] || [ "$HAS_PYTHON_WORKER" = "true" ] || \
   [ "$HAS_TS_WORKER" = "true" ] || [ "$HAS_RUST_WORKER" = "true" ]; then
    ANY_WORKER="true"
fi

debug "Escalation: docs_only=$DOCS_ONLY ci_tooling_only=$CI_TOOLING_ONLY full_ci=$FULL_CI"
debug "  any_rust=$ANY_RUST any_worker=$ANY_WORKER"

# ---------------------------------------------------------------------------
# Compute output flags
# ---------------------------------------------------------------------------

# Helper: true if any argument is "true"
any_true() {
    while [ $# -gt 0 ]; do
        if [ "$1" = "true" ]; then
            return 0
        fi
        shift
    done
    return 1
}

# RUN_BUILD_POSTGRES: any code change (not docs-only)
RUN_BUILD_POSTGRES="false"
if [ "$DOCS_ONLY" = "false" ]; then
    RUN_BUILD_POSTGRES="true"
fi

# RUN_CODE_QUALITY: any code change (not docs-only)
RUN_CODE_QUALITY="false"
if [ "$DOCS_ONLY" = "false" ]; then
    RUN_CODE_QUALITY="true"
fi

# RUN_BUILD_WORKERS: rust or worker changes, config, or full CI
RUN_BUILD_WORKERS="false"
if any_true "$ANY_RUST" "$ANY_WORKER" "$HAS_CONFIG" "$FULL_CI"; then
    RUN_BUILD_WORKERS="true"
fi

# Per-worker build flags
RUN_BUILD_RUBY="false"
if any_true "$HAS_FFI_CORE" "$HAS_RUBY_WORKER" "$FULL_CI"; then
    RUN_BUILD_RUBY="true"
fi

RUN_BUILD_PYTHON="false"
if any_true "$HAS_FFI_CORE" "$HAS_PYTHON_WORKER" "$FULL_CI"; then
    RUN_BUILD_PYTHON="true"
fi

RUN_BUILD_TYPESCRIPT="false"
if any_true "$HAS_FFI_CORE" "$HAS_TS_WORKER" "$FULL_CI"; then
    RUN_BUILD_TYPESCRIPT="true"
fi

RUN_BUILD_RUST_WORKER="false"
if any_true "$HAS_FFI_CORE" "$HAS_RUST_WORKER" "$HAS_SERVER_CORE" "$FULL_CI"; then
    RUN_BUILD_RUST_WORKER="true"
fi

# RUN_INTEGRATION_TESTS: rust changes, config, or full CI
RUN_INTEGRATION_TESTS="false"
if any_true "$ANY_RUST" "$HAS_CONFIG" "$FULL_CI"; then
    RUN_INTEGRATION_TESTS="true"
fi

# Framework test flags
RUN_RUBY_FRAMEWORK="false"
if any_true "$HAS_FFI_CORE" "$HAS_RUBY_WORKER" "$FULL_CI"; then
    RUN_RUBY_FRAMEWORK="true"
fi

RUN_PYTHON_FRAMEWORK="false"
if any_true "$HAS_FFI_CORE" "$HAS_PYTHON_WORKER" "$FULL_CI"; then
    RUN_PYTHON_FRAMEWORK="true"
fi

RUN_TYPESCRIPT_FRAMEWORK="false"
if any_true "$HAS_FFI_CORE" "$HAS_TS_WORKER" "$FULL_CI"; then
    RUN_TYPESCRIPT_FRAMEWORK="true"
fi

# RUN_PERFORMANCE_ANALYSIS: any test job is enabled
RUN_PERFORMANCE_ANALYSIS="false"
if any_true "$RUN_INTEGRATION_TESTS" "$RUN_RUBY_FRAMEWORK" "$RUN_PYTHON_FRAMEWORK" "$RUN_TYPESCRIPT_FRAMEWORK"; then
    RUN_PERFORMANCE_ANALYSIS="true"
fi

# ---------------------------------------------------------------------------
# Scope summary
# ---------------------------------------------------------------------------
if [ "$DOCS_ONLY" = "true" ]; then
    SCOPE_SUMMARY="docs-only: skipping all CI jobs"
elif [ "$FULL_CI" = "true" ]; then
    SCOPE_SUMMARY="full-ci: cross-cutting change detected"
elif [ "$CI_TOOLING_ONLY" = "true" ]; then
    SCOPE_SUMMARY="ci-tooling-only: running code-quality only"
else
    # Build a summary of what's in scope
    PARTS=""
    if [ "$ANY_RUST" = "true" ]; then PARTS="${PARTS:+$PARTS, }rust-core"; fi
    if [ "$HAS_RUBY_WORKER" = "true" ]; then PARTS="${PARTS:+$PARTS, }ruby"; fi
    if [ "$HAS_PYTHON_WORKER" = "true" ]; then PARTS="${PARTS:+$PARTS, }python"; fi
    if [ "$HAS_TS_WORKER" = "true" ]; then PARTS="${PARTS:+$PARTS, }typescript"; fi
    if [ "$HAS_RUST_WORKER" = "true" ]; then PARTS="${PARTS:+$PARTS, }rust-worker"; fi
    if [ "$HAS_CONFIG" = "true" ]; then PARTS="${PARTS:+$PARTS, }config"; fi
    if [ -z "$PARTS" ]; then PARTS="general"; fi
    SCOPE_SUMMARY="scoped: ${PARTS}"
fi

# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------
OUTPUT_VARS="
RUN_BUILD_POSTGRES=${RUN_BUILD_POSTGRES}
RUN_BUILD_WORKERS=${RUN_BUILD_WORKERS}
RUN_BUILD_RUBY=${RUN_BUILD_RUBY}
RUN_BUILD_PYTHON=${RUN_BUILD_PYTHON}
RUN_BUILD_TYPESCRIPT=${RUN_BUILD_TYPESCRIPT}
RUN_BUILD_RUST_WORKER=${RUN_BUILD_RUST_WORKER}
RUN_CODE_QUALITY=${RUN_CODE_QUALITY}
RUN_INTEGRATION_TESTS=${RUN_INTEGRATION_TESTS}
RUN_RUBY_FRAMEWORK=${RUN_RUBY_FRAMEWORK}
RUN_PYTHON_FRAMEWORK=${RUN_PYTHON_FRAMEWORK}
RUN_TYPESCRIPT_FRAMEWORK=${RUN_TYPESCRIPT_FRAMEWORK}
RUN_PERFORMANCE_ANALYSIS=${RUN_PERFORMANCE_ANALYSIS}
SCOPE_SUMMARY=${SCOPE_SUMMARY}
"

# Print eval-safe output to stdout
echo "$OUTPUT_VARS" | while IFS= read -r line; do
    # Skip empty lines
    if [ -n "$line" ]; then
        echo "$line"
    fi
done

# Write to GITHUB_OUTPUT if requested
if [ "$USE_GITHUB_OUTPUT" = "true" ] && [ -n "${GITHUB_OUTPUT:-}" ]; then
    {
        # Convert to kebab-case for GitHub Actions outputs
        echo "run-build-postgres=${RUN_BUILD_POSTGRES}"
        echo "run-build-workers=${RUN_BUILD_WORKERS}"
        echo "run-build-ruby=${RUN_BUILD_RUBY}"
        echo "run-build-python=${RUN_BUILD_PYTHON}"
        echo "run-build-typescript=${RUN_BUILD_TYPESCRIPT}"
        echo "run-build-rust-worker=${RUN_BUILD_RUST_WORKER}"
        echo "run-code-quality=${RUN_CODE_QUALITY}"
        echo "run-integration-tests=${RUN_INTEGRATION_TESTS}"
        echo "run-ruby-framework=${RUN_RUBY_FRAMEWORK}"
        echo "run-python-framework=${RUN_PYTHON_FRAMEWORK}"
        echo "run-typescript-framework=${RUN_TYPESCRIPT_FRAMEWORK}"
        echo "run-performance-analysis=${RUN_PERFORMANCE_ANALYSIS}"
        echo "scope-summary=${SCOPE_SUMMARY}"
    } >> "$GITHUB_OUTPUT"
fi
