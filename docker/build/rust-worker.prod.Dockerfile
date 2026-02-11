# =============================================================================
# Rust Worker Service - Production Dockerfile
# =============================================================================
# Optimized for production deployment with minimal size and maximum security
# Context: tasker-core/ directory (workspace root)
# Usage: docker build -f docker/build/rust-worker.prod.Dockerfile -t tasker-worker-rust:prod .

FROM rust:1.90-bookworm AS chef

# Install cargo-chef for dependency layer caching
RUN cargo install cargo-chef

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

# Copy workspace crates needed by rust worker
COPY tasker-shared/ ./tasker-shared/
COPY tasker-client/ ./tasker-client/
COPY tasker-ctl/ ./tasker-ctl/
COPY tasker-pgmq/ ./tasker-pgmq/
COPY tasker-worker/ ./tasker-worker/
COPY workers/rust/ ./workers/rust/
COPY migrations/ ./migrations/
COPY proto/ ./proto/

# Copy minimal workspace structure for crates we don't actually need
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-orchestration workers/ruby workers/python workers/typescript
COPY tasker-orchestration/Cargo.toml ./tasker-orchestration/
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

# Copy workspace crates needed by rust worker
COPY tasker-shared/ ./tasker-shared/
COPY tasker-client/ ./tasker-client/
COPY tasker-ctl/ ./tasker-ctl/
COPY tasker-pgmq/ ./tasker-pgmq/
COPY tasker-worker/ ./tasker-worker/
COPY workers/rust/ ./workers/rust/
COPY migrations/ ./migrations/
COPY proto/ ./proto/

# Copy minimal workspace structure for crates we don't actually need
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-orchestration workers/ruby workers/python workers/typescript
COPY tasker-orchestration/Cargo.toml ./tasker-orchestration/
COPY workers/ruby/ext/tasker_core/Cargo.toml ./workers/ruby/ext/tasker_core/
COPY workers/python/Cargo.toml ./workers/python/
COPY workers/typescript/Cargo.toml ./workers/typescript/

# Set offline mode for SQLx
ENV SQLX_OFFLINE=true

# Build optimized release binary
# IMPORTANT: Use --locked to ensure Cargo.lock is respected (prevents serde version conflicts)
RUN cargo build --release --all-features --locked --bin rust-worker -p tasker-worker-rust

# Strip binary for minimal size
RUN strip target/release/rust-worker

# =============================================================================
# Runtime - Minimal runtime image
# =============================================================================
FROM cgr.dev/chainguard/wolfi-base:latest AS runtime

LABEL org.opencontainers.image.source="https://github.com/tasker-systems/tasker-core"
LABEL org.opencontainers.image.description="Tasker Rust worker - native Rust step handler execution"
LABEL org.opencontainers.image.licenses="MIT"

WORKDIR /app

# Install runtime dependencies (Wolfi/apk packages)
RUN apk add --no-cache \
    bash \
    libpq-16 \
    openssl \
    curl \
    ca-certificates-bundle

# Copy binary from builder
COPY --from=builder /app/target/release/rust-worker ./

# Create scripts directory and copy worker entrypoint script
RUN mkdir -p ./scripts
COPY docker/scripts/worker-entrypoint.sh ./scripts/worker-entrypoint.sh

# Make scripts executable before switching to non-root user
RUN chmod +x ./scripts/*.sh

# Set environment variables for the service
ENV APP_NAME=tasker-worker-rust

# Health check
HEALTHCHECK --interval=10s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8081/health || exit 1

USER nonroot

EXPOSE 8081 9191

# Use worker-specific entrypoint
ENTRYPOINT ["./scripts/worker-entrypoint.sh"]
CMD ["./rust-worker"]
