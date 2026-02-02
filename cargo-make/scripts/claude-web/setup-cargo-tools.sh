#!/usr/bin/env bash
# =============================================================================
# Cargo Binary Tools Setup
# =============================================================================
#
# Installs cargo-make, sqlx-cli, and cargo-nextest. These are the minimum
# tools needed for building, testing, and database management.
#
# Idempotent: skips tools that are already installed.
#
# Usage:
#   source cargo-make/scripts/claude-web/setup-common.sh
#   source cargo-make/scripts/claude-web/setup-cargo-tools.sh
#
# =============================================================================

set -euo pipefail

SETUP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SETUP_LIB_DIR}/setup-common.sh"

install_cargo_tool() {
  local binary="$1"
  local crate="$2"
  shift 2
  local extra_args=("$@")

  if command_exists "$binary" || cargo install --list 2>/dev/null | grep -q "^${crate} "; then
    log_ok "$crate"
  else
    log_install "$crate"
    cargo install --quiet "$crate" "${extra_args[@]}"
  fi
}

setup_cargo_tools() {
  log_section "Cargo tools"

  install_cargo_tool cargo-make cargo-make --locked
  install_cargo_tool sqlx        sqlx-cli  --no-default-features --features postgres,rustls
  install_cargo_tool cargo-nextest cargo-nextest --locked
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  setup_cargo_tools
fi
