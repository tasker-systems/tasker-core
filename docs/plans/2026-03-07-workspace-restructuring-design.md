# TAS-361: Workspace Restructuring Design

**Date**: 2026-03-07
**Status**: Approved
**Ticket**: TAS-361

## Problem

The tasker-core workspace has grown from 2 crates to 12+ crates with extensive tooling, but maintains a flat directory structure that mixes crates with infrastructure. This doesn't match the idiomatic Rust workspace convention used by mature projects (Bevy, Ratatui, Nushell).

## Target Structure

```
tasker-core/
├── Cargo.toml              # Workspace root + tasker-core meta-package
├── Cargo.lock
├── src/lib.rs              # tasker-core library (stays at root)
├── crates/
│   ├── tasker-shared/
│   ├── tasker-orchestration/
│   ├── tasker-worker/
│   ├── tasker-client/
│   ├── tasker-ctl/
│   ├── tasker-sdk/
│   ├── tasker-mcp/
│   ├── tasker-pgmq/
│   ├── tasker-grammar/
│   └── workers/
│       ├── ruby/           # (renamed in TAS-362)
│       ├── python/         # (renamed in TAS-362)
│       ├── rust/           # (renamed in TAS-362)
│       └── typescript/     # (renamed in TAS-362)
├── tools/
│   ├── cargo-make/         # Task runner configs + scripts/
│   ├── bin/                # Setup scripts (setup-dev, setup-claude-web)
│   └── scripts/            # Build, release, FFI, codegen scripts
├── proto/                  # Stays: shared system-level resource
├── migrations/             # Stays: shared system-level resource
├── tests/                  # Stays: workspace-level integration/e2e tests
├── config/                 # Stays: runtime configuration
├── docker/                 # Stays: deployment infrastructure
├── docs/                   # Stays
├── .github/                # Stays
├── schemas/                # Stays
├── vendor/                 # Stays: patch dependencies
└── Makefile.toml           # Stays: extends tools/cargo-make/main.toml
```

## Key Decisions

### D1: Root crate stays at root

The `tasker-core` meta-package (workspace root with `src/lib.rs`) stays at root as the `"."` workspace member. It's the workspace anchor, never published, and splitting the `[workspace]` section from the package would add complexity for no benefit.

### D2: All build tooling consolidates into `tools/`

`cargo-make/`, `bin/`, and `scripts/` all move into `tools/`. Since we're already updating every `Makefile.toml` extend path for the crate moves, the incremental cost is minimal. Most scripts use `SCRIPTS_DIR` indirection, limiting the actual update surface.

### D3: `docker/`, `proto/`, `migrations/`, `tests/`, `config/` stay at root

These are workspace-level shared resources and deployment infrastructure, not build tooling. They belong at root alongside `Cargo.toml`.

### D4: Worker directory structure preserved (TAS-362 scope)

Workers move to `crates/workers/{lang}` maintaining their current directory names. Crate renaming (e.g., `workers/ruby` to `tasker-rb`) is TAS-362, a separate ticket to keep blast radius focused.

## Path Update Analysis

### Cargo.toml workspace members

All member paths gain a `crates/` prefix:

```toml
members = [
  ".",
  "crates/tasker-pgmq",
  "crates/tasker-client",
  "crates/tasker-ctl",
  "crates/tasker-sdk",
  "crates/tasker-mcp",
  "crates/tasker-orchestration",
  "crates/tasker-shared",
  "crates/tasker-worker",
  "crates/tasker-grammar",
  "crates/workers/python",
  "crates/workers/ruby/ext/tasker_core",
  "crates/workers/rust",
  "crates/workers/typescript",
]
```

### Inter-crate path dependencies

| From | To | Current | After |
|------|----|---------|-------|
| Root dev-deps | any crate | `path = "tasker-foo"` | `path = "crates/tasker-foo"` |
| Root workspace deps | tasker-pgmq, tasker-sdk | `path = "tasker-foo"` | `path = "crates/tasker-foo"` |
| Crate → sibling crate | e.g., orchestration → shared | `path = "../tasker-shared"` | `path = "../tasker-shared"` (unchanged) |
| Crate → root package | e.g., orchestration → tasker-core | `path = "../"` | `path = "../../"` |
| Worker → crate | e.g., python → shared | `path = "../../tasker-shared"` | `path = "../../tasker-shared"` (unchanged) |
| Ruby ext → crate | ext/tasker_core → shared | `path = "../../../../tasker-shared"` | `path = "../../../../tasker-shared"` (unchanged) |
| Ruby ext → root | ext/tasker_core → tasker-core | `path = "../../../../"` | `path = "../../../../../"` |
| Worker rust → root | → tasker-core | `path = "../../"` | `path = "../../../"` |

The key insight: sibling crate references (`../tasker-foo`) are unchanged because all crates move together. Only references to the workspace root (`../` or `../../`) need updating since crates are now one level deeper.

### tasker-shared symlinks

```
# Current (tasker-shared at root level)
proto -> ../proto
migrations -> ../migrations

# After (tasker-shared at crates/ level)
proto -> ../../proto
migrations -> ../../migrations
```

### build.rs proto detection

`tasker-shared/build.rs` uses `CARGO_MANIFEST_DIR.parent()` to find workspace root. After the move, workspace root is two levels up. The existing fallback logic handles this — `parent()` gives `crates/`, need `parent().parent()` or adjust the workspace proto path detection.

### Makefile.toml extend paths

| Location | Current | After |
|----------|---------|-------|
| Root | `extend = "./cargo-make/main.toml"` | `extend = "./tools/cargo-make/main.toml"` |
| Top-level crates | `extend = "../cargo-make/base-tasks.toml"` | `extend = "../../tools/cargo-make/base-tasks.toml"` |
| Workers | `extend = "../../cargo-make/base-tasks.toml"` | `extend = "../../../tools/cargo-make/base-tasks.toml"` |

### SCRIPTS_DIR

```toml
# Current
SCRIPTS_DIR = "cargo-make/scripts"

# After
SCRIPTS_DIR = "tools/cargo-make/scripts"
```

### CI workflows

GitHub Actions workflow files in `.github/workflows/` reference:
- Crate paths for caching and build contexts
- `bin/` script paths for setup
- `scripts/` paths for CI helpers
- `cargo-make/scripts/` paths

All need updating to reflect `crates/` and `tools/` prefixes.

### Documentation

All path references in these files need updating:
- `CLAUDE.md` (root + parent)
- `AGENTS.md` (root + crate-level)
- `.claude/skills/*.md` (17 skill files)
- `docs/` guides and architecture docs that reference crate paths

### .sqlx cache

Prepared query cache keys embed crate paths. Full `cargo sqlx prepare --workspace` required after the move to regenerate `.sqlx/` contents.

## Execution Constraints

- **Atomic commit**: Incremental moves create untestable broken states. All moves, path updates, and fixes must land in a single commit.
- **Verification sequence**: `cargo check --all-features` → `cargo make test-no-infra` → `cargo sqlx prepare` → `cargo publish --dry-run -p tasker-shared`
- **Cross-repo impact**: `tasker-contrib` and `tasker-book` may reference tasker-core paths — verify after completion.

## Out of Scope

- Crate renaming (TAS-362)
- Any functional code changes
- Dependency version updates
