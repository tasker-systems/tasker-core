#!/usr/bin/env bash
# =============================================================================
# grpcurl Setup (optional, for gRPC API testing)
# =============================================================================
#
# Downloads the grpcurl binary for testing gRPC endpoints (TAS-177).
# Non-critical: failures are logged as warnings.
#
# Idempotent: skips if grpcurl is already available.
#
# Usage:
#   source cargo-make/scripts/claude-web/setup-common.sh
#   source cargo-make/scripts/claude-web/setup-grpcurl.sh
#
# =============================================================================

set -euo pipefail

SETUP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SETUP_LIB_DIR}/setup-common.sh"

GRPCURL_VERSION="${GRPCURL_VERSION:-1.9.1}"

setup_grpcurl() {
  log_section "gRPC tooling"

  if command_exists grpcurl; then
    log_ok "grpcurl"
    return 0
  fi

  local grpcurl_tar="grpcurl_${GRPCURL_VERSION}_linux_x86_64.tar.gz"
  log_install "grpcurl v${GRPCURL_VERSION}"

  curl -sSL -o "/tmp/${grpcurl_tar}" \
    "https://github.com/fullstorydev/grpcurl/releases/download/v${GRPCURL_VERSION}/${grpcurl_tar}" 2>/dev/null && \
  mkdir -p "$HOME/.local/bin" && \
  tar -xzf "/tmp/${grpcurl_tar}" -C "$HOME/.local/bin" grpcurl 2>/dev/null && \
  chmod +x "$HOME/.local/bin/grpcurl" && \
  rm -f "/tmp/${grpcurl_tar}" && \
  log_ok "grpcurl installed to ~/.local/bin" || \
  log_warn "grpcurl installation failed (non-critical)"
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  setup_grpcurl
fi
