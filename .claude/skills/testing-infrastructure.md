# Skill: Testing Infrastructure

## When to Use

Use this skill when running tests, writing new tests, understanding test feature flags, configuring test environments, or troubleshooting test failures in tasker-core.

## Test Feature Flags (TAS-73)

Tests are organized by infrastructure requirements using Cargo feature gates:

| Feature Flag | Infrastructure Required | Test Scope |
|-------------|------------------------|------------|
| *(none)* | Nothing (pure Rust) | In-module `#[test]` unit tests |
| `test-messaging` | PostgreSQL + messaging (PGMQ or RabbitMQ) | Unit/integration tests, DB operations |
| `test-services` | + running services (orchestration + workers) | E2E tests via HTTP/gRPC |
| `test-cluster` | + multi-instance cluster | Cluster/race condition tests |
| `--all-features` | Everything | All tests including cluster |

### Running Tests by Level

```bash
# Pure unit tests (no DB, no services - instant)
cargo test --workspace --lib
# Or: cargo nextest run --workspace --lib

# Unit tests (DB + messaging only)
cargo test --features test-messaging --lib

# E2E tests (requires services running)
cargo test --features test-services

# Cluster tests (requires cluster-start first)
cargo test --features test-cluster

# All tests
cargo test --all-features

# Using cargo-make shortcuts
cargo make test-no-infra      # tni - Pure unit tests (no infrastructure)
cargo make test-web           # tw  - DB + messaging (auto-setup PostgreSQL)
cargo make test-rust-unit     # tu  - Unit tests (DB + messaging, manual setup)
cargo make test-rust-e2e      # te  - E2E tests (requires services)
cargo make test-rust-cluster  # tc  - Cluster tests (requires: cluster-start)
cargo make test-rust-all      # All tests
```

### Claude Code Web Session Testing

For Claude Code web sessions where services are not pre-installed:

| Task | Alias | What it does | Setup required |
|------|-------|-------------|----------------|
| `test-no-infra` | `tni` | `cargo nextest run --workspace --lib` | None |
| `test-web` | `tw` | Auto-installs tools, starts PostgreSQL, runs migrations, runs `test-messaging` tests | None (self-bootstrapping) |

**`test-no-infra`** runs all in-module `#[cfg(test)]` blocks: state machine guards, config parsing, NoOp cache, InProcessEventBus, etc. Falls back to `cargo test` if nextest is not installed.

**`test-web`** is a self-bootstrapping script that installs `cargo-nextest` and `sqlx-cli` if missing, starts native PostgreSQL, configures extensions (PGMQ, uuidv7), runs migrations, then executes the full `test-messaging` test suite. This covers ~729 sqlx-backed tests plus all unit tests.

### Running Specific Tests

```bash
# By package
cargo test --features test-services --package tasker-orchestration

# By test name
cargo test --features test-services test_name_here

# With nextest (parallel, per-test process isolation)
cargo nextest run --profile default
cargo nextest run --profile ci         # CI profile with JUnit XML
cargo nextest run --profile local      # Fail-fast mode
```

## Critical Testing Rules

1. **Never use `SQLX_OFFLINE=true`** -- always export `DATABASE_URL` from `.env`
2. **Always use `--all-features`** for builds and consistency checks
3. **Never remove assertions to fix compilation/test failures** -- this hides problems. Fix the underlying issue instead.
4. E2E tests use `TASKER_FIXTURE_PATH` for fixture locations
5. Cluster tests are NOT run in CI (GitHub Actions resource constraints) -- run locally only

## Test Environment Setup

### Required Services

```bash
# Start PostgreSQL with PGMQ
cargo make docker-up

# For E2E tests, also start services
docker compose -f docker/docker-compose.test.yml up -d

# For cluster tests
cargo make cluster-start-all
```

### Environment Variables for Tests

```bash
export DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test
export TASKER_ENV=test
```

### Environment File Layering

