# TAS-376: ResourceDefinitionSource Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement StaticConfigSource, DefinitionBasedResolver, watch() trait method, and ResourceType::Pgmq→Messaging rename to complete Lane 2C of the composition architecture.

**Architecture:** StaticConfigSource provides HashMap-based resource definition lookup loaded at startup. DefinitionBasedResolver bridges definitions to live handles by dispatching to feature-gated `from_config` constructors. The `watch()` trait method with newtype channel wrappers prepares for future credential rotation.

**Tech Stack:** Rust, async-trait, tokio mpsc channels, serde (alias for backward compat), tasker-secure handles (PostgresHandle, HttpHandle)

**Spec:** `docs/superpowers/specs/2026-03-14-tas-376-resource-definition-source-design.md`

**Test command:** `cargo test --package tasker-runtime --lib && cargo test --package tasker-runtime --test '*' && cargo test --package tasker-secure --test '*'`

**Build check:** `cargo check --all-features --workspace && cargo clippy --all-targets --all-features --workspace`

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/tasker-secure/src/resource/types.rs` | Modify | Rename `Pgmq` → `Messaging` with serde alias |
| `crates/tasker-secure/tests/resource_types_test.rs` | Modify | Update `Pgmq` references to `Messaging` |
| `crates/tasker-secure/tests/resource_handle_test.rs` | Modify | Update `Pgmq` references to `Messaging` |
| `crates/tasker-secure/tests/in_memory_resource_test.rs` | Modify | Update `Pgmq` references to `Messaging` |
| `crates/tasker-secure/tests/resource_definition_test.rs` | Modify | Update `Pgmq` references to `Messaging`, add `"pgmq"` alias test |
| `crates/tasker-runtime/tests/runtime_provider_tests.rs` | Modify | Update `Pgmq` references to `Messaging` |
| `crates/tasker-runtime/tests/adapter_registry_tests.rs` | Modify | Update `Pgmq` references to `Messaging` |
| `crates/tasker-runtime/src/sources/mod.rs` | Modify | Add `watch()`, newtypes, `pub mod resolver`, re-export |
| `crates/tasker-runtime/src/sources/static_config.rs` | Modify | Replace stubs with HashMap implementation |
| `crates/tasker-runtime/src/sources/resolver.rs` | Create | `DefinitionBasedResolver` bridging definitions to handles |
| `crates/tasker-runtime/src/provider.rs` | Modify | Make `map_resource_error` `pub(crate)` |
| `crates/tasker-runtime/src/lib.rs` | Modify | Add re-exports for new public types |
| `crates/tasker-runtime/tests/static_config_source_tests.rs` | Create | Integration tests for StaticConfigSource |
| `crates/tasker-runtime/tests/definition_resolver_tests.rs` | Create | Integration tests for DefinitionBasedResolver |

---

## Chunk 1: ResourceType Rename (Pgmq → Messaging)

### Task 1: Rename ResourceType::Pgmq to Messaging in tasker-secure

**Files:**
- Modify: `crates/tasker-secure/src/resource/types.rs:10-35`

- [ ] **Step 1: Update the enum variant and Display impl**

In `crates/tasker-secure/src/resource/types.rs`, change:

```rust
// Line 10-24: Enum definition
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    /// PostgreSQL database connection.
    Postgres,
    /// HTTP/HTTPS endpoint.
    Http,
    /// Messaging provider (PGMQ, RabbitMQ). Provider-agnostic.
    #[serde(alias = "pgmq")]
    Messaging,
    /// User-defined resource type.
    Custom {
        /// The name of the custom resource type.
        type_name: String,
    },
}

