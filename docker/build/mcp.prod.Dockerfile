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
# Strip mold linker config — mold is a dev-only optimization, not available in containers
RUN sed -i '/\[target\.x86_64/,/^$/d' .cargo/config.toml
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

# Copy workspace root files (needed for cargo chef cook)
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/
# Strip mold linker config — mold is a dev-only optimization, not available in containers
RUN sed -i '/\[target\.x86_64/,/^$/d' .cargo/config.toml
COPY src/ ./src/
COPY vendor/ ./vendor/

# Copy minimal workspace structure for ALL crates (stubs satisfy cargo workspace validation)
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-mcp tasker-sdk tasker-shared tasker-client tasker-pgmq \
    tasker-orchestration tasker-worker tasker-ctl \
    tasker-example-rs tasker-rb tasker-py tasker-ts \
    tasker-grammar tasker-secure tasker-runtime
# Copy real Cargo.toml for all stubbed crates (stubs only create src/lib.rs)
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-shared/Cargo.toml ./crates/tasker-shared/
COPY crates/tasker-client/Cargo.toml ./crates/tasker-client/
COPY crates/tasker-pgmq/Cargo.toml ./crates/tasker-pgmq/
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
# Proto and migrations needed for dependency compilation (tasker-shared build.rs)
COPY proto/ ./proto/
COPY migrations/ ./migrations/

# Build dependencies from recipe (cached layer — invalidated only when deps change)
# This compiles all dependencies with stub sources; the real source is copied AFTER
# so that cargo detects source changes and rebuilds the actual binaries.
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# NOW copy real source for the crates we need (overwrites stubs, triggers rebuild).
# cargo chef cook overwrites Cargo.toml files with recipe versions that strip
# [workspace.lints] and use placeholder versions. Re-copy everything needed.
COPY Cargo.toml Cargo.lock ./
# tasker-mcp depends on: tasker-sdk, tasker-shared, tasker-client
COPY crates/tasker-mcp/ ./crates/tasker-mcp/
COPY crates/tasker-sdk/ ./crates/tasker-sdk/
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
# Re-copy Cargo.toml for stub crates (chef cook replaces them with v0.0.1 placeholders)
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
