# TAS-376: ResourceDefinitionSource Implementation Design

**Date:** 2026-03-14
**Ticket:** TAS-376
**Lane:** 2C (Phase 2: Runtime Infrastructure)
**Branch:** `jcoletaylor/tas-376-implement-resourcedefinitionsource-for-runtime-resource-resolution`

---

## Overview

Implement `ResourceDefinitionSource` trait additions, `StaticConfigSource`, and a bridging `DefinitionBasedResolver` that connects resource definitions to live handle initialization. Fix `ResourceType::Pgmq` → `Messaging` spec drift. Defer `SopsFileWatcher` to a follow-up ticket.

## Scope

**In scope:**
- `StaticConfigSource` — HashMap-based lookup from `Vec<ResourceDefinition>`, loaded once at startup
- `watch()` method on `ResourceDefinitionSource` trait with default `None` impl
- `ResourceDefinitionWatcher` / `ResourceDefinitionNotifier` newtype pair wrapping bounded mpsc channel
- `DefinitionBasedResolver` — bridges `ResourceDefinitionSource` + `SecretsProvider` → `ResourceHandleResolver`
- `ResourceType::Pgmq` → `ResourceType::Messaging` rename with `#[serde(alias = "pgmq")]` backward compat
- Tests at the `test-no-infra` tier

**Out of scope (deferred):**
- `SopsFileWatcher` — stays as stub with `unimplemented!()`
- `MessagingHandle` — future work for emit operations through the resource handle pattern
- TOML config file parsing — Phase 4 (tasker-rs binary) concern
- `watch()` implementations — credential rotation mechanism, future work

---

## Design

### 1. ResourceType Enum Fix

**File:** `crates/tasker-secure/src/resource/types.rs`

Rename `Pgmq` variant to `Messaging`. Messaging is provider-agnostic (PGMQ, RabbitMQ) and already fully abstracted through `MessagingProvider` in tasker-shared. The `Pgmq` name was spec drift.

```rust
pub enum ResourceType {
    Postgres,
    Http,
    Messaging,
    Custom { type_name: String },
}
```

- `Display` impl: `Messaging` renders as `"messaging"` (lowercase, matching existing convention for `"postgres"`, `"http"`)
- `Deserialize`: add `#[serde(alias = "pgmq")]` for backward compatibility with existing TOML configs
- All references to `ResourceType::Pgmq` across the workspace are updated

**Future note:** A `MessagingHandle` implementing `ResourceHandle` will be needed when emit operations use the resource handle pattern instead of directly wrapping `MessagingProvider`. Not in this ticket's scope.

### 2. ResourceDefinitionSource Trait Additions

**File:** `crates/tasker-runtime/src/sources/mod.rs`

The existing `ResourceDefinitionEvent` enum (Added/Updated/Removed variants) remains unchanged. Add `watch()` with a default implementation:

```rust
#[async_trait]
pub trait ResourceDefinitionSource: Send + Sync + std::fmt::Debug {
    async fn resolve(&self, name: &str) -> Option<ResourceDefinition>;
    async fn list_names(&self) -> Vec<String>;

    /// Watch for resource definition changes (additions, updates, removals).
    ///
    /// Used for credential rotation and dynamic resource lifecycle.
    /// Sources that don't support watching return `None`.
    async fn watch(&self) -> Option<ResourceDefinitionWatcher> {
        None
    }
}
```

### 3. ResourceDefinitionWatcher / ResourceDefinitionNotifier Newtypes

**File:** `crates/tasker-runtime/src/sources/mod.rs`

Newtype wrappers over `tokio::sync::mpsc` to provide named, typed channels that avoid confusion with other receivers in the system:

```rust
/// A receiver for resource definition change events.
#[derive(Debug)]
pub struct ResourceDefinitionWatcher(pub tokio::sync::mpsc::Receiver<ResourceDefinitionEvent>);

/// The sender half for resource definition change events.
#[derive(Debug, Clone)]
pub struct ResourceDefinitionNotifier(pub tokio::sync::mpsc::Sender<ResourceDefinitionEvent>);

impl ResourceDefinitionWatcher {
    /// Create a bounded channel pair for resource definition events.
    pub fn channel(capacity: usize) -> (ResourceDefinitionNotifier, ResourceDefinitionWatcher) {
        let (tx, rx) = tokio::sync::mpsc::channel(capacity);
        (ResourceDefinitionNotifier(tx), ResourceDefinitionWatcher(rx))
    }
}
```

