# =============================================================================
# MCP Server - Production Dockerfile
# =============================================================================
# Builds the tasker-mcp binary for LLM agent integration
# Context: tasker-core/ directory (workspace root)
# Usage: docker build -f docker/build/mcp.prod.Dockerfile -t tasker-mcp:prod .

FROM rust:1.90-bookworm AS chef

RUN cargo install cargo-chef

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libpq-dev \
    build-essential \
    ca-certificates \
    protobuf-compiler \
    libprotobuf-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# =============================================================================
# Planner - Generate recipe for dependency caching
# =============================================================================
FROM chef AS planner

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/
COPY src/ ./src/
COPY vendor/ ./vendor/

# tasker-mcp depends on: tasker-sdk, tasker-shared, tasker-client
COPY crates/tasker-mcp/ ./crates/tasker-mcp/
COPY crates/tasker-sdk/ ./crates/tasker-sdk/
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
COPY proto/ ./proto/
COPY migrations/ ./migrations/

# Stubs for crates not needed by tasker-mcp
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-orchestration tasker-worker tasker-ctl \
    tasker-example-rs tasker-rb tasker-py tasker-ts \
    tasker-grammar tasker-secure tasker-runtime
COPY crates/tasker-orchestration/Cargo.toml ./crates/tasker-orchestration/
COPY crates/tasker-worker/Cargo.toml ./crates/tasker-worker/
COPY crates/tasker-ctl/Cargo.toml ./crates/tasker-ctl/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-rb/ext/tasker_core/Cargo.toml ./crates/tasker-rb/ext/tasker_core/
COPY crates/tasker-py/Cargo.toml ./crates/tasker-py/
COPY crates/tasker-ts/Cargo.toml ./crates/tasker-ts/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/

RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Builder
# =============================================================================
FROM chef AS builder

WORKDIR /app

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/
COPY src/ ./src/

COPY crates/tasker-mcp/ ./crates/tasker-mcp/
COPY crates/tasker-sdk/ ./crates/tasker-sdk/
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
COPY proto/ ./proto/
COPY migrations/ ./migrations/

COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-orchestration tasker-worker tasker-ctl \
    tasker-example-rs tasker-rb tasker-py tasker-ts \
    tasker-grammar tasker-secure tasker-runtime
COPY crates/tasker-orchestration/Cargo.toml ./crates/tasker-orchestration/
COPY crates/tasker-worker/Cargo.toml ./crates/tasker-worker/
COPY crates/tasker-ctl/Cargo.toml ./crates/tasker-ctl/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-rb/ext/tasker_core/Cargo.toml ./crates/tasker-rb/ext/tasker_core/
COPY crates/tasker-py/Cargo.toml ./crates/tasker-py/
COPY crates/tasker-ts/Cargo.toml ./crates/tasker-ts/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/

ENV SQLX_OFFLINE=true

# NOTE: Do NOT use --all-features here. Default features are sufficient.
RUN cargo build --release --locked --bin tasker-mcp -p tasker-mcp
RUN strip target/release/tasker-mcp

# =============================================================================
# Runtime
# =============================================================================
FROM cgr.dev/chainguard/wolfi-base:latest AS runtime

LABEL org.opencontainers.image.source="https://github.com/tasker-systems/tasker-core"
LABEL org.opencontainers.image.description="Tasker MCP server - LLM agent integration via Model Context Protocol"
LABEL org.opencontainers.image.licenses="MIT"

WORKDIR /app

RUN apk add --no-cache \
    bash \
    openssl \
    ca-certificates-bundle \
    curl

COPY --from=builder /app/target/release/tasker-mcp ./tasker-mcp

ENV APP_NAME=tasker-mcp

USER nonroot

# tasker-mcp uses stdio transport by default (no HTTP port)
# If running in HTTP mode, expose the configured port
EXPOSE 8090

ENTRYPOINT ["./tasker-mcp"]
