# =============================================================================
# Python Worker Service - Production Dockerfile
# =============================================================================
# Python-driven worker that bootstraps Rust foundation via FFI (PyO3)
# Context: tasker-core/ directory (workspace root)
# Usage: docker build -f docker/build/python-worker.prod.Dockerfile -t tasker-python-worker:prod .

# =============================================================================
# Python Builder - Compile PyO3 extensions with both Python and Rust available
# =============================================================================
FROM cgr.dev/chainguard/python:latest-dev AS python_builder

USER root

# Install system dependencies for PyO3 compilation (Wolfi/apk packages)
RUN apk add --no-cache \
    build-base \
    pkgconf \
    libffi-dev \
    openssl-dev \
    postgresql-16-dev \
    clang-19 \
    ca-certificates-bundle \
    protobuf-dev \
    curl

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:$PATH"

# Set libclang path for bindgen (Wolfi LLVM)
ENV LIBCLANG_PATH=/usr/lib

# Install UV using official Astral image (recommended approach)
# See: https://docs.astral.sh/uv/guides/integration/docker/
COPY --from=ghcr.io/astral-sh/uv:0.9.17 /uv /uvx /bin/

# UV configuration for Docker builds
ENV UV_LINK_MODE=copy
ENV UV_COMPILE_BYTECODE=1
ENV UV_PYTHON_DOWNLOADS=never

WORKDIR /app

# Copy workspace root files for Cargo workspace resolution
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/
# Strip mold linker config — mold is a dev-only optimization, not available in containers
RUN sed -i '/\[target\.x86_64/,/^$/d' .cargo/config.toml
COPY src/ ./src/
COPY vendor/ ./vendor/

# Copy workspace crates needed by Python FFI extension
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-worker/ ./crates/tasker-worker/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-ctl/ ./crates/tasker-ctl/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
COPY proto/ ./proto/

# Copy minimal workspace structure for crates we don't actually need
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-orchestration tasker-example-rs tasker-rb tasker-ts \
    tasker-sdk tasker-mcp tasker-grammar tasker-secure tasker-runtime
COPY crates/tasker-orchestration/Cargo.toml ./crates/tasker-orchestration/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-rb/ext/tasker_core/Cargo.toml ./crates/tasker-rb/ext/tasker_core/
COPY crates/tasker-ts/Cargo.toml ./crates/tasker-ts/
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/

# Copy Python worker source code to proper workspace location
COPY crates/tasker-py/ ./crates/tasker-py/
COPY migrations/ ./migrations/

# Set working directory and environment for Python worker
ENV SQLX_OFFLINE=true
WORKDIR /app/crates/tasker-py

# Create virtual environment using UV
RUN uv venv /app/.venv
ENV VIRTUAL_ENV=/app/.venv
ENV PATH="/app/.venv/bin:$PATH"

# Install Python dependencies using UV (no dev deps for production)
# --active tells uv to use the venv specified by VIRTUAL_ENV rather than .venv in project dir
RUN uv sync --no-dev --locked --active

# Install maturin for PyO3 compilation (build dependency)
RUN uv pip install maturin>=1.7

# Compile Python FFI extensions
# NOTE: No BuildKit cache mounts — stale pythonize/serde artifacts cause "can't find crate" errors.
# IMPORTANT: Use --locked to ensure Cargo.lock is respected (prevents version conflicts)
RUN maturin develop --release --locked

# =============================================================================
# Runtime - Python-driven worker image
# =============================================================================
FROM cgr.dev/chainguard/python:latest-dev AS runtime

LABEL org.opencontainers.image.source="https://github.com/tasker-systems/tasker-core"
LABEL org.opencontainers.image.description="Tasker Python worker - Python FFI step handler execution via PyO3"
LABEL org.opencontainers.image.licenses="MIT"

WORKDIR /app

USER root

# Install runtime dependencies (Wolfi/apk packages)
RUN apk add --no-cache \
    bash \
    libpq-16 \
    openssl \
    libffi \
    ca-certificates-bundle \
    curl

# Copy virtual environment from builder (includes compiled bytecode)
COPY --from=python_builder /app/.venv /app/.venv
ENV VIRTUAL_ENV=/app/.venv
ENV PATH="/app/.venv/bin:$PATH"

# Copy Python worker source code (NOT test handlers — those are volume-mounted)
COPY --from=python_builder /app/crates/tasker-py/python ./python_worker/python
COPY --from=python_builder /app/crates/tasker-py/bin ./python_worker/bin

# Ensure all Python files are readable
RUN chmod -R 755 ./python_worker/bin && \
    chmod -R 644 ./python_worker/python && find ./python_worker/python -type d -exec chmod 755 {} \;

# Copy Python worker entrypoint script
COPY docker/scripts/python-worker-entrypoint.sh /app/python_worker_entrypoint.sh
RUN chmod 755 /app/python_worker_entrypoint.sh

# Set environment variables for Python worker
ENV APP_NAME=tasker-python-worker
ENV PYTHON_WORKER_ENABLED=true
ENV PYTHONPATH=/app/python_worker/python

# Python-specific environment
ENV PYTHONUNBUFFERED=1
ENV PYTHONDONTWRITEBYTECODE=1
ENV PYTHONOPTIMIZE=2

# Production environment settings
ENV TASKER_ENV=production

# Template discovery paths for Python handlers
ENV TASKER_TEMPLATE_PATH=/app/python_templates
ENV PYTHON_HANDLER_PATH=/app/python_handlers

# Python worker will expose its own health check via the bootstrap system
HEALTHCHECK --interval=15s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8081/health || exit 1

USER nonroot

EXPOSE 8081 9300

WORKDIR /app/python_worker

# Run Python worker entrypoint
ENTRYPOINT ["/app/python_worker_entrypoint.sh"]
