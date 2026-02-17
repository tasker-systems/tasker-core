# =============================================================================
# TypeScript Worker Service - Production Dockerfile
# =============================================================================
# TypeScript/Bun-driven worker that bootstraps Rust foundation via FFI
# Context: tasker-core/ directory (workspace root)
# Usage: docker build -f docker/build/typescript-worker.prod.Dockerfile -t tasker-typescript-worker:prod .

# =============================================================================
# TypeScript Builder - Compile FFI extensions with both Bun and Rust available
# =============================================================================
# NOTE: Chainguard bun image requires paid access — keep oven/bun for builder (ephemeral, not in prod image)
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
    libprotobuf-dev \
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

# Install Bun dependencies (includes @napi-rs/cli for building)
WORKDIR /app/workers/typescript
RUN bun install --frozen-lockfile

# Build napi-rs FFI module (places .node file in package root)
# IMPORTANT: --locked ensures Cargo.lock is respected (prevents serde version conflicts)
RUN bunx napi build --platform --release -- --locked

# Build TypeScript
RUN bun run build

# =============================================================================
# Runtime - Chainguard hardened runtime with bun copied from builder
# =============================================================================
FROM cgr.dev/chainguard/wolfi-base:latest AS runtime

LABEL org.opencontainers.image.source="https://github.com/tasker-systems/tasker-core"
LABEL org.opencontainers.image.description="Tasker TypeScript worker - TypeScript FFI step handler execution via Bun"
LABEL org.opencontainers.image.licenses="MIT"

WORKDIR /app

# Install runtime dependencies (Wolfi/apk packages)
RUN apk add --no-cache \
    bash \
    libpq-16 \
    openssl \
    libffi \
    ca-certificates-bundle \
    curl

# Copy bun binary from builder (Chainguard bun image requires paid access)
COPY --from=typescript_builder /usr/local/bin/bun /usr/local/bin/bun

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

# napi-rs .node file — FfiLayer auto-discovers from package root
# Set TASKER_FFI_MODULE_PATH only if .node file is in a non-standard location

# Production environment settings
ENV TASKER_ENV=production

# Template discovery paths for TypeScript handlers
ENV TASKER_TEMPLATE_PATH=/app/typescript_templates
ENV TYPESCRIPT_HANDLER_PATH=/app/typescript_handlers

# TypeScript worker will expose its own health check via the bootstrap system
HEALTHCHECK --interval=15s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8081/health || exit 1

USER nonroot

EXPOSE 8081 9400

WORKDIR /app/typescript_worker

# Run TypeScript worker entrypoint
ENTRYPOINT ["/app/typescript_worker_entrypoint.sh"]
