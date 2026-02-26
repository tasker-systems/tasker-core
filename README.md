# Tasker Core

[![CI](https://github.com/tasker-systems/tasker-core/actions/workflows/ci.yml/badge.svg)](https://github.com/tasker-systems/tasker-core/actions/workflows/ci.yml)
![GitHub](https://img.shields.io/github/license/tasker-systems/tasker-core)
![GitHub release (latest SemVer)](https://img.shields.io/github/v/release/tasker-systems/tasker-core?color=blue&sort=semver)

Workflow orchestration in Rust with PostgreSQL-native messaging. Tasker Core provides DAG-based task execution, event-driven coordination, and multi-language worker support for complex workflows.

**Status**: Early Development — the core engine is functional, published, and under active development. APIs may evolve between minor versions.

---

## What Is Tasker?

Tasker is an orchestration framework for workflows that need more than a job queue but less than a full BPM platform. You define tasks as directed acyclic graphs of steps, and Tasker handles dependency resolution, state management, retry logic, and worker coordination.

- **DAG-Based Workflows** — Define steps with dependencies; Tasker resolves execution order and parallelism automatically
- **PostgreSQL as Source of Truth** — All state lives in PostgreSQL. Messaging via PGMQ (zero extra dependencies) or RabbitMQ (high-throughput)
- **Event-Driven Coordination** — Real-time step discovery with LISTEN/NOTIFY push and polling fallback, and RabbitMQ for high-throughput messaging
- **Multi-Language Workers** — Write step handlers in Rust, Ruby, Python, or TypeScript. All languages share the same orchestration engine
- **Operational Tooling** — CLI (`tasker-ctl`) for task management, health monitoring, DLQ investigation, configuration validation, and project scaffolding

### Good Fit

- Order fulfillment workflows with inventory, payment, and shipping coordination
- Data processing pipelines with complex step dependencies and parallel execution
- Payment processing with retry logic and idempotency guarantees
- Microservices orchestration where steps span multiple services

### Not The Right Tool

- Simple cron jobs — use native cron or a job scheduler
- Single-step operations — the orchestration overhead isn't justified
- Sub-millisecond latency requirements — Tasker adds ~10-20ms architectural overhead per step

---

## Getting Started

### Prerequisites

- **Rust** 1.75+ | **PostgreSQL** 17+ (uuidv7 support) | **Docker** (recommended)
- Optional: RabbitMQ (for high-throughput messaging)

### Quick Start

```bash
# Start PostgreSQL (includes PGMQ extension)
docker-compose up -d postgres

# Run migrations
export DATABASE_URL="postgresql://tasker:tasker@localhost/tasker_rust_test"
cargo sqlx migrate run

# Start orchestration server
docker-compose --profile server up -d

# Create a task
curl -X POST http://localhost:8080/v1/tasks \
  -H "Content-Type: application/json" \
  -d '{"template_name": "linear_workflow", "namespace": "example"}'

# Check health
curl http://localhost:8080/health
```

**Full Guide**: [docs/guides/quick-start.md](docs/guides/quick-start.md)

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Client Applications                     │
│               (REST API / gRPC / tasker-ctl)                │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              Tasker Orchestration Server                    │
│    Task Init → Step Discovery → Enqueueing → Finalization  │
└──────────┬──────────────────────────────────┬───────────────┘
           │                                  │
           ▼                                  ▼
┌──────────────────────┐          ┌──────────────────────────┐
│  PostgreSQL + Broker │◄────────►│  Namespace Worker Pools  │
│ (PGMQ or RabbitMQ)   │          │  (Horizontal Scaling)    │
└──────────────────────┘          └──────────────────────────┘
```

**Key Patterns**: Dual state machines (12 task + 9 step states), event-driven with polling fallback, autonomous workers, PostgreSQL as source of truth.

**Deep Dive**: [Architecture Documentation](docs/architecture/)

---

## Workspace Structure

| Crate | Purpose |
|-------|---------|
| `tasker-pgmq` | PGMQ wrapper with atomic notify (PostgreSQL messaging backend) |
| `tasker-shared` | Core types, state machines, messaging abstraction |
| `tasker-orchestration` | Task coordination, REST/gRPC API |
| `tasker-worker` | Step execution, FFI layer |
| `tasker-client` | REST/gRPC client library |
| `tasker-tooling` | Shared developer tooling — codegen, template parsing, schema inspection |
| `tasker-ctl` | CLI binary — operations, config management, plugin system |
| `tasker-mcp` | MCP server exposing Tasker tooling to LLM agents |
| `workers/rust` | Native Rust worker implementation |
| `workers/ruby` | Ruby FFI bindings (`tasker-rb` gem) |
| `workers/python` | Python FFI bindings (`tasker-py` package) |
| `workers/typescript` | TypeScript FFI bindings (`@tasker-systems/tasker`) |

---

## Documentation

All documentation is organized in the **[Documentation Hub](docs/README.md)**:

| Section | Description |
|---------|-------------|
| **[Quick Start](docs/guides/quick-start.md)** | Get running in 5 minutes |
| **[Architecture](docs/architecture/)** | System design, state machines, event systems, CLI architecture |
| **[Guides](docs/guides/)** | Workflows, batch processing, configuration |
| **[Workers](docs/workers/)** | Ruby, Python, TypeScript, Rust handler development |
| **[Operations](docs/operations/)** | Deployment, monitoring, tuning |
| **[Principles](docs/principles/)** | Design philosophy and tenets |
| **[Decisions](docs/decisions/)** | Architecture Decision Records (ADRs) |

**Development Guide**: [docs/development/](docs/development/) | **AI Assistant Context**: [CLAUDE.md](CLAUDE.md)

---

## Development

```bash
# Build (always use --all-features)
cargo build --all-features

# Test (requires PostgreSQL running)
cargo test --all-features

# Lint
cargo fmt && cargo clippy --all-targets --all-features

# Run server
cargo run --bin tasker-server
```

The project uses **cargo-make** as its task runner. See [CLAUDE.md](CLAUDE.md) for complete development context including test levels, database operations, and container setup.

---

## Published Packages

Tasker is published across multiple registries:

| Package | Registry | Language |
|---------|----------|----------|
| `tasker-shared`, `tasker-pgmq`, `tasker-client`, `tasker-ctl`, `tasker-orchestration`, `tasker-worker` | [crates.io](https://crates.io/crates/tasker-shared) | Rust |
| `tasker-rb` | [RubyGems](https://rubygems.org/gems/tasker-rb) | Ruby |
| `tasker-py` | [PyPI](https://pypi.org/project/tasker-py/) | Python |
| `@tasker-systems/tasker` | [npm](https://www.npmjs.com/package/@tasker-systems/tasker) | TypeScript |

---

## Contributing

1. Review [CLAUDE.md](CLAUDE.md) for project context and conventions
2. Run tests: `cargo test --all-features`
3. Format and lint before PR: `cargo fmt && cargo clippy --all-targets --all-features`
4. See the [Documentation Hub](docs/README.md) for documentation structure

---

## License

MIT License — see [LICENSE](LICENSE) for details.

---

**Workflow orchestration for teams that need reliability without ceremony.**

[Quick Start](docs/guides/quick-start.md) | [Documentation](docs/README.md) | [Architecture](docs/architecture/)
