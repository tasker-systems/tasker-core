# =============================================================================
# FFI Builder - Cross-Architecture Multi-Language Build Container
# =============================================================================
# Single "fat" builder with ALL toolchains (Rust, Python, Ruby, Bun).
# Maximizes sccache hits: all 3 FFI builds share ~709 compiled rlibs.
#
# Context: tasker-core/ directory (workspace root)
# Usage:
#   docker build -f docker/build/ffi-builder.Dockerfile -t tasker-ffi-builder .
#   docker run --rm -v ./artifacts:/app/artifacts tasker-ffi-builder
#
# Supports multi-arch via docker buildx (linux/amd64, linux/arm64).

# Ruby 3.4 on Debian bookworm — provides Ruby >=3.4 needed by tasker-rb gemspec.
# Debian bookworm's system ruby is 3.1.2 which is too old.
FROM ruby:3.4-slim-bookworm AS builder

ARG TARGETARCH

LABEL org.opencontainers.image.source="https://github.com/tasker-systems/tasker-core"
LABEL org.opencontainers.image.description="Tasker FFI builder - cross-architecture multi-language build"

# =============================================================================
# System dependencies
# =============================================================================
# Ruby already provided by base image; install everything else.
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libffi-dev \
    libssl-dev \
    libpq-dev \
    libclang-dev \
    libyaml-dev \
    protobuf-compiler \
    libprotobuf-dev \
    ca-certificates \
    curl \
    unzip \
    git \
    file \
    python3 \
    python3-pip \
    python3-venv \
    python3-dev \
    && rm -rf /var/lib/apt/lists/*

# =============================================================================
# Rust toolchain + sccache
# =============================================================================
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install sccache (arch-aware download)
ARG SCCACHE_VERSION=0.8.2
RUN case "${TARGETARCH}" in \
        amd64) SCCACHE_ARCH="x86_64-unknown-linux-musl" ;; \
        arm64) SCCACHE_ARCH="aarch64-unknown-linux-musl" ;; \
        *) echo "Unsupported arch: ${TARGETARCH}" && exit 1 ;; \
    esac && \
    curl -sL "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/sccache-v${SCCACHE_VERSION}-${SCCACHE_ARCH}.tar.gz" \
    | tar xz --strip-components=1 -C /usr/local/bin "sccache-v${SCCACHE_VERSION}-${SCCACHE_ARCH}/sccache" && \
    chmod +x /usr/local/bin/sccache

# =============================================================================
# Python toolchain (maturin + uv)
# =============================================================================
# Install uv from Astral
COPY --from=ghcr.io/astral-sh/uv:0.9.17 /uv /uvx /usr/local/bin/

# Install maturin globally
RUN pip3 install --break-system-packages maturin>=1.7

# Ruby toolchain — ruby:3.4-slim-bookworm includes gem and bundler.
# Only update bundler if the Gemfile.lock requires a newer version.
RUN gem update bundler --no-document 2>/dev/null || true

# =============================================================================
# Bun runtime
# =============================================================================
RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"

# =============================================================================
# sccache configuration (defaults, overridable at runtime)
# =============================================================================
ENV SCCACHE_DIR=/root/.cache/sccache
ENV SCCACHE_CACHE_SIZE=10G
ENV RUSTC_WRAPPER=sccache

# =============================================================================
# Workspace setup
# =============================================================================
WORKDIR /app

# Copy workspace root files for Cargo workspace resolution
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/
COPY src/ ./src/

# Copy shared crates needed by all FFI extensions
COPY tasker-shared/ ./tasker-shared/
COPY tasker-worker/ ./tasker-worker/
COPY tasker-client/ ./tasker-client/
COPY tasker-ctl/ ./tasker-ctl/
COPY tasker-pgmq/ ./tasker-pgmq/
COPY proto/ ./proto/
COPY migrations/ ./migrations/

# Copy SQLx offline query cache
COPY .sqlx/ ./.sqlx/

# Create workspace stubs for crates we don't build
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-orchestration workers/rust
COPY tasker-orchestration/Cargo.toml ./tasker-orchestration/
COPY workers/rust/Cargo.toml ./workers/rust/

# Copy all three FFI worker sources
COPY workers/python/ ./workers/python/
COPY workers/ruby/ ./workers/ruby/
COPY workers/typescript/ ./workers/typescript/

# Install Ruby bundle dependencies at image build time
WORKDIR /app/workers/ruby
RUN bundle config set --local without 'development' && bundle install --jobs 4

# Return to workspace root
WORKDIR /app

# Copy build scripts
COPY scripts/ffi-build/ ./scripts/ffi-build/
RUN chmod +x ./scripts/ffi-build/*.sh ./scripts/ffi-build/lib/common.sh

# SQLx offline mode
ENV SQLX_OFFLINE=true

# Default entry point: build all FFI libs for current platform
ENTRYPOINT ["./scripts/ffi-build/build-all.sh"]
