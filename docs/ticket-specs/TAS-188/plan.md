# TAS-188: Decouple tasker-client into tasker-client (library) + tasker-cli (binary)

**Status:** Complete
**Priority:** P1
**Related:** TAS-175 (docs-gen), TAS-177 (gRPC transport/profiles), TAS-150 (auth)

## Summary

Split the `tasker-client` crate into two crates with clear separation of concerns:

- **`tasker-client`** — Pure API client library (REST + gRPC transports, config, error types)
- **`tasker-cli`** — CLI tool binary that depends on `tasker-client`

This decoupling enables FFI-based client bindings (`clients/{python,ruby,typescript}`), supports
the `tasker-contrib` plugin architecture, and keeps the client library lightweight for future
UI frontends (TUI or web).

## Background & Motivation

The `tasker-client` crate currently serves two roles in a single package:

1. **API client library** — Transport-agnostic REST/gRPC client with unified traits
2. **CLI tool** — `tasker-cli` binary with clap-based commands for task management, config
   generation, auth key management, DLQ operations, and documentation generation

This coupling creates several problems:

- **FFI bindings bloat**: Future `clients/{python,ruby,typescript}` FFI crates (mirroring the
  `workers/` pattern) would unnecessarily pull in `clap`, `rsa`, `askama`, and all CLI code
- **Plugin architecture dependency**: `tasker-contrib` plugins need the API client, not the CLI
  argument parser. The CLI itself should be a consumer of the client library
- **Frontend flexibility**: Whether the future UI is a TUI (ratatui) or web (Svelte/React/Vue),
  it needs the client library without CLI overhead
- **Binary size**: CLI dependencies (`clap`, `askama`, `rsa`) inflate the library for all consumers

## Architecture

### Before

```
tasker-client (lib + bin + docs-gen + clap + rsa + askama)
     ↑
tasker-worker (uses OrchestrationApiClient only)
```

### After

```
tasker-shared          (foundation types, proto generation)
     ↑
tasker-client          (API client library: REST + gRPC, config, errors)
     ↑           ↑
tasker-cli    tasker-worker
     ↑
(future: tasker-contrib plugins)

Future FFI bindings (mirrors workers/ pattern):
tasker-client → clients/{python,ruby,typescript}
```

### Workspace Member Change

```toml
# Cargo.toml [workspace.members] — add tasker-cli
members = [
  ".",
  "tasker-pgmq",
  "tasker-client",     # library only (no [[bin]])
  "tasker-cli",        # new: CLI binary crate
  "tasker-orchestration",
  "tasker-shared",
  "tasker-worker",
  "workers/python",
  "workers/ruby/ext/tasker_core",
  "workers/rust",
  "workers/typescript",
]
```

## What Stays in tasker-client (Library)

| Module | Description |
|--------|-------------|
| `src/api_clients/` | REST client implementations (reqwest-based) |
| `src/grpc_clients/` | gRPC client implementations (tonic-based, feature-gated) |
| `src/transport.rs` | `OrchestrationClient`/`WorkerClient` traits, unified clients |
| `src/error.rs` | `ClientError`, `ClientResult` |
| `src/config.rs` | `Transport`, `ApiEndpointConfig`, `ClientAuthConfig`, `ClientConfig`, profile loading |

**Dependencies retained**: `reqwest`, `tonic` (optional), `serde`, `serde_json`, `tokio`,
`tasker-shared`, `thiserror`, `config`, `dirs`, `toml`, `tracing`, `uuid`, `chrono`,
`async-trait`, `futures`, `once_cell`, `validator`, `url`

**Dependencies removed**: `clap`, `rsa`, `rand`, `askama`, `tracing-subscriber`, `serde_yaml`

**Features after split**:

```toml
[features]
default = ["grpc"]
grpc = ["tonic", "prost-types", "tasker-shared/grpc-api"]
```

## What Moves to tasker-cli (New Crate)

