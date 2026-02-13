#!/usr/bin/env bash
set -euo pipefail

# Checks if any publish or build jobs failed.
#
# Env:
#   CRATES_RESULT, RUBY_RESULT, PYTHON_RESULT, TS_RESULT, CONTAINERS_RESULT
#   FFI_RESULT
#
# Output (GITHUB_OUTPUT):
#   has_failures - "true" or "false"

FAILED=false
for result in "${CRATES_RESULT}" "${RUBY_RESULT}" "${PYTHON_RESULT}" "${TS_RESULT}" "${CONTAINERS_RESULT}" "${FFI_RESULT:-}"; do
  if [[ "$result" == "failure" ]]; then
    FAILED=true
  fi
done

echo "has_failures=$FAILED" >> "$GITHUB_OUTPUT"
if [[ "$FAILED" == "true" ]]; then
  echo "One or more publish jobs failed. Check individual job logs."
  exit 1
fi
