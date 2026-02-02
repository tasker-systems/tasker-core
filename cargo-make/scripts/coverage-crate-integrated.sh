#!/bin/bash
# =============================================================================
# coverage-crate-integrated.sh
# =============================================================================
# Run coverage for a crate including root integration tests.
#
# Uses cargo llvm-cov show-env for consistent instrumentation across both
# test steps, then cargo llvm-cov report to merge all profraw data.
#
# Usage:
#   CRATE_NAME=tasker-orchestration cargo make coverage-crate-integrated
#
# Prerequisites:
#   - PostgreSQL running (integration tests need a database)
#   - cargo-llvm-cov installed
# =============================================================================

set -euo pipefail

# Source environment like test tasks do
set -a
source .env 2>/dev/null || true
set +a

if [ -z "${CRATE_NAME:-}" ]; then
    echo "CRATE_NAME environment variable not set"
    echo "   Usage: CRATE_NAME=tasker-orchestration cargo make coverage-crate-integrated"
    exit 1
fi

echo "Running integrated coverage for: ${CRATE_NAME}"
mkdir -p coverage-reports/rust

# Clean previous profraw data to start fresh
cargo llvm-cov clean --workspace

# Set up instrumentation environment (shared by both test steps)
eval "$(cargo llvm-cov show-env --export-prefix)"

# Build instrumented workspace
echo "  Building instrumented workspace..."
cargo nextest run --no-run --all-features 2>&1 | tail -1

# Step 1: Run crate's own tests
echo "  Step 1/3: Running ${CRATE_NAME} crate tests..."
cargo nextest run --package "${CRATE_NAME}" --all-features

# Step 2: Run root integration tests (profraw accumulates in same directory)
echo "  Step 2: Running root integration tests..."
cargo nextest run --all-features --test integration_tests || \
    echo "  Note: Some integration tests failed; collecting coverage from what ran."

# Step 2b: Run additional test targets if specified (e.g., e2e_tests)
if [ -n "${EXTRA_TEST_TARGETS:-}" ]; then
    for target in ${EXTRA_TEST_TARGETS}; do
        echo "  Running extra test target: ${target}..."
        cargo nextest run --all-features --test "${target}" || \
            echo "  Note: Some ${target} tests failed; collecting coverage from what ran."
    done
fi

# Step 3: Generate combined report + normalize
echo "  Step 3: Generating report..."
cargo llvm-cov report --json --output-path "coverage-reports/rust/${CRATE_NAME}-raw.json"

uv run --project cargo-make/scripts/coverage python3 cargo-make/scripts/coverage/normalize-rust.py \
  "coverage-reports/rust/${CRATE_NAME}-raw.json" \
  "coverage-reports/rust/${CRATE_NAME}-coverage.json" \
  --crate "${CRATE_NAME}"

echo ""
echo "Integrated coverage complete for ${CRATE_NAME}"
echo "   Raw: coverage-reports/rust/${CRATE_NAME}-raw.json"
echo "   Normalized: coverage-reports/rust/${CRATE_NAME}-coverage.json"
