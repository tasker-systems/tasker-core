#!/usr/bin/env bash
# .github/scripts/test-detect-ci-scope.sh
#
# Test harness for detect-ci-scope.sh.
# Exercises the detection script via --stdin with mock file lists.
#
# Usage:
#   bash .github/scripts/test-detect-ci-scope.sh [--verbose]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DETECT_SCRIPT="${SCRIPT_DIR}/detect-ci-scope.sh"
VERBOSE_FLAG=""
PASS=0
FAIL=0

if [ "${1:-}" = "--verbose" ]; then
    VERBOSE_FLAG="--verbose"
fi

# ---------------------------------------------------------------------------
# Test runner
# ---------------------------------------------------------------------------
run_test() {
    local test_name="$1"
    local file_list="$2"
    shift 2
    # Remaining args are assertions: "VAR=expected_value"

    local output
    output="$(echo "$file_list" | bash "$DETECT_SCRIPT" --stdin $VERBOSE_FLAG 2>/dev/null)"

    local test_passed=true
    local failures=""

    while [ $# -gt 0 ]; do
        local assertion="$1"
        local var_name="${assertion%%=*}"
        local expected="${assertion#*=}"
        shift

        # Extract actual value from output
        local actual
        actual="$(echo "$output" | grep "^${var_name}=" | head -1 | cut -d= -f2-)"

        if [ "$actual" != "$expected" ]; then
            test_passed=false
            failures="${failures}    ${var_name}: expected='${expected}' actual='${actual}'
"
        fi
    done

    if [ "$test_passed" = "true" ]; then
        echo "  PASS: ${test_name}"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: ${test_name}"
        echo "$failures"
        FAIL=$((FAIL + 1))
    fi
}

# ---------------------------------------------------------------------------
# Test cases
# ---------------------------------------------------------------------------
echo "Running detect-ci-scope.sh tests..."
echo ""

# --- docs-only ---
run_test "docs-only: all jobs skipped" \
    "README.md
docs/architecture/actors.md
CHANGELOG.md" \
    "RUN_BUILD_POSTGRES=false" \
    "RUN_BUILD_WORKERS=false" \
    "RUN_CODE_QUALITY=false" \
    "RUN_INTEGRATION_TESTS=false" \
    "RUN_RUBY_FRAMEWORK=false" \
    "RUN_PYTHON_FRAMEWORK=false" \
    "RUN_TYPESCRIPT_FRAMEWORK=false" \
    "RUN_PERFORMANCE_ANALYSIS=false"

# --- ci-tooling-only ---
run_test "ci-tooling-only: code-quality only" \
    ".github/workflows/ci.yml
cargo-make/tasks.toml
Makefile.toml" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=false" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=false" \
    "RUN_RUBY_FRAMEWORK=false" \
    "RUN_PYTHON_FRAMEWORK=false" \
    "RUN_TYPESCRIPT_FRAMEWORK=false"

# --- server-core change ---
run_test "server-core: builds + integration, no framework tests" \
    "tasker-orchestration/src/lib.rs
tasker-client/src/client.rs" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_BUILD_RUST_WORKER=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=true" \
    "RUN_RUBY_FRAMEWORK=false" \
    "RUN_PYTHON_FRAMEWORK=false" \
    "RUN_TYPESCRIPT_FRAMEWORK=false"

# --- ffi-core change (cascades to all workers) ---
run_test "ffi-core: everything enabled" \
    "tasker-shared/src/types.rs" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_BUILD_RUBY=true" \
    "RUN_BUILD_PYTHON=true" \
    "RUN_BUILD_TYPESCRIPT=true" \
    "RUN_BUILD_RUST_WORKER=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=true" \
    "RUN_RUBY_FRAMEWORK=true" \
    "RUN_PYTHON_FRAMEWORK=true" \
    "RUN_TYPESCRIPT_FRAMEWORK=true" \
    "RUN_PERFORMANCE_ANALYSIS=true"

# --- ruby-worker-only ---
run_test "ruby-worker-only: ruby build + framework, others skipped" \
    "workers/ruby/lib/tasker_core/handler.rb
workers/ruby/spec/integration/handler_spec.rb" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_BUILD_RUBY=true" \
    "RUN_BUILD_PYTHON=false" \
    "RUN_BUILD_TYPESCRIPT=false" \
    "RUN_CODE_QUALITY=true" \
    "RUN_RUBY_FRAMEWORK=true" \
    "RUN_PYTHON_FRAMEWORK=false" \
    "RUN_TYPESCRIPT_FRAMEWORK=false"

# --- python-worker-only ---
run_test "python-worker-only: python build + framework, others skipped" \
    "workers/python/python/tasker_core/worker.py
workers/python/tests/test_worker.py" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_BUILD_RUBY=false" \
    "RUN_BUILD_PYTHON=true" \
    "RUN_BUILD_TYPESCRIPT=false" \
    "RUN_CODE_QUALITY=true" \
    "RUN_RUBY_FRAMEWORK=false" \
    "RUN_PYTHON_FRAMEWORK=true" \
    "RUN_TYPESCRIPT_FRAMEWORK=false"

# --- ts-worker-only ---
run_test "ts-worker-only: typescript build + framework, others skipped" \
    "workers/typescript/src/index.ts
workers/typescript/tests/worker.test.ts" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_BUILD_RUBY=false" \
    "RUN_BUILD_PYTHON=false" \
    "RUN_BUILD_TYPESCRIPT=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_RUBY_FRAMEWORK=false" \
    "RUN_PYTHON_FRAMEWORK=false" \
    "RUN_TYPESCRIPT_FRAMEWORK=true"

# --- proto change: full CI ---
run_test "proto change: full CI" \
    "proto/tasker/v1/task.proto" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_BUILD_RUBY=true" \
    "RUN_BUILD_PYTHON=true" \
    "RUN_BUILD_TYPESCRIPT=true" \
    "RUN_BUILD_RUST_WORKER=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=true" \
    "RUN_RUBY_FRAMEWORK=true" \
    "RUN_PYTHON_FRAMEWORK=true" \
    "RUN_TYPESCRIPT_FRAMEWORK=true" \
    "RUN_PERFORMANCE_ANALYSIS=true"

# --- config change: builds + integration, no framework tests ---
run_test "config change: builds + integration" \
    "Cargo.toml
config/tasker/base/common.toml" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=true" \
    "RUN_RUBY_FRAMEWORK=false" \
    "RUN_PYTHON_FRAMEWORK=false" \
    "RUN_TYPESCRIPT_FRAMEWORK=false"

# --- mixed docs+ruby: ruby scope (docs don't reduce scope) ---
run_test "mixed docs+ruby: ruby scope wins" \
    "README.md
workers/ruby/lib/tasker_core/handler.rb" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_BUILD_RUBY=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_RUBY_FRAMEWORK=true" \
    "RUN_PYTHON_FRAMEWORK=false" \
    "RUN_TYPESCRIPT_FRAMEWORK=false"

# --- self-referential: full CI ---
run_test "self-referential: full CI" \
    ".github/scripts/detect-ci-scope.sh" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_BUILD_RUBY=true" \
    "RUN_BUILD_PYTHON=true" \
    "RUN_BUILD_TYPESCRIPT=true" \
    "RUN_BUILD_RUST_WORKER=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=true" \
    "RUN_RUBY_FRAMEWORK=true" \
    "RUN_PYTHON_FRAMEWORK=true" \
    "RUN_TYPESCRIPT_FRAMEWORK=true" \
    "SCOPE_SUMMARY=full-ci: cross-cutting change detected"

# --- self-referential mixed with ci-tooling: full CI wins ---
run_test "detect-script + ci-tooling: full-ci scope (not ci-tooling-only)" \
    ".github/scripts/detect-ci-scope.sh
.github/scripts/test-detect-ci-scope.sh
.github/workflows/ci.yml
cargo-make/scripts/ci-sanity-check.sh" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_INTEGRATION_TESTS=true" \
    "SCOPE_SUMMARY=full-ci: cross-cutting change detected"

# --- migration change: full CI ---
run_test "migration change: full CI" \
    "migrations/20240101_create_tasks.sql" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_BUILD_RUBY=true" \
    "RUN_BUILD_PYTHON=true" \
    "RUN_BUILD_TYPESCRIPT=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=true" \
    "RUN_RUBY_FRAMEWORK=true" \
    "RUN_PYTHON_FRAMEWORK=true" \
    "RUN_TYPESCRIPT_FRAMEWORK=true"

# --- sqlx cache change: full CI ---
run_test "sqlx cache change: full CI" \
    ".sqlx/query-abc123.json" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=true" \
    "RUN_RUBY_FRAMEWORK=true" \
    "RUN_PYTHON_FRAMEWORK=true" \
    "RUN_TYPESCRIPT_FRAMEWORK=true"

# --- docker change: full CI ---
run_test "docker change: full CI" \
    "docker/Dockerfile.server" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=true" \
    "RUN_RUBY_FRAMEWORK=true" \
    "RUN_PYTHON_FRAMEWORK=true" \
    "RUN_TYPESCRIPT_FRAMEWORK=true"

# --- rust-worker-only ---
run_test "rust-worker-only: rust worker build, no framework tests" \
    "workers/rust/src/main.rs" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_BUILD_RUST_WORKER=true" \
    "RUN_BUILD_RUBY=false" \
    "RUN_BUILD_PYTHON=false" \
    "RUN_BUILD_TYPESCRIPT=false" \
    "RUN_CODE_QUALITY=true" \
    "RUN_RUBY_FRAMEWORK=false" \
    "RUN_PYTHON_FRAMEWORK=false" \
    "RUN_TYPESCRIPT_FRAMEWORK=false"

# --- Cargo.lock change: config scope ---
run_test "Cargo.lock change: config scope (builds + integration)" \
    "Cargo.lock" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=true"

# --- .env change: config scope ---
run_test ".env change: config scope" \
    ".env.example" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=true" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=true"

# --- doc files in crate dirs don't trigger code scope ---
run_test "crate-dir docs only: docs-only scope" \
    "tasker-pgmq/CLAUDE.md
tasker-shared/README.md
workers/ruby/CHANGELOG.md" \
    "RUN_BUILD_POSTGRES=false" \
    "RUN_BUILD_WORKERS=false" \
    "RUN_CODE_QUALITY=false" \
    "RUN_INTEGRATION_TESTS=false" \
    "RUN_RUBY_FRAMEWORK=false"

# --- ci-tooling + crate-dir docs: ci-tooling scope ---
run_test "ci-tooling + crate docs: ci-tooling scope" \
    "cargo-make/scripts/ci-sanity-check.sh
tasker-pgmq/CLAUDE.md" \
    "RUN_BUILD_POSTGRES=true" \
    "RUN_BUILD_WORKERS=false" \
    "RUN_CODE_QUALITY=true" \
    "RUN_INTEGRATION_TESTS=false" \
    "RUN_RUBY_FRAMEWORK=false"

# --- scope summary checks ---
run_test "scope summary: docs-only" \
    "README.md" \
    "SCOPE_SUMMARY=docs-only: skipping all CI jobs"

run_test "scope summary: full-ci" \
    "proto/tasker/v1/task.proto" \
    "SCOPE_SUMMARY=full-ci: cross-cutting change detected"

run_test "scope summary: scoped ruby" \
    "workers/ruby/lib/handler.rb" \
    "SCOPE_SUMMARY=scoped: ruby"

# ---------------------------------------------------------------------------
# Results
# ---------------------------------------------------------------------------
echo ""
echo "Results: ${PASS} passed, ${FAIL} failed (total: $((PASS + FAIL)))"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
