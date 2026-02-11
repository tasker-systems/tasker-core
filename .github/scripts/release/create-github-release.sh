#!/usr/bin/env bash
set -euo pipefail

# Creates a GitHub Release with a table of published packages.
#
# Env:
#   CORE_VERSION
#   CRATES_RESULT, RUBY_RESULT, PYTHON_RESULT, TS_RESULT, CONTAINERS_RESULT
#   RUBY_VERSION, PYTHON_VERSION, TS_VERSION
#   GH_TOKEN (for gh CLI authentication)

VERSION="${CORE_VERSION}"
TAG="v${VERSION}"

BODY="## Published Packages"$'\n\n'
BODY+="| Package | Version | Registry |"$'\n'
BODY+="|---------|---------|----------|"$'\n'

if [[ "${CRATES_RESULT}" == "success" ]]; then
  BODY+="| tasker-pgmq | ${VERSION} | [crates.io](https://crates.io/crates/tasker-pgmq) |"$'\n'
  BODY+="| tasker-shared | ${VERSION} | [crates.io](https://crates.io/crates/tasker-shared) |"$'\n'
  BODY+="| tasker-client | ${VERSION} | [crates.io](https://crates.io/crates/tasker-client) |"$'\n'
  BODY+="| tasker-orchestration | ${VERSION} | [crates.io](https://crates.io/crates/tasker-orchestration) |"$'\n'
  BODY+="| tasker-worker | ${VERSION} | [crates.io](https://crates.io/crates/tasker-worker) |"$'\n'
  BODY+="| tasker-ctl | ${VERSION} | [crates.io](https://crates.io/crates/tasker-ctl) |"$'\n'
fi

if [[ "${RUBY_RESULT}" == "success" && "${RUBY_VERSION}" != "unchanged" ]]; then
  BODY+="| tasker-rb | ${RUBY_VERSION} | [RubyGems](https://rubygems.org/gems/tasker-rb) |"$'\n'
fi

if [[ "${PYTHON_RESULT}" == "success" && "${PYTHON_VERSION}" != "unchanged" ]]; then
  BODY+="| tasker-py | ${PYTHON_VERSION} | [PyPI](https://pypi.org/project/tasker-py/) |"$'\n'
fi

if [[ "${TS_RESULT}" == "success" && "${TS_VERSION}" != "unchanged" ]]; then
  BODY+="| @tasker-systems/tasker | ${TS_VERSION} | [npm](https://www.npmjs.com/package/@tasker-systems/tasker) |"$'\n'
fi

if [[ "${CONTAINERS_RESULT}" == "success" ]]; then
  BODY+=$'\n'"## Container Images"$'\n\n'
  BODY+="| Image | Registry |"$'\n'
  BODY+="|-------|----------|"$'\n'
  BODY+="| tasker-orchestration | [GHCR](https://ghcr.io/tasker-systems/tasker-orchestration) |"$'\n'
  BODY+="| tasker-worker-ruby | [GHCR](https://ghcr.io/tasker-systems/tasker-worker-ruby) |"$'\n'
  BODY+="| tasker-worker-python | [GHCR](https://ghcr.io/tasker-systems/tasker-worker-python) |"$'\n'
  BODY+="| tasker-worker-typescript | [GHCR](https://ghcr.io/tasker-systems/tasker-worker-typescript) |"$'\n'
fi

gh release create "$TAG" \
  --title "Tasker ${VERSION}" \
  --notes "$BODY" \
  --generate-notes \
  --latest