// Line 26-35: Display impl
impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres => write!(f, "postgres"),
            Self::Http => write!(f, "http"),
            Self::Messaging => write!(f, "messaging"),
            Self::Custom { type_name } => write!(f, "{type_name}"),
        }
    }
}
```

- [ ] **Step 2: Run tasker-secure tests to see which ones fail from the rename**

Run: `cargo test --package tasker-secure --test '*' 2>&1 | head -60`
Expected: Several test failures referencing `Pgmq` (the tests still use the old name).

### Task 2: Update tasker-secure tests for Messaging rename

**Files:**
- Modify: `crates/tasker-secure/tests/resource_types_test.rs:16,40-41`
- Modify: `crates/tasker-secure/tests/resource_handle_test.rs:84,86`
- Modify: `crates/tasker-secure/tests/in_memory_resource_test.rs:61`
- Modify: `crates/tasker-secure/tests/resource_definition_test.rs:73-83,156,177`

- [ ] **Step 3: Update resource_types_test.rs**

Change line 16:
```rust
// Old: assert_eq!(format!("{}", ResourceType::Pgmq), "pgmq");
assert_eq!(format!("{}", ResourceType::Messaging), "messaging");
```

Change lines 40-41 — the TOML deserialization test. The `"pgmq"` string in the TOML should still work (serde alias), but the assertion should compare against the new variant:
```rust
let pgmq: Wrapper = toml::from_str(r#"rt = "pgmq""#).unwrap();
assert_eq!(pgmq.rt, ResourceType::Messaging);
```

Add a new test for the canonical `"messaging"` deserialization after the existing test:
```rust
#[test]
fn resource_type_deserialize_messaging_canonical() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        rt: ResourceType,
    }

    let messaging: Wrapper = toml::from_str(r#"rt = "messaging""#).unwrap();
    assert_eq!(messaging.rt, ResourceType::Messaging);
}
```

- [ ] **Step 4: Update resource_handle_test.rs**

Change line 84:
```rust
// Old: Arc::new(TestHandle::new("arc_db", ResourceType::Pgmq))
let handle: Arc<dyn ResourceHandle> = Arc::new(TestHandle::new("arc_db", ResourceType::Messaging));
```

Change line 86:
```rust
// Old: assert_eq!(handle.resource_type(), &ResourceType::Pgmq);
assert_eq!(handle.resource_type(), &ResourceType::Messaging);
```

- [ ] **Step 5: Update in_memory_resource_test.rs**

Change line 61:
```rust
// Old: InMemoryResourceHandle::new("events", ResourceType::Pgmq)
let handle = InMemoryResourceHandle::new("events", ResourceType::Messaging);
```

- [ ] **Step 6: Update resource_definition_test.rs**

Change lines 73-83 — rename the test and update assertion:
```rust
#[test]
fn deserialize_messaging_resource_definition() {
    let toml_str = r#"
        name = "task_queue"
        resource_type = "pgmq"
    "#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.name, "task_queue");
    assert_eq!(def.resource_type, ResourceType::Messaging);
    assert!(def.config.get("anything").is_none());
}
```

Change line 156 — the TOML string stays `"pgmq"` (backward compat), but the assertion updates:
```rust
// Line 156 TOML stays: resource_type = "pgmq"
// Line 177 assertion changes:
assert_eq!(list.resources[1].resource_type, ResourceType::Messaging);
```

- [ ] **Step 7: Run tasker-secure tests to verify all pass**

Run: `cargo test --package tasker-secure --test '*'`
Expected: All tests pass.

### Task 3: Update tasker-runtime tests for Messaging rename

**Files:**
- Modify: `crates/tasker-runtime/tests/runtime_provider_tests.rs:234-235`
- Modify: `crates/tasker-runtime/tests/adapter_registry_tests.rs:61-63`

- [ ] **Step 8: Update runtime_provider_tests.rs**

Change lines 234-235:
```rust
// Register a Messaging handle — no adapter factory exists for Messaging
let handle = Arc::new(InMemoryResourceHandle::new("queue", ResourceType::Messaging));
```

- [ ] **Step 9: Update adapter_registry_tests.rs**

Change lines 61-63 — update function name and body:
```rust
#[test]
fn messaging_has_no_persist_factory() {
    let registry = AdapterRegistry::standard();
    let handle = Arc::new(InMemoryResourceHandle::new("queue", ResourceType::Messaging));
    let result = registry.as_persistable(handle);
    let err = expect_err(result);
    let msg = format!("{err}");
    assert!(msg.contains("No persist adapter registered"), "Got: {msg}");
}
```

- [ ] **Step 10: Run full workspace check and tests**

Run: `cargo check --all-features --workspace && cargo test --package tasker-secure --test '*' && cargo test --package tasker-runtime --test '*'`
Expected: All compile and pass.

- [ ] **Step 11: Commit the ResourceType rename**

```bash
git add crates/tasker-secure/src/resource/types.rs \
       crates/tasker-secure/tests/resource_types_test.rs \
       crates/tasker-secure/tests/resource_handle_test.rs \
       crates/tasker-secure/tests/in_memory_resource_test.rs \
       crates/tasker-secure/tests/resource_definition_test.rs \
       crates/tasker-runtime/tests/runtime_provider_tests.rs \
       crates/tasker-runtime/tests/adapter_registry_tests.rs
