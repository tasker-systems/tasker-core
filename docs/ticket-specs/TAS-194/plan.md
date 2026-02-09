# TAS-194: Registry Accounts & Credential Setup — Release Pipeline Implementation

## Overview

This document is the authoritative specification for the Tasker release pipeline, superseding the stale portions of TAS-170 Phases 2-4. It covers the per-registry publish scripts and the CI release workflow that bridges local dry-run tooling (TAS-170 Phase 1) to automated publishing.

## Registry Configuration

### Credential Strategy

| Registry | Auth Method | Credential Source | Notes |
|----------|-------------|-------------------|-------|
| crates.io | API token | `CARGO_REGISTRY_TOKEN` secret | Standard cargo publish flow |
| RubyGems | API token | `GEM_HOST_API_KEY` secret | Standard gem push flow |
| PyPI | OIDC trusted publisher | GitHub Actions `environment: pypi` + `id-token: write` | No token management needed |
| npm | OIDC trusted publisher | GitHub Actions `environment: npm` + `id-token: write` | Provenance attestation via `--provenance` |

### Package Name Mapping (post TAS-233)

| Registry | Package Name | Source |
|----------|-------------|--------|
| crates.io | `tasker-pgmq`, `tasker-shared`, `tasker-client`, `tasker-orchestration`, `tasker-worker`, `tasker-cli` | Rust workspace crates |
| RubyGems | `tasker-rb` | `workers/ruby/tasker-rb.gemspec` |
| PyPI | `tasker-py` | `workers/python/pyproject.toml` |
| npm | `@tasker-systems/tasker` | `workers/typescript/package.json` |

## Publish Scripts

All scripts follow a unified interface:

```
./scripts/release/publish-<target>.sh VERSION [--dry-run] [--on-duplicate=skip|warn|fail]
```

All scripts source `scripts/release/lib/common.sh` for shared helpers.

### publish-crates.sh

Publishes six Rust crates to crates.io in dependency order:

| Phase | Crates |
|-------|--------|
| 1 | `tasker-pgmq` |
| 2 | `tasker-shared` |
| 3 | `tasker-client`, `tasker-orchestration` |
| 4 | `tasker-worker`, `tasker-cli` |

- 30-second sleep between phases for crates.io index propagation
- `SQLX_OFFLINE=true` required (no DB in release runner)
- Dry-run uses `cargo publish -p <crate> --dry-run --allow-dirty`

### publish-ruby.sh

Builds native gem and publishes to RubyGems:

1. `bundle exec rake compile` (native extension)
2. `gem build tasker-rb.gemspec`
3. `gem push tasker-rb-VERSION.gem`

Gem ships source; users compile native extension at install time.

### publish-python.sh

Builds wheel via maturin and publishes to PyPI:

1. `uv run maturin build --release`
2. `uv run maturin publish`

No credential check — PyPI uses OIDC. Initial release is Linux x86_64 only.

### publish-typescript.sh

Builds TS package and publishes to npm:

1. `bun run build` (tsup)
2. `npm publish --provenance --access public --tag alpha`

Ships TS SDK layer only — no Rust cdylib bundled. No Rust toolchain needed.

## Workflow Architecture

### `.github/workflows/release.yml`

**Triggers:**
- Tag push: `release-*`, `v*`
- `workflow_dispatch` with `dry_run` (boolean) and `from_ref` (string, optional)

**Forced dry-run:** Top-level env var `RELEASE_DRY_RUN: 'true'` overrides all jobs. Change to `'false'` when ready to go live.

**Concurrency:** `group: release`, `cancel-in-progress: false`

### Job DAG

```
detect-and-calculate  (5 min)
        |
  pre-flight-check    (15 min)
        |
  publish-crates       (20 min)
        |
  +----------+----------+
  |          |          |
publish-  publish-   publish-
ruby      python     typescript
(20 min)  (20 min)   (15 min)
  |          |          |
  +----------+----------+
        |
  release-summary
```

### Per-Job Configuration

| Job | Permissions | Environment | Toolchain |
|-----|-------------|-------------|-----------|
| detect-and-calculate | contents: read | — | git only |
| pre-flight-check | contents: read | — | Rust, protobuf |
| publish-crates | contents: write | — | Rust, protobuf |
| publish-ruby | contents: write | — | Rust, Ruby 3.4, protobuf |
| publish-python | contents: write, id-token: write | `pypi` | Rust, Python 3.12, uv, maturin, protobuf |
| publish-typescript | contents: write, id-token: write | `npm` | Bun, Node 22 |
| release-summary | contents: read | — | — |

FFI publish jobs use `always() && !cancelled() && !failure()` so they run even if `publish-crates` was skipped (binding-only change).

All Rust compilation jobs set `SQLX_OFFLINE: 'true'`.

## Relationship to TAS-170

TAS-170 Phase 1 (local dry-run tooling) is complete and in production use:
- `scripts/release/detect-changes.sh`
- `scripts/release/calculate-versions.sh`
- `scripts/release/update-versions.sh`
- `scripts/release/release.sh`
- `scripts/release/lib/common.sh`

TAS-170 Phases 2-4 are **superseded** by this implementation with:
- Updated package names (post TAS-233 rename)
- OIDC authentication for PyPI and npm (eliminating token management)
- Forced dry-run safety mechanism
- Full workflow with job DAG

## Open Items (Future Work)

1. **Multi-platform Python wheels** — matrix build for Linux x86_64, macOS arm64, macOS x86_64
2. **TypeScript native binary distribution** — platform-specific npm packages for `libtasker_ts`
3. **RELEASING.md** — human-readable release runbook
4. **Go-live** — flip `RELEASE_DRY_RUN` to `'false'` after validation
5. **GitHub Release creation** — automatic release notes in `release-summary` job
