# TAS-373: Scaffold tasker-runtime Crate

*Design document — 2026-03-09*

## Purpose

New workspace member bridging tasker-grammar operation traits to tasker-secure resource handles. This scaffold provides the type structure and trait signatures for all Phase 2 lanes (2A-2D) without implementing behavior.

## Crate Topology

```
tasker-secure ←── tasker-runtime ──→ tasker-grammar
```

Neither tasker-secure nor tasker-grammar depends on tasker-runtime. Dependency is strictly one-way inward.

## Module Structure

```
crates/tasker-runtime/
├── Cargo.toml
├── Makefile.toml
└── src/
    ├── lib.rs                    # Public exports, module declarations
    ├── provider.rs               # RuntimeOperationProvider (2D) — implements OperationProvider
    ├── adapters/
    │   ├── mod.rs                # AdapterRegistry type + trait
    │   ├── postgres.rs           # PostgresPersistAdapter, PostgresAcquireAdapter  [cfg(postgres)]
    │   └── http.rs               # HttpPersistAdapter, HttpAcquireAdapter, HttpEmitAdapter  [cfg(http)]
    ├── pool_manager/
    │   ├── mod.rs                # ResourcePoolManager struct + public API signatures
    │   ├── lifecycle.rs          # ResourceOrigin, EvictionStrategy, AdmissionStrategy
    │   └── metrics.rs            # ResourceAccessMetrics for eviction decisions
    ├── sources/
    │   ├── mod.rs                # ResourceDefinitionSource trait
    │   ├── static_config.rs      # StaticConfigSource  [always available]
    │   └── sops.rs               # SopsFileWatcher  [cfg(sops)]
    └── context/
        └── mod.rs                # CompositionExecutionContext placeholder (Phase 3B)
```

## Feature Flags

```toml
[features]
default = []
postgres = ["tasker-secure/postgres"]
http = ["tasker-secure/http"]
sops = ["tasker-secure/sops"]
```

Feature gates mirror tasker-secure. Adapter modules are `#[cfg(feature = "...")]` gated at the module level.

## Dependencies (scaffold only)

- `tasker-grammar` — operation traits, types, errors
- `tasker-secure` — handle types, registry, resource definitions
- `async-trait` — async trait definitions
- `serde`, `serde_json` — `Value` in signatures

No `sqlx`, `reqwest`, or other driver crates at scaffold time.

## Key Types & Traits

All method bodies are `unimplemented!()` — real implementations come in 2A-2D.

### `provider.rs` (Lane 2D)

`RuntimeOperationProvider` implementing `OperationProvider` from tasker-grammar. Holds references to `ResourcePoolManager` and `AdapterRegistry`.

### `adapters/` (Lane 2A)

`AdapterRegistry` maps resource references to adapter instances. Feature-gated adapter modules expose structs implementing `PersistableResource`, `AcquirableResource`, `EmittableResource`.

### `pool_manager/` (Lane 2B)

`ResourcePoolManager` wrapping `ResourceRegistry` with lifecycle config. Supporting types: `PoolManagerConfig`, `ResourceOrigin` (Static vs Dynamic), `EvictionStrategy`, `AdmissionStrategy`, `ResourceAccessMetrics`.

### `sources/` (Lane 2C)

`ResourceDefinitionSource` async trait for runtime resource resolution. `StaticConfigSource` for worker.toml. `SopsFileWatcher` behind `sops` feature.

### `context/` (Phase 3B)

Placeholder module for `CompositionExecutionContext`.

## Workspace Integration

- Register `crates/tasker-runtime` in root `Cargo.toml` workspace members
- Add `tasker-runtime` to `[workspace.dependencies]`
- `Makefile.toml` extending `../../tools/cargo-make/base-tasks.toml`
- CI scope detection updated if needed

## Exclusions

- No real implementations — all `unimplemented!()`
- No `test-utils` feature flag
- No `sqlx`/`reqwest` direct dependencies
- No integration with tasker-worker (Phase 3-4)
- No tests beyond compilation verification
