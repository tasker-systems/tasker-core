# Skill: Versioning and Releases

## When to Use

Use this skill when working with version management, release tooling, understanding publish order for crates and FFI bindings, or running the dry-run release process.

## Current State

- All Rust crates at **v0.1.0** (early release)
- `VERSION` file at repo root is the source of truth for core version
- Regular release cadence during 0.1.x series

## Versioning Strategy

### Core Rust Crates (0.1.N)

All six publishable Rust crates version together during the 0.1.x series:

| Phase | Crates | Reason |
|-------|--------|--------|
| 1 | `tasker-pgmq` | No internal deps |
| 2 | `tasker-shared` | Depends on pgmq |
| 3 | `tasker-client`, `tasker-orchestration` | Depend on shared (can publish in parallel) |
| 4 | `tasker-worker`, `tasker-ctl` | Depend on client/shared (can publish in parallel) |

### FFI Language Bindings (0.1.N)

FFI packages use the same 3-part semver as the core VERSION file. When core or FFI-facing code changes, bindings track the core version. When only a binding changes, it uses the current core version.

Examples:
- Core at 0.1.8, any binding change: `0.1.8`
- Core bumps to 0.1.9: all bindings get `0.1.9`

### Version Files Updated During Release

| File | Field |
|------|-------|
| `VERSION` (root) | Central source of truth |
| `Cargo.toml` (root + all 6 crates) | `version` field |
| `workers/ruby/lib/tasker_core/version.rb` | `VERSION`, `RUST_CORE_VERSION` |
| `workers/ruby/ext/tasker_core/Cargo.toml` | `version` |
| `workers/python/pyproject.toml` | `version` |
| `workers/python/Cargo.toml` | `version` |
| `workers/typescript/package.json` | `version` |
| `workers/typescript/Cargo.toml` | `version` |

## Release Tooling

### Scripts Location

```
scripts/release/
├── release.sh              # Single-command orchestrator
├── detect-changes.sh       # Identifies what changed since last release
├── calculate-versions.sh   # Determines next version numbers
├── update-versions.sh      # Updates all version files
└── lib/
    └── common.sh           # Shared functions (logging, version arithmetic, registry checks)
```

### Running a Dry Run

```bash
# Show what would happen without modifying files
./scripts/release/release.sh --dry-run

# Override base reference
./scripts/release/release.sh --dry-run --from v0.1.0
```

### Running a Release

```bash
# Apply version changes and create tag
./scripts/release/release.sh

# Push tag to trigger CI
git push origin <tag>
```

### Change Detection Logic

```
FFI-facing core changed (tasker-pgmq, tasker-shared, tasker-worker):
  -> Publish ALL core crates + ALL FFI bindings (reset patch to .0)

Server/client core changed (tasker-orchestration, tasker-client, tasker-ctl):
  -> Publish core crates only (no FFI rebuild needed)

Individual binding changed (workers/ruby, workers/python, workers/typescript):
  -> Publish that binding only (increment .P)
```

### Git Tagging Convention

| Tag Format | Trigger |
|-----------|---------|
| `release-YYYYMMDD-HHMM` | Human-initiated release |
| `core-vX.Y.Z` | Created by CI after successful crates.io publish |
| `ruby-vX.Y.Z` | Created by CI after successful gem publish |
| `python-vX.Y.Z` | Created by CI after successful PyPI publish |
| `typescript-vX.Y.Z` | Created by CI after successful npm publish |

## Publishing

### Package Registries

| Package | Registry | Build Tool |
|---------|----------|------------|
| Rust crates (6) | crates.io | `cargo publish` |
| `tasker-rb` | RubyGems | `rake compile` + `gem push` |
| `tasker-py` | PyPI | `maturin build` + `maturin publish` |
| `@tasker-systems/tasker` | npm | `cargo build` + `bun run build` + `npm publish` |

### Not Published

- `tasker-worker-rust` (workers/rust) -- example crate
- Root `tasker-core` crate -- workspace root

### Idempotent Publishing

Each publish script checks if the version already exists on the registry before publishing. Re-running after a partial failure skips successful packages and continues. Controlled by `--on-duplicate` flag: `skip`, `warn` (default), `fail`.

### Required Credentials (CI only)

| Registry | Auth Method |
|----------|-------------|
| crates.io | `CARGO_REGISTRY_TOKEN` secret |
| RubyGems | OIDC trusted publishing (environment: `rbgem`) |
| PyPI | OIDC trusted publishing (environment: `pypi`) |
| npm | OIDC trusted publishing (environment: `npm`) |

## References

- Release plan: `docs/ticket-specs/TAS-170/plan.md`
- Release scripts: `scripts/release/`
