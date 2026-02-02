# Skill: Database and SQLx

## When to Use

Use this skill when working with database operations, SQLx queries, migrations, the PGMQ message queue, SQL functions, or troubleshooting database connectivity.

## Database Stack

- **PostgreSQL 16** with PGMQ extension
- **SQLx** for compile-time verified SQL queries
- **PGMQ** for PostgreSQL-native message queuing (default messaging backend)
- **RabbitMQ** as optional alternative messaging backend

## Critical Rules

1. **Never use `SQLX_OFFLINE=true`** -- always export `DATABASE_URL`
2. **Always use `--all-features`** for builds that include SQLx queries
3. After modifying any `sqlx::query!` macro, SQL, or schema: update the SQLx cache

## SQLx Query Cache

The `.sqlx/` directory contains cached query metadata for CI builds where no database is available.

### Updating the Cache

```bash
DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test \
cargo sqlx prepare --workspace -- --all-targets --all-features

git add .sqlx/
```

### When to Update

- Adding new `sqlx::query!` or `sqlx::query_as!` macros
- Modifying SQL in existing query macros
- Changing database schema (new migrations)
- Seeing "SQLX_OFFLINE but no cached data" errors

### Using cargo-make

```bash
cargo make sqlx-prepare    # Update cache
cargo make sqlx-check      # Verify cache is current
```

## Database Operations

### Setup and Connectivity

```bash
# Via cargo-make (preferred)
cargo make db-setup       # Setup database with migrations
cargo make db-check       # Check connectivity
cargo make db-migrate     # Run migrations
cargo make db-reset       # Drop and recreate

# Direct
export DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test
cargo sqlx migrate run
psql $DATABASE_URL -c "SELECT 1"
```

### Starting PostgreSQL

```bash
docker-compose up -d postgres     # Includes PGMQ extension
```

## Schema Structure

All Tasker tables live in the `tasker` schema:

| Table | Purpose |
|-------|---------|
| `tasker.tasks` | Task definitions and state |
| `tasker.workflow_steps` | Step definitions and state |
| `tasker.task_transitions` | Task state change audit trail |
| `tasker.workflow_step_transitions` | Step state change audit trail |
| `tasker.workflow_step_result_audit` | SOC2 audit trail (TAS-62) |
| `tasker.task_templates` | Workflow templates |
| `tasker.named_steps` | Step definitions within templates |

### Key SQL Functions

| Function | Purpose |
|----------|---------|
| `get_step_readiness_status()` | Determine which steps are ready to execute |
| `get_task_execution_context()` | Full task context with step states |
| `tasker_claim_and_execute_step()` | Atomic step claiming for workers |
| `detect_and_transition_stuck_tasks()` | Staleness detection |

## PGMQ (PostgreSQL Message Queue)

### Queue Pattern

- Dispatch queues: `tasker_dispatch_{namespace}` -- work sent to workers
- Completion queues: `tasker_completion_{namespace}` -- results back to orchestration
- Notification channel: `pgmq_message_ready` -- pg_notify for real-time events

### Checking PGMQ State

```bash
psql $DATABASE_URL -c "SELECT * FROM pgmq.meta"           # Queue metadata
psql $DATABASE_URL -c "SELECT * FROM pgmq.q_tasker_dispatch_default LIMIT 5"  # Queue contents
```

### tasker-pgmq Crate

The `tasker-pgmq` crate wraps PGMQ with:
- Notification support via PostgreSQL LISTEN/NOTIFY
- Configurable visibility timeouts
- Archive/delete semantics
- Queue lifecycle management

## Migrations

```
migrations/
├── 20260110000001_create_base_schema.sql
├── 20260110000002_add_base_data.sql
├── 20260110000003_sql_functions.sql
└── archive/                    # Historical migrations (not run)
```

Run migrations: `cargo sqlx migrate run`

## Connection Pool Configuration

Defined in `config/tasker/base/common.toml`:

```toml
[database.pool]
max_connections = 30      # Base default
min_connections = 8
acquire_timeout_secs = 5
idle_timeout_secs = 300
max_lifetime_secs = 1800
```

Environment scaling: Test (10) -> Development (25) -> Production (50)

## Production Database Considerations

- Total connections = (Orch replicas x pool) + (Worker replicas x pool)
- Ensure PostgreSQL `max_connections` > total + buffer
- Consider PgBouncer for connection pooling at infrastructure level
- Circuit breakers protect against connection exhaustion

## Troubleshooting

- **Connection errors**: `pg_isready -U tasker`, check DATABASE_URL format
- **PGMQ errors**: `psql $DATABASE_URL -c "SELECT * FROM pgmq.meta"`
- **Migration issues**: `cargo sqlx migrate run`, check migration order
- **SQLx cache stale**: `cargo make sqlx-prepare && git add .sqlx/`
- **"no cached data" in CI**: Run `cargo make sqlx-prepare` locally, commit `.sqlx/`

## References

- SQL functions: `docs/reference/task-and-step-readiness-and-execution.md`
- Configuration: `docs/guides/configuration-management.md`
- PGMQ crate: `tasker-pgmq/`
