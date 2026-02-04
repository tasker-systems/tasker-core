# tasker-worker

Worker foundation for the [Tasker](https://github.com/tasker-systems/tasker-core) workflow system. Executes workflow steps with event-driven processing, multi-language FFI support, and configurable deployment modes.

## Overview

`tasker-worker` processes workflow steps dispatched by the orchestration server. It supports Rust-native handlers and FFI-based handlers for Ruby, Python, and TypeScript. The worker uses a dual-channel architecture with push notifications for low latency and polling for reliability.

## Architecture

```
┌─────────────────┐    ┌──────────────────────┐    ┌──────────────────────┐
│ WorkerBootstrap │───▶│  WorkerEventSystem   │───▶│ HandlerDispatchSvc   │
│ (Config-Driven) │    │  (Push + Poll)       │    │ (Semaphore-Bounded)  │
└─────────────────┘    └──────────────────────┘    └──────────────────────┘
                              │                              │
                              ▼                              ▼
                       ┌─────────────────┐          ┌───────────────────┐
                       │ PostgreSQL      │          │ Step Handlers     │
                       │ LISTEN/NOTIFY   │          │ ┌───────────────┐ │
                       │ + Fallback Poll │          │ │ Rust (native) │ │
                       └─────────────────┘          │ │ Ruby (FFI)    │ │
                                                    │ │ Python (PyO3) │ │
                                                    │ │ TypeScript    │ │
                                                    │ └───────────────┘ │
                                                    └───────────────────┘
```

## Key Features

- **Event-driven processing** — real-time PostgreSQL LISTEN/NOTIFY with fallback polling
- **Three deployment modes** — `PollingOnly`, `EventDrivenOnly`, `Hybrid` (recommended)
- **Multi-language FFI** — Ruby via Magnus, Python via PyO3, TypeScript via C ABI
- **Semaphore-bounded dispatch** — configurable concurrency limits per worker
- **Batch processing** — aggregate multiple rows into single handler invocations
- **Handler capabilities** — declarative traits: `APICapable`, `BatchableCapable`, `DecisionCapable`
- **REST and gRPC APIs** — health monitoring, status, handler registration
- **Database-as-API** — workers hydrate full context from step message UUIDs

## Deployment Modes

| Mode | Latency | Reliability | Use Case |
|------|---------|-------------|----------|
| `PollingOnly` | Higher (~1s) | High | Simple deployments, no pg_notify |
| `EventDrivenOnly` | Lowest (<10ms) | Lower | Low-latency requirements |
| `Hybrid` | Low (<10ms) | Highest | **Recommended** — push + poll fallback |

## Usage

### Rust Worker

```rust,no_run
use tasker_worker::{WorkerBootstrap, WorkerBootstrapConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut handle = WorkerBootstrap::bootstrap().await?;

    // Worker is now processing steps via push notifications + polling
    // Graceful shutdown on signal
    handle.stop().await?;
    Ok(())
}
```

### Handler Development

Handlers implement `process()` and optionally `process_results()`:

```rust,ignore
use tasker_worker::{HandlerCapabilities, BatchableCapable, DecisionCapable};

struct MyHandler;

impl HandlerCapabilities for MyHandler {
    // Declare what this handler can do
}
```

For FFI handlers (Ruby, Python, TypeScript), see the [cross-language development guide](https://github.com/tasker-systems/tasker-core/blob/main/docs/development/).

## Running

```bash
# Start the worker server (requires PostgreSQL + orchestration)
cargo run --bin tasker-worker

# Or with Docker Compose
docker-compose --profile server up -d
```

Default ports: REST on `8081`, gRPC on `9191`.

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `web-api` | Axum REST API with OpenAPI docs | Yes |
| `grpc-api` | Tonic gRPC API with reflection | Yes |
| `postgres` | PostgreSQL via SQLx | Yes |
| `test-utils` | Test helpers and factories | Yes |
| `tokio-console` | Runtime introspection | No |

## Configuration

The worker reads from `config/tasker/base/worker.toml` with environment overrides:

```toml
[worker.web]
enabled = true
host = "0.0.0.0"
port = 8081

[worker.event_systems.worker]
deployment_mode = "hybrid"

[worker.event_systems.worker.metadata.push_consumer]
enabled = true
max_concurrent_steps = 100

[worker.event_systems.worker.metadata.fallback_poller]
enabled = true
poll_interval_seconds = 5
```

## License

MIT License — see [LICENSE](LICENSE) for details.

## Contributing

See the [Tasker Core contributing guide](https://github.com/tasker-systems/tasker-core) and [Code of Conduct](CODE_OF_CONDUCT.md).
