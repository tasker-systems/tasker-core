# Twelve-Factor App Alignment

The [Twelve-Factor App](https://12factor.net/) methodology, authored by Adam Wiggins and contributors at Heroku, has been a foundational influence on Tasker Core's systems design. These principles were not adopted as a checklist but absorbed over years of building production systems. Some factors are deeply embedded in the architecture; others remain aspirational or partially realized.

This document maps each factor to where it shows up in the codebase, where we fall short, and what contributors should keep in mind. It is meant as practical guidance, not a compliance scorecard.

---

## I. Codebase

*One codebase tracked in revision control, many deploys.*

Tasker Core is a single Git monorepo containing all deployable services: orchestration server, workers (Rust, Ruby, Python, TypeScript), CLI, and shared libraries.

**Where this lives:**

- Root `Cargo.toml` defines the workspace with all crate members
- Environment-specific Docker Compose files produce different deploys from the same source: `docker/docker-compose.prod.yml`, `docker/docker-compose.dev.yml`, `docker/docker-compose.test.yml`, `docker/docker-compose.ci.yml`
- Feature flags (`web-api`, `grpc-api`, `test-services`, `test-cluster`) control build variations without code branches

**Gaps:** The monorepo means all crates share a single version today (v0.1.0). As the project matures toward independent crate publishing, version coordination will need more tooling. Independent crate versioning and release management tooling will need to evolve as the project matures.

---

## II. Dependencies

*Explicitly declare and isolate dependencies.*

Rust's Cargo ecosystem makes this natural. All dependencies are declared in `Cargo.toml` with workspace-level management and pinned in `Cargo.lock`.

**Where this lives:**

- Root `Cargo.toml` `[workspace.dependencies]` section — single source of truth for shared dependency versions
- `Cargo.lock` committed to the repository for reproducible builds
- Multi-stage Docker builds (`docker/build/orchestration.prod.Dockerfile`) use `cargo-chef` for cached, reproducible dependency resolution
- No runtime dependency fetching — everything resolved at build time

**Gaps:** FFI workers each bring their own dependency ecosystem (Python's `uv`/`pyproject.toml`, Ruby's `Bundler`/`Gemfile`, TypeScript's `bun`/`package.json`). These are well-declared but not unified — contributors working across languages need to manage multiple lock files.

---

## III. Config

*Store config in the environment.*

This is one of the strongest alignments. All runtime configuration flows through environment variables, with TOML files providing structured defaults that reference those variables.

**Where this lives:**

- `config/dotenv/` — environment-specific `.env` files (`base.env`, `test.env`, `orchestration.env`)
- `config/tasker/base/*.toml` — role-based defaults with `${ENV_VAR:-default}` interpolation
- `config/tasker/environments/{test,development,production}/` — environment overrides
- `docker/.env.prod.template` — production variable template
- `tasker-shared/src/config/` — config loading with environment variable resolution
- No secrets in source: `DATABASE_URL`, `POSTGRES_PASSWORD`, JWT keys all via environment

**For contributors:** Never hard-code connection strings, credentials, or deployment-specific values. Use environment variables with sensible defaults in the TOML layer. The configuration structure is role-based (orchestration/worker/common), not component-based — see `CLAUDE.md` for details.

---

## IV. Backing Services

*Treat backing services as attached resources.*

Backing services are abstracted behind trait interfaces and swappable via configuration alone.

**Where this lives:**

- **Database**: PostgreSQL connection via `DATABASE_URL`, pool settings in `config/tasker/base/common.toml` under `[common.database.pool]`
- **Messaging**: PGMQ or RabbitMQ selected via `TASKER_MESSAGING_BACKEND` environment variable — same code paths, different drivers
- **Cache**: Redis, Moka (in-process), or disabled entirely via `[common.cache]` configuration
- **Observability**: OpenTelemetry with pluggable backends (Honeycomb, Jaeger, Grafana Tempo) via `OTEL_EXPORTER_OTLP_ENDPOINT`
- Circuit breakers protect against backing service failures: `[common.circuit_breakers.component_configs]`

**For contributors:** When adding a new backing service dependency, ensure it can be configured via environment variables and that the system degrades gracefully when it's unavailable. Follow the messaging abstraction pattern — trait-based interfaces, not concrete types.

---

## V. Build, Release, Run

*Strictly separate build and run stages.*

The Docker build pipeline enforces this cleanly with multi-stage builds.

**Where this lives:**

- **Build**: `docker/build/orchestration.prod.Dockerfile` — `cargo-chef` dependency caching, `cargo build --release --all-features --locked`, binary stripping
- **Release**: Tagged Docker images with only runtime dependencies (no build tools), non-root user (`tasker:999`), read-only config mounts
- **Run**: `docker/scripts/orchestration-entrypoint.sh` — environment validation, database availability check, migrations, then `exec` into the application binary
- Deployment modes control startup behavior: `standard`, `migrate-only`, `no-migrate`, `safe`, `emergency`

**Gaps:** Local development doesn't enforce the same separation — developers run `cargo run` directly, which conflates build and run. This is fine for development ergonomics but worth noting as a difference from the production path.

---

## VI. Processes

*Execute the app as one or more stateless processes.*

All persistent state lives in PostgreSQL. Processes can be killed and restarted at any time without data loss.

**Where this lives:**

- Orchestration server: stateless HTTP/gRPC service backed by `tasker.tasks` and `tasker.steps` tables
- Workers: claim steps from message queues, execute handlers, write results back — no in-memory state across requests
- Message queue visibility timeouts (`visibility_timeout_seconds` in worker config) ensure unacknowledged messages are reclaimed by other workers
- Docker Compose `replicas` setting scales workers horizontally

**For contributors:** Never store workflow state in memory across requests. If you need coordination state, it belongs in PostgreSQL. In-memory caches (Moka) are optimization layers, not sources of truth — the system must function correctly without them.

---

## VII. Port Binding

*Export services via port binding.*

Each service is self-contained and binds its own ports.

**Where this lives:**

- REST: `config/tasker/base/orchestration.toml` — `[orchestration.web] bind_address = "${TASKER_WEB_BIND_ADDRESS:-0.0.0.0:8080}"`
- gRPC: `[orchestration.grpc] bind_address = "${TASKER_ORCHESTRATION_GRPC_BIND_ADDRESS:-0.0.0.0:9190}"`
- Worker REST/gRPC on separate ports (8081/9191)
- Health endpoints on both transports for load balancer integration
- Docker exposes ports via environment-configurable mappings

---

## VIII. Concurrency

*Scale out via the process model.*

The system scales horizontally by adding worker processes and vertically by tuning concurrency settings.

**Where this lives:**

- Horizontal: `docker/docker-compose.prod.yml` — `replicas: ${WORKER_REPLICAS:-2}`, each worker is independent
- Vertical: `config/tasker/base/orchestration.toml` — `max_concurrent_operations`, `batch_size` per event system
- Worker handler parallelism: `[worker.mpsc_channels.handler_dispatch] max_concurrent_handlers = 10`
- Load shedding: `[worker.mpsc_channels.handler_dispatch.load_shedding] capacity_threshold_percent = 80.0`

**Gaps:** The actor pattern within a single process is more vertical than horizontal — actors share a Tokio runtime and scale via async concurrency, not OS processes. This is a pragmatic choice for Rust's async model but means single-process scaling has limits that multiple processes solve.

---

## IX. Disposability

*Maximize robustness with fast startup and graceful shutdown.*

This factor gets significant attention due to the distributed nature of task orchestration.

**Where this lives:**

- **Graceful shutdown**: Signal handlers (SIGTERM, SIGINT) in `tasker-orchestration/src/bin/server.rs` and `tasker-worker/src/bin/` — actors drain in-flight work, OpenTelemetry flushes spans, connections close cleanly
- **Fast startup**: Compiled binary, pooled database connections, environment-driven config (no service discovery delays)
- **Crash recovery**: PGMQ visibility timeouts requeue unacknowledged messages; steps claimed by a crashed worker reappear for others after `visibility_timeout_seconds`
- **Entrypoint**: `docker/scripts/orchestration-entrypoint.sh` uses `exec` to replace shell with app process (proper PID 1 signal handling)
- **Health checks**: Docker `start_period` allows grace time before liveness probes begin

**For contributors:** When adding new async subsystems, ensure they participate in the shutdown sequence. Bounded channels and drain timeouts (`shutdown_drain_timeout_ms`) prevent shutdown from hanging indefinitely.

---

## X. Dev/Prod Parity

*Keep development, staging, and production as similar as possible.*

The same code, same migrations, and same config structure run everywhere — only values change.

**Where this lives:**

- `config/tasker/base/` provides defaults; `config/tasker/environments/` overrides per-environment — structure is identical
- `migrations/` directory contains SQL migrations shared across all environments
- Docker images use the same base (`debian:bullseye-slim`) and runtime user (`tasker:999`)
- Structured logging format (tracing crate) is consistent; only verbosity changes (`RUST_LOG`)
- E2E tests (`--features test-services`) exercise the same code paths as production

**Gaps:** Development uses `cargo run` with debug builds while production uses release-optimized Docker images. The observability stack (Grafana LGTM) is available in `docker-compose.dev.yml` but most local development happens without it. These are standard trade-offs, but contributors should periodically test against the full Docker stack to catch environment-specific issues.

---

## XI. Logs

*Treat logs as event streams.*

All logging goes to stdout/stderr. No file-based logging is built into the application.

**Where this lives:**

- `tasker-shared/src/logging.rs` — tracing subscriber writes to stdout, JSON format in production, ANSI colors in development (TTY-detected)
- OpenTelemetry integration exports structured traces via `OTEL_EXPORTER_OTLP_ENDPOINT`
- Correlation IDs (`correlation_id`) propagate through tasks, steps, actors, and message queues for distributed tracing
- `docker-compose.dev.yml` includes Loki for log aggregation and Grafana for visualization
- Entrypoint scripts log to stdout/stderr with role-prefixed format

**For contributors:** Use the `tracing` crate's `#[instrument]` macro and structured fields (`tracing::info!(task_id = %id, "processing")`) rather than string interpolation. Never write to log files directly.

---

## XII. Admin Processes

*Run admin/management tasks as one-off processes.*

The CLI and deployment scripts serve this role.

**Where this lives:**

- `tasker-ctl/` — task management (`create`, `list`, `cancel`), DLQ investigation (`dlq list`, `dlq recover`), system health, auth token management
- `docker/scripts/orchestration-entrypoint.sh` — `DEPLOYMENT_MODE=migrate-only` runs migrations and exits without starting the server
- `config-validator` binary validates TOML configuration as a one-off check
- Database migrations run as a distinct phase before application startup, with retry logic and timeout protection

**Gaps:** Some administrative operations (cache invalidation, circuit breaker reset) are only available through the REST/gRPC API, not the CLI. As the CLI matures, these should become first-class admin commands.

---

## Using This as a Contributor

These factors are not rules to enforce mechanically. They're a lens for evaluating design decisions:

- **Adding a new service dependency?** Factor IV says treat it as an attached resource — configure via environment, degrade gracefully without it.
- **Storing state?** Factor VI says processes are stateless — put it in PostgreSQL, not in memory.
- **Adding configuration?** Factor III says environment variables — use the existing TOML-with-env-var-interpolation pattern.
- **Writing logs?** Factor XI says event streams — stdout, structured fields, correlation IDs.
- **Building deployment artifacts?** Factor V says separate build/release/run — don't bake configuration into images.

When a factor conflicts with practical needs, document the trade-off. The goal is not purity but awareness.

---

## Attribution

The Twelve-Factor App methodology was created by Adam Wiggins with contributions from many others, originally published at [12factor.net](https://12factor.net/). It is made available under the MIT License and has influenced how a generation of developers think about building software-as-a-service applications. Its influence on this project is gratefully acknowledged.
