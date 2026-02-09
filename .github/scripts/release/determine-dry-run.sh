#!/usr/bin/env bash
set -euo pipefail

# Determines if the release should be a dry run.
#
# Env:
#   RELEASE_DRY_RUN_OVERRIDE - global forced dry-run flag
#   EVENT_NAME               - github.event_name
#   INPUT_DRY_RUN            - workflow_dispatch dry_run input
#
# Output (GITHUB_OUTPUT):
#   is_dry_run - "true" or "false"

if [[ "${RELEASE_DRY_RUN_OVERRIDE}" == "true" ]]; then
  echo "is_dry_run=true" >> "$GITHUB_OUTPUT"
  echo "Forced dry-run mode enabled (RELEASE_DRY_RUN=true)"
elif [[ "${EVENT_NAME}" == "workflow_dispatch" ]]; then
  echo "is_dry_run=${INPUT_DRY_RUN}" >> "$GITHUB_OUTPUT"
  echo "Manual trigger: dry_run=${INPUT_DRY_RUN}"
else
  echo "is_dry_run=false" >> "$GITHUB_OUTPUT"
  echo "Tag-triggered release"
fi