Follows project convention: all channels are bounded, no `unbounded_channel()`.

### 4. StaticConfigSource Implementation

**File:** `crates/tasker-runtime/src/sources/static_config.rs`

HashMap-based lookup, loaded once from `Vec<ResourceDefinition>` at construction:

```rust
#[derive(Debug)]
pub struct StaticConfigSource {
    definitions: HashMap<String, ResourceDefinition>,
}

impl StaticConfigSource {
    pub fn new(definitions: Vec<ResourceDefinition>) -> Self {
        let definitions = definitions
            .into_iter()
            .map(|d| (d.name.clone(), d))
            .collect();
        Self { definitions }
    }
}

#[async_trait]
impl ResourceDefinitionSource for StaticConfigSource {
    async fn resolve(&self, name: &str) -> Option<ResourceDefinition> {
        self.definitions.get(name).cloned()
    }

    async fn list_names(&self) -> Vec<String> {
        self.definitions.keys().cloned().collect()
    }

    // watch() inherits default None — static source, no rotation
}
```

Returns owned clones of `ResourceDefinition`. No `Arc` wrapping needed — definitions are config descriptors, not shared mutable state.

### 5. DefinitionBasedResolver

**New file:** `crates/tasker-runtime/src/sources/resolver.rs`

Bridges `ResourceDefinitionSource` → `ResourceHandleResolver` by looking up a definition, then dispatching to the appropriate `from_config` constructor based on `ResourceType`.

Imports: `SecretsProvider` from `tasker_secure`, handle types (`PostgresHandle`, `HttpHandle`) behind their respective feature gates.

```rust
#[derive(Debug)]
pub struct DefinitionBasedResolver {
    source: Arc<dyn ResourceDefinitionSource>,
    secrets: Arc<dyn SecretsProvider>,
}

impl DefinitionBasedResolver {
    pub fn new(
        source: Arc<dyn ResourceDefinitionSource>,
        secrets: Arc<dyn SecretsProvider>,
    ) -> Self {
        Self { source, secrets }
    }
}

#[async_trait]
impl ResourceHandleResolver for DefinitionBasedResolver {
    async fn resolve(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceOperationError> {
        let definition = self.source.resolve(resource_ref).await
            .ok_or_else(|| ResourceOperationError::EntityNotFound {
                entity: resource_ref.to_string(),
            })?;

        let handle: Arc<dyn ResourceHandle> = match definition.resource_type {
            #[cfg(feature = "postgres")]
            ResourceType::Postgres => {
                Arc::new(
                    PostgresHandle::from_config(
                        &definition.name, &definition.config, self.secrets.as_ref()
                    )
                    .await
                    .map_err(|e| map_resource_error(e))?
                )
            }
            #[cfg(feature = "http")]
            ResourceType::Http => {
                Arc::new(
                    HttpHandle::from_config(
                        &definition.name, &definition.config, self.secrets.as_ref()
                    )
                    .await
                    .map_err(|e| map_resource_error(e))?
                )
            }
            other => {
                return Err(ResourceOperationError::ValidationFailed {
                    message: format!(
                        "No handle factory for resource type: {other}"
                    ),
                });
            }
        };

        Ok(handle)
    }
}
```

The `map_resource_error` helper reuses the same function from `provider.rs` (made `pub(crate)` or extracted to a shared location). It provides the same semantic mapping: `MissingConfigKey` → `ValidationFailed`, initialization/health/credential failures → `Unavailable`, etc. Note: `from_config` only produces a subset of `ResourceError` variants (`MissingConfigKey`, `InitializationFailed`, `SecretResolution`), but the exhaustive match is correct and future-proof.

**Design decisions:**
- Feature-gated: `postgres` and `http` arms compile only when those features are enabled
- The `other` catch-all handles `Messaging`, `Custom`, and feature-disabled types with `ValidationFailed`
- Error mapping: `from_config` `ResourceError` variants are mapped with the same semantics as `provider.rs` (not collapsed to a single variant)
- No caching at this level — `AdapterCache` in `RuntimeOperationProvider` handles that
- `SecretsProvider` held at construction time, not per-request (matches `ResourceRegistry` pattern)

