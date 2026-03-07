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

# When run via cargo-make, the script is copied to /tmp. Use CARGO_MAKE env
# vars for the workspace root, falling back to BASH_SOURCE for standalone use.
if [ -n "${CARGO_MAKE_WORKING_DIRECTORY:-}" ]; then
  PROJECT_DIR="${CARGO_MAKE_WORKING_DIRECTORY}"
else
  SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  PROJECT_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
fi
CLAUDE_WEB_DIR="${PROJECT_DIR}/tools/cargo-make/scripts/claude-web"

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

# ---- Step 2b: Fix PGMQ on native pg16 (no .so library needed) ----
# Native pg16 doesn't ship PGMQ. The setup_postgres script installs the SQL
# files from source, but the control file references a C module that doesn't
# exist for pure-SQL PGMQ. Patch it and ensure the extension is created.
fix_pgmq_native() {
  local control="/usr/share/postgresql/16/extension/pgmq.control"
  local sharedir="/usr/share/postgresql/16/extension"
  local psql_super="${PSQL_SUPER:-sudo -u postgres psql}"

  # Only needed for native pg16 (not Docker which ships pre-built PGMQ)
  if [ ! -f "$control" ]; then
    return 0
  fi

  # Remove module_pathname if it references a missing .so
  if grep -q "module_pathname" "$control" 2>/dev/null; then
    local libdir
    libdir="$(pg_config --pkglibdir 2>/dev/null || echo /usr/lib/postgresql/16/lib)"
    if [ ! -f "${libdir}/pgmq.so" ]; then
      log_section "Patching PGMQ for pure-SQL mode (pg16)"
      sudo sed -i "s/^module_pathname = .*//" "$control"
      log_ok "Removed module_pathname from pgmq.control"
    fi
  fi

  # Ensure base install SQL exists
  if [ ! -f "${sharedir}/pgmq--1.8.1.sql" ]; then
    local pgmq_dir="/tmp/pgmq-install"
    rm -rf "$pgmq_dir"
    if git clone --depth 1 --branch v1.8.1 https://github.com/tembo-io/pgmq.git "$pgmq_dir" 2>/dev/null; then
      sudo cp "${pgmq_dir}/pgmq-extension/sql/pgmq.sql" "${sharedir}/pgmq--1.8.1.sql"
      rm -rf "$pgmq_dir"
      log_ok "Installed pgmq--1.8.1.sql"
    fi
  fi

  # Create extension in tasker_rust_test and template1
  for db in tasker_rust_test template1; do
    $psql_super -d "$db" -c "CREATE EXTENSION IF NOT EXISTS pgmq CASCADE;" 2>/dev/null || true
  done
}
fix_pgmq_native

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

# Use --lib to run library unit tests only (includes #[sqlx::test] tests).
# This excludes heavyweight integration/E2E tests that take 10+ minutes each.
# For full integration tests, use: cargo make test-rust-unit (tu)
#
# Uses --profile ci for sensible timeouts (60s slow warning, terminate at 120s)
# which prevents PGMQ LISTEN/NOTIFY tests from hanging indefinitely.
cargo nextest run --workspace --features test-messaging --lib --profile ci

echo ""
echo "test-web complete."
