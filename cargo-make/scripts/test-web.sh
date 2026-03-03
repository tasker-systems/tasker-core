#!/usr/bin/env bash
# =============================================================================
# test-web: DB + messaging tests with automatic infrastructure setup
# =============================================================================
#
# Designed for Claude Code web sessions where PostgreSQL may not be running
# and cargo tools may not be installed. This script:
#
#   1. Installs cargo-nextest and sqlx-cli if missing
#   2. Starts PostgreSQL (native) and configures extensions
#   3. Runs database migrations
#   4. Runs test-messaging level tests (DB + messaging, no services)
#
# Usage:
#   cargo make test-web    # or: cargo make tw
#   ./cargo-make/scripts/test-web.sh  # standalone
#
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
CLAUDE_WEB_DIR="${SCRIPT_DIR}/claude-web"

cd "$PROJECT_DIR"

# Source common helpers
source "${CLAUDE_WEB_DIR}/setup-common.sh"

# ---- Step 1: Ensure cargo tools ----
log_section "Checking cargo tools"

if ! command_exists cargo-nextest; then
  log_install "cargo-nextest"
  cargo install --quiet cargo-nextest --locked
else
  log_ok "cargo-nextest"
fi

if ! command_exists sqlx; then
  log_install "sqlx-cli"
  cargo install --quiet sqlx-cli --no-default-features --features postgres,rustls
else
  log_ok "sqlx-cli"
fi

# ---- Step 2: Setup PostgreSQL ----
source "${CLAUDE_WEB_DIR}/setup-postgres.sh"
setup_postgres

if [ "$PG_READY" != "true" ]; then
  echo ""
  echo "ERROR: PostgreSQL is not available."
  echo "Cannot run test-web without a database. Options:"
  echo "  - Run 'cargo make test-no-infra' for pure unit tests (no DB needed)"
  echo "  - Install PostgreSQL manually"
  exit 1
fi

# ---- Step 3: Run migrations ----
source "${CLAUDE_WEB_DIR}/setup-db-migrations.sh"
setup_db_migrations

# ---- Step 4: Run tests ----
log_section "Running tests (test-messaging level)"

# Source .env for DATABASE_URL and other settings
set -a
source .env 2>/dev/null || true
set +a

echo "  DATABASE_URL: ${DATABASE_URL:-not set}"
echo "  TASKER_ENV: ${TASKER_ENV:-not set}"
echo ""

cargo nextest run --workspace --features test-messaging \
  -E 'not binary(e2e_tests)'

echo ""
echo "test-web complete."
