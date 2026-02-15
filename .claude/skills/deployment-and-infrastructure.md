# Skill: Deployment and Infrastructure

## When to Use

Use this skill when working with Docker setup, CI pipelines, deployment modes, container configuration, or understanding how tasker-core services run in local, CI, and production environments.

## Container Runtime

The project uses **Docker** as the container runtime.

### Local Service Management

```bash
# Start PostgreSQL with PGMQ
docker compose up -d postgres

# Full server (orchestration + workers)
docker compose --profile server up -d

# Test services (includes RabbitMQ)
docker compose -f docker/docker-compose.test.yml up -d
```

## Deployment Modes

Tasker supports three deployment modes for orchestration:

| Mode | Latency | Reliability | Use Case |
|------|---------|-------------|----------|
| **Hybrid** (Recommended) | Low | Highest | Production default |
| **EventDrivenOnly** | Lowest (~10ms) | Good | Stable networks, high throughput |
| **PollingOnly** | Higher (~100-500ms) | Good | Restricted environments |

### Production Mixed-Mode Architecture (Recommended)

Deploy multiple orchestration containers with different modes:

- **EventDrivenOnly containers**: 8-12 replicas (handles 80-90% of workload via pg_notify)
- **PollingOnly containers**: 2-3 replicas (safety net for missed events)

Both coordinate through atomic SQL operations -- no conflicts. Single Kubernetes Service load-balances across all pods.

## Messaging Backends (TAS-133)

| Backend | Infrastructure | Best For |
|---------|---------------|----------|
| **PGMQ** (Default) | PostgreSQL only | Simpler ops, single-dependency deployments |
| **RabbitMQ** (Optional) | PostgreSQL + RabbitMQ | Higher throughput, existing broker infrastructure |

```bash
TASKER_MESSAGING_BACKEND=pgmq       # Default
TASKER_MESSAGING_BACKEND=rabbitmq   # Optional
RABBITMQ_URL=amqp://user:password@rabbitmq:5672/%2F
```

**Rule**: Start with PGMQ. Migrate to RabbitMQ only when throughput demands it.

## Service Ports

### REST and gRPC Port Allocation

| Service | REST Port | gRPC Port |
|---------|-----------|-----------|
| Orchestration | 8080 | 9190 |
| Rust Worker | 8081 | 9191 |
| Ruby Worker | 8082 | 9200 |
| Python Worker | 8083 | 9300 |
| TypeScript Worker | 8085 | 9400 |

### Cluster Port Ranges (up to 10 instances each)

| Service Type | REST Port Range |
|-------------|----------------|
| Orchestration | 8080-8089 |
| Rust Workers | 8100-8109 |
| Ruby Workers | 8200-8209 |
| Python Workers | 8300-8309 |
| TypeScript Workers | 8400-8409 |

## Docker Build (Production)

### Config Generation at Build Time (TAS-50 Phase 3)

```dockerfile
# Generate merged config at build time
RUN tasker-ctl config generate \
    --context orchestration \
    --environment production

# Runtime uses single merged file
ENV TASKER_CONFIG_PATH=/app/config/orchestration.toml
ENV TASKER_ENV=production
```

### Docker Compose Profiles

Services use profiles to control what starts:

```bash
# Just database
docker-compose up -d postgres

# Full server stack
docker-compose --profile server up -d
```

## CI Integration

### CI Tasks

```bash
cargo make ci-check    # fmt-check, clippy, docs, audit
cargo make ci-test     # nextest with CI profile
cargo make ci-flow     # Complete CI flow
```

### CI Environment Setup

The `cargo-make/scripts/setup-env.sh` script mirrors `.github/actions/setup-env`:
- Installs required tools
- Configures environment variables
- Validates prerequisites

### Key CI Considerations

- Cluster tests are NOT run in CI (GitHub Actions resource constraints)
- E2E tests require `docker compose -f docker/docker-compose.test.yml up -d`
- SQLx cache (`.sqlx/`) must be committed for CI offline builds
- `SQLX_OFFLINE=true` is forbidden -- always use `DATABASE_URL`

## Health Checks

```bash
# REST health
curl http://localhost:8080/health
curl http://localhost:8080/health/detailed

# gRPC health
grpcurl -plaintext localhost:9190 tasker.v1.HealthService/CheckLiveness
grpcurl -plaintext localhost:9190 tasker.v1.HealthService/CheckReadiness

# Kubernetes probes
livenessProbe:
  httpGet: { path: /health, port: 8080 }
readinessProbe:
  httpGet: { path: /health, port: 8080 }
```

## Kubernetes Deployment

### Key Environment Variables

| Variable | Purpose |
|----------|---------|
| `TASKER_ENV` | Environment (test, development, production) |
| `TASKER_CONFIG_PATH` | Path to single merged config file (required for production) |
| `DATABASE_URL` | PostgreSQL connection string (use Secrets) |
| `TASKER_MESSAGING_BACKEND` | `pgmq` or `rabbitmq` |
| `RABBITMQ_URL` | RabbitMQ connection (if using RabbitMQ backend) |
| `RUST_LOG` | Log level (info, debug, etc.) |

### Scaling Guidelines

- Orchestration: Scale based on API request rate and task throughput
- Workers: Scale per namespace based on queue depth
- Database: Monitor total connections = (orch replicas x pool size) + (worker replicas x pool size)
- Use HPA for automatic scaling on CPU/memory and custom metrics

## References

- Deployment patterns: `docs/architecture/deployment-patterns.md`
- Configuration management: `docs/guides/configuration-management.md`
- Docker compose files: `docker/` directory
