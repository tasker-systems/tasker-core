# =============================================================================
# Ruby Worker Service - Production Dockerfile
# =============================================================================
# Ruby-driven worker that bootstraps Rust foundation via FFI
# Context: tasker-core/ directory (workspace root)
# Usage: docker build -f docker/build/ruby-worker.prod.Dockerfile -t tasker-ruby-worker:prod .

# =============================================================================
# Ruby Builder - Compile Ruby FFI extensions with both Ruby and Rust available
# =============================================================================
FROM cgr.dev/chainguard/ruby:latest-dev AS ruby_builder

USER root

# Install system dependencies for Ruby FFI compilation (Wolfi/apk packages)
RUN apk add --no-cache \
    build-base \
    pkgconf \
    libffi-dev \
    openssl-dev \
    postgresql-16-dev \
    clang-19 \
    yaml-dev \
    zlib-dev \
    ca-certificates-bundle \
    protobuf-dev \
    curl

# Install Rust toolchain for FFI compilation
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
# Set libclang path for bindgen (Wolfi LLVM)
ENV LIBCLANG_PATH=/usr/lib

WORKDIR /app

# Copy workspace root files for Cargo workspace resolution
COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/
COPY src/ ./src/

# Copy workspace crates needed by Ruby FFI extension
COPY tasker-shared/ ./tasker-shared/
COPY tasker-worker/ ./tasker-worker/
COPY tasker-client/ ./tasker-client/
COPY tasker-ctl/ ./tasker-ctl/
COPY tasker-pgmq/ ./tasker-pgmq/
COPY proto/ ./proto/

# Copy minimal workspace structure for crates we don't actually need
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-orchestration workers/rust workers/python workers/typescript
COPY tasker-orchestration/Cargo.toml ./tasker-orchestration/
COPY workers/rust/Cargo.toml ./workers/rust/
COPY workers/python/Cargo.toml ./workers/python/
COPY workers/typescript/Cargo.toml ./workers/typescript/

# Copy Ruby worker source code to proper workspace location
COPY workers/ruby/ ./workers/ruby/
COPY migrations/ ./migrations/

# Install Ruby dependencies
WORKDIR /app/workers/ruby
RUN bundle config set --local deployment 'true'
RUN bundle config set --local without 'development'
RUN bundle install

ENV SQLX_OFFLINE=true
ENV RB_SYS_CARGO_PROFILE=release
# IMPORTANT: Use --locked to ensure Cargo.lock is respected (prevents serde version conflicts)
ENV RB_SYS_CARGO_BUILD_ARGS="--locked"
# Compile Ruby FFI extensions
# rb_sys will handle all Rust compilation via bundle exec rake compile
RUN bundle exec rake compile

# =============================================================================
# Runtime - Ruby-driven worker image (Chainguard hardened)
# =============================================================================
FROM cgr.dev/chainguard/ruby:latest-dev AS runtime

LABEL org.opencontainers.image.source="https://github.com/tasker-systems/tasker-core"
LABEL org.opencontainers.image.description="Tasker Ruby worker - Ruby FFI step handler execution via Magnus"
LABEL org.opencontainers.image.licenses="MIT"

WORKDIR /app

USER root

# Install runtime dependencies (Wolfi/apk packages)
RUN apk add --no-cache \
    bash \
    libpq-16 \
    openssl \
    libffi \
    yaml \
    zlib \
    ca-certificates-bundle \
    curl \
    postgresql-16-client

# OPTIMIZATION: Copy only necessary Ruby worker files (exclude tmp/, spec/, doc/, etc.)
# This avoids copying 1.3GB of Rust build artifacts from tmp/ directory
WORKDIR /app/ruby_worker
COPY --from=ruby_builder /app/workers/ruby/bin ./bin
COPY --from=ruby_builder /app/workers/ruby/lib ./lib
COPY --from=ruby_builder /app/workers/ruby/Gemfile* ./
COPY --from=ruby_builder /app/workers/ruby/*.gemspec ./
COPY --from=ruby_builder /app/workers/ruby/Rakefile ./

# Copy bundled gems from builder (includes compiled extensions and all gems)
# Chainguard Ruby images use /usr/lib/ruby/gems as the gem home
# Copy from wherever bundler installed gems in the builder
COPY --from=ruby_builder /usr/lib/ruby/gems /usr/lib/ruby/gems

# Copy Ruby worker entrypoint script
COPY docker/scripts/ruby-worker-entrypoint.sh /app/ruby_worker_entrypoint.sh
RUN chmod +x /app/ruby_worker_entrypoint.sh

# Set environment variables for Ruby worker (production)
ENV APP_NAME=tasker-ruby-worker
ENV RUBY_WORKER_ENABLED=true
ENV BUNDLE_GEMFILE=/app/ruby_worker/Gemfile

# Production environment settings
ENV TASKER_ENV=production
ENV RUBY_ENV=production

# Template discovery paths for Ruby handlers
ENV TASKER_TEMPLATE_PATH=/app/ruby_templates
ENV RUBY_HANDLER_PATH=/app/ruby_handlers

# Production performance optimizations
ENV RUBY_GC_HEAP_GROWTH_FACTOR=1.1
ENV RUBY_GC_HEAP_GROWTH_MAX_SLOTS=100000
ENV RUBY_GC_HEAP_INIT_SLOTS=600000

# Ruby worker will expose its own health check via the bootstrap system
HEALTHCHECK --interval=15s --timeout=10s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8081/health || exit 1

USER nonroot

EXPOSE 8081 9200

# Run Ruby worker entrypoint (not Rust binary)
ENTRYPOINT ["/app/ruby_worker_entrypoint.sh"]
