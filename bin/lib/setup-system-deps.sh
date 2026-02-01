#!/usr/bin/env bash
# =============================================================================
# System Dependencies Setup (apt packages)
# =============================================================================
#
# Installs required system libraries for building Rust crates with native
# dependencies (OpenSSL, libpq, protobuf).
#
# Idempotent: skips packages that are already installed.
#
# Usage:
#   source bin/lib/setup-common.sh
#   source bin/lib/setup-system-deps.sh
#
# =============================================================================

set -euo pipefail

SETUP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SETUP_LIB_DIR}/setup-common.sh"

setup_system_deps() {
  log_section "System dependencies"

  if ! command_exists apt-get; then
    log_skip "apt-get not available (non-Debian system)"
    return 0
  fi

  local pkgs_needed=""

  for pkg in libssl-dev libpq-dev pkg-config cmake jq unzip curl; do
    if ! dpkg -l "$pkg" >/dev/null 2>&1; then
      pkgs_needed="$pkgs_needed $pkg"
    fi
  done

  if [ -n "$pkgs_needed" ]; then
    log_install "apt packages:$pkgs_needed"
    sudo apt-get update -qq 2>/dev/null
    sudo apt-get install -y -qq $pkgs_needed 2>/dev/null
  else
    log_ok "all system packages present"
  fi
}

# Run if executed directly (not sourced)
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  setup_system_deps
fi
