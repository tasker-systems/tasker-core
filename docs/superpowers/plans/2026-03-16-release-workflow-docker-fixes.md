# Release Workflow & Docker Build Fixes — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all release workflow failures caused by TAS-361/362 workspace restructuring and integrate tasker-mcp into the main release pipeline.

**Architecture:** Update Docker COPY destinations to use `crates/` prefix matching post-restructuring workspace layout. Add 5 missing workspace crates as stubs to all Dockerfiles. Add mold linker to CLI build workflow. Expand CLI binary build matrix to include tasker-mcp. Add tasker-mcp container to publish-containers matrix.

**Tech Stack:** Docker, GitHub Actions, cargo-chef, bash

---

## File Map

### Modified files

| File | Responsibility |
|------|---------------|
| `docker/scripts/create-workspace-stubs.sh` | Stub creation script — update CRATE_PATHS to `crates/` prefix, add 5 new crates |
| `docker/build/orchestration.prod.Dockerfile` | Fix COPY destinations, add 5 missing stubs (planner + builder stages) |
| `docker/build/ruby-worker.prod.Dockerfile` | Fix COPY destinations, add 5 missing stubs |
| `docker/build/python-worker.prod.Dockerfile` | Fix COPY destinations, add 5 missing stubs |
| `docker/build/typescript-worker.prod.Dockerfile` | Fix COPY destinations, add 5 missing stubs |
| `docker/build/ffi-builder.Dockerfile` | Fix COPY destinations, add 5 missing stubs, fix path inconsistencies |
| `.github/workflows/build-cli-binaries.yml` | Add mold setup, expand matrix to build both tasker-ctl and tasker-mcp |
| `.github/workflows/release.yml` | Add tasker-mcp to publish-containers matrix, update release-summary artifact downloads |

### New files

| File | Responsibility |
|------|---------------|
| `docker/build/mcp.prod.Dockerfile` | Production Dockerfile for tasker-mcp (cargo-chef pattern, mirrors orchestration) |

### Deleted files

| File | Reason |
|------|--------|
| `.github/workflows/release-mcp.yml` | Replaced by integration into main release.yml + build-cli-binaries.yml |
| `crates/tasker-mcp/Dockerfile` | Replaced by docker/build/mcp.prod.Dockerfile |

---

## Chunk 1: Docker workspace stubs and path fixes

### Task 1: Update create-workspace-stubs.sh

**Files:**
- Modify: `docker/scripts/create-workspace-stubs.sh`

- [ ] **Step 1: Update CRATE_PATHS to use `crates/` prefix and add 5 new crates**

The workspace members in `Cargo.toml` are `crates/tasker-xxx`. Docker COPY destinations must match. Update the map:

```bash
declare -A CRATE_PATHS=(
    ["tasker-orchestration"]="crates/tasker-orchestration"
    ["tasker-worker"]="crates/tasker-worker"
    ["tasker-shared"]="crates/tasker-shared"
    ["tasker-client"]="crates/tasker-client"
    ["tasker-ctl"]="crates/tasker-ctl"
    ["tasker-pgmq"]="crates/tasker-pgmq"
    ["tasker-sdk"]="crates/tasker-sdk"
    ["tasker-mcp"]="crates/tasker-mcp"
    ["tasker-grammar"]="crates/tasker-grammar"
    ["tasker-secure"]="crates/tasker-secure"
    ["tasker-runtime"]="crates/tasker-runtime"
    ["tasker-example-rs"]="crates/tasker-example-rs"
    ["tasker-rb"]="crates/tasker-rb/ext/tasker_core"
    ["tasker-py"]="crates/tasker-py"
    ["tasker-ts"]="crates/tasker-ts"
)
```

- [ ] **Step 2: Commit**

```bash
git add docker/scripts/create-workspace-stubs.sh
git commit -m "fix(docker): update workspace stub paths to crates/ prefix and add new crates"
```

---

### Task 2: Fix orchestration.prod.Dockerfile

**Files:**
- Modify: `docker/build/orchestration.prod.Dockerfile`

