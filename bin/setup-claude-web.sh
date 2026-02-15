#!/usr/bin/env bash
# =============================================================================
# Tasker Core - Claude Code on the Web Environment Setup
# =============================================================================
#
# Companion to bin/setup-dev.sh (macOS). This script sets up the Claude Code
# on the web (remote) environment with the tools needed for Rust development,
# PostgreSQL, and environment configuration.
#
# This script is invoked automatically via .claude/settings.json SessionStart
# hook when running in a Claude Code remote environment. It is idempotent and
# safe to run on session resume/compact events.
#
# What it installs:
#   - System libraries (libssl-dev, libpq-dev, pkg-config, cmake)
#   - Protocol Buffers compiler (protoc)
#   - Rust toolchain (if not present)
#   - cargo-make, sqlx-cli, cargo-nextest
#   - GitHub CLI (gh) for PR creation and GitHub API
#   - PostgreSQL database (via Docker or native) with PGMQ + uuidv7
#   - Redis cache server
#   - Project environment variables
#
# What it skips (to minimize overhead):
#   - Profiling tools (samply, flamegraph, tokio-console)
#   - Code quality tools (cargo-audit, cargo-machete, cargo-llvm-cov)
#   - Ruby/Python/TypeScript runtimes (focus on Rust core)
#   - Telemetry stack (Grafana, OTLP)
#
# Architecture:
#   This script composes smaller, isolated scripts from cargo-make/scripts/claude-web/
#   that each handle a specific concern. Each script can also be run independently
#   for debugging or partial setup. Keeping them alongside other cargo-make scripts
#   makes it easier to audit and unify shared patterns.
#
# Usage:
#   ./bin/setup-claude-web.sh           # Auto-invoked by SessionStart hook
#   FORCE_SETUP=1 ./bin/setup-claude-web.sh  # Run even outside remote env
#
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Environment Guard
# ---------------------------------------------------------------------------
# Only run in Claude Code remote environments unless FORCE_SETUP is set
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ] && [ "${FORCE_SETUP:-}" != "1" ]; then
  exit 0
fi

# ---------------------------------------------------------------------------
# Bootstrap
# ---------------------------------------------------------------------------
PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
LIB_DIR="${PROJECT_DIR}/cargo-make/scripts/claude-web"

# Ensure common tool directories are in PATH from the start
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

# Source the shared helpers (makes log_*, command_exists, persist_env available)
source "${LIB_DIR}/setup-common.sh"

# Persist PATH for the Claude session
persist_env 'export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"'

echo ""
echo "==> Setting up tasker-core for Claude Code on the web"
echo "  Project: $PROJECT_DIR"

# ---------------------------------------------------------------------------
# Phase 1: System-level dependencies
# ---------------------------------------------------------------------------
source "${LIB_DIR}/setup-system-deps.sh"
setup_system_deps

source "${LIB_DIR}/setup-protoc.sh"
setup_protoc

# ---------------------------------------------------------------------------
# Phase 2: Rust toolchain and cargo tools
# ---------------------------------------------------------------------------
source "${LIB_DIR}/setup-rust.sh"
setup_rust

source "${LIB_DIR}/setup-cargo-tools.sh"
setup_cargo_tools

# ---------------------------------------------------------------------------
# Phase 3: Optional tools
# ---------------------------------------------------------------------------
source "${LIB_DIR}/setup-grpcurl.sh"
setup_grpcurl

source "${LIB_DIR}/setup-gh.sh"
setup_gh

# ---------------------------------------------------------------------------
# Phase 4: Data services (PostgreSQL + Redis)
# ---------------------------------------------------------------------------
source "${LIB_DIR}/setup-postgres.sh"
setup_postgres

source "${LIB_DIR}/setup-redis.sh"
setup_redis

# ---------------------------------------------------------------------------
# Phase 5: Environment configuration
# ---------------------------------------------------------------------------
log_section "Environment configuration"

# Core project paths
persist_env "export WORKSPACE_PATH=\"$PROJECT_DIR\""
persist_env "export TASKER_CONFIG_ROOT=\"$PROJECT_DIR/config\""
persist_env "export TASKER_FIXTURE_PATH=\"$PROJECT_DIR/tests/fixtures\""
persist_env 'export TASKER_ENV="test"'
persist_env 'export RUST_LOG="warn"'
persist_env 'export LOG_LEVEL="warn"'

# Database
persist_env 'export DATABASE_URL="postgresql://tasker:tasker@localhost:5432/tasker_rust_test"'
persist_env 'export TEST_DATABASE_URL="postgresql://tasker:tasker@localhost:5432/tasker_rust_test"'

# PGMQ database URL (must be explicitly set - empty default causes hangs)
persist_env 'export PGMQ_DATABASE_URL="postgresql://tasker:tasker@localhost:5432/tasker_rust_test"'

# Messaging: default to pgmq (simpler, no RabbitMQ needed)
persist_env 'export TASKER_MESSAGING_BACKEND="pgmq"'

# Config and template paths
persist_env "export TASKER_CONFIG_PATH=\"$PROJECT_DIR/config/tasker/generated/complete-test.toml\""
persist_env "export TASKER_TEMPLATE_PATH=\"$PROJECT_DIR/config/tasks\""

# Web API
persist_env 'export WEB_API_ENABLED="true"'
persist_env 'export WEB_API_TLS_ENABLED="false"'
persist_env "export WEB_API_TLS_CERT_PATH=\"$PROJECT_DIR/tests/web/certs/server.crt\""
persist_env "export WEB_API_TLS_KEY_PATH=\"$PROJECT_DIR/tests/web/certs/server.key\""

