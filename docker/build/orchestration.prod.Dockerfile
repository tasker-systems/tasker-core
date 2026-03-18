# =============================================================================
# Orchestration Service - Production Dockerfile
# =============================================================================
# Optimized for production deployment with minimal size and maximum security
# Context: tasker-core/ directory (workspace root)
# Usage: docker build -f docker/build/orchestration.prod.Dockerfile -t tasker-orchestration:prod .

FROM rust:1.90-bookworm AS chef

# Install cargo-chef and sqlx-cli for dependency layer caching and migrations
RUN cargo install cargo-chef
RUN cargo install sqlx-cli --features postgres

# Install system dependencies
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

# Copy workspace root files
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/
# Strip mold linker config — mold is a dev-only optimization, not available in containers
RUN sed -i '/\[target\.x86_64/,/^$/d' .cargo/config.toml
COPY src/ ./src/
COPY vendor/ ./vendor/

# Copy workspace crates needed by orchestration
COPY crates/tasker-orchestration/ ./crates/tasker-orchestration/
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-ctl/ ./crates/tasker-ctl/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
COPY migrations/ ./migrations/
COPY proto/ ./proto/

# Copy minimal workspace structure for crates we don't actually need
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-worker tasker-example-rs tasker-rb tasker-py tasker-ts \
    tasker-sdk tasker-mcp tasker-grammar tasker-secure tasker-runtime
COPY crates/tasker-worker/Cargo.toml ./crates/tasker-worker/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-rb/ext/tasker_core/Cargo.toml ./crates/tasker-rb/ext/tasker_core/
COPY crates/tasker-py/Cargo.toml ./crates/tasker-py/
COPY crates/tasker-ts/Cargo.toml ./crates/tasker-ts/
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/

# Generate dependency recipe
RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Builder - Build dependencies and application
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
    /tmp/create-workspace-stubs.sh tasker-orchestration tasker-shared tasker-client tasker-ctl tasker-pgmq \
    tasker-worker tasker-example-rs tasker-rb tasker-py tasker-ts \
    tasker-sdk tasker-mcp tasker-grammar tasker-secure tasker-runtime
# Copy real Cargo.toml for all stubbed crates (stubs only create src/lib.rs)
COPY crates/tasker-orchestration/Cargo.toml ./crates/tasker-orchestration/
COPY crates/tasker-shared/Cargo.toml ./crates/tasker-shared/
COPY crates/tasker-client/Cargo.toml ./crates/tasker-client/
COPY crates/tasker-ctl/Cargo.toml ./crates/tasker-ctl/
COPY crates/tasker-pgmq/Cargo.toml ./crates/tasker-pgmq/
COPY crates/tasker-worker/Cargo.toml ./crates/tasker-worker/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-rb/ext/tasker_core/Cargo.toml ./crates/tasker-rb/ext/tasker_core/
COPY crates/tasker-py/Cargo.toml ./crates/tasker-py/
COPY crates/tasker-ts/Cargo.toml ./crates/tasker-ts/
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
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
COPY crates/tasker-orchestration/ ./crates/tasker-orchestration/
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-ctl/ ./crates/tasker-ctl/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
# Re-copy Cargo.toml for stub crates (chef cook replaces them with v0.0.1 placeholders)
COPY crates/tasker-worker/Cargo.toml ./crates/tasker-worker/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-rb/ext/tasker_core/Cargo.toml ./crates/tasker-rb/ext/tasker_core/
COPY crates/tasker-py/Cargo.toml ./crates/tasker-py/
COPY crates/tasker-ts/Cargo.toml ./crates/tasker-ts/
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/

# Set offline mode for SQLx
ENV SQLX_OFFLINE=true

# Build optimized release binary
# IMPORTANT: Use --locked to ensure Cargo.lock is respected (prevents serde version conflicts)
# NOTE: Do NOT use --all-features here. Default features include everything needed
# (grpc-api, postgres, web-api). Using --all-features would pull in tokio-console,
# which panics at runtime without RUSTFLAGS="--cfg tokio_unstable" (TAS-278).
RUN cargo build --release --locked --bin tasker-server -p tasker-orchestration

# Strip binary for minimal size
RUN strip target/release/tasker-server

# =============================================================================
# Runtime - Minimal runtime image
# =============================================================================
FROM cgr.dev/chainguard/wolfi-base:latest AS runtime

LABEL org.opencontainers.image.source="https://github.com/tasker-systems/tasker-core"
LABEL org.opencontainers.image.description="Tasker orchestration service - workflow task orchestration engine"
LABEL org.opencontainers.image.licenses="MIT"

WORKDIR /app

# Install runtime dependencies (Wolfi/apk packages)
RUN apk add --no-cache \
    bash \
    libpq-16 \
    openssl \
    curl \
    ca-certificates-bundle \
    postgresql-16-client

# Copy binary from builder (workspace target directory)
COPY --from=builder /app/target/release/tasker-server ./tasker-orchestration

# Copy SQLx CLI from builder
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx

# Copy migration scripts and migrations
COPY docker/scripts/ ./scripts/
COPY migrations/ ./migrations/

# Make scripts executable before switching to non-root user
RUN chmod +x ./scripts/*.sh

# Set environment variables for the service
ENV APP_NAME=tasker-orchestration

# Health check
HEALTHCHECK --interval=10s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

USER nonroot

EXPOSE 8080 9190

# Use orchestration-specific entrypoint that handles migrations
ENTRYPOINT ["./scripts/orchestration-entrypoint.sh"]
CMD ["./tasker-orchestration"]