The pattern: every `./tasker-xxx/` destination becomes `./crates/tasker-xxx/`. Both the planner and builder stages must be updated identically.

- [ ] **Step 1: Update planner stage COPY destinations (lines 40-56)**

Full-source crates — add `crates/` prefix to destinations:
```dockerfile
COPY crates/tasker-orchestration/ ./crates/tasker-orchestration/
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-ctl/ ./crates/tasker-ctl/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
```

Update stub creation to include 5 new crates:
```dockerfile
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-worker tasker-example-rs tasker-rb tasker-py tasker-ts \
    tasker-sdk tasker-mcp tasker-grammar tasker-secure tasker-runtime
```

Update existing stub Cargo.toml COPYs — add `crates/` prefix to destinations:
```dockerfile
COPY crates/tasker-worker/Cargo.toml ./crates/tasker-worker/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-rb/ext/tasker_core/Cargo.toml ./crates/tasker-rb/ext/tasker_core/
COPY crates/tasker-py/Cargo.toml ./crates/tasker-py/
COPY crates/tasker-ts/Cargo.toml ./crates/tasker-ts/
```

Add 5 new stub Cargo.toml COPYs:
```dockerfile
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/
```

- [ ] **Step 2: Update builder stage identically (lines 78-94)**

Apply the same COPY destination changes to the builder stage. The builder stage mirrors the planner stage exactly.

- [ ] **Step 3: Commit**

```bash
git add docker/build/orchestration.prod.Dockerfile
git commit -m "fix(docker): update orchestration Dockerfile for crates/ workspace layout"
```

---

### Task 3: Fix ruby-worker.prod.Dockerfile

**Files:**
- Modify: `docker/build/ruby-worker.prod.Dockerfile`

- [ ] **Step 1: Update COPY destinations and add missing stubs**

Full-source crates (lines 43-47) — add `crates/` prefix:
```dockerfile
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-worker/ ./crates/tasker-worker/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-ctl/ ./crates/tasker-ctl/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
```

Update stub creation (line 53) to include 5 new crates:
```dockerfile
/tmp/create-workspace-stubs.sh tasker-orchestration tasker-example-rs tasker-py tasker-ts \
    tasker-sdk tasker-mcp tasker-grammar tasker-secure tasker-runtime
```

Update existing stub Cargo.toml COPYs (lines 54-57) — add `crates/` prefix:
```dockerfile
COPY crates/tasker-orchestration/Cargo.toml ./crates/tasker-orchestration/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-py/Cargo.toml ./crates/tasker-py/
COPY crates/tasker-ts/Cargo.toml ./crates/tasker-ts/
```

Add 5 new stub Cargo.toml COPYs:
```dockerfile
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/
```

Update Ruby worker source COPY (line 60) — add `crates/` prefix:
```dockerfile
COPY crates/tasker-rb/ ./crates/tasker-rb/
```

Update WORKDIR for bundle install (line 64):
```dockerfile
WORKDIR /app/crates/tasker-rb
```

- [ ] **Step 2: Commit**

```bash
git add docker/build/ruby-worker.prod.Dockerfile
git commit -m "fix(docker): update ruby-worker Dockerfile for crates/ workspace layout"
```

---

### Task 4: Fix python-worker.prod.Dockerfile

**Files:**
- Modify: `docker/build/python-worker.prod.Dockerfile`

- [ ] **Step 1: Update COPY destinations and add missing stubs**

Same pattern as ruby-worker. Full-source crates (lines 51-55) — add `crates/` prefix:
```dockerfile
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-worker/ ./crates/tasker-worker/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-ctl/ ./crates/tasker-ctl/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
```

Update stub creation (line 61) to include 5 new crates:
```dockerfile
/tmp/create-workspace-stubs.sh tasker-orchestration tasker-example-rs tasker-rb tasker-ts \
    tasker-sdk tasker-mcp tasker-grammar tasker-secure tasker-runtime
```