| Content | Source → Destination |
|---------|---------------------|
| CLI entry point | `src/bin/tasker-cli.rs` → `tasker-cli/src/main.rs` |
| Command handlers | `src/bin/cli/commands/*.rs` → `tasker-cli/src/commands/*.rs` |
| CLI module root | `src/bin/cli/mod.rs` → `tasker-cli/src/commands/mod.rs` |
| Docs generation | `src/docs/` → `tasker-cli/src/docs/` |
| Templates | `templates/` → `tasker-cli/templates/` |
| Askama config | `askama.toml` → `tasker-cli/askama.toml` |
| CLI test | `tests/config_commands_test.rs` → `tasker-cli/tests/` |
| Benchmarks | `benches/` → removed (benchmarking handled at workspace level) |

## Config Boundary Split

### Current: `ClientConfig` contains CLI-specific fields

```rust
pub struct ClientConfig {
    pub transport: Transport,              // stays
    pub orchestration: ApiEndpointConfig,  // stays
    pub worker: ApiEndpointConfig,         // stays
    pub cli: CliConfig,                    // moves to tasker-cli
}
```

### After: Clean separation

**In `tasker-client`** — library config only:

```rust
pub struct ClientConfig {
    pub transport: Transport,
    pub orchestration: ApiEndpointConfig,
    pub worker: ApiEndpointConfig,
}
```

**In `tasker-cli`** — composed config:

```rust
pub struct CliAppConfig {
    pub client: ClientConfig,  // from tasker-client
    pub cli: CliConfig,        // CLI-specific (format, colors, verbosity)
}
```

### Profile Handling (Option B)

`ProfileCliConfig` moves entirely to `tasker-cli`. The library's `ProfileConfig` drops
the `cli` field. Both crates read the same `.config/tasker-client.toml` file without
conflict because the library's deserializer uses `#[serde(default)]` and ignores
unknown fields (no `deny_unknown_fields`).

## Implementation Phases

### Phase 1: Create tasker-cli crate structure

1. Create `tasker-cli/` directory with `Cargo.toml`, `src/main.rs`
2. Move CLI code (`src/bin/cli/` → `tasker-cli/src/commands/`)
3. Move docs generation (`src/docs/` → `tasker-cli/src/docs/`)
4. Move templates and askama.toml
5. Move CLI test (`tests/config_commands_test.rs`)
6. Add `tasker-cli` to workspace members
7. Wire up imports to use `tasker_client::` instead of `crate::`

### Phase 2: Config boundary adjustment

1. Remove `cli: CliConfig` from `ClientConfig` in tasker-client
2. Remove `ProfileCliConfig` from `ProfileConfig`
3. Add `#[serde(default)]` for backwards-compatible deserialization
4. Create `CliAppConfig` in tasker-cli that composes both concerns
5. Update profile loading in tasker-cli to handle CLI fields

### Phase 3: Clean up tasker-client

1. Remove `[[bin]]` target from `Cargo.toml`
2. Remove `src/bin/` directory entirely
3. Remove `src/docs/`, `templates/`, `askama.toml`
4. Remove `benches/` directory
5. Remove CLI-only dependencies (`clap`, `rsa`, `rand`, `askama`, `serde_yaml`)
6. Remove `docs-gen` and `benchmarks` features
7. Clean up `lib.rs` module declarations and re-exports

### Phase 4: Update build infrastructure

**Makefile.toml** (3 locations):

- Lines 1951, 1998: `CLI="cargo run --all-features --package tasker-cli --bin tasker-cli --"`
- Lines 1989-1990: `args = ["run", "--all-features", "--package", "tasker-cli", "--bin", "tasker-cli", ...]`

**GitHub Actions** (2 files):

- `.github/actions/generate-test-config/action.yml` lines 22, 29
- `.github/workflows/build-workers.yml` lines 90, 99-102

**Docker** (9 files):

