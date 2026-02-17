#!/usr/bin/env bash
set -euo pipefail

# Writes version detection results to GITHUB_STEP_SUMMARY.
#
# Env:
#   CORE_CHANGED, FFI_CORE_CHANGED, RUBY_CHANGED, PYTHON_CHANGED, TS_CHANGED, CONTAINERS_CHANGED
#   CORE_VERSION, RUBY_VERSION, PYTHON_VERSION, TS_VERSION
#   RUBY_PUBLISHED, PYTHON_PUBLISHED, TS_PUBLISHED
#   IS_DRY_RUN

published_badge() {
  if [[ "${1:-false}" == "true" ]]; then
    echo "published"
  else
    echo "not yet"
  fi
}

echo "## Release Detection Results" >> "$GITHUB_STEP_SUMMARY"
echo "" >> "$GITHUB_STEP_SUMMARY"
echo "| Component | Changed | Version | Registry |" >> "$GITHUB_STEP_SUMMARY"
echo "|-----------|---------|---------|----------|" >> "$GITHUB_STEP_SUMMARY"
echo "| Core (Rust) | ${CORE_CHANGED} | ${CORE_VERSION} | — |" >> "$GITHUB_STEP_SUMMARY"
echo "| FFI core | ${FFI_CORE_CHANGED:-false} | ${CORE_VERSION} | — |" >> "$GITHUB_STEP_SUMMARY"
echo "| Ruby | ${RUBY_CHANGED} | ${RUBY_VERSION} | $(published_badge "${RUBY_PUBLISHED:-}") |" >> "$GITHUB_STEP_SUMMARY"
echo "| Python | ${PYTHON_CHANGED} | ${PYTHON_VERSION} | $(published_badge "${PYTHON_PUBLISHED:-}") |" >> "$GITHUB_STEP_SUMMARY"
echo "| TypeScript | ${TS_CHANGED} | ${TS_VERSION} | $(published_badge "${TS_PUBLISHED:-}") |" >> "$GITHUB_STEP_SUMMARY"
echo "| Containers | ${CONTAINERS_CHANGED:-false} | ${CORE_VERSION} | — |" >> "$GITHUB_STEP_SUMMARY"
echo "" >> "$GITHUB_STEP_SUMMARY"
echo "**Dry run:** ${IS_DRY_RUN}" >> "$GITHUB_STEP_SUMMARY"

# Log FFI build skip summary if any packages are already published
SKIP_COUNT=0
[[ "${RUBY_PUBLISHED:-false}" == "true" ]] && SKIP_COUNT=$((SKIP_COUNT + 1))
[[ "${PYTHON_PUBLISHED:-false}" == "true" ]] && SKIP_COUNT=$((SKIP_COUNT + 1))
[[ "${TS_PUBLISHED:-false}" == "true" ]] && SKIP_COUNT=$((SKIP_COUNT + 1))
if [[ "$SKIP_COUNT" -gt 0 ]]; then
  echo "" >> "$GITHUB_STEP_SUMMARY"
  echo "**FFI build optimization:** ${SKIP_COUNT}/3 language(s) already published — their FFI builds will be skipped." >> "$GITHUB_STEP_SUMMARY"
fi