Update existing stub Cargo.toml COPYs (lines 62-65) — add `crates/` prefix:
```dockerfile
COPY crates/tasker-orchestration/Cargo.toml ./crates/tasker-orchestration/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-rb/ext/tasker_core/Cargo.toml ./crates/tasker-rb/ext/tasker_core/
COPY crates/tasker-ts/Cargo.toml ./crates/tasker-ts/
```

Add 5 new stub Cargo.toml COPYs:
```dockerfile
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/
```

Update Python worker source COPY (line 68) — add `crates/` prefix:
```dockerfile
COPY crates/tasker-py/ ./crates/tasker-py/
```

Update WORKDIR for uv/maturin (line 73):
```dockerfile
WORKDIR /app/crates/tasker-py
```

- [ ] **Step 2: Commit**

```bash
git add docker/build/python-worker.prod.Dockerfile
git commit -m "fix(docker): update python-worker Dockerfile for crates/ workspace layout"
```

---

### Task 5: Fix typescript-worker.prod.Dockerfile

**Files:**
- Modify: `docker/build/typescript-worker.prod.Dockerfile`

- [ ] **Step 1: Update COPY destinations and add missing stubs**

Same pattern. Full-source crates (lines 43-47) — add `crates/` prefix:
```dockerfile
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-worker/ ./crates/tasker-worker/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-ctl/ ./crates/tasker-ctl/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
```

Update stub creation (line 53) to include 5 new crates:
```dockerfile
/tmp/create-workspace-stubs.sh tasker-orchestration tasker-example-rs tasker-rb tasker-py \
    tasker-sdk tasker-mcp tasker-grammar tasker-secure tasker-runtime
```

Update existing stub Cargo.toml COPYs (lines 54-57) — add `crates/` prefix:
```dockerfile
COPY crates/tasker-orchestration/Cargo.toml ./crates/tasker-orchestration/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
COPY crates/tasker-rb/ext/tasker_core/Cargo.toml ./crates/tasker-rb/ext/tasker_core/
COPY crates/tasker-py/Cargo.toml ./crates/tasker-py/
```

Add 5 new stub Cargo.toml COPYs:
```dockerfile
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/
```

Update TypeScript worker source COPY (line 60) — add `crates/` prefix:
```dockerfile
COPY crates/tasker-ts/ ./crates/tasker-ts/
```

Update WORKDIR for bun install (line 67):
```dockerfile
WORKDIR /app/crates/tasker-ts
```

- [ ] **Step 2: Commit**

```bash
git add docker/build/typescript-worker.prod.Dockerfile
git commit -m "fix(docker): update typescript-worker Dockerfile for crates/ workspace layout"
```

---

### Task 6: Fix ffi-builder.Dockerfile

**Files:**
- Modify: `docker/build/ffi-builder.Dockerfile`

The ffi-builder has a mix of correct (`./crates/`) and incorrect (`./tasker-xxx/`) paths. Standardize everything to `./crates/`.

- [ ] **Step 1: Update shared crate COPY destinations (lines 102-106)**

```dockerfile
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-worker/ ./crates/tasker-worker/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-ctl/ ./crates/tasker-ctl/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
```

- [ ] **Step 2: Update stub creation and fix path for tasker-example-rs (lines 114-118)**

```dockerfile
COPY docker/scripts/create-workspace-stubs.sh /tmp/
RUN chmod +x /tmp/create-workspace-stubs.sh && \
    /tmp/create-workspace-stubs.sh tasker-orchestration tasker-example-rs \
    tasker-sdk tasker-mcp tasker-grammar tasker-secure tasker-runtime
COPY crates/tasker-orchestration/Cargo.toml ./crates/tasker-orchestration/
COPY crates/tasker-example-rs/Cargo.toml ./crates/tasker-example-rs/
```

Add 5 new stub Cargo.toml COPYs:
```dockerfile
COPY crates/tasker-sdk/Cargo.toml ./crates/tasker-sdk/
COPY crates/tasker-mcp/Cargo.toml ./crates/tasker-mcp/
COPY crates/tasker-grammar/Cargo.toml ./crates/tasker-grammar/
COPY crates/tasker-secure/Cargo.toml ./crates/tasker-secure/
COPY crates/tasker-runtime/Cargo.toml ./crates/tasker-runtime/
```

