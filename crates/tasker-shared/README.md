# tasker-shared

Shared foundation library for the [Tasker](https://github.com/tasker-systems/tasker-core) workflow orchestration system. Provides the core types, models, state machines, configuration, database operations, messaging abstractions, and infrastructure used by all other Tasker crates.

## Overview

`tasker-shared` is the foundational crate in the Tasker workspace. It contains everything that the orchestration server, workers, and client need to share: data models, state machine logic, configuration loading, database access, messaging protocols, and resilience patterns.

## Key Modules

| Module | Description |
|--------|-------------|
| `models` | Complete data model layer — tasks, steps, templates, namespaces, transitions, audit trails |
| `state_machine` | Dual state machines: 12 task states and 8 step states with guard-based transitions |
| `config` | TOML-based configuration with base/environment layering and runtime merge |
| `database` | Connection pooling, SQL function execution, migrations via SQLx |
| `messaging` | Provider-agnostic messaging — PGMQ (default) and RabbitMQ backends |
| `events` | Domain event system with typed publishers and subscriber registry |
| `resilience` | Circuit breaker patterns with configurable thresholds and recovery |
| `cache` | Multi-backend caching — Redis, Moka (in-process), Memcached, or noop |
| `metrics` | OpenTelemetry metrics for database, messaging, orchestration, and security |
| `web` | Shared web middleware — authentication, authorization, API key and JWT support |
| `proto` | gRPC/Protobuf type conversions (requires `grpc-api` feature) |
| `registry` | Step handler resolution with pluggable resolver chains |
| `scopes` | Database query scopes mirroring Rails-style named scopes |

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `web-api` | Axum web framework, JWT/API-key auth, OpenAPI docs | Yes |
| `grpc-api` | Tonic gRPC framework and Protobuf types | Yes |
| `cache-redis` | Redis caching backend | Yes |
| `cache-moka` | Moka in-process caching backend | Yes |
| `cache-memcached` | Memcached caching backend | No |
| `postgres` | PostgreSQL via SQLx | Yes |
| `test-utils` | Test factories and helpers | Yes |
| `tokio-console` | Runtime introspection via tokio-console | No |

## Usage

```rust,no_run
use tasker_shared::config::{ConfigLoader, tasker::TaskerConfig};

// Load configuration from TOML files based on TASKER_ENV
let config = ConfigLoader::load_from_env().expect("Failed to load config");

// Access configuration sections
assert!(config.common.database.pool.max_connections > 0);
```

State machine transitions:

```rust,ignore
use tasker_shared::state_machine::{TaskStateMachine, TaskState, TaskEvent};

// Task lifecycle: Pending → Initializing → EnqueuingSteps → StepsInProcess → Complete
let machine = TaskStateMachine::new();
let next = machine.transition(TaskState::Pending, TaskEvent::Initialize)?;
```

## Configuration

Configuration uses a layered TOML structure:

```
config/tasker/
├── base/                    # Defaults for all environments
│   ├── common.toml          # Shared: database, cache, circuit breakers
│   ├── orchestration.toml   # Orchestration-specific settings
│   └── worker.toml          # Worker-specific settings
└── environments/
    ├── development/         # Dev overrides
    ├── test/                # Test overrides
    └── production/          # Production overrides
```

Set `TASKER_ENV` to select the environment (defaults to `development`).

## License

MIT License — see [LICENSE](LICENSE) for details.

## Contributing

See the [Tasker Core contributing guide](https://github.com/tasker-systems/tasker-core) and [Code of Conduct](CODE_OF_CONDUCT.md).
