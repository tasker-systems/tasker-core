#!/usr/bin/env bash
set -euo pipefail

# Logs what GitHub Release would have been created in dry-run mode.
#
# Env:
#   NEXT_CORE_VERSION

VERSION="${NEXT_CORE_VERSION}"
echo "DRY RUN: Would create GitHub Release v${VERSION}"
echo "  gh release create v${VERSION} --title 'Tasker ${VERSION}' --generate-notes --latest"
echo "" >> "$GITHUB_STEP_SUMMARY"
echo "**Would create:** GitHub Release \`v${VERSION}\`" >> "$GITHUB_STEP_SUMMARY"
