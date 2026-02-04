# tasker-orchestration

Orchestration server for the [Tasker](https://github.com/tasker-systems/tasker-core) workflow system. Coordinates DAG-based task execution through an actor-based architecture with REST and gRPC APIs.

## Overview

`tasker-orchestration` is the central coordinator for Tasker workflows. It receives task requests, resolves step dependencies, dispatches work to workers via message queues, processes results, and drives tasks through their complete lifecycle. The server exposes REST and gRPC APIs for external integration.

## Architecture

Four lightweight actors handle the orchestration lifecycle:

```
                    ┌───────────────────┐
  Task Request ────▶│ TaskRequestActor  │──── Initialize task, resolve DAG
                    └───────┬───────────┘
                            │
                            ▼
                    ┌───────────────────┐
                    │ StepEnqueuerActor │──── Batch-enqueue ready steps to message queue
                    └───────┬───────────┘
                            │
                            ▼
                    ┌───────────────────────┐
  Step Results ────▶│ ResultProcessorActor  │──── Process results, unblock dependents
                    └───────┬───────────────┘
                            │
                            ▼
                    ┌───────────────────────┐
                    │ TaskFinalizerActor    │──── Evaluate completion, transition state
                    └───────────────────────┘
```

Each actor communicates through bounded MPSC channels, providing backpressure and isolation. The orchestration loop runs continuously, discovering ready steps and driving tasks to completion.

## Key Features

- **DAG execution** — steps with dependency edges, parallel execution of independent branches
- **Dual state machines** — 12 task states and 8 step states with guard-based transitions
- **Actor pattern** — bounded channels, backpressure, per-actor metrics
- **REST API** — task CRUD, analytics, health monitoring, OpenAPI documentation
- **gRPC API** — full parity with REST, Protobuf types, health/reflection services
- **Circuit breakers** — configurable failure thresholds with automatic recovery
- **Dead Letter Queue** — failed tasks captured for investigation and retry
- **Batch step enqueueing** — efficient bulk dispatch to message queues

## Running

```bash
# Start the orchestration server (requires PostgreSQL)
cargo run --bin tasker-server

# Or with Docker Compose
docker-compose --profile server up -d
```

Default ports: REST on `8080`, gRPC on `9190`.

## API Examples

```bash
# Create a task
curl -X POST http://localhost:8080/v1/tasks \
  -H "Content-Type: application/json" \
  -H "X-API-Key: your-api-key" \
  -d '{"name": "order_fulfillment", "namespace": "orders", "version": "1.0.0"}'

# Check health
curl http://localhost:8080/health

# gRPC health check
grpcurl -plaintext localhost:9190 tasker.v1.HealthService/CheckLiveness

# OpenAPI documentation
open http://localhost:8080/docs
```

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `web-api` | Axum REST API with OpenAPI docs | Yes |
| `grpc-api` | Tonic gRPC API with reflection | Yes |
| `postgres` | PostgreSQL via SQLx | Yes |
| `test-utils` | Test helpers and factories | Yes |
| `tokio-console` | Runtime introspection | No |

## Configuration

The orchestration server reads from `config/tasker/base/orchestration.toml` with environment overrides:

```toml
[orchestration.web]
enabled = true
host = "0.0.0.0"
port = 8080

[orchestration.grpc]
enabled = true
port = 9190

[orchestration.actors]
task_request_channel_size = 1000
result_processor_channel_size = 2000
step_enqueuer_channel_size = 1000
task_finalizer_channel_size = 500
```

See the [configuration guide](https://github.com/tasker-systems/tasker-core/blob/main/docs/guides/configuration-management.md) for full reference.

## License

MIT License — see [LICENSE](LICENSE) for details.

## Contributing

See the [Tasker Core contributing guide](https://github.com/tasker-systems/tasker-core) and [Code of Conduct](CODE_OF_CONDUCT.md).
