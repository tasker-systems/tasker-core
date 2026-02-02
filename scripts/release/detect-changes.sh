#!/usr/bin/env bash
# scripts/release/detect-changes.sh
#
# Detect what changed since the last release tag.
#
# Usage:
#   ./scripts/release/detect-changes.sh [--from TAG]
#
# Output:
#   KEY=VALUE pairs suitable for `eval "$(./detect-changes.sh)"`:
#     FFI_CORE_CHANGED=true|false     - tasker-pgmq, tasker-shared, tasker-worker
#     SERVER_CORE_CHANGED=true|false   - tasker-orchestration, tasker-client, tasker-cli
#     CORE_CHANGED=true|false          - any Rust crate changed
#     RUBY_CHANGED=true|false
#     PYTHON_CHANGED=true|false
#     TYPESCRIPT_CHANGED=true|false
#     CHANGES_BASE_REF=<tag|commit>    - the ref we compared against

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "${SCRIPT_DIR}/lib/common.sh"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
FROM_REF=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --from) FROM_REF="$2"; shift 2 ;;
        --from=*) FROM_REF="${1#*=}"; shift ;;
        *) die "Unknown argument: $1" ;;
    esac
done

# ---------------------------------------------------------------------------
# Determine base reference
# ---------------------------------------------------------------------------
if [[ -n "$FROM_REF" ]]; then
    BASE_REF="$FROM_REF"
elif BASE_REF=$(git describe --tags --match 'release-*' --abbrev=0 HEAD 2>/dev/null); then
    : # Found a release-* tag
elif BASE_REF=$(git describe --tags --match 'v*' --abbrev=0 HEAD 2>/dev/null); then
    : # Found a v* tag
else
    # No release tags exist yet — compare against the initial commit
    local_roots=$(git rev-list --max-parents=0 HEAD 2>/dev/null)
    BASE_REF=$(head -n1 <<< "$local_roots")
fi

log_info "Comparing HEAD to ${BASE_REF}" >&2

# ---------------------------------------------------------------------------
# Get changed files
# ---------------------------------------------------------------------------
CHANGED_FILES=$(git diff "${BASE_REF}" HEAD --name-only 2>/dev/null || true)

if [[ -z "$CHANGED_FILES" ]]; then
    log_info "No files changed since ${BASE_REF}" >&2
fi

# ---------------------------------------------------------------------------
# Classify changes
# ---------------------------------------------------------------------------

# Helper: check if any changed file matches a pattern.
# Uses herestring to avoid SIGPIPE with large file lists.
changes_match() {
    local pattern="$1"
    grep -qE "$pattern" <<< "$CHANGED_FILES"
}

# FFI-facing core: changes here require rebuilding all FFI bindings
FFI_CORE_CHANGED=false
if changes_match '^(tasker-pgmq|tasker-shared|tasker-worker)/'; then
    FFI_CORE_CHANGED=true
fi

# Server/client core: changes here only affect Rust crates, no FFI rebuild
SERVER_CORE_CHANGED=false
if changes_match '^(tasker-orchestration|tasker-client|tasker-cli)/'; then
    SERVER_CORE_CHANGED=true
fi

# Any core change means all Rust crates get published
CORE_CHANGED=false
if [[ "$FFI_CORE_CHANGED" == "true" || "$SERVER_CORE_CHANGED" == "true" ]]; then
    CORE_CHANGED=true
fi

# Language bindings
RUBY_CHANGED=false
if changes_match '^workers/ruby/'; then
    RUBY_CHANGED=true
fi

PYTHON_CHANGED=false
if changes_match '^workers/python/'; then
    PYTHON_CHANGED=true
fi

TYPESCRIPT_CHANGED=false
if changes_match '^workers/typescript/'; then
    TYPESCRIPT_CHANGED=true
fi

# ---------------------------------------------------------------------------
# Output — eval-safe KEY=VALUE pairs
# ---------------------------------------------------------------------------
echo "CHANGES_BASE_REF=${BASE_REF}"
echo "FFI_CORE_CHANGED=${FFI_CORE_CHANGED}"
echo "SERVER_CORE_CHANGED=${SERVER_CORE_CHANGED}"
echo "CORE_CHANGED=${CORE_CHANGED}"
echo "RUBY_CHANGED=${RUBY_CHANGED}"
echo "PYTHON_CHANGED=${PYTHON_CHANGED}"
echo "TYPESCRIPT_CHANGED=${TYPESCRIPT_CHANGED}"