git commit -m "refactor(TAS-376): rename ResourceType::Pgmq to Messaging

Messaging is provider-agnostic (PGMQ, RabbitMQ) and already abstracted
through MessagingProvider. Added serde alias for backward compat with
existing configs using \"pgmq\"."
```

---

## Chunk 2: ResourceDefinitionSource Trait Additions (watch + newtypes)

### Task 4: Add ResourceDefinitionWatcher/Notifier newtypes and watch() to trait

**Files:**
- Modify: `crates/tasker-runtime/src/sources/mod.rs`

- [ ] **Step 12: Write the channel newtype test first**

Add a test module at the bottom of `crates/tasker-runtime/src/sources/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn watcher_channel_sends_and_receives() {
        let (notifier, mut watcher) = ResourceDefinitionWatcher::channel(8);

        let event = ResourceDefinitionEvent::Added {
            name: "test-db".to_string(),
            definition: ResourceDefinition {
                name: "test-db".to_string(),
                resource_type: tasker_secure::ResourceType::Postgres,
                config: tasker_secure::ResourceConfig::default(),
                secrets_provider: None,
            },
        };

        notifier.0.send(event).await.unwrap();

        let received = watcher.0.recv().await.unwrap();
        assert!(matches!(received, ResourceDefinitionEvent::Added { name, .. } if name == "test-db"));
    }
}
```

- [ ] **Step 13: Run the test to verify it fails (types don't exist yet)**

Run: `cargo test --package tasker-runtime --lib sources::tests 2>&1 | head -20`
Expected: Compilation error — `ResourceDefinitionWatcher` not found.

- [ ] **Step 14: Add the newtypes and watch() method**

In `crates/tasker-runtime/src/sources/mod.rs`, add after the `ResourceDefinitionEvent` enum (after line 34) and before the trait definition:

```rust
/// A receiver for resource definition change events.
///
/// Wraps `tokio::sync::mpsc::Receiver<ResourceDefinitionEvent>` with a
/// named type to avoid confusion with other channel receivers in the system.
#[derive(Debug)]
pub struct ResourceDefinitionWatcher(pub tokio::sync::mpsc::Receiver<ResourceDefinitionEvent>);

/// The sender half for resource definition change events.
#[derive(Debug, Clone)]
pub struct ResourceDefinitionNotifier(pub tokio::sync::mpsc::Sender<ResourceDefinitionEvent>);

impl ResourceDefinitionWatcher {
    /// Create a bounded channel pair for resource definition events.
    ///
    /// Follows project convention: all channels are bounded.
    pub fn channel(capacity: usize) -> (ResourceDefinitionNotifier, ResourceDefinitionWatcher) {
        let (tx, rx) = tokio::sync::mpsc::channel(capacity);
        (ResourceDefinitionNotifier(tx), ResourceDefinitionWatcher(rx))
    }
}
```

Then add `watch()` to the `ResourceDefinitionSource` trait (after `list_names`):

```rust
    /// Watch for resource definition changes (additions, updates, removals).
    ///
    /// Used for credential rotation and dynamic resource lifecycle.
    /// Sources that don't support watching return `None`.
    async fn watch(&self) -> Option<ResourceDefinitionWatcher> {
        None
    }
