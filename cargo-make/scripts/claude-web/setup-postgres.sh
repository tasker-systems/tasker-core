#!/usr/bin/env bash
# =============================================================================
# PostgreSQL Setup (server, role, database, extensions, template1)
# =============================================================================
#
# Handles the complete PostgreSQL setup for testing:
#   1. Start PostgreSQL server (Docker preferred, native fallback)
#   2. Create tasker role and database
#   3. Install required extensions (pgcrypto, PGMQ)
#   4. Create uuid_generate_v7() compatibility function
#   5. Prepare template1 for sqlx test database creation
#
# The template1 setup is critical: sqlx's #[sqlx::test] macro creates fresh
# databases from template1 for each test. Without PGMQ and uuidv7 in
# template1, these per-test databases fail migration.
#
# Outputs:
#   Sets PG_READY=true/false indicating whether PostgreSQL is available.
#   Sets PSQL_SUPER to the superuser psql command string.
#
# Usage:
#   source cargo-make/scripts/claude-web/setup-common.sh
#   source cargo-make/scripts/claude-web/setup-postgres.sh
#   setup_postgres  # Sets PG_READY
#
# =============================================================================

set -euo pipefail

SETUP_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SETUP_LIB_DIR}/setup-common.sh"

PG_READY=false
PSQL_SUPER=""

# ---------------------------------------------------------------------------
# Strategy 1: Docker (gives us pg18 + PGMQ from our Dockerfile)
# ---------------------------------------------------------------------------
setup_postgres_docker() {
  if ! command_exists docker; then
    return 1
  fi

  if ! docker info >/dev/null 2>&1; then
    return 1
  fi

  echo "  Docker available, starting PostgreSQL with PGMQ via docker-compose..."

  local compose_file="${PROJECT_DIR}/docker/docker-compose.test.yml"
  if [ ! -f "$compose_file" ]; then
    return 1
  fi

  # Start only the postgres service (skip observability, rabbitmq, dragonfly)
  docker compose -f "$compose_file" up -d postgres 2>/dev/null || \
    docker-compose -f "$compose_file" up -d postgres 2>/dev/null || \
    return 1

  # Wait for readiness (up to 30 seconds)
  echo "  Waiting for PostgreSQL..."
  local retries=30
  while [ $retries -gt 0 ]; do
    if pg_isready -h localhost -p 5432 -U tasker -q 2>/dev/null; then
      log_ok "PostgreSQL ready (Docker, pg18 + PGMQ)"
      return 0
    fi
    retries=$((retries - 1))
    sleep 1
  done

  log_warn "PostgreSQL Docker container started but not ready"
  return 1
}

# ---------------------------------------------------------------------------
# Strategy 2: Native PostgreSQL (web environment typically has pg16)
# ---------------------------------------------------------------------------
setup_postgres_native() {
  if ! command_exists psql; then
    return 1
  fi

  # Try to start PostgreSQL if not running
  if ! pg_isready -q 2>/dev/null; then
    sudo service postgresql start 2>/dev/null || \
      sudo systemctl start postgresql 2>/dev/null || \
      true

    local retries=10
    while [ $retries -gt 0 ]; do
      pg_isready -q 2>/dev/null && break
      retries=$((retries - 1))
      sleep 1
    done
  fi

  if ! pg_isready -q 2>/dev/null; then
    return 1
  fi

  echo "  Native PostgreSQL is running"

  # Determine superuser connection method
  if sudo -u postgres psql -c "SELECT 1" >/dev/null 2>&1; then
    PSQL_SUPER="sudo -u postgres psql"
  elif psql -U postgres -c "SELECT 1" >/dev/null 2>&1; then
    PSQL_SUPER="psql -U postgres"
  elif psql -c "SELECT 1" >/dev/null 2>&1; then
    PSQL_SUPER="psql"
  else
    log_warn "Cannot connect to PostgreSQL as superuser"
    return 1
  fi

  # Create tasker role if it doesn't exist
  if ! $PSQL_SUPER -tAc "SELECT 1 FROM pg_roles WHERE rolname='tasker'" 2>/dev/null | grep -q 1; then
    $PSQL_SUPER -c "CREATE ROLE tasker WITH LOGIN PASSWORD 'tasker' SUPERUSER;" 2>/dev/null || true
  fi

  # Create database if it doesn't exist
  if ! $PSQL_SUPER -tAc "SELECT 1 FROM pg_database WHERE datname='tasker_rust_test'" 2>/dev/null | grep -q 1; then
    $PSQL_SUPER -c "CREATE DATABASE tasker_rust_test OWNER tasker;" 2>/dev/null || true
  fi

  log_ok "PostgreSQL configured (native)"
  return 0
}