### 6. Module Structure and Exports

**`crates/tasker-runtime/src/sources/mod.rs`** adds:
```rust
pub mod resolver;

// Re-export for convenience
pub use resolver::DefinitionBasedResolver;
```

**`crates/tasker-runtime/src/lib.rs`** re-exports:
```rust
pub use sources::{
    DefinitionBasedResolver,
    ResourceDefinitionSource,
    ResourceDefinitionWatcher,
    ResourceDefinitionNotifier,
    ResourceHandleResolver,
};
```

`StaticConfigSource` accessed via `sources::static_config::StaticConfigSource` — not a top-level re-export (implementation detail, same pattern as individual adapters).

### 7. SopsFileWatcher — Deferred

`crates/tasker-runtime/src/sources/sops.rs` remains as a stub with `unimplemented!()`. The feature gate (`sops`) stays in place. No changes in this ticket.

---

## Testing Strategy

All tests at the `test-no-infra` tier. Higher test tiers already validate cross-layer coherence.

### StaticConfigSource Unit Tests (in `static_config.rs`)

| Test | Validates |
|------|-----------|
| `resolve_existing` | Finds a definition by name |
| `resolve_missing` | Returns `None` for unknown name |
| `list_names` | Returns all registered names |
| `empty_source` | Empty vec produces empty source |
| `watch_returns_none` | Default `watch()` impl returns `None` |

### DefinitionBasedResolver Unit Tests (in `resolver.rs`)

| Test | Validates |
|------|-----------|
| `resolve_missing_definition` | Source returns `None` → `EntityNotFound` |
| `resolve_unsupported_type` | `Messaging` or `Custom` type → `ValidationFailed` |
| `resolve_initialization_failure` | `from_config` fails → `Unavailable` error mapping |

Note: Happy-path tests for Postgres/HTTP handle initialization are already covered by tasker-secure's `from_config` tests. The resolver tests focus on dispatch logic and error paths using test doubles.

### ResourceDefinitionWatcher Unit Tests (in `mod.rs`)

| Test | Validates |
|------|-----------|
| `channel_sends_and_receives` | Basic send/receive through newtype pair |

### Integration Test (in `crates/tasker-runtime/tests/`)

| Test | Validates |
|------|-----------|
| `definition_resolver_end_to_end` | `StaticConfigSource` with `Custom` type → mock `ResourceHandleResolver` that wraps `InMemoryResourceHandle` → `ResourcePoolManager::get_or_initialize` → handle returned. Validates the source lookup and pool registration flow without real Postgres/HTTP connections. |

Note: The integration test uses a test `ResourceHandleResolver` (not `DefinitionBasedResolver` directly) because `DefinitionBasedResolver`'s `from_config` calls establish real connections. The test-no-infra integration test validates that `StaticConfigSource` plugs into the pool manager's lazy initialization path correctly. The `DefinitionBasedResolver` → real handle path is covered by higher test tiers.

---

## Files Changed

| File | Change |
|------|--------|
| `crates/tasker-secure/src/resource/types.rs` | `Pgmq` → `Messaging` with serde alias |
| `crates/tasker-secure/src/resource/types.rs` | `Display` impl update |
| Various test files referencing `ResourceType::Pgmq` | Update to `Messaging` |
| `crates/tasker-runtime/src/sources/mod.rs` | Add `watch()`, newtypes, `pub mod resolver` |
| `crates/tasker-runtime/src/sources/static_config.rs` | Full implementation |
| `crates/tasker-runtime/src/sources/resolver.rs` | New file — `DefinitionBasedResolver` |
| `crates/tasker-runtime/src/lib.rs` | Re-exports for new public types |

---

## Dependencies

- **Depends on:** TAS-358 (`ResourceDefinition` type — complete)
- **Consumed by:** `RuntimeOperationProvider::with_source()` (TAS-377 — complete)
- **Future dependents:** SopsFileWatcher (deferred), MessagingHandle (future emit work), Phase 4 tasker-rs binary startup