Lines 121-123 (FFI worker COPYs) already use `./crates/` — leave as-is.

Line 117 (orchestration Cargo.toml) currently uses `./tasker-orchestration/` — fix to `./crates/tasker-orchestration/`.

- [ ] **Step 3: Commit**

```bash
git add docker/build/ffi-builder.Dockerfile
git commit -m "fix(docker): update ffi-builder Dockerfile for crates/ workspace layout"
```

---

## Chunk 2: CLI build workflow fixes and tasker-mcp integration

### Task 7: Add mold linker and tasker-mcp to build-cli-binaries.yml

**Files:**
- Modify: `.github/workflows/build-cli-binaries.yml`

- [ ] **Step 1: Add mold setup step**

After the "Install protobuf compiler" step (line 81), add for Linux runners only:

```yaml
      - name: Setup mold linker
        if: runner.os == 'Linux'
        uses: rui314/setup-mold@v1
```

- [ ] **Step 2: Add binary dimension to the matrix**

Replace the existing matrix with a two-dimensional matrix (binary × target). Update `strategy.matrix`:

```yaml
    strategy:
      fail-fast: false
      matrix:
        binary:
          - name: tasker-ctl
            package: tasker-ctl
          - name: tasker-mcp
            package: tasker-mcp
        target:
          - name: linux-amd64
            runner: ubuntu-22.04
            triple: x86_64-unknown-linux-gnu
            cross: false
          - name: linux-arm64
            runner: ubuntu-22.04
            triple: aarch64-unknown-linux-gnu
            cross: true
            cross_packages: gcc-aarch64-linux-gnu
            linker: aarch64-linux-gnu-gcc
          - name: darwin-amd64
            runner: macos-14
            triple: x86_64-apple-darwin
            cross: false
          - name: darwin-arm64
            runner: macos-14
            triple: aarch64-apple-darwin
            cross: false
```

- [ ] **Step 3: Update job name, build command, and artifact naming**

Job name:
```yaml
    name: ${{ matrix.binary.name }} ${{ matrix.target.name }}
```

Build step:
```yaml
      - name: Build ${{ matrix.binary.name }}
        run: |
          cargo build --release --package ${{ matrix.binary.package }} --target ${{ matrix.target.triple }}
```

Package step — use `matrix.binary.name` instead of hardcoded `tasker-ctl`:
```yaml
      - name: Package binary
        run: |
          VERSION="${{ inputs.version || 'dev' }}"
          TARGET="${{ matrix.target.triple }}"
          BINARY="${{ matrix.binary.name }}"

          BIN_PATH="target/${TARGET}/release/${BINARY}"
          if [[ ! -f "$BIN_PATH" ]]; then
            echo "::error::Binary not found at ${BIN_PATH}"
            exit 1
          fi

          ARCHIVE="${BINARY}-${VERSION}-${TARGET}.tar.gz"
          mkdir -p staging
          cp "$BIN_PATH" staging/
          tar -czf "$ARCHIVE" -C staging "$BINARY"

          echo "Created ${ARCHIVE}"
          ls -lh "$ARCHIVE"
          echo "Binary size: $(ls -lh staging/${BINARY} | awk '{print $5}')"
```

Upload artifact — include binary name in artifact name:
```yaml
      - name: Upload artifact
        uses: actions/upload-artifact@v7
        with:
          name: cli-${{ inputs.version || 'dev' }}-${{ matrix.binary.name }}-${{ matrix.target.triple }}
          path: ${{ matrix.binary.name }}-*.tar.gz
          retention-days: 7
          if-no-files-found: error
```

Update outputs:
```yaml
    outputs:
      artifact-prefix: cli-${{ inputs.version || 'dev' }}
```

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/build-cli-binaries.yml
git commit -m "fix(ci): add mold linker and tasker-mcp to CLI binary build matrix"
```

---

### Task 8: Create mcp.prod.Dockerfile

**Files:**
- Create: `docker/build/mcp.prod.Dockerfile`

The MCP server is a pure Rust binary (like orchestration) but simpler — no database migrations, no SQLx CLI needed. Model after orchestration.prod.Dockerfile but lighter.

- [ ] **Step 1: Write the Dockerfile**

```dockerfile
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
COPY src/ ./src/

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

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY Cargo.toml Cargo.lock ./
COPY .cargo/ ./.cargo/
COPY src/ ./src/