# ---------------------------------------------------------------------------
# Install PGMQ extension
# ---------------------------------------------------------------------------
setup_pgmq_extension() {
  local db="${1:-tasker_rust_test}"

  # Check if PGMQ is already available
  if $PSQL_SUPER -d "$db" -c "CREATE EXTENSION IF NOT EXISTS pgmq CASCADE;" 2>/dev/null; then
    log_ok "PGMQ extension available in $db"
    return 0
  fi

  # PGMQ not available as system extension - try to install from source
  log_install "PGMQ extension from source"

  if ! command_exists make; then
    log_warn "make not available - cannot build PGMQ from source"
    return 1
  fi

  # Check for pg_config (needed for extension installation)
  if ! command_exists pg_config; then
    # Try to install postgresql-server-dev
    local pg_version
    pg_version=$(psql --version 2>/dev/null | grep -oP '\d+' | head -1)
    if [ -n "$pg_version" ] && command_exists apt-get; then
      log_install "postgresql-server-dev-${pg_version}"
      sudo apt-get install -y -qq "postgresql-server-dev-${pg_version}" 2>/dev/null || true
    fi
  fi

  if ! command_exists pg_config; then
    log_warn "pg_config not available - cannot install PGMQ extension"
    return 1
  fi

  local pgmq_version="${PGMQ_VERSION:-1.8.1}"
  local pgmq_dir="/tmp/pgmq-build"
  rm -rf "$pgmq_dir"

  # Clone and build PGMQ
  if git clone --depth 1 --branch "v${pgmq_version}" \
    https://github.com/tembo-io/pgmq.git "$pgmq_dir" 2>/dev/null; then

    cd "${pgmq_dir}/pgmq-extension"
    sudo make install 2>/dev/null || true

    # The Makefile installs the base SQL but may not create the versioned symlink
    local sharedir
    sharedir="$(pg_config --sharedir)/extension"
    if [ -f "${sharedir}/pgmq.sql" ] && [ ! -f "${sharedir}/pgmq--${pgmq_version}.sql" ]; then
      sudo cp "${sharedir}/pgmq.sql" "${sharedir}/pgmq--${pgmq_version}.sql"
    fi

    cd "${PROJECT_DIR}"
    rm -rf "$pgmq_dir"

    # Try creating the extension again
    if $PSQL_SUPER -d "$db" -c "CREATE EXTENSION IF NOT EXISTS pgmq CASCADE;" 2>/dev/null; then
      log_ok "PGMQ extension installed from source in $db"
      return 0
    fi
  fi

  log_warn "PGMQ extension installation failed - some messaging tests will fail"
  return 1
}

# ---------------------------------------------------------------------------
# Install pgcrypto extension (needed for gen_random_bytes in uuidv7 fallback)
# ---------------------------------------------------------------------------
setup_pgcrypto_extension() {
  local db="$1"
  # Install pgcrypto - it may land in any schema depending on who installed it.
  # We handle this by using a search_path that includes common schemas.
  $PSQL_SUPER -d "$db" -c "CREATE EXTENSION IF NOT EXISTS pgcrypto;" 2>/dev/null || true
}

