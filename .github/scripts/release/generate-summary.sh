#!/usr/bin/env bash
set -euo pipefail

# Generates the release summary table as GITHUB_STEP_SUMMARY.
#
# Env:
#   IS_DRY_RUN
#   CRATES_RESULT, RUBY_RESULT, PYTHON_RESULT, TS_RESULT
#   NEXT_CORE_VERSION, NEXT_RUBY_VERSION, NEXT_PYTHON_VERSION, NEXT_TS_VERSION

echo "## Release Summary" >> "$GITHUB_STEP_SUMMARY"
echo "" >> "$GITHUB_STEP_SUMMARY"

if [[ "${IS_DRY_RUN}" == "true" ]]; then
  echo "> **DRY RUN** — No packages were actually published." >> "$GITHUB_STEP_SUMMARY"
  echo "" >> "$GITHUB_STEP_SUMMARY"
fi

echo "| Component | Result | Version |" >> "$GITHUB_STEP_SUMMARY"
echo "|-----------|--------|---------|" >> "$GITHUB_STEP_SUMMARY"
echo "| Rust crates | ${CRATES_RESULT} | ${NEXT_CORE_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| Ruby gem | ${RUBY_RESULT} | ${NEXT_RUBY_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| Python wheel | ${PYTHON_RESULT} | ${NEXT_PYTHON_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
echo "| TypeScript | ${TS_RESULT} | ${NEXT_TS_VERSION} |" >> "$GITHUB_STEP_SUMMARY"
