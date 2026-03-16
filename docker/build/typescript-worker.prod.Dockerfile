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
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-worker/ ./crates/tasker-worker/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-ctl/ ./crates/tasker-ctl/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
COPY proto/ ./proto/

# Copy minimal workspace structure for crates we don't actually need
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-orchestration tasker-example-rs tasker-rb tasker-py \
    tasker-sdk tasker-mcp tasker-grammar tasker-secure tasker-runtime
COPY crates/tasker-orchestration/Cargo.toml ./crates/tasker-orchestration/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-rb/ext/tasker_core/Cargo.toml ./crates/tasker-rb/ext/tasker_core/
COPY crates/tasker-py/Cargo.toml ./crates/tasker-py/
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/

# Copy TypeScript worker source code to proper workspace location
COPY crates/tasker-ts/ ./crates/tasker-ts/
COPY migrations/ ./migrations/

# Set environment for build
ENV SQLX_OFFLINE=true

# Install Bun dependencies (includes @napi-rs/cli for building)
WORKDIR /app/crates/tasker-ts
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

# Copy TypeScript worker from builder (NOT test handlers — those are volume-mounted)
WORKDIR /app/typescript_worker
COPY --from=typescript_builder /app/crates/tasker-ts/bin ./bin
COPY --from=typescript_builder /app/crates/tasker-ts/src ./src
COPY --from=typescript_builder /app/crates/tasker-ts/dist ./dist
COPY --from=typescript_builder /app/crates/tasker-ts/package.json ./
COPY --from=typescript_builder /app/crates/tasker-ts/tsconfig.json ./
COPY --from=typescript_builder /app/crates/tasker-ts/node_modules ./node_modules

# Copy napi-rs .node FFI modules (built by `bunx napi build --platform`)
COPY --from=typescript_builder /app/crates/tasker-ts/tasker_ts.*.node ./

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
