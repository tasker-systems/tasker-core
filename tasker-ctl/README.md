# tasker-ctl

Command-line interface for the [Tasker](https://github.com/tasker-systems/tasker-core) orchestration system. Manage tasks, monitor workers, inspect configuration, investigate DLQ entries, generate documentation, and scaffold projects from plugin templates.

## Overview

`tasker-ctl` is the primary developer and operator interface for Tasker. Built on `tasker-client`, it supports REST and gRPC transports with profile-based configuration, and includes an extensible plugin system for discovering and generating code from community templates.

## Installation

```bash
cargo install tasker-ctl
```

Or build from source:

```bash
cargo build --release -p tasker-ctl
```

## Commands

### Task Management

| Command | Description |
|---------|-------------|
| `task create` | Create a new task with namespace, context, and priority |
| `task get <UUID>` | Get task details including health and progress |
| `task list` | List tasks with status/namespace filters |
| `task cancel <UUID>` | Cancel a running task |
| `task steps <UUID>` | List workflow steps for a task |
| `task step <TASK> <STEP>` | Get individual step details |
| `task reset-step` | Reset a step for automatic retry |
| `task resolve-step` | Mark a step as manually resolved |
| `task complete-step` | Complete a step with execution results |
| `task step-audit` | View step audit trail (SOC2 compliance) |

### Worker & System

| Command | Description |
|---------|-------------|
| `worker list` | List worker templates and capabilities |
| `worker status` | Detailed worker health and system info |
| `worker health` | Full health check with component status |
| `system health` | System health across orchestration and workers |
| `system info` | System information summary |

### Configuration

| Command | Description |
|---------|-------------|
| `config generate` | Generate merged config from base + environment |
| `config validate` | Validate a configuration file |
| `config validate-sources` | Validate source files without generating output |
| `config explain` | Explain configuration parameters with documentation |
| `config analyze-usage` | Analyze configuration usage across the codebase |
| `config dump` | Export configuration as JSON/YAML/TOML |
| `config show` | Show current CLI configuration and profiles |

### Dead Letter Queue

| Command | Description |
|---------|-------------|
| `dlq list` | List DLQ entries with status filtering |
| `dlq get <UUID>` | Get DLQ entry with task snapshot |
| `dlq update` | Update DLQ investigation status |
| `dlq stats` | DLQ statistics by reason |

### Authentication

| Command | Description |
|---------|-------------|
| `auth generate-keys` | Generate RSA key pair for JWT signing |
| `auth generate-token` | Generate JWT with specified permissions |
| `auth show-permissions` | List all known API permissions |
| `auth validate-token` | Validate a JWT token |

### Documentation Generation

| Command | Description |
|---------|-------------|
| `docs reference` | Generate configuration reference documentation |
| `docs annotated` | Generate annotated configuration example |
| `docs section` | Document a specific configuration section |
| `docs coverage` | Show documentation coverage statistics |
| `docs explain` | Template-rendered parameter explanation |
| `docs index` | Generate documentation index with coverage |

### Plugins & Templates

| Command | Description |
|---------|-------------|
| `plugin list` | Discover and list plugins from configured paths |
| `plugin validate <PATH>` | Validate a plugin directory |
| `template list` | List available templates (filterable by language/framework) |
| `template info <NAME>` | Show template parameters and output files |
| `template generate <NAME>` | Generate files from a plugin template |

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

# Discover plugins and generate from templates
tasker-ctl plugin list
tasker-ctl template list --language ruby
tasker-ctl template generate step_handler \
  --param name=ProcessPayment --language ruby --output ./app/handlers/
```

## Configuration

### Client Profiles

The CLI loads client configuration (server URLs, transport, auth) with this precedence:

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

### Plugin Configuration

Plugin discovery paths are configured in `.tasker-cli.toml`:

```toml
# .tasker-cli.toml
plugin-paths = [
    "./tasker-cli-plugins",
    "~/projects/tasker-systems/tasker-contrib",
]
default-language = "ruby"
default-output-dir = "./app/handlers"
```

Discovery checks: `./.tasker-cli.toml` then `~/.config/tasker-cli.toml`.

## Terminal Output

`tasker-ctl` uses styled terminal output with automatic detection of TTY capabilities. Colors and formatting degrade gracefully when output is piped or redirected, so commands like `tasker-ctl config dump` and `tasker-ctl auth generate-token` remain safe for scripting.

## License

MIT License — see [LICENSE](LICENSE) for details.

## Contributing

See the [Tasker Core contributing guide](https://github.com/tasker-systems/tasker-core) and [Code of Conduct](CODE_OF_CONDUCT.md).
