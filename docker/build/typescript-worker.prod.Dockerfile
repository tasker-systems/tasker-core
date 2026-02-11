# =============================================================================
# TypeScript Worker Service - Production Dockerfile
# =============================================================================
# TypeScript/Bun-driven worker that bootstraps Rust foundation via FFI
# Context: tasker-core/ directory (workspace root)
# Usage: docker build -f docker/build/typescript-worker.prod.Dockerfile -t tasker-typescript-worker:prod .

# =============================================================================
# TypeScript Builder - Compile FFI extensions with both Bun and Rust available
# =============================================================================
FROM oven/bun:1.3-debian AS typescript_builder

# Install system dependencies for FFI compilation
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libffi-dev \
    libssl-dev \
    libpq-dev \
    libclang-dev \
    ca-certificates \
    curl \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"

# Set libclang path for bindgen (Debian uses LLVM from default packages)
ENV LIBCLANG_PATH=/usr/lib/llvm-14/lib

WORKDIR /app

# Copy workspace root files for Cargo workspace resolution
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/
COPY src/ ./src/

# Copy workspace crates needed by TypeScript FFI extension
COPY tasker-shared/ ./tasker-shared/
COPY tasker-worker/ ./tasker-worker/
COPY tasker-client/ ./tasker-client/
COPY tasker-ctl/ ./tasker-ctl/
COPY tasker-pgmq/ ./tasker-pgmq/
COPY proto/ ./proto/

# Copy minimal workspace structure for crates we don't actually need
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-orchestration workers/rust workers/ruby workers/python
COPY tasker-orchestration/Cargo.toml ./tasker-orchestration/
COPY workers/rust/Cargo.toml ./workers/rust/
COPY workers/ruby/ext/tasker_core/Cargo.toml ./workers/ruby/ext/tasker_core/
COPY workers/python/Cargo.toml ./workers/python/

# Copy TypeScript worker source code to proper workspace location
COPY workers/typescript/ ./workers/typescript/
COPY migrations/ ./migrations/

# Set environment for build
ENV SQLX_OFFLINE=true

# Build Rust FFI extension (no BuildKit cache mounts for reproducible prod builds)
# IMPORTANT: Use --locked to ensure Cargo.lock is respected (prevents serde version conflicts)
RUN cargo build -p tasker-ts --release --locked && \
    mkdir -p /app/lib && \
    cp /app/target/release/libtasker_ts.so /app/lib/ 2>/dev/null || \
    cp /app/target/release/libtasker_ts.dylib /app/lib/ 2>/dev/null || \
    true

# Install Bun dependencies
WORKDIR /app/workers/typescript
RUN bun install --frozen-lockfile

# Build TypeScript
RUN bun run build

# =============================================================================
# Runtime - TypeScript/Bun-driven worker image
# =============================================================================
FROM oven/bun:1.3-slim AS runtime

LABEL org.opencontainers.image.source="https://github.com/tasker-systems/tasker-core"
LABEL org.opencontainers.image.description="Tasker TypeScript worker - TypeScript FFI step handler execution via Bun"
LABEL org.opencontainers.image.licenses="MIT"

WORKDIR /app

# Install runtime dependencies only (no build tools)
# Note: oven/bun:1.3-slim is based on Debian Bookworm which uses libssl3 and libffi8
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    libpq5 \
    libffi8 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -r -g daemon -u 999 tasker

# Copy FFI library from builder
COPY --from=typescript_builder /app/lib/ /app/lib/

# Copy TypeScript worker from builder (NOT test handlers — those are volume-mounted)
WORKDIR /app/typescript_worker
COPY --from=typescript_builder /app/workers/typescript/bin ./bin
COPY --from=typescript_builder /app/workers/typescript/src ./src
COPY --from=typescript_builder /app/workers/typescript/dist ./dist
COPY --from=typescript_builder /app/workers/typescript/package.json ./
COPY --from=typescript_builder /app/workers/typescript/tsconfig.json ./
COPY --from=typescript_builder /app/workers/typescript/node_modules ./node_modules

# Ensure all files are readable
RUN chmod -R 755 ./bin && \
    chmod -R 644 ./src && find ./src -type d -exec chmod 755 {} \;

# Copy TypeScript worker entrypoint script
COPY docker/scripts/typescript-worker-entrypoint.sh /app/typescript_worker_entrypoint.sh
RUN chmod 755 /app/typescript_worker_entrypoint.sh

# Set environment variables for TypeScript worker
ENV APP_NAME=tasker-typescript-worker
ENV TYPESCRIPT_WORKER_ENABLED=true

# Bun-specific environment
ENV BUN_VERSION=1.3

# FFI library path for runtime discovery
ENV TASKER_FFI_LIBRARY_PATH=/app/lib/libtasker_ts.so

# Production environment settings
ENV TASKER_ENV=production

# Template discovery paths for TypeScript handlers
ENV TASKER_TEMPLATE_PATH=/app/typescript_templates
ENV TYPESCRIPT_HANDLER_PATH=/app/typescript_handlers

# TypeScript worker will expose its own health check via the bootstrap system
HEALTHCHECK --interval=15s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8081/health || exit 1

USER tasker

EXPOSE 8081 9400

WORKDIR /app/typescript_worker

# Run TypeScript worker entrypoint
ENTRYPOINT ["/app/typescript_worker_entrypoint.sh"]