```
config/dotenv/
├── base.env              # Core paths, logging
├── test.env              # Test environment settings
├── test-split.env        # Split database configuration (TAS-78)
├── cluster.env           # Multi-instance cluster settings
├── orchestration.env     # Orchestration service configuration
├── rust-worker.env       # Rust worker configuration
├── ruby-worker.env       # Ruby worker configuration
├── python-worker.env     # Python worker configuration
└── typescript-worker.env # TypeScript worker configuration

Layering:
  Single: base.env -> test.env -> service-specific.env
  Cluster: base.env -> test.env -> cluster.env -> service-specific.env
```

## Test Managers

The project provides test manager utilities for different scenarios:

- **`LifecycleTestManager`**: Integration tests that exercise orchestration code in-process
- **`IntegrationTestManager`**: Single-instance E2E tests via HTTP
- **`MultiInstanceTestManager`**: Cluster tests across multiple instances

## Cluster Testing (TAS-73)

### Port Allocation for Cluster

| Service Type | Port Range | Formula |
|-------------|------------|---------|
| Orchestration | 8080-8089 | BASE + (INSTANCE - 1) |
| Rust Workers | 8100-8109 | 8100 + (INSTANCE - 1) |
| Ruby Workers | 8200-8209 | 8200 + (INSTANCE - 1) |
| Python Workers | 8300-8309 | 8300 + (INSTANCE - 1) |
| TypeScript Workers | 8400-8409 | 8400 + (INSTANCE - 1) |

### Cluster Test Workflow

```bash
cargo make setup-env-cluster         # Setup cluster env
cargo make cluster-start-all         # Start all instances
cargo make cluster-status            # Verify health
cargo make test-rust-cluster         # Run cluster tests
cargo make cluster-stop              # Stop cluster
```

### Cluster Instance Environment Variables

```bash
TASKER_ORCHESTRATION_INSTANCES=2
TASKER_WORKER_RUST_INSTANCES=2
TASKER_TEST_ORCHESTRATION_URLS=http://localhost:8080,http://localhost:8081
TASKER_TEST_WORKER_RUST_URLS=http://localhost:8100,http://localhost:8101
```

## gRPC Testing (TAS-177)

### Port Allocation (gRPC)

| Service | REST Port | gRPC Port |
|---------|-----------|-----------|
| Orchestration | 8080 | 9190 |
| Rust Worker | 8081 | 9191 |
| Ruby Worker | 8082 | 9200 |
| Python Worker | 8083 | 9300 |
| TypeScript Worker | 8085 | 9400 |

### gRPC Test Commands

```bash
cargo make test-grpc                 # All gRPC tests
cargo make test-grpc-parity          # REST/gRPC response parity
cargo make test-e2e-grpc             # E2E with gRPC transport
cargo make test-both-transports      # E2E with REST and gRPC

# grpcurl examples (requires services running)
grpcurl -plaintext localhost:9190 list
grpcurl -plaintext localhost:9190 tasker.v1.HealthService/CheckLiveness
```

## FFI Worker Tests

```bash
cargo make test-python-ffi           # Python FFI integration tests
cargo make test-ruby-ffi             # Ruby FFI integration tests
cargo make test-typescript-ffi       # TypeScript FFI tests (Bun/Node/Deno)
cargo make test-ffi-all              # All FFI integration tests
```

## Troubleshooting Test Failures

- Run with `--nocapture` or `cargo make test-rust-verbose` for full output
- Ensure `--all-features` flag is used
- Check DATABASE_URL is exported (not SQLX_OFFLINE)
- For cluster tests: Ensure cluster is running (`cargo make cluster-start-all`)
- Auth integration tests may pollute env vars -- nextest isolates per-test process

## References

- Cluster testing guide: `docs/testing/cluster-testing-guide.md`
- Lifecycle testing: `docs/testing/comprehensive-lifecycle-testing-guide.md`
- Decision point tests: `docs/testing/decision-point-e2e-tests.md`