COPY crates/tasker-mcp/ ./crates/tasker-mcp/
COPY crates/tasker-sdk/ ./crates/tasker-sdk/
COPY crates/tasker-shared/ ./crates/tasker-shared/
COPY crates/tasker-client/ ./crates/tasker-client/
COPY crates/tasker-pgmq/ ./crates/tasker-pgmq/
COPY proto/ ./proto/
COPY migrations/ ./migrations/

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

ENV SQLX_OFFLINE=true

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
```

- [ ] **Step 2: Commit**

```bash
git add docker/build/mcp.prod.Dockerfile
git commit -m "feat(docker): add production Dockerfile for tasker-mcp"
```

---

### Task 9: Add tasker-mcp container to release.yml

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Add tasker-mcp to publish-containers matrix**

In the `publish-containers` job matrix (around line 602), add:

```yaml
        image:
          - name: tasker-orchestration
            dockerfile: docker/build/orchestration.prod.Dockerfile
          - name: tasker-worker-ruby
            dockerfile: docker/build/ruby-worker.prod.Dockerfile
          - name: tasker-worker-python
            dockerfile: docker/build/python-worker.prod.Dockerfile
          - name: tasker-worker-typescript
            dockerfile: docker/build/typescript-worker.prod.Dockerfile
          - name: tasker-mcp
            dockerfile: docker/build/mcp.prod.Dockerfile
```

- [ ] **Step 2: Update release-summary artifact download pattern**

The CLI artifact download pattern (around line 677) currently uses `cli-*`. Since artifact names now include the binary name (`cli-{version}-tasker-ctl-{triple}`, `cli-{version}-tasker-mcp-{triple}`), the existing glob pattern `cli-${{ ... }}-*` will still match both. No change needed.

Verify the release attachment step (around line 757) uses `find cli-artifacts/ -name '*.tar.gz'` — this will pick up both tasker-ctl and tasker-mcp archives. No change needed.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "feat(ci): add tasker-mcp container to release publish matrix"
```

---

### Task 10: Delete standalone release-mcp.yml and old Dockerfile

**Files:**
- Delete: `.github/workflows/release-mcp.yml`
- Delete: `crates/tasker-mcp/Dockerfile`

- [ ] **Step 1: Remove the files**

```bash
git rm .github/workflows/release-mcp.yml
git rm crates/tasker-mcp/Dockerfile
```

- [ ] **Step 2: Commit**

```bash
git commit -m "chore(ci): remove standalone release-mcp workflow (consolidated into main release)"
```

---

## Chunk 3: Validation

### Task 11: Local validation

- [ ] **Step 1: Verify workspace still compiles**

```bash
SQLX_OFFLINE=true cargo check --all-features
```

Expected: clean compilation.

- [ ] **Step 2: Validate orchestration Dockerfile builds (quick smoke test)**

```bash
docker build -f docker/build/orchestration.prod.Dockerfile -t tasker-orchestration:test . 2>&1 | tail -20
```

Expected: `cargo chef prepare` succeeds (proves workspace resolution works with new paths).

**Note:** A full build will take several minutes. You can cancel after `cargo chef prepare` succeeds — that's the critical validation point where workspace member resolution happens.

- [ ] **Step 3: Validate ffi-builder Dockerfile builds (quick smoke test)**

```bash
docker build -f docker/build/ffi-builder.Dockerfile -t tasker-ffi-builder:test . 2>&1 | tail -20
```

Expected: workspace resolution succeeds.

- [ ] **Step 4: Validate mcp Dockerfile builds (quick smoke test)**

```bash
docker build -f docker/build/mcp.prod.Dockerfile -t tasker-mcp:test . 2>&1 | tail -20
```

Expected: workspace resolution succeeds.
