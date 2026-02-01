#!/usr/bin/env bash
# =============================================================================
# Redis Cache Server Setup
# =============================================================================
#
# Starts a Redis server for the distributed template cache (TAS-156).
# The test configuration has cache.backend = "redis" with enabled = true,
# so Redis must be running or SystemContext initialization will hang
# on the connection timeout.
#
# Strategies (in order):
#   1. Redis already running - skip
#   2. redis-server available - start as daemon
#   3. Docker available - start redis container
#   4. Not available - warn (tests requiring cache will hang/fail)
#
# Idempotent: skips if Redis is already responding to PING.
#
# Usage:
#   source cargo-make/scripts/claude-web/setup-common.sh
#   source cargo-make/scripts/claude-web/setup-redis.sh
#   setup_redis  # Sets REDIS_READY
#
# =============================================================================

set -euo pipefail

SETUP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SETUP_LIB_DIR}/setup-common.sh"

REDIS_READY=false

setup_redis() {
  log_section "Redis cache server"

  # Check if Redis is already running
  if command_exists redis-cli && redis-cli ping >/dev/null 2>&1; then
    log_ok "Redis already running"
    REDIS_READY=true
    return 0
  fi

  # Strategy 1: Start native redis-server
  if command_exists redis-server; then
    log_install "starting redis-server (daemonized)"
    redis-server --daemonize yes 2>/dev/null || true
    sleep 1

    if redis-cli ping >/dev/null 2>&1; then
      log_ok "Redis started"
      REDIS_READY=true
      return 0
    fi
  fi

  # Strategy 2: Install via apt if available
  if command_exists apt-get && ! command_exists redis-server; then
    log_install "redis-server via apt"
    sudo apt-get install -y -qq redis-server 2>/dev/null || true

    if command_exists redis-server; then
      redis-server --daemonize yes 2>/dev/null || true
      sleep 1
      if redis-cli ping >/dev/null 2>&1; then
        log_ok "Redis installed and started"
        REDIS_READY=true
        return 0
      fi
    fi
  fi

  # Strategy 3: Docker
  if command_exists docker && docker info >/dev/null 2>&1; then
    log_install "Redis via Docker"
    docker run -d --name tasker-redis -p 6379:6379 redis:7-alpine 2>/dev/null || true
    sleep 2

    if command_exists redis-cli && redis-cli ping >/dev/null 2>&1; then
      log_ok "Redis started (Docker)"
      REDIS_READY=true
      return 0
    fi
  fi

  log_warn "Redis not available - tests requiring cache will hang or fail"
  log_warn "Install redis-server or ensure Docker is available"
  return 0
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  setup_redis
  echo ""
  echo "REDIS_READY=$REDIS_READY"
fi
