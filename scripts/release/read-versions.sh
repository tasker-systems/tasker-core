#!/usr/bin/env bash
# scripts/release/read-versions.sh
#
# Read committed version numbers from source files.
#
# Used by CI to get versions that were already bumped and committed
# by `release-prepare.sh`, instead of calculating them at release time.
#
# Output:
#   KEY=VALUE pairs suitable for >> $GITHUB_OUTPUT:
#     CORE_VERSION=0.1.1
#     RUBY_VERSION=0.1.1.0
#     PYTHON_VERSION=0.1.1.0
#     TYPESCRIPT_VERSION=0.1.1.0

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# ---------------------------------------------------------------------------
# Core version (from VERSION file)
# ---------------------------------------------------------------------------
VERSION_FILE="${REPO_ROOT}/VERSION"
if [[ ! -f "$VERSION_FILE" ]]; then
    echo "ERROR: VERSION file not found at ${VERSION_FILE}" >&2
    exit 1
fi
CORE_VERSION=$(tr -d '[:space:]' < "$VERSION_FILE")
echo "CORE_VERSION=${CORE_VERSION}"

# ---------------------------------------------------------------------------
# Ruby version (from version.rb)
# ---------------------------------------------------------------------------
RUBY_VERSION_FILE="${REPO_ROOT}/workers/ruby/lib/tasker_core/version.rb"
if [[ -f "$RUBY_VERSION_FILE" ]]; then
    RUBY_VERSION=$(grep -m1 "VERSION = '" "$RUBY_VERSION_FILE" | sed "s/.*VERSION = '\([^']*\)'.*/\1/")
    echo "RUBY_VERSION=${RUBY_VERSION}"
else
    echo "RUBY_VERSION=${CORE_VERSION}" >&2
    echo "RUBY_VERSION=${CORE_VERSION}"
fi

# ---------------------------------------------------------------------------
# Python version (from pyproject.toml)
# ---------------------------------------------------------------------------
PYTHON_PYPROJECT="${REPO_ROOT}/workers/python/pyproject.toml"
if [[ -f "$PYTHON_PYPROJECT" ]]; then
    PYTHON_VERSION=$(grep -m1 '^version = ' "$PYTHON_PYPROJECT" | sed 's/version = "\(.*\)"/\1/')
    echo "PYTHON_VERSION=${PYTHON_VERSION}"
else
    echo "PYTHON_VERSION=${CORE_VERSION}" >&2
    echo "PYTHON_VERSION=${CORE_VERSION}"
fi

# ---------------------------------------------------------------------------
# TypeScript version (from package.json)
# ---------------------------------------------------------------------------
TS_PACKAGE="${REPO_ROOT}/workers/typescript/package.json"
if [[ -f "$TS_PACKAGE" ]]; then
    TYPESCRIPT_VERSION=$(grep -m1 '"version"' "$TS_PACKAGE" | sed 's/.*"version": "\([^"]*\)".*/\1/')
    echo "TYPESCRIPT_VERSION=${TYPESCRIPT_VERSION}"
else
    echo "TYPESCRIPT_VERSION=${CORE_VERSION}" >&2
    echo "TYPESCRIPT_VERSION=${CORE_VERSION}"
fi
