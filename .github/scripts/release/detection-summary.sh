#!/usr/bin/env bash
set -euo pipefail

# Writes version detection results to GITHUB_STEP_SUMMARY.
#
# Env:
#   CORE_CHANGED, FFI_CORE_CHANGED, RUBY_CHANGED, PYTHON_CHANGED, TS_CHANGED, CONTAINERS_CHANGED
#   CORE_VERSION, RUBY_VERSION, PYTHON_VERSION, TS_VERSION
#   IS_DRY_RUN

echo "## Release Detection Results" >> "$GITHUB_STEP_SUMMARY"
echo "" >> "$GITHUB_STEP_SUMMARY"
echo "| Component | Changed | Version |" >> "$GITHUB_STEP_SUMMARY"
echo "|-----------|---------|---------|" >> "$GITHUB_STEP_SUMMARY"
echo "| Core (Rust) | ${CORE_CHANGED} | ${CORE_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| FFI core | ${FFI_CORE_CHANGED:-false} | ${CORE_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| Ruby | ${RUBY_CHANGED} | ${RUBY_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| Python | ${PYTHON_CHANGED} | ${PYTHON_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| TypeScript | ${TS_CHANGED} | ${TS_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| Containers | ${CONTAINERS_CHANGED:-false} | ${CORE_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "" >> "$GITHUB_STEP_SUMMARY"
echo "**Dry run:** ${IS_DRY_RUN}" >> "$GITHUB_STEP_SUMMARY"
