#!/usr/bin/env bash
# =============================================================================
# Database Migrations Setup
# =============================================================================
#
# Runs sqlx migrations against the test database. Should be called after
# PostgreSQL is ready and extensions are installed.
#
# Usage:
#   source bin/lib/setup-common.sh
#   source bin/lib/setup-db-migrations.sh
#   DB_READY=true setup_db_migrations
#
# =============================================================================

set -euo pipefail

SETUP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SETUP_LIB_DIR}/setup-common.sh"

setup_db_migrations() {
  if [ "${PG_READY:-false}" != "true" ]; then
    log_skip "PostgreSQL not ready - skipping migrations"
    return 0
  fi

  if ! command_exists sqlx; then
    log_skip "sqlx-cli not installed - skipping migrations"
    return 0
  fi

  log_section "Database migrations"

  cd "${PROJECT_DIR}"
  export DATABASE_URL="${DATABASE_URL:-postgresql://tasker:tasker@localhost:5432/tasker_rust_test}"

  # Create database if it doesn't exist (sqlx database create is idempotent)
  sqlx database create 2>/dev/null || true

  if sqlx migrate run 2>/dev/null; then
    log_ok "migrations applied"
  else
    log_warn "migrations failed - you may need to run them manually"
    log_warn "try: cargo make db-migrate"
  fi
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  setup_db_migrations
fi
