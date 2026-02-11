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
COPY src/ ./src/

# Copy workspace crates needed by orchestration
COPY tasker-orchestration/ ./tasker-orchestration/
COPY tasker-shared/ ./tasker-shared/
COPY tasker-client/ ./tasker-client/
COPY tasker-ctl/ ./tasker-ctl/
COPY tasker-pgmq/ ./tasker-pgmq/
COPY migrations/ ./migrations/
COPY proto/ ./proto/

# Copy minimal workspace structure for crates we don't actually need
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-worker workers/rust workers/ruby workers/python workers/typescript
COPY tasker-worker/Cargo.toml ./tasker-worker/
COPY workers/rust/Cargo.toml ./workers/rust/
COPY workers/ruby/ext/tasker_core/Cargo.toml ./workers/ruby/ext/tasker_core/
COPY workers/python/Cargo.toml ./workers/python/
COPY workers/typescript/Cargo.toml ./workers/typescript/

# Generate dependency recipe
RUN cargo chef prepare --recipe-path recipe.json

# =============================================================================
# Builder - Build dependencies and application
# =============================================================================
FROM chef AS builder

WORKDIR /app

# Copy recipe and build dependencies (cached layer)
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Copy workspace root files and all source
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/
COPY src/ ./src/

# Copy workspace crates needed by orchestration
COPY tasker-orchestration/ ./tasker-orchestration/
COPY tasker-shared/ ./tasker-shared/
COPY tasker-client/ ./tasker-client/
COPY tasker-ctl/ ./tasker-ctl/
COPY tasker-pgmq/ ./tasker-pgmq/
COPY migrations/ ./migrations/
COPY proto/ ./proto/

# Copy minimal workspace structure for crates we don't actually need
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-worker workers/rust workers/ruby workers/python workers/typescript
COPY tasker-worker/Cargo.toml ./tasker-worker/
COPY workers/rust/Cargo.toml ./workers/rust/
COPY workers/ruby/ext/tasker_core/Cargo.toml ./workers/ruby/ext/tasker_core/
COPY workers/python/Cargo.toml ./workers/python/
COPY workers/typescript/Cargo.toml ./workers/typescript/

# Set offline mode for SQLx
ENV SQLX_OFFLINE=true

# Build optimized release binary
# IMPORTANT: Use --locked to ensure Cargo.lock is respected (prevents serde version conflicts)
RUN cargo build --release --all-features --locked --bin tasker-server -p tasker-orchestration

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
