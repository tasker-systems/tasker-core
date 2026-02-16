#!/usr/bin/env bash
# =============================================================================
# GitHub CLI (gh) Setup (optional, for PR creation and GitHub API)
# =============================================================================
#
# Downloads the gh CLI binary for creating pull requests, viewing issues,
# and interacting with the GitHub API from Claude Code sessions.
# Non-critical: failures are logged as warnings.
#
# Idempotent: skips if gh is already available.
#
# Usage:
#   source cargo-make/scripts/claude-web/setup-common.sh
#   source cargo-make/scripts/claude-web/setup-gh.sh
#
# =============================================================================

set -euo pipefail

SETUP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SETUP_LIB_DIR}/setup-common.sh"

GH_VERSION="${GH_VERSION:-2.65.0}"

setup_gh() {
  log_section "GitHub CLI"

  if command_exists gh; then
    log_ok "gh"
    return 0
  fi

  local gh_tar="gh_${GH_VERSION}_linux_amd64.tar.gz"
  log_install "gh v${GH_VERSION}"

  curl -sSL -o "/tmp/${gh_tar}" \
    "https://github.com/cli/cli/releases/download/v${GH_VERSION}/${gh_tar}" 2>/dev/null && \
  mkdir -p "$HOME/.local/bin" && \
  tar -xzf "/tmp/${gh_tar}" -C /tmp "gh_${GH_VERSION}_linux_amd64/bin/gh" 2>/dev/null && \
  mv "/tmp/gh_${GH_VERSION}_linux_amd64/bin/gh" "$HOME/.local/bin/gh" && \
  chmod +x "$HOME/.local/bin/gh" && \
  rm -rf "/tmp/${gh_tar}" "/tmp/gh_${GH_VERSION}_linux_amd64" && \
  log_ok "gh installed to ~/.local/bin" || \
  log_warn "gh installation failed (non-critical)"
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  setup_gh
fi
