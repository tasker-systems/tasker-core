#!/usr/bin/env bash
set -euo pipefail

# Tags a component release and pushes the tag to origin.
# Idempotent: skips if the tag already exists locally or on the remote.
#
# Env:
#   COMPONENT - tag prefix (e.g., "core", "ruby", "python", "typescript")
#   VERSION   - version string (e.g., "0.1.1")

TAG="${COMPONENT}-v${VERSION}"

if git rev-parse "$TAG" >/dev/null 2>&1; then
    echo "Tag $TAG already exists locally, skipping"
elif git ls-remote --tags origin "$TAG" | grep -q "$TAG"; then
    echo "Tag $TAG already exists on remote, skipping"
else
    git tag "$TAG"
    git push origin "$TAG"
fi
