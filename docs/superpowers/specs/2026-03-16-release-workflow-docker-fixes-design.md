# Release Workflow & Docker Build Fixes

**Date**: 2026-03-16
**Scope**: CI/CD infrastructure — Docker build contexts, CLI build workflow, tasker-mcp integration
**Trigger**: v0.1.7 dry-run release failed across 6 jobs due to workspace restructuring (TAS-361/362)

## Problem

The TAS-361/362 restructuring moved all crates under `crates/` but Docker build contexts and CI workflows were not updated:

1. **Docker COPY destinations** still use flat layout (`./tasker-xxx/`) instead of `./crates/tasker-xxx/`
2. **5 new workspace crates** (tasker-sdk, tasker-mcp, tasker-grammar, tasker-secure, tasker-runtime) have no representation in any Dockerfile
3. **`build-cli-binaries.yml`** missing mold linker setup (`.cargo/config.toml` requires it on Linux)
4. **tasker-mcp** has a standalone `release-mcp.yml` instead of being integrated into the main release flow

## Fix 1: Docker Build Contexts

### Path correction (all 5 prod Dockerfiles + ffi-builder)

Every `COPY crates/X/ ./X/` destination must become `./crates/X/` to match the workspace member paths in `Cargo.toml`:

```
members = ["crates/tasker-pgmq", "crates/tasker-shared", ...]
```

### Add 5 missing crates as stubs

For each Dockerfile, add the 5 new crates to the `create-workspace-stubs.sh` call and copy their `Cargo.toml`:

- `tasker-sdk` → stub + Cargo.toml
- `tasker-mcp` → stub + Cargo.toml (or full COPY for MCP container image)
- `tasker-grammar` → stub + Cargo.toml
- `tasker-secure` → stub + Cargo.toml
- `tasker-runtime` → stub + Cargo.toml

### Update `create-workspace-stubs.sh`

Add 5 new entries to `CRATE_PATHS` map with `crates/` prefix. Update all existing entries to use `crates/` prefix.

### Affected Dockerfiles

| Dockerfile | Full-source crates | Stub crates (Cargo.toml only) |
|---|---|---|
| `orchestration.prod.Dockerfile` | orchestration, shared, client, ctl, pgmq | worker, example-rs, rb, py, ts, **sdk, mcp, grammar, secure, runtime** |
| `ruby-worker.prod.Dockerfile` | shared, worker, client, ctl, pgmq, rb | orchestration, example-rs, py, ts, **sdk, mcp, grammar, secure, runtime** |
| `python-worker.prod.Dockerfile` | shared, worker, client, ctl, pgmq, py | orchestration, example-rs, rb, ts, **sdk, mcp, grammar, secure, runtime** |
| `typescript-worker.prod.Dockerfile` | shared, worker, client, ctl, pgmq, ts | orchestration, example-rs, rb, py, **sdk, mcp, grammar, secure, runtime** |
| `ffi-builder.Dockerfile` | shared, worker, client, ctl, pgmq, py, rb, ts | orchestration, example-rs, **sdk, mcp, grammar, secure, runtime** |

## Fix 2: Mold Linker in CLI Build Workflow

Add `rui314/setup-mold@v1` step to `build-cli-binaries.yml` for Linux runners. The main `release.yml` already does this in pre-flight and publish-crates jobs.

## Fix 3: Integrate tasker-mcp into Main Release Flow

### Build: extend `build-cli-binaries.yml` matrix

Add a `binary` dimension to build both `tasker-ctl` and `tasker-mcp`:

```yaml
matrix:
  binary:
    - name: tasker-ctl
      package: tasker-ctl
    - name: tasker-mcp
      package: tasker-mcp
  target:
    - name: linux-amd64
      ...
```

This produces 8 jobs (4 targets × 2 binaries), all running in parallel. Artifact naming: `cli-{version}-{triple}` → `{binary.name}-{version}-{triple}`.

### Container: add MCP to `publish-containers` matrix

Create `docker/build/mcp.prod.Dockerfile` following the orchestration pattern (cargo-chef, multi-stage). Add to the `publish-containers` matrix in `release.yml`.

### Release summary

Update `release-summary` to download and attach MCP binary artifacts alongside CLI artifacts.

### Gating

tasker-mcp uses the same `core_changed == 'true'` gate as tasker-ctl — it depends on core crates.

### Cleanup

Delete `release-mcp.yml` and `crates/tasker-mcp/Dockerfile` (replaced by `docker/build/mcp.prod.Dockerfile`).

## Out of Scope

- Test Dockerfiles (`*.test.Dockerfile`) — not used in release, can be updated separately
- `rust-worker.prod.Dockerfile` — not in the release matrix, update separately
- Docker image multi-arch (arm64) — current release only builds amd64 containers
