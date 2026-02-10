# tasker-ctl

Command-line interface for the [Tasker](https://github.com/tasker-systems/tasker-core) orchestration system. Manage tasks, monitor workers, inspect configuration, investigate DLQ entries, and generate documentation from the terminal.

## Overview

`tasker-ctl` provides operator-facing commands for the full Tasker lifecycle. Built on the `tasker-client` library, it supports REST and gRPC transports with profile-based configuration.

## Installation

```bash
cargo install tasker-ctl
```

Or build from source:

```bash
cargo build --release -p tasker-ctl
```

## Commands

| Command | Description |
|---------|-------------|
| `task create` | Create a new task with namespace, context, and priority |
| `task get <UUID>` | Get task details |
| `task list` | List tasks with status/namespace filters |
| `task cancel <UUID>` | Cancel a running task |
| `task steps <UUID>` | List workflow steps for a task |
| `task reset-step` | Reset a step for automatic retry |
| `task resolve-step` | Mark a step as manually resolved |
| `task complete-step` | Complete a step with execution results |
| `task step-audit` | View step audit trail (SOC2 compliance) |
| `worker list` | List active workers |
| `worker health` | Check worker health |
| `system health` | System health across orchestration and workers |
| `config generate` | Generate merged config from base + environment |
| `config validate` | Validate a configuration file |
| `config explain` | Explain configuration parameters |
| `config dump` | Export configuration as JSON/YAML/TOML |
| `dlq list` | List Dead Letter Queue entries |
| `dlq stats` | DLQ statistics |
| `auth generate-keys` | Generate RSA key pair for JWT signing |
| `auth generate-token` | Generate JWT with specified permissions |
| `auth validate-token` | Validate a JWT token |
| `docs reference` | Generate configuration reference documentation |
| `docs annotated` | Generate annotated configuration example |

## Usage

```bash
# Create a task
tasker-ctl task create --name data_processing --namespace analytics \
  --input '{"file": "/data/input.csv"}'

# Monitor system health
tasker-ctl system health --orchestration --workers

# Investigate DLQ
tasker-ctl dlq list --status pending

# Generate config documentation
tasker-ctl docs reference --context all --output docs/config-reference.md

# Use a specific profile
tasker-ctl --profile staging task list
```

## Configuration

The CLI loads configuration with this precedence:

1. `--config <path>` — explicit config file
2. `--profile <name>` — named profile from `.config/tasker-client.toml`
3. `TASKER_CLIENT_PROFILE` env var
4. Default config discovery

```toml
# .config/tasker-client.toml
[profiles.local]
transport = "rest"
orchestration_url = "http://localhost:8080"

[profiles.production]
transport = "grpc"
orchestration_url = "https://tasker.example.com:9190"
api_key_env = "TASKER_API_KEY"
```

## License

MIT License — see [LICENSE](LICENSE) for details.

## Contributing

See the [Tasker Core contributing guide](https://github.com/tasker-systems/tasker-core) and [Code of Conduct](CODE_OF_CONDUCT.md).
