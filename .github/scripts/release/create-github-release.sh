#!/usr/bin/env bash
set -euo pipefail

# Creates a GitHub Release with a table of published packages.
#
# Env:
#   NEXT_CORE_VERSION
#   CRATES_RESULT, RUBY_RESULT, PYTHON_RESULT, TS_RESULT
#   NEXT_RUBY_VERSION, NEXT_PYTHON_VERSION, NEXT_TS_VERSION
#   GH_TOKEN (for gh CLI authentication)

VERSION="${NEXT_CORE_VERSION}"
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
  BODY+="| tasker-cli | ${VERSION} | [crates.io](https://crates.io/crates/tasker-cli) |"$'\n'
fi

RUBY_VERSION="${NEXT_RUBY_VERSION}"
if [[ "${RUBY_RESULT}" == "success" && "${RUBY_VERSION}" != "unchanged" ]]; then
  BODY+="| tasker-rb | ${RUBY_VERSION} | [RubyGems](https://rubygems.org/gems/tasker-rb) |"$'\n'
fi

PY_VERSION="${NEXT_PYTHON_VERSION}"
if [[ "${PYTHON_RESULT}" == "success" && "${PY_VERSION}" != "unchanged" ]]; then
  BODY+="| tasker-py | ${PY_VERSION} | [PyPI](https://pypi.org/project/tasker-py/) |"$'\n'
fi

TS_VERSION="${NEXT_TS_VERSION}"
if [[ "${TS_RESULT}" == "success" && "${TS_VERSION}" != "unchanged" ]]; then
  BODY+="| @tasker-systems/tasker | ${TS_VERSION} | [npm](https://www.npmjs.com/package/@tasker-systems/tasker) |"$'\n'
fi

gh release create "$TAG" \
  --title "Tasker ${VERSION}" \
  --notes "$BODY" \
  --generate-notes \
  --latest
