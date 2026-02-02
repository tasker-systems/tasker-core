# Skill: cargo-make Tooling

## When to Use

Use this skill when working with the tasker-core build system, running tasks, adding new cargo-make tasks, or troubleshooting build/test/check commands. This skill covers the unified task runner that orchestrates Rust core and polyglot workers (Python, Ruby, TypeScript).

## Core Commands

### Top-Level Tasks (run from workspace root)

| Task | Alias | Description |
|------|-------|-------------|
| `cargo make check` | `c` | All quality checks across workspace |
| `cargo make test` | `t` | All tests (requires services running) |
| `cargo make fix` | `f` | Auto-fix all fixable issues |
| `cargo make build` | `b` | Build everything |

### Test Hierarchy (TAS-73 feature-gated)

| Task | Alias | Feature Flag | Requirements |
|------|-------|-------------|-------------|
| `cargo make test-rust-unit` | `tu` | `test-messaging` | DB + messaging only |
| `cargo make test-rust-e2e` | `te` | `test-services` | Services running |
| `cargo make test-rust-cluster` | `tc` | `test-cluster` | `cluster-start` first |
| `cargo make test-rust-all` | - | `--all-features` | All of the above |

### Language-Specific Checks

| Task | Description |
|------|-------------|
| `cargo make check-rust` | Rust: fmt, clippy, docs, doctests |
| `cargo make check-python` | Python: ruff format, ruff lint, mypy, pytest |
| `cargo make check-ruby` | Ruby: rubocop, rust-check, compile, rspec |
| `cargo make check-typescript` | TypeScript: biome, tsc, vitest |

### Database Tasks

| Task | Description |
|------|-------------|
| `cargo make db-setup` | Setup database with migrations |
| `cargo make db-check` | Check database connectivity |
| `cargo make db-migrate` | Run migrations |
| `cargo make db-reset` | Reset database (drop/recreate) |
| `cargo make sqlx-prepare` | Prepare SQLX query cache |
| `cargo make sqlx-check` | Verify SQLX cache is up to date |

### Docker Tasks

| Task | Description |
|------|-------------|
| `cargo make docker-up` | Start PostgreSQL with PGMQ |
| `cargo make docker-down` | Stop Docker services |
| `cargo make docker-logs` | Show Docker logs |

### CI Tasks

| Task | Description |
|------|-------------|
| `cargo make ci-check` | CI quality checks (fmt, clippy, docs, audit) |
| `cargo make ci-test` | CI test run with CI profile |
| `cargo make ci-flow` | Complete CI flow |

### Cluster Tasks (TAS-73)

| Task | Description |
|------|-------------|
| `cargo make cluster-start` | Start default cluster (orchestration + rust workers) |
| `cargo make cluster-start-all` | Start cluster with all worker types |
| `cargo make cluster-stop` | Stop all cluster instances |
| `cargo make cluster-status` | Check health of all instances |
| `cargo make cluster-logs` | Tail logs from all instances |

### gRPC Testing Tasks (TAS-177)

| Task | Alias | Description |
|------|-------|-------------|
| `cargo make test-grpc` | `tg` | All gRPC tests |
| `cargo make test-grpc-parity` | `tgp` | REST/gRPC response parity |
| `cargo make test-e2e-grpc` | `tge` | E2E tests with gRPC transport |
| `cargo make test-both-transports` | - | E2E with REST and gRPC |

### Environment Setup

| Task | Description |
|------|-------------|
| `cargo make setup-env` | Generate root .env for single-instance mode |
| `cargo make setup-env-cluster` | Generate .env with cluster configuration |
| `cargo make setup-workers` | Setup all polyglot workers |
| `cargo make clean-workers` | Clean all worker artifacts |

## Architecture

The cargo-make configuration follows hierarchical inheritance:

```
Makefile.toml (root)
    extends -> cargo-make/main.toml
                   extends -> cargo-make/base-tasks.toml

cargo-make/
├── main.toml              # Entry point, chains all modules
├── base-tasks.toml        # Base task templates for extension
├── workspace-config.toml  # Shared workspace configuration
├── cross-cutting.toml     # Cross-language quality tasks
├── test-tasks.toml        # Test configuration and profiles
├── ci-integration.toml    # CI workflow alignment
└── scripts/               # Shell scripts for operations
```

Crate-level Makefile.toml files extend `cargo-make/base-tasks.toml`. Worker directories (Python, Ruby, TypeScript) have their own complete Makefile.toml files.

## Crate-Level Pattern

All crate Makefile.toml files follow this pattern:

```toml
# MUST be at root level, NOT inside [config]
extend = "../cargo-make/base-tasks.toml"

[config]
default_to_workspace = false

[env]
CRATE_NAME = "crate-name-here"

[tasks.default]
alias = "check"

[tasks.check]
dependencies = ["format-check", "lint", "test"]

[tasks.format-check]
extend = "base-rust-format"

[tasks.lint]
extend = "base-rust-lint"

[tasks.test]
extend = "base-rust-test"
```

## Adding New Tasks

- **Workspace-wide**: Add to `cargo-make/main.toml` or appropriate module file
- **Crate-specific**: Add to the crate's `Makefile.toml`
- **New base task**: Add to `cargo-make/base-tasks.toml` with `base-` prefix
- **Shell operations**: Create script in `cargo-make/scripts/`, reference via `script = { file = "${SCRIPTS_DIR}/script-name.sh" }`

## Common Troubleshooting

- **`extend` not working**: Must be at root level of TOML, NOT inside `[config]`
- **Script path errors**: Use relative path `SCRIPTS_DIR = "cargo-make/scripts"`, not absolute
- **Task not found**: Check crate's Makefile.toml extends base-tasks correctly
- **Worker setup failures**: Run `cargo make clean-workers && cargo make setup-workers`

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `postgresql://tasker:tasker@localhost:5432/tasker_rust_test` | Database connection |
| `TASKER_ENV` | `test` | Environment (test, development, production) |
| `SCRIPTS_DIR` | `cargo-make/scripts` | Path to shell scripts |

## References

- Full documentation: `docs/development/tooling.md`
- Development patterns: `docs/development/development-patterns.md`