# Authentication
persist_env 'export WEB_AUTH_ENABLED="true"'
persist_env 'export WEB_JWT_ISSUER="tasker-core-test"'
persist_env 'export WEB_JWT_AUDIENCE="tasker-api-test"'
persist_env 'export WEB_JWT_TOKEN_EXPIRY_HOURS="1"'

# CORS
persist_env 'export WEB_CORS_ENABLED="true"'
persist_env 'export WEB_CORS_ALLOWED_ORIGINS="http://localhost:3000,https://localhost:3000"'

# Rate limiting (disabled for test)
persist_env 'export WEB_RATE_LIMITING_ENABLED="false"'

# Circuit breaker
persist_env 'export WEB_CIRCUIT_BREAKER_ENABLED="true"'

# Resource monitoring
persist_env 'export WEB_RESOURCE_MONITORING_ENABLED="true"'

# Redis
persist_env 'export REDIS_URL="redis://localhost:6379"'

log_ok "environment variables persisted to session"

# ---------------------------------------------------------------------------
# Phase 5b: Git hooks
# ---------------------------------------------------------------------------
log_section "Git hooks"
if git rev-parse --is-inside-work-tree &>/dev/null; then
  git config core.hooksPath .githooks
  chmod +x "$PROJECT_DIR/.githooks/"* 2>/dev/null || true
  log_ok "git hooks configured (.githooks/pre-commit)"
else
  log_warn "not a git repo, skipping hooks"
fi

# ---------------------------------------------------------------------------
# Fallback .env generator (used when cargo-make is not available)
# ---------------------------------------------------------------------------
generate_fallback_env() {
  cat > "$PROJECT_DIR/.env" << ENVEOF
# Generated by setup-claude-web.sh (fallback)
WORKSPACE_PATH=$PROJECT_DIR
TASKER_CONFIG_ROOT=$PROJECT_DIR/config
TASKER_FIXTURE_PATH=$PROJECT_DIR/tests/fixtures
TASKER_ENV=test
RUST_LOG=warn
LOG_LEVEL=warn
DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test
TEST_DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test
PGMQ_DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test
TASKER_MESSAGING_BACKEND=pgmq
TASKER_CONFIG_PATH=$PROJECT_DIR/config/tasker/generated/complete-test.toml
TASKER_TEMPLATE_PATH=$PROJECT_DIR/config/tasks
WEB_API_ENABLED=true
WEB_API_TLS_ENABLED=false
WEB_AUTH_ENABLED=true
WEB_JWT_ISSUER=tasker-core-test
WEB_JWT_AUDIENCE=tasker-api-test
WEB_JWT_TOKEN_EXPIRY_HOURS=1
WEB_CORS_ENABLED=true
WEB_RATE_LIMITING_ENABLED=false
WEB_CIRCUIT_BREAKER_ENABLED=true
WEB_RESOURCE_MONITORING_ENABLED=true
REDIS_URL=redis://localhost:6379
ENVEOF
  log_ok "minimal .env created"
}

# ---------------------------------------------------------------------------
# Phase 6: Generate .env file (via cargo make)
# ---------------------------------------------------------------------------
log_section "Generating .env file"

export WORKSPACE_PATH="$PROJECT_DIR"

if command_exists cargo-make; then
  cd "$PROJECT_DIR"
  if cargo make setup-env-claude-web 2>/dev/null; then
    log_ok ".env generated via cargo make setup-env-claude-web"
  elif cargo make setup-env 2>/dev/null; then
    log_ok ".env generated via cargo make setup-env (fallback)"
  else
    log_warn "cargo make setup-env failed - creating minimal .env"
    generate_fallback_env
  fi
else
  log_warn "cargo-make not available - creating minimal .env"
  generate_fallback_env
fi

# ---------------------------------------------------------------------------
# Phase 7: Database migrations
# ---------------------------------------------------------------------------
source "${LIB_DIR}/setup-db-migrations.sh"
setup_db_migrations

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
log_section "Setup complete"

echo ""
echo "  Tools installed:"
command_exists protoc     && echo "    protoc:        $(protoc --version 2>/dev/null | head -1 || echo 'yes')"
command_exists cargo      && echo "    cargo:         $(cargo --version 2>/dev/null | awk '{print $2}' || echo 'yes')"
command_exists cargo-make && echo "    cargo-make:    yes"
command_exists sqlx       && echo "    sqlx-cli:      yes"
command_exists cargo-nextest && echo "    cargo-nextest: yes"
command_exists grpcurl    && echo "    grpcurl:       yes"
command_exists gh         && echo "    gh:            $(gh --version 2>/dev/null | head -1 | awk '{print $3}' || echo 'yes')"
echo ""

echo "  Services:"
if [ "${PG_READY:-false}" = true ]; then
  echo "    PostgreSQL:    ready"
else
  echo "    PostgreSQL:    NOT available (compilation will use .sqlx/ cache)"
fi
if [ "${REDIS_READY:-false}" = true ]; then
  echo "    Redis:         ready"
else
  echo "    Redis:         NOT available (cache-dependent tests may fail)"
fi
echo ""

echo "  Quick start:"
echo "    cargo make check    # Run all quality checks"
echo "    cargo make build    # Build everything"
if [ "${PG_READY:-false}" = true ]; then
  echo "    cargo make test     # Run tests"
fi
echo ""
