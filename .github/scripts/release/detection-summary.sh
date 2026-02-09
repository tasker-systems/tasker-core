#!/usr/bin/env bash
set -euo pipefail

# Writes version detection results to GITHUB_STEP_SUMMARY.
#
# Env:
#   CORE_CHANGED, RUBY_CHANGED, PYTHON_CHANGED, TS_CHANGED
#   NEXT_CORE_VERSION, NEXT_RUBY_VERSION, NEXT_PYTHON_VERSION, NEXT_TS_VERSION
#   IS_DRY_RUN

echo "## Release Detection Results" >> "$GITHUB_STEP_SUMMARY"
echo "" >> "$GITHUB_STEP_SUMMARY"
echo "| Component | Changed | Next Version |" >> "$GITHUB_STEP_SUMMARY"
echo "|-----------|---------|--------------|" >> "$GITHUB_STEP_SUMMARY"
echo "| Core (Rust) | ${CORE_CHANGED} | ${NEXT_CORE_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| Ruby | ${RUBY_CHANGED} | ${NEXT_RUBY_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| Python | ${PYTHON_CHANGED} | ${NEXT_PYTHON_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| TypeScript | ${TS_CHANGED} | ${NEXT_TS_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "" >> "$GITHUB_STEP_SUMMARY"
echo "**Dry run:** ${IS_DRY_RUN}" >> "$GITHUB_STEP_SUMMARY"
