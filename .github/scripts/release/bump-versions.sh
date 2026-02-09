#!/usr/bin/env bash
set -euo pipefail

# Bumps version files in the working tree before building/publishing.
# Each publish job checks out fresh, so each needs its own version bump.
#
# Env:
#   NEXT_CORE_VERSION        - required, e.g. "0.1.1"
#   NEXT_RUBY_VERSION        - optional, e.g. "0.1.1.0" or "unchanged"
#   NEXT_PYTHON_VERSION      - optional, e.g. "0.1.1.0" or "unchanged"
#   NEXT_TYPESCRIPT_VERSION  - optional, e.g. "0.1.1.0" or "unchanged"

ARGS="--core ${NEXT_CORE_VERSION}"

if [[ -n "${NEXT_RUBY_VERSION:-}" && "${NEXT_RUBY_VERSION}" != "unchanged" ]]; then
  ARGS="${ARGS} --ruby ${NEXT_RUBY_VERSION}"
fi

if [[ -n "${NEXT_PYTHON_VERSION:-}" && "${NEXT_PYTHON_VERSION}" != "unchanged" ]]; then
  ARGS="${ARGS} --python ${NEXT_PYTHON_VERSION}"
fi

if [[ -n "${NEXT_TYPESCRIPT_VERSION:-}" && "${NEXT_TYPESCRIPT_VERSION}" != "unchanged" ]]; then
  ARGS="${ARGS} --typescript ${NEXT_TYPESCRIPT_VERSION}"
fi

echo "Bumping versions: ${ARGS}"
# shellcheck disable=SC2086
./scripts/release/update-versions.sh ${ARGS}
