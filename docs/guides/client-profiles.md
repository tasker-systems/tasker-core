# Client Profiles

**Audience**: Developers, Operators
**Status**: Active
**Related Docs**: [Configuration Management](./configuration-management.md), [MCP Setup](./mcp/setup.md), [API Security](./api-security.md)

---

## Overview

Tasker uses a profile system (similar to `~/.aws/config` or `.config/nextest.toml`) for managing connections to different Tasker environments. Profiles define transport type, endpoint URLs, authentication, and timeouts. Both `tasker-ctl` and `tasker-mcp` consume the same profile configuration.

## Profile Config File

Profiles are defined in a TOML file with `[profile.<name>]` sections:

```toml
[profile.default]
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"
timeout_ms = 30000
max_retries = 3

[profile.default.worker]
base_url = "http://localhost:8081"
timeout_ms = 30000
max_retries = 3

[profile.default.cli]
default_format = "table"
colored_output = true
```

### File Search Order

The client searches these paths in order and uses the first file found:

| Priority | Path | Use Case |
|----------|------|----------|
| 1 | `.config/tasker-client.toml` | Project-local (like nextest) |
| 2 | `./tasker-client.toml` | Current directory |
| 3 | `~/.config/tasker/client.toml` | User config (XDG) |
| 4 | `~/.tasker/client.toml` | User config (dotfile) |

## Profile Fields

Each profile supports these sections:

```toml
[profile.<name>]
transport = "rest"              # "rest" (default) or "grpc"
description = "Staging cluster" # Optional human-readable description
namespaces = ["billing", "ops"] # Optional namespace hints

[profile.<name>.orchestration]
base_url = "http://host:port"   # Orchestration API endpoint
timeout_ms = 30000              # Request timeout
max_retries = 3                 # Retry count

[profile.<name>.orchestration.auth]
method = { type = "ApiKey", value = { key = "...", header_name = "X-API-Key" } }

[profile.<name>.worker]
base_url = "http://host:port"   # Worker API endpoint
timeout_ms = 30000
max_retries = 3

[profile.<name>.worker.auth]
method = { type = "ApiKey", value = { key = "...", header_name = "X-API-Key" } }

[profile.<name>.cli]
default_format = "table"        # Output format for tasker-ctl
colored_output = true
```

The `description` and `namespaces` fields are metadata used by tools like `tasker-mcp` to display profile context to LLM agents. They don't affect connectivity.

## Profile Resolution

When a profile is loaded, settings are resolved through layering:

```
1. Hardcoded defaults (lowest priority)
2. [profile.default] settings
3. [profile.<selected>] settings (overrides default)
4. Environment variables (highest priority)
```

This means `[profile.default]` acts as a base — named profiles only need to specify what differs. For example, a gRPC profile only needs to change `transport` and endpoint URLs:

```toml
[profile.grpc]
transport = "grpc"

[profile.grpc.orchestration]
base_url = "http://localhost:9190"

[profile.grpc.worker]
base_url = "http://localhost:9191"
# timeout_ms and max_retries inherited from [profile.default]
```

## Environment Variables

Environment variables override all profile settings:

| Variable | Effect |
|----------|--------|
| `TASKER_CLIENT_PROFILE` | Select profile by name (instead of CLI flag) |
| `TASKER_CLIENT_TRANSPORT` | Override transport (`rest` or `grpc`) |
| `TASKER_TEST_TRANSPORT` | Override transport (test contexts) |

## Using Profiles

### tasker-ctl

```bash
# Use default profile
tasker-ctl task list

# Use a named profile
tasker-ctl --profile grpc task list

# Override via environment
TASKER_CLIENT_PROFILE=staging tasker-ctl task list
```

### tasker-mcp

```bash
# Connected mode — loads all profiles, probes health at startup
tasker-mcp

# Set initial active profile
tasker-mcp --profile staging

# Offline mode — no profiles, Tier 1 tools only
tasker-mcp --offline
```

Once running, MCP agents can use the `connection_status` tool to list profiles and their health, and `use_environment` to switch the active profile.

## Health Probing

The `ProfileManager` (used by `tasker-mcp`) probes endpoint health for each profile:

| Status | Meaning |
|--------|---------|
| `Unknown` | Not yet probed |
| `Healthy` | Both orchestration and worker endpoints responding |
| `Degraded` | One endpoint responding, the other failing |
| `Unreachable` | No endpoints responding |

Health probes are:
- **Non-blocking** — failed probes don't prevent profile loading
- **Timeout-bounded** — 5 second default per endpoint
- **Refreshable** — agents can re-probe via `connection_status(refresh: true)`
- **Cached** — results cached until the next explicit probe

## Example: Multi-Environment Setup

```toml
[profile.default]
transport = "rest"
description = "Local development"

[profile.default.orchestration]
base_url = "http://localhost:8080"
timeout_ms = 10000
max_retries = 1

[profile.default.worker]
base_url = "http://localhost:8081"
timeout_ms = 10000

[profile.staging]
transport = "rest"
description = "Staging environment"
namespaces = ["staging"]

[profile.staging.orchestration]
base_url = "https://staging.example.com:8080"
timeout_ms = 30000
max_retries = 3

[profile.staging.orchestration.auth]
method = { type = "ApiKey", value = { key = "staging-key", header_name = "X-API-Key" } }

[profile.staging.worker]
base_url = "https://staging.example.com:8081"
timeout_ms = 30000

[profile.staging.worker.auth]
method = { type = "ApiKey", value = { key = "staging-key", header_name = "X-API-Key" } }

[profile.ci]
transport = "rest"
description = "CI pipeline"

[profile.ci.orchestration]
base_url = "http://localhost:8080"
timeout_ms = 60000
max_retries = 5

[profile.ci.worker]
base_url = "http://localhost:8081"
timeout_ms = 60000

[profile.ci.cli]
colored_output = false
```

## Related

- [Configuration Management](./configuration-management.md) — Server-side TOML configuration
- [MCP Setup](./mcp/setup.md) — Installing and configuring the MCP server
- [API Security](./api-security.md) — Authentication methods for API access
