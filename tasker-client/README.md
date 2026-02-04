# tasker-client

Transport-agnostic API client library for the [Tasker](https://github.com/tasker-systems/tasker-core) workflow orchestration system. Provides REST and gRPC clients for programmatic interaction with Tasker orchestration and worker APIs.

## Overview

`tasker-client` is the primary interface for external systems to interact with Tasker. It handles HTTP/gRPC communication, authentication, and provides strongly-typed interfaces for all API endpoints. The CLI tool (`tasker-cli`) is built on top of this library.

## Features

- **Orchestration client** — create tasks, query status, view analytics, manage DLQ entries
- **Worker client** — health checks, status monitoring, worker management
- **Transport abstraction** — unified traits for REST and gRPC with transparent switching
- **Authentication** — API key, JWT token, or no-auth modes
- **Profile-based config** — TOML configuration with named profiles (like nextest)
- **Error categorization** — structured errors: network, auth, server, validation

## Quick Start

```rust,ignore
use tasker_client::{OrchestrationApiClient, OrchestrationApiConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = OrchestrationApiConfig::default();
    let client = OrchestrationApiClient::new(config)?;

    // Create a task
    let response = client.create_task(task_request).await?;
    println!("Task created: {}", response.task_id);

    // Check health
    let health = client.health_check().await?;
    println!("Status: {:?}", health.status);

    Ok(())
}
```

### gRPC Transport

```rust,ignore
use tasker_client::{UnifiedOrchestrationClient, ClientConfig, Transport};

let config = ClientConfig {
    transport: Transport::Grpc,
    ..ClientConfig::load()?
};

// Same API, different transport
let client = UnifiedOrchestrationClient::new(&config)?;
let health = client.health_check().await?;
```

## Configuration

### Environment Variables

| Variable | Description |
|----------|-------------|
| `ORCHESTRATION_URL` | Base URL for orchestration service |
| `ORCHESTRATION_API_KEY` | API key for authentication |
| `WORKER_URL` | Base URL for worker service |
| `CLIENT_TIMEOUT_MS` | Request timeout in milliseconds |
| `TASKER_CLIENT_PROFILE` | Named profile from config file |

### Profile Configuration

Create `.config/tasker-client.toml` in your project:

```toml
[profiles.local]
transport = "rest"
orchestration_url = "http://localhost:8080"

[profiles.staging]
transport = "grpc"
orchestration_url = "https://staging.example.com:9190"
api_key = "your-staging-key"
```

Use with `--profile staging` or `TASKER_CLIENT_PROFILE=staging`.

## Features

| Feature | Description | Default |
|---------|-------------|---------|
| `grpc` | gRPC transport via Tonic | Yes |

## License

MIT License — see [LICENSE](LICENSE) for details.

## Contributing

See the [Tasker Core contributing guide](https://github.com/tasker-systems/tasker-core) and [Code of Conduct](CODE_OF_CONDUCT.md).
