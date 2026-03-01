# MCP Server Setup Guide

This guide covers installing and configuring `tasker-mcp`, the Model Context Protocol server that exposes 23 Tasker tools to LLM agents and developer tooling.

## Installation

### From Source (Development)

```bash
# Build
cargo build -p tasker-mcp --all-features

# Install to ~/.cargo/bin
cargo install --path tasker-mcp
```

### From crates.io

```bash
cargo install tasker-mcp
```

### Docker

```bash
docker run --rm -i ghcr.io/tasker-systems/tasker-mcp:latest
```

## Operating Modes

### Offline Mode (Tier 1 only)

```bash
tasker-mcp --offline
```

All 7 developer tooling tools work locally with no network calls. Use this when you only need template validation, code generation, and schema inspection.

### Connected Mode (default)

```bash
tasker-mcp                    # Loads profiles from tasker-client.toml
tasker-mcp --profile staging  # Set initial active profile
```

All 23 tools available. Requires a profile configuration pointing to a running Tasker orchestration server. Connected tools accept an optional `profile` parameter to target a specific environment.

## Client Configuration

### Claude Code

Copy the example config at the repo root:

```bash
cp .mcp.json.example .mcp.json
```

Or create `.mcp.json` in your project directory:

```json
{
  "mcpServers": {
    "tasker": {
      "command": "tasker-mcp",
      "env": { "RUST_LOG": "tasker_mcp=info" }
    }
  }
}
```

For user-scope (all projects), place in `~/.claude/.mcp.json`.

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "tasker": {
      "command": "tasker-mcp",
      "env": { "RUST_LOG": "tasker_mcp=info" }
    }
  }
}
```

### Cursor / VS Code / Windsurf

These editors support MCP via their settings. Add the server configuration in the MCP settings section, pointing to the `tasker-mcp` binary.

### mcphost (Ollama / Local Models)

Create `mcphost.json`:

```json
{
  "mcpServers": {
    "tasker": {
      "command": "tasker-mcp",
      "env": { "RUST_LOG": "tasker_mcp=info" }
    }
  }
}
```

Run with a local model:

```bash
mcphost --config mcphost.json --model ollama:qwen2.5-coder:14b
```

## Profile Configuration (Connected Mode)

Create `.config/tasker-client.toml` in your project or home directory:

```toml
[profile.default]
description = "Local development server"
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"

[profile.default.worker]
base_url = "http://localhost:8081"

[profile.staging]
description = "Staging environment"
transport = "rest"

[profile.staging.orchestration]
base_url = "https://staging-orchestration.example.com"

[profile.staging.worker]
base_url = "https://staging-worker.example.com"
```

Verify connectivity with the `connection_status` tool after configuring profiles.

## Available Tools (23)

### Tier 1 — Offline Developer Tools (7)

| Tool | Description |
|------|-------------|
| `template_generate` | Generate task template YAML from a structured spec |
| `template_validate` | Validate template for structural correctness and cycles |
| `template_inspect` | Inspect DAG structure, execution order, root/leaf steps |
| `handler_generate` | Generate typed handlers, models, and tests |
| `schema_inspect` | Inspect result_schema field details and consumer relationships |
| `schema_compare` | Compare producer/consumer schema compatibility |
| `schema_diff` | Detect field-level changes between two template versions |

### Profile Management (1)

| Tool | Description |
|------|-------------|
| `connection_status` | List profiles with health status, refresh endpoint probes |

### Tier 2 — Connected Read-Only Tools (15)

All accept an optional `profile` parameter to target a specific environment.

**Task & Step Inspection**

| Tool | Description |
|------|-------------|
| `task_list` | List tasks with namespace/status filtering and pagination |
| `task_inspect` | Task details with step breakdown and completion percentage |
| `step_inspect` | Step details including results, timing, and retry info |
| `step_audit` | SOC2-compliant audit trail for a step |

**DLQ Investigation**

| Tool | Description |
|------|-------------|
| `dlq_list` | List DLQ entries with resolution status filtering |
| `dlq_inspect` | Detailed DLQ entry with error context and snapshots |
| `dlq_stats` | DLQ statistics aggregated by reason code |
| `dlq_queue` | Prioritized investigation queue ranked by severity |
| `staleness_check` | Task staleness monitoring with health annotations |

**Analytics**

| Tool | Description |
|------|-------------|
| `analytics_performance` | System-wide performance metrics |
| `analytics_bottlenecks` | Slow steps and bottleneck identification |

**System**

| Tool | Description |
|------|-------------|
| `system_health` | Detailed component health (DB, queues, circuit breakers) |
| `system_config` | Orchestration configuration (secrets redacted) |

**Remote Templates**

| Tool | Description |
|------|-------------|
| `template_list_remote` | List templates registered on the server |
| `template_inspect_remote` | Template details from the server |

## Troubleshooting

### Server doesn't start

The MCP server uses stdio transport — it reads from stdin and writes to stdout, with logs on stderr.

```bash
# Test manually (should start without error, then wait for input)
tasker-mcp
```

### Enable debug logging

```bash
RUST_LOG=tasker_mcp=debug tasker-mcp
```

### Tools not appearing

1. Verify the binary is on your PATH: `which tasker-mcp`
2. Check the `.mcp.json` file is in the correct location
3. Restart your MCP client after configuration changes
4. Check stderr output for initialization errors

### Offline tools timeout

Tier 1 tools run locally against in-memory data (no network calls). If you see timeouts, the issue is likely transport-level. Check that the stdio pipe is connected properly.

### Connected tools return "offline_mode" error

You're running in offline mode but trying to use Tier 2 tools. Remove the `--offline` flag or ensure a profile configuration exists.

### Connected tools return "profile_not_found" error

The requested profile doesn't exist in your `tasker-client.toml`. Use `connection_status` to see available profiles.

### Connected tools return "connection_failed" error

The profile's orchestration server is unreachable. Verify:

1. The server is running at the configured URL
2. Network connectivity (firewalls, VPN)
3. Use `connection_status` with `refresh=true` to re-probe endpoints

### Common parameter errors

- **YAML parse errors**: The `template_yaml` parameter expects raw YAML text, not a file path. When passing YAML in JSON, ensure proper escaping of newlines.
- **Invalid language**: `handler_generate` accepts: `python`, `ruby`, `typescript`, `rust`.
- **Step not found**: `schema_compare` requires both `producer_step` and `consumer_step` to exist in the template.
