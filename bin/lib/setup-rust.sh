#!/usr/bin/env bash
# =============================================================================
# Rust Toolchain Setup
# =============================================================================
#
# Installs Rust via rustup (minimal profile) and adds required components
# (rustfmt, clippy). Ensures cargo env is sourced for the current session.
#
# Idempotent: skips if Rust is already installed.
#
# Usage:
#   source bin/lib/setup-common.sh
#   source bin/lib/setup-rust.sh
#
# =============================================================================

set -euo pipefail

SETUP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SETUP_LIB_DIR}/setup-common.sh"

setup_rust() {
  log_section "Rust toolchain"

  if command_exists cargo; then
    log_ok "Rust $(rustc --version 2>/dev/null | awk '{print $2}' || echo 'installed')"
  else
    log_install "Rust (stable, minimal profile)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
      --default-toolchain stable \
      --profile minimal \
      --no-modify-path

    persist_env 'source "$HOME/.cargo/env"'
    log_ok "Rust installed"
  fi

  # Ensure cargo env is loaded for this session
  if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"
  fi

  # Add required components
  if command_exists rustup; then
    rustup component add rustfmt clippy 2>/dev/null || true
  fi
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  setup_rust
fi
