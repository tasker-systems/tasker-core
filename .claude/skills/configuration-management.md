# Skill: Configuration Management

## When to Use

Use this skill when working with Tasker configuration files, TOML structure, environment-specific settings, the config CLI tools, or understanding how configuration loads at runtime.

## Configuration Structure

Configuration is **role-based** (common, orchestration, worker), NOT component-based. Never create separate component files like `auth.toml` or `circuit_breakers.toml`.

```
config/tasker/
├── base/                           # Base configuration (defaults)
│   ├── common.toml                 # Shared: database, circuit breakers, telemetry
│   ├── orchestration.toml          # Orchestration-specific settings
│   └── worker.toml                 # Worker-specific settings
│
├── environments/                   # Environment-specific overrides
│   ├── test/
│   │   ├── common.toml
│   │   ├── orchestration.toml
│   │   └── worker.toml
│   ├── development/
│   │   ├── common.toml
│   │   ├── orchestration.toml
│   │   └── worker.toml
│   └── production/
│       ├── common.toml
│       ├── orchestration.toml
│       └── worker.toml
│
└── generated/                      # Generated merged configs (for runtime)
    ├── orchestration-test.toml
    ├── orchestration-production.toml
    ├── worker-test.toml
    └── worker-production.toml
```

## Three Configuration Contexts

| Context | Purpose | Contains |
|---------|---------|----------|
| **Common** | Shared settings | Database, circuit breakers, telemetry, backoff, system |
| **Orchestration** | Orchestration-specific | Web API, MPSC channels, event systems, shutdown, gRPC |
| **Worker** | Worker-specific | Handler discovery, resource limits, health monitoring |

## Environment Scaling Pattern (1:5:50)

| Component | Test | Development | Production |
|-----------|------|-------------|------------|
| Database Connections | 10 | 25 | 50 |
| Concurrent Steps | 10 | 50 | 500 |
| MPSC Channel Buffers | 100-500 | 500-1000 | 2000-50000 |
| Memory Limits | 512MB | 2GB | 4GB |

## Runtime Configuration Loading

### Two Loading Strategies (TAS-50 Phase 3)

1. **`TASKER_CONFIG_PATH`** (Production/Docker): Explicit single merged file
   ```bash
   export TASKER_CONFIG_PATH=/app/config/tasker/orchestration-production.toml
   ```

2. **`TASKER_CONFIG_ROOT`** (Tests/Development): Convention-based
   ```bash
   export TASKER_CONFIG_ROOT=/config
   # Convention: {ROOT}/tasker/generated/{context}-{environment}.toml
   ```

If neither is set, the system fails with a clear error (Fail Loudly tenet).

### Merging Strategy

Environment overrides win. Only specify what changes:

```toml
# base/common.toml
[database.pool]
max_connections = 30
min_connections = 8

# environments/production/common.toml
[database.pool]
max_connections = 50
# Result: max_connections = 50, min_connections = 8 (inherited)
```

## CLI Tools

### Generate Merged Config

```bash
# Generate for deployment
tasker-cli config generate \
    --context orchestration \
    --environment production

# Output: config/tasker/generated/orchestration-production.toml
```

### Validate Config

```bash
# Validate before deployment
tasker-cli config validate \
    --context orchestration \
    --environment production

# Quick validator binary
TASKER_ENV=test cargo run --bin config-validator
```

## Runtime Observability

Both orchestration and worker expose `/config` endpoints with automatic secret redaction:

```bash
curl http://localhost:8080/config | jq    # Orchestration config
curl http://localhost:8081/config | jq    # Worker config
```

Sensitive fields (password, secret, token, key, api_key, url, credentials, etc.) are automatically replaced with `***REDACTED***`.

## Environment Detection

Via `TASKER_ENV` environment variable:

```bash
export TASKER_ENV=test          # Small values, fast execution
export TASKER_ENV=development   # Medium values, local Docker
export TASKER_ENV=production    # Large values, scale-out
```

Default: `development` if not set.

## Best Practices

- Use environment variables for secrets (`${DATABASE_URL}`)
- Validate configs before deployment
- Generate single deployable artifacts for production
- Keep environment overrides minimal (only what changes)
- Never commit production secrets to config files
- Use unbounded configuration values sparingly (see Bounded Resources tenet)

## References

- Full documentation: `docs/guides/configuration-management.md`
- Environment comparison: `docs/guides/environment-configuration-comparison.md`
- Deployment patterns: `docs/architecture/deployment-patterns.md`
