#!/usr/bin/env bash
# =============================================================================
# Protocol Buffers Compiler (protoc) Setup
# =============================================================================
#
# Installs protoc for gRPC proto compilation (required by tasker-shared build.rs).
# Downloads the official release binary to ~/.local/bin.
#
# Idempotent: skips if protoc is already available.
#
# Usage:
#   source cargo-make/scripts/claude-web/setup-common.sh
#   source cargo-make/scripts/claude-web/setup-protoc.sh
#
# =============================================================================

set -euo pipefail

SETUP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SETUP_LIB_DIR}/setup-common.sh"

PROTOC_VERSION="${PROTOC_VERSION:-28.3}"

setup_protoc() {
  log_section "Protocol Buffers compiler (protoc)"

  if command_exists protoc; then
    log_ok "protoc $(protoc --version 2>/dev/null | head -1 || echo 'installed')"
    return 0
  fi

  local protoc_zip="protoc-${PROTOC_VERSION}-linux-x86_64.zip"
  log_install "protoc v${PROTOC_VERSION}"

  curl -sSL -o "/tmp/${protoc_zip}" \
    "https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/${protoc_zip}"
  mkdir -p "$HOME/.local"
  unzip -qo "/tmp/${protoc_zip}" -d "$HOME/.local"
  rm -f "/tmp/${protoc_zip}"

  log_ok "protoc installed to ~/.local/bin"
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  setup_protoc
fi
