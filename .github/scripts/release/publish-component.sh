#!/usr/bin/env bash
set -euo pipefail

# Generic wrapper to publish a component with dry-run support.
# All four publish jobs (crates, ruby, python, typescript) use this script.
#
# Env:
#   PUBLISH_SCRIPT - path to the component's publish script (e.g., ./scripts/release/publish-crates.sh)
#   VERSION        - version to publish
#   IS_DRY_RUN     - "true" for dry-run, anything else for real publish

DRY_RUN_FLAG=""
if [[ "${IS_DRY_RUN}" == "true" ]]; then
  DRY_RUN_FLAG="--dry-run"
fi

# shellcheck disable=SC2086
"${PUBLISH_SCRIPT}" "${VERSION}" ${DRY_RUN_FLAG} --on-duplicate=skip