```

- [ ] **Step 15: Run the test to verify it passes**

Run: `cargo test --package tasker-runtime --lib sources::tests`
Expected: PASS.

- [ ] **Step 16: Run full crate check (trait change may affect implementors)**

Run: `cargo check --all-features --workspace`
Expected: Passes. The `watch()` method has a default impl, so existing implementors (StaticConfigSource stub, SopsFileWatcher stub) don't need changes.

- [ ] **Step 17: Commit the trait additions**

```bash
git add crates/tasker-runtime/src/sources/mod.rs
git commit -m "feat(TAS-376): add watch() to ResourceDefinitionSource with channel newtypes

ResourceDefinitionWatcher/ResourceDefinitionNotifier wrap bounded mpsc
channels for credential rotation and dynamic resource lifecycle events.
watch() has a default None impl for sources that don't support watching."
```

---

## Chunk 3: StaticConfigSource Implementation

### Task 5: Implement StaticConfigSource

**Files:**
- Modify: `crates/tasker-runtime/src/sources/static_config.rs`
- Create: `crates/tasker-runtime/tests/static_config_source_tests.rs`

- [ ] **Step 18: Write failing tests for StaticConfigSource**

Create `crates/tasker-runtime/tests/static_config_source_tests.rs`:

```rust
//! Tests for StaticConfigSource resource definition lookup.

use tasker_runtime::sources::static_config::StaticConfigSource;
use tasker_runtime::ResourceDefinitionSource;
use tasker_secure::{ResourceConfig, ResourceDefinition, ResourceType};

fn make_definition(name: &str, rt: ResourceType) -> ResourceDefinition {
    ResourceDefinition {
        name: name.to_string(),
        resource_type: rt,
        config: ResourceConfig::default(),
        secrets_provider: None,
    }
}

#[tokio::test]
async fn resolve_existing_returns_definition() {
    let source = StaticConfigSource::new(vec![
        make_definition("db1", ResourceType::Postgres),
        make_definition("api1", ResourceType::Http),
    ]);

    let result = source.resolve("db1").await;
    assert!(result.is_some());
    let def = result.unwrap();
    assert_eq!(def.name, "db1");
    assert_eq!(def.resource_type, ResourceType::Postgres);
}