- `docker/scripts/create-workspace-stubs.sh` — add `["tasker-cli"]="tasker-cli"` entry
- 8 Dockerfiles — add `COPY tasker-cli/ ./tasker-cli/` where tasker-client is already copied

**Scripts** (2 files):

- `scripts/code_check.sh` — add `tasker-cli` to `RUST_CORE_PROJECTS` array
- `cargo-make/scripts/sqlx-prepare.sh` — add `tasker-cli` to CRATES if it uses sqlx queries

### Phase 5: Documentation and verification

1. Update `CLAUDE.md` workspace structure section
2. Update documentation with `--package tasker-cli` examples
3. `cargo check --all-features` passes
4. `cargo clippy --all-targets --all-features` passes
5. `cargo fmt` passes
6. `cargo build --all-features` produces `tasker-cli` binary

## Files Changed

### New Files

- `tasker-cli/Cargo.toml`
- `tasker-cli/Makefile.toml`
- `tasker-cli/src/main.rs`
- `tasker-cli/src/commands/mod.rs`
- `tasker-cli/src/commands/{task,worker,system,config,auth,dlq,docs}.rs`
- `tasker-cli/src/docs/mod.rs`
- `tasker-cli/src/docs/templates.rs`
- `tasker-cli/templates/*.md`, `templates/*.toml`, `templates/*.txt`
- `tasker-cli/askama.toml`
- `tasker-cli/tests/config_commands_test.rs`

### Modified Files

- `Cargo.toml` (workspace members)
- `tasker-client/Cargo.toml` (remove bin, deps, features)
- `tasker-client/src/lib.rs` (remove docs module)
- `tasker-client/src/config.rs` (remove CliConfig)
- `Makefile.toml` (3 CLI references)
- `.github/actions/generate-test-config/action.yml` (2 references)
- `.github/workflows/build-workers.yml` (2 references)
- `docker/scripts/create-workspace-stubs.sh` (1 entry)
- `scripts/code_check.sh` (1 array entry)
- `cargo-make/scripts/sqlx-prepare.sh` (conditional)
- 8 Dockerfiles (add COPY line)

### Deleted Files

- `tasker-client/src/bin/tasker-cli.rs`
- `tasker-client/src/bin/cli/mod.rs`
- `tasker-client/src/bin/cli/commands/*.rs`
- `tasker-client/src/docs/mod.rs`
- `tasker-client/src/docs/templates.rs`
- `tasker-client/templates/*`
- `tasker-client/askama.toml`
- `tasker-client/benches/task_initialization.rs`
- `tasker-client/tests/config_commands_test.rs`

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|-----------|
| Config file format breakage | Users with existing `.config/tasker-client.toml` get parse errors | `#[serde(default)]` and no `deny_unknown_fields` — library ignores CLI keys |
| CI/CD breakage | Build/test pipelines fail | Phase 4 explicitly updates all references atomically |
| Dockerfile cache invalidation | Longer rebuild times temporarily | One-time cost; layer structure improves long-term |
| tasker-worker compile breakage | Worker can't find API client types | Zero risk — unchanged types stay in tasker-client |
| Feature flag interaction | `grpc` feature needed for both crates | tasker-cli enables `tasker-client/grpc` transitively |

## Success Criteria

- [x] `cargo check --all-features` passes with both crates
- [x] `cargo clippy --all-targets --all-features` clean (pre-existing warnings only)
- [x] `tasker-cli` binary builds identically to before
- [x] `tasker-worker` builds without changes to its own code
- [x] `tasker-client` has no CLI dependencies (clap, rsa, askama)
- [x] Profile config files work for both library and CLI consumers (`#[serde(default)]`)

## Future Work (Out of Scope)

- `clients/{python,ruby,typescript}` FFI bindings for tasker-client
- `tasker-contrib` plugin architecture using tasker-cli as foundation
- TUI or web frontend using tasker-client directly
- Renaming `.config/tasker-client.toml` (profile file location unchanged)

---

## Metadata

- Identifier: TAS-188
- Status: Complete
- Priority: P1
