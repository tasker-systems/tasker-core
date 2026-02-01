#!/usr/bin/env bash
# =============================================================================
# Shared helper functions for setup scripts
# =============================================================================
#
# Source this file from other setup scripts:
#   SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
#   source "${SCRIPT_DIR}/setup-common.sh"
#
# Provides:
#   - Logging functions (log_section, log_ok, log_skip, log_warn, log_install)
#   - Environment helpers (command_exists, persist_env)
#   - PROJECT_DIR resolution
#
# =============================================================================

# Resolve PROJECT_DIR from any calling script location
# Callers can override by setting PROJECT_DIR before sourcing.
# When run standalone from cargo-make/scripts/claude-web/, go up 3 levels.
# When sourced from bin/setup-claude-web.sh, the caller already sets PROJECT_DIR.
if [ -z "${PROJECT_DIR:-}" ]; then
  _SETUP_COMMON_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "${_SETUP_COMMON_DIR}/../../.." && pwd)}"
  unset _SETUP_COMMON_DIR
fi

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------

log_section() {
  echo ""
  echo "==> $1"
}

log_ok() {
  echo "  [ok] $1"
}

log_skip() {
  echo "  [skip] $1"
}

log_warn() {
  echo "  [warn] $1"
}

log_install() {
  echo "  [install] $1"
}

# ---------------------------------------------------------------------------
# Environment Helpers
# ---------------------------------------------------------------------------

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

persist_env() {
  # Write an export statement to CLAUDE_ENV_FILE for session-wide persistence
  if [ -n "${CLAUDE_ENV_FILE:-}" ]; then
    echo "$1" >> "$CLAUDE_ENV_FILE"
  fi
}
