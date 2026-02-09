#!/usr/bin/env bash
set -euo pipefail

# Tags a component release and pushes the tag to origin.
#
# Env:
#   COMPONENT - tag prefix (e.g., "core", "ruby", "python", "typescript")
#   VERSION   - version string (e.g., "0.1.0")

git tag "${COMPONENT}-v${VERSION}"
git push origin "${COMPONENT}-v${VERSION}"