#[tokio::test]
async fn resolve_missing_returns_none() {
    let source = StaticConfigSource::new(vec![
        make_definition("db1", ResourceType::Postgres),
    ]);

    let result = source.resolve("nonexistent").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn list_names_returns_all_registered() {
    let source = StaticConfigSource::new(vec![
        make_definition("db1", ResourceType::Postgres),
        make_definition("api1", ResourceType::Http),
        make_definition("queue1", ResourceType::Messaging),
    ]);

    let mut names = source.list_names().await;
    names.sort();
    assert_eq!(names, vec!["api1", "db1", "queue1"]);
}

#[tokio::test]
async fn empty_source_has_no_names() {
    let source = StaticConfigSource::new(vec![]);

    let names = source.list_names().await;
    assert!(names.is_empty());

    let result = source.resolve("anything").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn watch_returns_none_for_static_source() {
    let source = StaticConfigSource::new(vec![
        make_definition("db1", ResourceType::Postgres),
    ]);

    let watcher = source.watch().await;
    assert!(watcher.is_none());
}
```

- [ ] **Step 19: Run tests to verify they fail**

Run: `cargo test --package tasker-runtime --test static_config_source_tests 2>&1 | head -20`
Expected: Compilation error or panics from `unimplemented!()`.

- [ ] **Step 20: Implement StaticConfigSource**

Replace the entire contents of `crates/tasker-runtime/src/sources/static_config.rs`:

```rust
//! Static configuration source for resource definitions.
//!
//! Reads resource definitions from a pre-loaded list, typically originating
//! from worker.toml `[[resources]]` sections.

use std::collections::HashMap;

use async_trait::async_trait;

use tasker_secure::ResourceDefinition;

use super::ResourceDefinitionSource;

/// Resolves resource definitions from static configuration.
///
/// Loaded once at startup from a `Vec<ResourceDefinition>`.
/// Does not support watching — `watch()` returns `None`.
#[derive(Debug)]
pub struct StaticConfigSource {
    definitions: HashMap<String, ResourceDefinition>,
}

impl StaticConfigSource {
    /// Create a new static config source from a list of definitions.
    ///
    /// Indexes by `definition.name`. Duplicate names are resolved by
    /// last-write-wins (later entries overwrite earlier ones).
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

- [ ] **Step 21: Run tests to verify they pass**

Run: `cargo test --package tasker-runtime --test static_config_source_tests`
Expected: All 5 tests pass.

- [ ] **Step 22: Commit StaticConfigSource**

```bash
git add crates/tasker-runtime/src/sources/static_config.rs \
       crates/tasker-runtime/tests/static_config_source_tests.rs
git commit -m "feat(TAS-376): implement StaticConfigSource with HashMap-based lookup

Replaces unimplemented stubs with a HashMap indexed by definition name.
Returns owned clones. watch() inherits default None."
```

---

## Chunk 4: DefinitionBasedResolver + map_resource_error sharing

### Task 6: Make map_resource_error pub(crate)

**Files:**
- Modify: `crates/tasker-runtime/src/provider.rs:147`

- [ ] **Step 23: Change visibility of map_resource_error**

In `crates/tasker-runtime/src/provider.rs`, change line 147:
```rust
// Old: fn map_resource_error(err: ResourceError) -> ResourceOperationError {
pub(crate) fn map_resource_error(err: ResourceError) -> ResourceOperationError {
```

- [ ] **Step 24: Verify it still compiles**

Run: `cargo check --package tasker-runtime --all-features`
Expected: Passes.

### Task 7: Implement DefinitionBasedResolver

**Files:**
- Create: `crates/tasker-runtime/src/sources/resolver.rs`
- Modify: `crates/tasker-runtime/src/sources/mod.rs` (add module + re-export)

- [ ] **Step 25: Write failing tests for DefinitionBasedResolver**

Create `crates/tasker-runtime/tests/definition_resolver_tests.rs`:

```rust
//! Tests for DefinitionBasedResolver — bridges ResourceDefinitionSource
//! to ResourceHandleResolver by dispatching to handle constructors.

use std::collections::HashMap;
use std::sync::Arc;

use tasker_grammar::operations::ResourceOperationError;
use tasker_runtime::sources::static_config::StaticConfigSource;
use tasker_runtime::{DefinitionBasedResolver, ResourceDefinitionSource, ResourceHandleResolver};
use tasker_secure::testing::{InMemoryResourceHandle, InMemorySecretsProvider};
use tasker_secure::{ResourceConfig, ResourceDefinition, ResourceType};

fn test_secrets() -> Arc<dyn tasker_secure::SecretsProvider> {
    Arc::new(InMemorySecretsProvider::new(HashMap::new()))
}

fn make_definition(name: &str, rt: ResourceType) -> ResourceDefinition {
    ResourceDefinition {
        name: name.to_string(),
        resource_type: rt,
        config: ResourceConfig::default(),
        secrets_provider: None,
    }
}

#[tokio::test]
async fn resolve_missing_definition_returns_entity_not_found() {
    let source: Arc<dyn ResourceDefinitionSource> =
        Arc::new(StaticConfigSource::new(vec![]));
    let resolver = DefinitionBasedResolver::new(source, test_secrets());

    let result = resolver.resolve("nonexistent").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, ResourceOperationError::EntityNotFound { entity } if entity == "nonexistent"),
        "Expected EntityNotFound, got: {err:?}"
    );
}

#[tokio::test]
async fn resolve_messaging_type_returns_validation_failed() {
    let source: Arc<dyn ResourceDefinitionSource> = Arc::new(StaticConfigSource::new(vec![
        make_definition("queue", ResourceType::Messaging),
    ]));
    let resolver = DefinitionBasedResolver::new(source, test_secrets());

    let result = resolver.resolve("queue").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, ResourceOperationError::ValidationFailed { message } if message.contains("No handle factory")),
        "Expected ValidationFailed with 'No handle factory', got: {err:?}"
    );
}

#[tokio::test]
async fn resolve_custom_type_returns_validation_failed() {
    let source: Arc<dyn ResourceDefinitionSource> = Arc::new(StaticConfigSource::new(vec![
        make_definition("cache", ResourceType::Custom { type_name: "redis".to_string() }),
    ]));
    let resolver = DefinitionBasedResolver::new(source, test_secrets());

    let result = resolver.resolve("cache").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, ResourceOperationError::ValidationFailed { message } if message.contains("No handle factory")),
        "Expected ValidationFailed with 'No handle factory', got: {err:?}"
    );
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn resolve_postgres_with_missing_config_returns_error() {
    // Postgres from_config requires at least "host" and "database" keys.
    // An empty config should fail during initialization (before any connection).
    let source: Arc<dyn ResourceDefinitionSource> = Arc::new(StaticConfigSource::new(vec![
        make_definition("bad-db", ResourceType::Postgres),
    ]));
    let resolver = DefinitionBasedResolver::new(source, test_secrets());

    let result = resolver.resolve("bad-db").await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    // MissingConfigKey maps to ValidationFailed via map_resource_error
    assert!(
        matches!(
            err,
            ResourceOperationError::ValidationFailed { .. }
                | ResourceOperationError::Unavailable { .. }
        ),
        "Expected ValidationFailed or Unavailable from missing config, got: {err:?}"
    );
}

#[tokio::test]
async fn debug_output_is_meaningful() {
    let source: Arc<dyn ResourceDefinitionSource> =
        Arc::new(StaticConfigSource::new(vec![]));
    let resolver = DefinitionBasedResolver::new(source, test_secrets());

    let debug = format!("{resolver:?}");
    assert!(debug.contains("DefinitionBasedResolver"));
}
```

- [ ] **Step 26: Run tests to verify they fail (resolver doesn't exist yet)**

Run: `cargo test --package tasker-runtime --test definition_resolver_tests 2>&1 | head -20`
Expected: Compilation error — `DefinitionBasedResolver` not found.

- [ ] **Step 27: Create the resolver module and add to sources/mod.rs**

Add to `crates/tasker-runtime/src/sources/mod.rs` after the `static_config` module declaration:

```rust
pub mod resolver;

// Re-export for convenience
pub use resolver::DefinitionBasedResolver;
```

Create `crates/tasker-runtime/src/sources/resolver.rs`:

```rust
//! Definition-based resource handle resolver.
//!
//! Bridges [`ResourceDefinitionSource`] to [`ResourceHandleResolver`] by
//! looking up a resource definition by name and dispatching to the
//! appropriate handle constructor based on [`ResourceType`].

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::ResourceOperationError;
use tasker_secure::{ResourceHandle, ResourceType, SecretsProvider};

#[cfg(feature = "http")]
use tasker_secure::resource::http::HttpHandle;
#[cfg(feature = "postgres")]
use tasker_secure::resource::postgres::PostgresHandle;

use super::{ResourceDefinitionSource, ResourceHandleResolver};
use crate::provider::map_resource_error;

/// Resolves resource handles by looking up definitions and initializing handles.
///
/// Given a resource name:
/// 1. Queries the [`ResourceDefinitionSource`] for a [`ResourceDefinition`]
/// 2. Dispatches to the appropriate `from_config` constructor based on [`ResourceType`]
/// 3. Returns the initialized handle as `Arc<dyn ResourceHandle>`
///
/// Feature-gated: `postgres` and `http` handle construction require their
/// respective features to be enabled. Unsupported types return
/// `ResourceOperationError::ValidationFailed`.
#[derive(Debug)]
pub struct DefinitionBasedResolver {
    source: Arc<dyn ResourceDefinitionSource>,
    secrets: Arc<dyn SecretsProvider>,
}

impl DefinitionBasedResolver {
    /// Create a new resolver backed by the given definition source and secrets provider.
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
        let definition = self
            .source
            .resolve(resource_ref)
            .await
            .ok_or_else(|| ResourceOperationError::EntityNotFound {
                entity: resource_ref.to_string(),
            })?;

        let handle: Arc<dyn ResourceHandle> = match definition.resource_type {
            #[cfg(feature = "postgres")]
            ResourceType::Postgres => Arc::new(
                PostgresHandle::from_config(
                    &definition.name,
                    &definition.config,
                    self.secrets.as_ref(),
                )
                .await
                .map_err(map_resource_error)?,
            ),
            #[cfg(feature = "http")]
            ResourceType::Http => Arc::new(
                HttpHandle::from_config(
                    &definition.name,
                    &definition.config,
                    self.secrets.as_ref(),
                )
                .await
                .map_err(map_resource_error)?,
            ),
            other => {
                return Err(ResourceOperationError::ValidationFailed {
                    message: format!("No handle factory for resource type: {other}"),
                });
            }
        };

        Ok(handle)
    }
}
```

- [ ] **Step 28: Run tests to verify they pass**

Run: `cargo test --package tasker-runtime --test definition_resolver_tests`
Expected: All 5 tests pass (4 base + 1 feature-gated postgres test if postgres feature enabled).

- [ ] **Step 29: Run full crate check**

Run: `cargo check --all-features --workspace`
Expected: Passes.

- [ ] **Step 30: Commit DefinitionBasedResolver**

```bash
git add crates/tasker-runtime/src/sources/resolver.rs \
       crates/tasker-runtime/src/sources/mod.rs \
       crates/tasker-runtime/src/provider.rs \
       crates/tasker-runtime/tests/definition_resolver_tests.rs
git commit -m "feat(TAS-376): implement DefinitionBasedResolver bridging definitions to handles

Dispatches to PostgresHandle::from_config and HttpHandle::from_config
behind feature gates. Reuses map_resource_error from provider.rs for
consistent error domain translation. Unsupported types return
ValidationFailed."
```

---

## Chunk 5: Exports + Integration Test + Final Verification

### Task 8: Update lib.rs re-exports

**Files:**
- Modify: `crates/tasker-runtime/src/lib.rs:39`

- [ ] **Step 31: Add new re-exports**

Change line 39 of `crates/tasker-runtime/src/lib.rs`:

```rust
// Old:
// pub use sources::{ResourceDefinitionSource, ResourceHandleResolver};
// New:
pub use sources::{
    DefinitionBasedResolver, ResourceDefinitionNotifier, ResourceDefinitionSource,
    ResourceDefinitionWatcher, ResourceHandleResolver,
};
```

- [ ] **Step 32: Verify it compiles**

Run: `cargo check --package tasker-runtime --all-features`
Expected: Passes.

### Task 9: Integration test — full pipeline

**Files:**
- Create: `crates/tasker-runtime/tests/source_integration_tests.rs`

- [ ] **Step 33: Write the integration test**

Create `crates/tasker-runtime/tests/source_integration_tests.rs`:

```rust
//! Integration test: StaticConfigSource → mock resolver → ResourcePoolManager.
//!
//! Validates the full pipeline without infrastructure. Uses a test
//! ResourceHandleResolver (not DefinitionBasedResolver) because from_config
//! establishes real connections.

use std::collections::HashMap;
use std::sync::Arc;

use tasker_grammar::operations::ResourceOperationError;
use tasker_runtime::pool_manager::{PoolManagerConfig, ResourcePoolManager};
use tasker_runtime::sources::static_config::StaticConfigSource;
use tasker_runtime::{ResourceDefinitionSource, ResourceHandleResolver};
use tasker_secure::testing::{InMemoryResourceHandle, InMemorySecretsProvider};
use tasker_secure::{ResourceConfig, ResourceDefinition, ResourceHandle, ResourceRegistry, ResourceType};

fn test_secrets() -> Arc<dyn tasker_secure::SecretsProvider> {
    Arc::new(InMemorySecretsProvider::new(HashMap::new()))
}

/// Test resolver that uses a StaticConfigSource to verify the definition exists,
/// then returns an InMemoryResourceHandle.
#[derive(Debug)]
struct StubSourceResolver {
    source: Arc<dyn ResourceDefinitionSource>,
}

#[async_trait::async_trait]
impl ResourceHandleResolver for StubSourceResolver {
    async fn resolve(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceOperationError> {
        let definition = self
            .source
            .resolve(resource_ref)
            .await
            .ok_or_else(|| ResourceOperationError::EntityNotFound {
                entity: resource_ref.to_string(),
            })?;

        Ok(Arc::new(InMemoryResourceHandle::new(
            &definition.name,
            definition.resource_type.clone(),
        )))
    }
}

#[tokio::test]
async fn static_source_feeds_pool_manager_lazy_init() {
    // 1. Set up a StaticConfigSource with definitions
    let source: Arc<dyn ResourceDefinitionSource> = Arc::new(StaticConfigSource::new(vec![
        ResourceDefinition {
            name: "orders-db".to_string(),
            resource_type: ResourceType::Postgres,
            config: ResourceConfig::default(),
            secrets_provider: None,
        },
        ResourceDefinition {
            name: "payment-api".to_string(),
            resource_type: ResourceType::Http,
            config: ResourceConfig::default(),
            secrets_provider: None,
        },
    ]));

    // 2. Create resolver that checks the source before returning a handle
    let resolver = StubSourceResolver {
        source: source.clone(),
    };

    // 3. Create pool manager with no pre-registered resources
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let pool = ResourcePoolManager::new(registry, PoolManagerConfig::default());

    // 4. Lazy init through pool manager — resource not pre-registered
    let handle = pool
        .get_or_initialize("orders-db", Some(&resolver))
        .await
        .unwrap();
    assert_eq!(handle.resource_name(), "orders-db");

    // 5. Second call should find it already registered
    let handle2 = pool.get("orders-db").await.unwrap();
    assert_eq!(handle2.resource_name(), "orders-db");

    // 6. Resource NOT in source should fail
    let err = pool
        .get_or_initialize("nonexistent", Some(&resolver))
        .await
        .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("initialization failed"), "Got: {msg}");
}

#[tokio::test]
async fn static_source_list_names_matches_definitions() {
    let source = StaticConfigSource::new(vec![
        ResourceDefinition {
            name: "db1".to_string(),
            resource_type: ResourceType::Postgres,
            config: ResourceConfig::default(),
            secrets_provider: None,
        },
        ResourceDefinition {
            name: "api1".to_string(),
            resource_type: ResourceType::Http,
            config: ResourceConfig::default(),
            secrets_provider: None,
        },
    ]);

    let mut names = source.list_names().await;
    names.sort();
    assert_eq!(names, vec!["api1", "db1"]);
}
```

- [ ] **Step 34: Run integration tests**

Run: `cargo test --package tasker-runtime --test source_integration_tests`
Expected: All tests pass.

### Task 10: Final verification

- [ ] **Step 35: Run full workspace build and clippy**

Run: `cargo clippy --all-targets --all-features --workspace 2>&1 | tail -20`
Expected: Zero warnings.

- [ ] **Step 36: Run all tasker-runtime tests**

Run: `cargo test --package tasker-runtime --lib && cargo test --package tasker-runtime --test '*'`
Expected: All tests pass (existing + new).

- [ ] **Step 37: Run all tasker-secure tests**

Run: `cargo test --package tasker-secure --test '*'`
Expected: All tests pass.

- [ ] **Step 38: Commit exports and integration test**

```bash
git add crates/tasker-runtime/src/lib.rs \
       crates/tasker-runtime/tests/source_integration_tests.rs
git commit -m "feat(TAS-376): add re-exports and integration test for source pipeline

Exports DefinitionBasedResolver, ResourceDefinitionWatcher, and
ResourceDefinitionNotifier from crate root. Integration test validates
StaticConfigSource → resolver → ResourcePoolManager lazy init flow."
```

- [ ] **Step 39: Final full test run**

Run: `cargo make test-no-infra`
Expected: All workspace tests at the no-infra tier pass.