# ---------------------------------------------------------------------------
# Create uuid_generate_v7() compatibility function
# ---------------------------------------------------------------------------
# pg18 has native uuidv7(). Older versions need a PL/pgSQL fallback.
# The function uses gen_random_bytes (from pgcrypto). Since pgcrypto may be
# installed in different schemas (public, tasker, etc.), the fallback function
# sets search_path to include common schemas where gen_random_bytes might live.
# Functions are created in the public schema so they're accessible regardless
# of the connecting user's default search_path.
setup_uuidv7_function() {
  local db="$1"

  # Ensure pgcrypto is installed somewhere
  setup_pgcrypto_extension "$db"

  $PSQL_SUPER -d "$db" -c "
    DO \$\$
    BEGIN
      -- Try pg18 native uuidv7 first
      PERFORM uuidv7();
      -- If available, create a simple alias in public schema
      CREATE OR REPLACE FUNCTION public.uuid_generate_v7() RETURNS uuid
        AS 'SELECT uuidv7();'
        LANGUAGE SQL VOLATILE PARALLEL SAFE;
      RAISE NOTICE 'uuid_generate_v7: using pg18 native uuidv7()';
    EXCEPTION WHEN undefined_function THEN
      -- Fallback: PL/pgSQL UUID v7 implementation for pg16/pg17
      -- Uses gen_random_uuid() (pg13+ built-in, no extensions needed)
      -- to get 10 random bytes instead of pgcrypto gen_random_bytes()
      CREATE OR REPLACE FUNCTION public.uuidv7() RETURNS uuid AS '
        DECLARE
          timestamp_ms bigint;
          uuid_bytes bytea;
          random_bytes bytea;
        BEGIN
          timestamp_ms := (extract(epoch FROM clock_timestamp()) * 1000)::bigint;
          -- Extract 10 random bytes from a random UUID v4
          random_bytes := substring(
            decode(replace(gen_random_uuid()::text, ''-'', ''''), ''hex'')
          from 1 for 10);
          -- Combine: 6 bytes timestamp + 10 bytes random = 16 bytes
          uuid_bytes := decode(lpad(to_hex(timestamp_ms), 12, ''0''), ''hex'') || random_bytes;
          -- Set version to 7 (0111 in bits 48-51)
          uuid_bytes := set_byte(uuid_bytes, 6, (get_byte(uuid_bytes, 6) & 15) | 112);
          -- Set variant to RFC 4122 (10xx in bits 64-65)
          uuid_bytes := set_byte(uuid_bytes, 8, (get_byte(uuid_bytes, 8) & 63) | 128);
          RETURN encode(uuid_bytes, ''hex'')::uuid;
        END
      ' LANGUAGE plpgsql VOLATILE PARALLEL SAFE;
      CREATE OR REPLACE FUNCTION public.uuid_generate_v7() RETURNS uuid
        AS 'SELECT public.uuidv7();'
        LANGUAGE SQL VOLATILE PARALLEL SAFE;
      RAISE NOTICE 'uuid_generate_v7: using PL/pgSQL fallback with gen_random_uuid()';
    END
    \$\$;
  " 2>/dev/null || log_warn "Could not create uuid_generate_v7 function in $db"
}

# ---------------------------------------------------------------------------
# Prepare template1 for sqlx test databases
# ---------------------------------------------------------------------------
# sqlx's #[sqlx::test] macro creates per-test databases from template1 and
# re-runs migrations. Without PGMQ and uuidv7 in template1, those migrations
# fail with "extension pgmq does not exist" or "function uuidv7() not found".
setup_template1() {
  log_section "Preparing template1 for sqlx test databases"

  # Install pgcrypto in template1 (needed for uuidv7 fallback)
  setup_pgcrypto_extension "template1"

  # Install PGMQ in template1
  if $PSQL_SUPER -d template1 -c "CREATE EXTENSION IF NOT EXISTS pgmq CASCADE;" 2>/dev/null; then
    log_ok "PGMQ in template1"
  else
    log_warn "PGMQ not available in template1 - sqlx::test databases may fail migration"
  fi

  # Create uuidv7 functions in template1
  setup_uuidv7_function "template1"

  # Verify
  if $PSQL_SUPER -d template1 -c "SELECT uuid_generate_v7();" >/dev/null 2>&1; then
    log_ok "uuid_generate_v7() working in template1"
  else
    log_warn "uuid_generate_v7() not working in template1"
  fi
}

# ---------------------------------------------------------------------------
# Main entry point
# ---------------------------------------------------------------------------
setup_postgres() {
  log_section "PostgreSQL database"

  # Try Docker first (preferred: gives us pg18 + PGMQ), then native
  if setup_postgres_docker; then
    PG_READY=true
    # Docker image already has PGMQ and uuidv7 - no extra setup needed
    return 0
  fi

  if setup_postgres_native; then
    PG_READY=true

    # Native PostgreSQL needs manual extension setup
    log_section "PostgreSQL extensions"

    # Install pgcrypto (needed for uuidv7 fallback on pg16/pg17)
    setup_pgcrypto_extension "tasker_rust_test"

    # Install PGMQ extension (non-fatal: messaging tests will fail without it)
    setup_pgmq_extension "tasker_rust_test" || true

    # Create uuid_generate_v7() compatibility function
    setup_uuidv7_function "tasker_rust_test"

    # Verify uuid_generate_v7 works
    if $PSQL_SUPER -d tasker_rust_test -c "SELECT uuid_generate_v7();" >/dev/null 2>&1; then
      log_ok "uuid_generate_v7() working in tasker_rust_test"
    else
      log_warn "uuid_generate_v7() not working - migrations may fail"
    fi

    # Prepare template1 for sqlx test databases
    setup_template1

    return 0
  fi

  log_warn "PostgreSQL not available - database tests will fail"
  log_warn "Compilation will still work using the SQLx offline query cache (.sqlx/)"
  return 0
}

# Run if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  setup_postgres
  echo ""
  echo "PG_READY=$PG_READY"
fi
