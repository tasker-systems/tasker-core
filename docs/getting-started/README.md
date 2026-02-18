# Getting Started — Contributing to Tasker Core

> **Looking for the consumer getting-started guide?** See
> [The Tasker Book](https://github.com/tasker-systems/tasker-book) for installation,
> tutorials, and how to build workflows with Tasker.

This section covers setting up a **development environment** for contributing to tasker-core itself.

## Prerequisites

- **Rust** (stable, latest) with `cargo-make` installed
- **Docker** and Docker Compose (for PostgreSQL, RabbitMQ, Dragonfly)
- **protoc** (Protocol Buffers compiler for gRPC)

## Quick Setup

```bash
# 1. Clone the repo
git clone https://github.com/tasker-systems/tasker-core.git
cd tasker-core

# 2. Start infrastructure services
docker compose up -d postgres
# Or for the full test stack (includes RabbitMQ, Dragonfly):
docker compose -f docker/docker-compose.test.yml up -d

# 3. Set up the database
export DATABASE_URL=postgresql://tasker:tasker@localhost:5432/tasker_rust_test
cargo make db-setup

# 4. Run quality checks
cargo make check

# 5. Run tests (requires services from step 2)
cargo make test-rust-unit      # Unit tests (DB + messaging only, fastest)
cargo make test-rust-e2e       # E2E tests (requires full test services)
```

## Crate Architecture

| Crate | Purpose |
|-------|---------|
| `tasker-shared` | Shared types, traits, configuration, utilities |
| `tasker-orchestration` | Core orchestration logic, actors, API (REST + gRPC) |
| `tasker-worker` | Step execution, handler dispatch, FFI integration |
| `tasker-pgmq` | PGMQ wrapper with notification support |
| `tasker-client` | API client library (REST + gRPC transport) |
| `tasker-ctl` | CLI binary and plugin system |
| `workers/ruby` | Ruby FFI bindings (Magnus) |
| `workers/python` | Python FFI bindings (PyO3/maturin) |
| `workers/typescript` | TypeScript FFI bindings (napi-rs) |
| `workers/rust` | Rust worker implementation |

For detailed module organization, see the `AGENTS.md` files in `tasker-orchestration/` and `tasker-worker/`.

## Key Resources

- **[Architecture docs](../architecture/)** — System design, actors, state machines
- **[Development tooling](../development/tooling.md)** — cargo-make tasks, build system
- **[Testing infrastructure](../testing/)** — Test tiers, cluster testing
- **[CLAUDE.md](../../CLAUDE.md)** — Full project context and command reference
- **[The Tasker Book](https://github.com/tasker-systems/tasker-book)** — Consumer-facing documentation
- **[Documentation Architecture](https://github.com/tasker-systems/tasker-book/blob/main/DOCUMENTATION-ARCHITECTURE.md)** — How documentation is organized across repos
