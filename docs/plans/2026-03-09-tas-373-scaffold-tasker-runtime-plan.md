# TAS-373: Scaffold tasker-runtime — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Scaffold the `tasker-runtime` crate as a new workspace member with type definitions and trait signatures for all Phase 2 lanes (2A-2D), gated by feature flags mirroring tasker-secure.

**Architecture:** tasker-runtime bridges tasker-grammar operation traits to tasker-secure resource handles. It depends on both but neither depends on it. All method bodies are `unimplemented!()` — real implementations come in subsequent tickets (2A-2D).

**Tech Stack:** Rust, async-trait, serde/serde_json, cargo-make. Feature-gated modules for postgres, http, sops.

---

### Task 1: Create Cargo.toml and register workspace member

**Files:**
- Create: `crates/tasker-runtime/Cargo.toml`
- Modify: `Cargo.toml` (root, workspace members list)

**Step 1: Create `crates/tasker-runtime/Cargo.toml`**

```toml
[package]
name = "tasker-runtime"
version = "0.1.6"
edition = "2021"
description = "Runtime adapters bridging tasker-grammar operation traits to tasker-secure resource handles"
readme = "README.md"
repository = "https://github.com/tasker-systems/tasker-core"
license = "MIT"
keywords = ["runtime", "adapter", "resource", "workflow", "orchestration"]
categories = ["concurrency", "database"]

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[lib]
crate-type = ["rlib"]
name = "tasker_runtime"

[features]
default = []
postgres = ["tasker-secure/postgres"]
http = ["tasker-secure/http"]
sops = ["tasker-secure/sops"]

[dependencies]
tasker-grammar = { path = "../tasker-grammar", version = "=0.1.6" }
tasker-secure = { path = "../tasker-secure", version = "=0.1.6" }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["full", "test-util"] }
serde_json = { workspace = true }

[lints]
workspace = true
```

**Step 2: Register in root `Cargo.toml` workspace members**

Add after the `tasker-secure` line:

```toml
  "crates/tasker-runtime", # TAS-373: Runtime adapters for grammar operations
```

**Step 3: Create minimal `src/lib.rs` to verify compilation**

```rust
//! Runtime adapters bridging tasker-grammar operation traits to tasker-secure
//! resource handles.

```

**Step 4: Verify compilation**

Run: `cargo check -p tasker-runtime --all-features`
Expected: Compiles with no errors (empty lib.rs)

**Step 5: Commit**

```bash
git add crates/tasker-runtime/Cargo.toml crates/tasker-runtime/src/lib.rs Cargo.toml Cargo.lock
git commit -m "feat(TAS-373): scaffold tasker-runtime crate with workspace registration"
```

---

### Task 2: Create Makefile.toml

**Files:**
- Create: `crates/tasker-runtime/Makefile.toml`

**Step 1: Create `crates/tasker-runtime/Makefile.toml`**

Follow the exact pattern from `crates/tasker-grammar/Makefile.toml`:

```toml
# =============================================================================
# tasker-runtime - cargo-make Task Definitions
# =============================================================================
#
# Runtime adapters bridging tasker-grammar operation traits to tasker-secure
# resource handles. Pool management, adapter registry, and operation provider.
#
# Quick Start:
#   cargo make check    # Run all quality checks
#   cargo make test     # Run tests
#   cargo make fix      # Auto-fix issues
#
# =============================================================================

extend = "../../tools/cargo-make/base-tasks.toml"

[config]
default_to_workspace = false

[env]
CRATE_NAME = "tasker-runtime"

# =============================================================================
# Main Tasks
# =============================================================================

[tasks.default]
alias = "check"

[tasks.check]
description = "Run quality checks"
dependencies = ["format-check", "lint", "test"]

[tasks.format-check]
extend = "base-rust-format"

[tasks.format-fix]
extend = "base-rust-format-fix"

[tasks.lint]
extend = "base-rust-lint"

[tasks.lint-fix]
extend = "base-rust-lint-fix"

[tasks.test]
extend = "base-rust-test"
description = "Run tasker-runtime tests"
args = ["nextest", "run", "-p", "${CRATE_NAME}", "--all-features"]

[tasks.fix]
description = "Fix all fixable issues"
dependencies = ["format-fix", "lint-fix"]

[tasks.clean]
description = "Clean build artifacts"
command = "cargo"
args = ["clean", "-p", "${CRATE_NAME}"]
```

**Step 2: Verify cargo-make works**

Run: `cd crates/tasker-runtime && cargo make check`
Expected: format-check, lint, and test all pass (empty crate)

**Step 3: Commit**

```bash
git add crates/tasker-runtime/Makefile.toml
git commit -m "chore(TAS-373): add cargo-make task definitions for tasker-runtime"
```

---

### Task 3: Pool manager types — lifecycle.rs and metrics.rs

**Files:**
- Create: `crates/tasker-runtime/src/pool_manager/lifecycle.rs`
- Create: `crates/tasker-runtime/src/pool_manager/metrics.rs`
- Create: `crates/tasker-runtime/src/pool_manager/mod.rs`

These are pure data types with no behavior — no `unimplemented!()` needed.

**Step 1: Create `crates/tasker-runtime/src/pool_manager/lifecycle.rs`**

```rust
//! Pool lifecycle configuration types.

use std::time::Duration;

/// Whether a resource was statically configured or dynamically created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceOrigin {
    /// From worker.toml configuration — never evicted.
    Static,
    /// Created at runtime by generative workflows — subject to eviction.
    Dynamic,
}

/// Strategy for evicting idle resource pools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionStrategy {
    /// Least Recently Used — evict the pool that was accessed longest ago.
    Lru,
    /// Least Frequently Used — evict the pool with the fewest accesses.
    Lfu,
    /// First In, First Out — evict the oldest pool.
    Fifo,
}

/// Strategy for admitting new resource pools when at capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionStrategy {
    /// Reject new pools when at capacity.
    Reject,
    /// Evict an existing pool to make room.
    EvictOne,
}

/// Configuration for the resource pool manager.
#[derive(Debug, Clone)]
pub struct PoolManagerConfig {
    /// Maximum number of distinct resource pools.
    pub max_pools: usize,
    /// Maximum total connections across all pools.
    pub max_total_connections: usize,
    /// Idle timeout before a dynamic pool becomes eligible for eviction.
    pub idle_timeout: Duration,
    /// Interval between eviction sweeps.
    pub sweep_interval: Duration,
    /// Strategy for choosing which pool to evict.
    pub eviction_strategy: EvictionStrategy,
    /// Strategy for handling new pool requests when at capacity.
    pub admission_strategy: AdmissionStrategy,
}

impl Default for PoolManagerConfig {
    fn default() -> Self {
        Self {
            max_pools: 32,
            max_total_connections: 256,
            idle_timeout: Duration::from_secs(300),
            sweep_interval: Duration::from_secs(60),
            eviction_strategy: EvictionStrategy::Lru,
            admission_strategy: AdmissionStrategy::EvictOne,
        }
    }
}
```

**Step 2: Create `crates/tasker-runtime/src/pool_manager/metrics.rs`**

```rust
//! Access metrics for pool eviction decisions.

use std::time::Instant;

/// Tracks access patterns for a single resource pool.
#[derive(Debug, Clone)]
pub struct ResourceAccessMetrics {
    /// When the pool was created.
    pub creation_time: Instant,
    /// When the pool was last accessed.
    pub last_accessed: Instant,
    /// Total number of accesses.
    pub access_count: u64,
}

impl ResourceAccessMetrics {
    /// Create metrics for a newly created pool.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            creation_time: now,
            last_accessed: now,
            access_count: 0,
        }
    }

    /// Record an access to the pool.
    pub fn record_access(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

impl Default for ResourceAccessMetrics {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 3: Create `crates/tasker-runtime/src/pool_manager/mod.rs`**

```rust
//! Resource pool manager with lifecycle management, eviction, and admission control.
//!
//! Wraps `tasker_secure::ResourceRegistry` with dynamic pool creation,
//! eviction policies, and connection budget enforcement.

mod lifecycle;
mod metrics;

pub use lifecycle::{
    AdmissionStrategy, EvictionStrategy, PoolManagerConfig, ResourceOrigin,
};
pub use metrics::ResourceAccessMetrics;

use std::sync::Arc;

use tasker_secure::{ResourceHandle, ResourceRegistry, ResourceSummary};

/// Manages resource pool lifecycle with eviction and admission control.
///
/// Wraps a `ResourceRegistry` and adds:
/// - Dynamic pool creation for generative workflows
/// - Eviction of idle dynamic pools based on configurable strategy
/// - Admission control when at capacity
/// - Connection budget enforcement across all pools
#[derive(Debug)]
pub struct ResourcePoolManager {
    registry: Arc<ResourceRegistry>,
    config: PoolManagerConfig,
}

impl ResourcePoolManager {
    /// Create a new pool manager wrapping the given registry.
    pub fn new(registry: Arc<ResourceRegistry>, config: PoolManagerConfig) -> Self {
        Self { registry, config }
    }

    /// Get or initialize a resource handle by name.
    ///
    /// If the pool doesn't exist yet, creates it subject to admission control.
    /// Updates access metrics on every call.
    pub async fn get_or_initialize(
        &self,
        _name: &str,
        _origin: ResourceOrigin,
    ) -> Result<Arc<dyn ResourceHandle>, tasker_secure::ResourceError> {
        unimplemented!("TAS-374: ResourcePoolManager::get_or_initialize")
    }

    /// Evict a specific resource pool by name.
    ///
    /// Static-origin pools cannot be evicted.
    pub async fn evict(
        &self,
        _name: &str,
    ) -> Result<(), tasker_secure::ResourceError> {
        unimplemented!("TAS-374: ResourcePoolManager::evict")
    }

    /// Run an eviction sweep based on the configured strategy.
    ///
    /// Returns the number of pools evicted and connections freed.
    pub async fn sweep(&self) -> (usize, usize) {
        unimplemented!("TAS-374: ResourcePoolManager::sweep")
    }

    /// List current pool summaries for introspection.
    pub async fn current_pools(&self) -> Vec<ResourceSummary> {
        unimplemented!("TAS-374: ResourcePoolManager::current_pools")
    }
}
```

**Step 4: Wire modules into `src/lib.rs`**

```rust
//! Runtime adapters bridging tasker-grammar operation traits to tasker-secure
//! resource handles.

pub mod pool_manager;
```

**Step 5: Verify compilation**

Run: `cargo check -p tasker-runtime --all-features`
Expected: Compiles with no errors

**Step 6: Commit**

```bash
git add crates/tasker-runtime/src/pool_manager/
git commit -m "feat(TAS-373): add pool manager types and ResourcePoolManager scaffold"
```

---

### Task 4: Adapter registry and feature-gated adapter stubs

**Files:**
- Create: `crates/tasker-runtime/src/adapters/mod.rs`
- Create: `crates/tasker-runtime/src/adapters/postgres.rs`
- Create: `crates/tasker-runtime/src/adapters/http.rs`

**Step 1: Create `crates/tasker-runtime/src/adapters/mod.rs`**

```rust
//! Adapter registry and resource-specific adapter implementations.
//!
//! Each adapter wraps a `tasker_secure::ResourceHandle` and implements
//! the corresponding grammar operation trait (`PersistableResource`,
//! `AcquirableResource`, `EmittableResource`).

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "http")]
pub mod http;

use std::sync::Arc;

use tasker_grammar::operations::{
    AcquirableResource, EmittableResource, PersistableResource, ResourceOperationError,
};
use tasker_secure::ResourceHandle;

/// Maps resource handles to the appropriate adapter implementation.
///
/// Registered at worker startup with available adapter factories.
/// When the `RuntimeOperationProvider` needs an operation trait object,
/// it asks the registry to wrap a handle in the right adapter.
#[derive(Debug)]
pub struct AdapterRegistry {
    // Internal adapter factory registrations will be added in TAS-375.
}

impl AdapterRegistry {
    /// Create an empty adapter registry.
    pub fn new() -> Self {
        Self {}
    }

    /// Wrap a resource handle as a `PersistableResource`.
    ///
    /// Returns an error if no adapter is registered for the handle's resource type.
    pub fn as_persistable(
        &self,
        _handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> {
        unimplemented!("TAS-375: AdapterRegistry::as_persistable")
    }

    /// Wrap a resource handle as an `AcquirableResource`.
    ///
    /// Returns an error if no adapter is registered for the handle's resource type.
    pub fn as_acquirable(
        &self,
        _handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError> {
        unimplemented!("TAS-375: AdapterRegistry::as_acquirable")
    }

    /// Wrap a resource handle as an `EmittableResource`.
    ///
    /// Returns an error if no adapter is registered for the handle's resource type.
    pub fn as_emittable(
        &self,
        _handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn EmittableResource>, ResourceOperationError> {
        unimplemented!("TAS-375: AdapterRegistry::as_emittable")
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 2: Create `crates/tasker-runtime/src/adapters/postgres.rs`**

```rust
//! PostgreSQL adapters for persist and acquire operations.
//!
//! Wraps `tasker_secure::resource::postgres::PostgresHandle` and implements
//! `PersistableResource` (SQL INSERT/UPSERT) and `AcquirableResource` (SQL SELECT).

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::{
    AcquireConstraints, AcquireResult, AcquirableResource, PersistConstraints, PersistResult,
    PersistableResource, ResourceOperationError,
};
use tasker_secure::resource::postgres::PostgresHandle;

/// Adapts a `PostgresHandle` for structured write operations.
#[derive(Debug)]
pub struct PostgresPersistAdapter {
    handle: Arc<PostgresHandle>,
}

impl PostgresPersistAdapter {
    /// Create a new persist adapter wrapping the given handle.
    pub fn new(handle: Arc<PostgresHandle>) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl PersistableResource for PostgresPersistAdapter {
    async fn persist(
        &self,
        _entity: &str,
        _data: serde_json::Value,
        _constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        unimplemented!("TAS-375: PostgresPersistAdapter::persist")
    }
}

/// Adapts a `PostgresHandle` for structured read operations.
#[derive(Debug)]
pub struct PostgresAcquireAdapter {
    handle: Arc<PostgresHandle>,
}

impl PostgresAcquireAdapter {
    /// Create a new acquire adapter wrapping the given handle.
    pub fn new(handle: Arc<PostgresHandle>) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl AcquirableResource for PostgresAcquireAdapter {
    async fn acquire(
        &self,
        _entity: &str,
        _params: serde_json::Value,
        _constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        unimplemented!("TAS-375: PostgresAcquireAdapter::acquire")
    }
}
```

**Step 3: Create `crates/tasker-runtime/src/adapters/http.rs`**

```rust
//! HTTP adapters for persist, acquire, and emit operations.
//!
//! Wraps `tasker_secure::resource::http::HttpHandle` and implements
//! `PersistableResource` (POST/PUT), `AcquirableResource` (GET),
//! and `EmittableResource` (POST webhook).

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::{
    AcquireConstraints, AcquireResult, AcquirableResource, EmitMetadata, EmitResult,
    EmittableResource, PersistConstraints, PersistResult, PersistableResource,
    ResourceOperationError,
};
use tasker_secure::resource::http::HttpHandle;

/// Adapts an `HttpHandle` for structured write operations (POST/PUT).
#[derive(Debug)]
pub struct HttpPersistAdapter {
    handle: Arc<HttpHandle>,
}

impl HttpPersistAdapter {
    /// Create a new persist adapter wrapping the given handle.
    pub fn new(handle: Arc<HttpHandle>) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl PersistableResource for HttpPersistAdapter {
    async fn persist(
        &self,
        _entity: &str,
        _data: serde_json::Value,
        _constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        unimplemented!("TAS-375: HttpPersistAdapter::persist")
    }
}

/// Adapts an `HttpHandle` for structured read operations (GET).
#[derive(Debug)]
pub struct HttpAcquireAdapter {
    handle: Arc<HttpHandle>,
}

impl HttpAcquireAdapter {
    /// Create a new acquire adapter wrapping the given handle.
    pub fn new(handle: Arc<HttpHandle>) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl AcquirableResource for HttpAcquireAdapter {
    async fn acquire(
        &self,
        _entity: &str,
        _params: serde_json::Value,
        _constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        unimplemented!("TAS-375: HttpAcquireAdapter::acquire")
    }
}

/// Adapts an `HttpHandle` for event emission (POST webhook).
#[derive(Debug)]
pub struct HttpEmitAdapter {
    handle: Arc<HttpHandle>,
}

impl HttpEmitAdapter {
    /// Create a new emit adapter wrapping the given handle.
    pub fn new(handle: Arc<HttpHandle>) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl EmittableResource for HttpEmitAdapter {
    async fn emit(
        &self,
        _topic: &str,
        _payload: serde_json::Value,
        _metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError> {
        unimplemented!("TAS-375: HttpEmitAdapter::emit")
    }
}
```

**Step 4: Wire adapters into `src/lib.rs`**

Add to `src/lib.rs`:

```rust
pub mod adapters;
```

**Step 5: Verify compilation with all features**

Run: `cargo check -p tasker-runtime --all-features`
Expected: Compiles with no errors

**Step 6: Verify compilation without features (adapter modules excluded)**

Run: `cargo check -p tasker-runtime`
Expected: Compiles with no errors (postgres.rs and http.rs not compiled)

**Step 7: Commit**

```bash
git add crates/tasker-runtime/src/adapters/
git commit -m "feat(TAS-373): add adapter registry and feature-gated adapter stubs"
```

---

### Task 5: Sources — ResourceDefinitionSource trait and stubs

**Files:**
- Create: `crates/tasker-runtime/src/sources/mod.rs`
- Create: `crates/tasker-runtime/src/sources/static_config.rs`
- Create: `crates/tasker-runtime/src/sources/sops.rs`

**Step 1: Create `crates/tasker-runtime/src/sources/mod.rs`**

```rust
//! Resource definition sources for runtime resource resolution.
//!
//! Provides the `ResourceDefinitionSource` trait and implementations
//! for resolving resource definitions from configuration files,
//! encrypted SOPS files, or other backends.

pub mod static_config;

#[cfg(feature = "sops")]
pub mod sops;

use async_trait::async_trait;

use tasker_secure::ResourceDefinition;

/// Event emitted when a resource definition changes at runtime.
#[derive(Debug, Clone)]
pub enum ResourceDefinitionEvent {
    /// A new resource definition was added.
    Added {
        name: String,
        definition: ResourceDefinition,
    },
    /// An existing resource definition was updated.
    Updated {
        name: String,
        definition: ResourceDefinition,
    },
    /// A resource definition was removed.
    Removed { name: String },
}

/// A source of resource definitions that can be queried at runtime.
///
/// Implementations resolve named resource definitions from various backends:
/// static configuration files, SOPS-encrypted files, remote config services, etc.
#[async_trait]
pub trait ResourceDefinitionSource: Send + Sync + std::fmt::Debug {
    /// Resolve a resource definition by name.
    ///
    /// Returns `None` if the resource is not defined in this source.
    async fn resolve(&self, name: &str) -> Option<ResourceDefinition>;

    /// List all resource names known to this source.
    async fn list_names(&self) -> Vec<String>;
}
```

**Step 2: Create `crates/tasker-runtime/src/sources/static_config.rs`**

```rust
//! Static configuration source for resource definitions.
//!
//! Reads resource definitions from worker.toml `[[resources]]` sections.

use async_trait::async_trait;

use tasker_secure::ResourceDefinition;

use super::ResourceDefinitionSource;

/// Resolves resource definitions from static configuration (worker.toml).
///
/// Loaded once at startup. Does not watch for changes.
#[derive(Debug)]
pub struct StaticConfigSource {
    // Resource definitions will be stored here in TAS-376.
}

impl StaticConfigSource {
    /// Create a new static config source from a list of definitions.
    pub fn new(_definitions: Vec<ResourceDefinition>) -> Self {
        unimplemented!("TAS-376: StaticConfigSource::new")
    }
}

#[async_trait]
impl ResourceDefinitionSource for StaticConfigSource {
    async fn resolve(&self, _name: &str) -> Option<ResourceDefinition> {
        unimplemented!("TAS-376: StaticConfigSource::resolve")
    }

    async fn list_names(&self) -> Vec<String> {
        unimplemented!("TAS-376: StaticConfigSource::list_names")
    }
}
```

**Step 3: Create `crates/tasker-runtime/src/sources/sops.rs`**

```rust
//! SOPS-encrypted file watcher for dynamic resource definitions.
//!
//! Watches a mounted volume for `.sops.yaml` or `.sops.json` files
//! and decrypts them to resolve resource definitions at runtime.

use async_trait::async_trait;

use tasker_secure::ResourceDefinition;

use super::ResourceDefinitionSource;

/// Watches SOPS-encrypted files for resource definitions.
///
/// Decrypts files on demand using the SOPS integration from tasker-secure.
#[derive(Debug)]
pub struct SopsFileWatcher {
    // File watching and decryption state will be added in TAS-376.
}

impl SopsFileWatcher {
    /// Create a new SOPS file watcher for the given directory.
    pub fn new(_watch_dir: std::path::PathBuf) -> Self {
        unimplemented!("TAS-376: SopsFileWatcher::new")
    }
}

#[async_trait]
impl ResourceDefinitionSource for SopsFileWatcher {
    async fn resolve(&self, _name: &str) -> Option<ResourceDefinition> {
        unimplemented!("TAS-376: SopsFileWatcher::resolve")
    }

    async fn list_names(&self) -> Vec<String> {
        unimplemented!("TAS-376: SopsFileWatcher::list_names")
    }
}
```

**Step 4: Wire sources into `src/lib.rs`**

Add to `src/lib.rs`:

```rust
pub mod sources;
```

**Step 5: Verify compilation**

Run: `cargo check -p tasker-runtime --all-features`
Expected: Compiles with no errors

**Step 6: Commit**

```bash
git add crates/tasker-runtime/src/sources/
git commit -m "feat(TAS-373): add ResourceDefinitionSource trait and source stubs"
```

---

### Task 6: RuntimeOperationProvider and context placeholder

**Files:**
- Create: `crates/tasker-runtime/src/provider.rs`
- Create: `crates/tasker-runtime/src/context/mod.rs`

**Step 1: Create `crates/tasker-runtime/src/provider.rs`**

```rust
//! `RuntimeOperationProvider` — the production implementation of
//! `tasker_grammar::operations::OperationProvider`.
//!
//! Bridges the pool manager and adapter registry to provide grammar
//! capability executors with their operation trait objects.

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::{
    AcquirableResource, EmittableResource, OperationProvider, PersistableResource,
    ResourceOperationError,
};

use crate::adapters::AdapterRegistry;
use crate::pool_manager::ResourcePoolManager;

/// Production implementation of `OperationProvider`.
///
/// When a grammar capability executor calls `get_persistable("orders-db")`,
/// this provider:
/// 1. Asks the `ResourcePoolManager` to get or initialize the handle
/// 2. Asks the `AdapterRegistry` to wrap the handle in the right adapter
/// 3. Returns the adapter as `Arc<dyn PersistableResource>`
///
/// The executor never sees handles, pools, or adapters — just the
/// operation trait it tested against `InMemoryOperations`.
#[derive(Debug)]
pub struct RuntimeOperationProvider {
    pool_manager: Arc<ResourcePoolManager>,
    adapter_registry: Arc<AdapterRegistry>,
}

impl RuntimeOperationProvider {
    /// Create a new runtime operation provider.
    pub fn new(
        pool_manager: Arc<ResourcePoolManager>,
        adapter_registry: Arc<AdapterRegistry>,
    ) -> Self {
        Self {
            pool_manager,
            adapter_registry,
        }
    }
}

#[async_trait]
impl OperationProvider for RuntimeOperationProvider {
    async fn get_persistable(
        &self,
        _resource_ref: &str,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> {
        unimplemented!("TAS-377: RuntimeOperationProvider::get_persistable")
    }

    async fn get_acquirable(
        &self,
        _resource_ref: &str,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError> {
        unimplemented!("TAS-377: RuntimeOperationProvider::get_acquirable")
    }

    async fn get_emittable(
        &self,
        _resource_ref: &str,
    ) -> Result<Arc<dyn EmittableResource>, ResourceOperationError> {
        unimplemented!("TAS-377: RuntimeOperationProvider::get_emittable")
    }
}
```

**Step 2: Create `crates/tasker-runtime/src/context/mod.rs`**

```rust
//! Composition execution context (Phase 3B).
//!
//! `CompositionExecutionContext` will wrap a `StepContext` (from tasker-worker)
//! with a `RuntimeOperationProvider`, providing grammar capability executors
//! with both step metadata and operation trait objects.
//!
//! This module is a placeholder — implementation comes in Phase 3B when
//! tasker-runtime gains a dependency on tasker-worker.
```

**Step 3: Wire provider and context into `src/lib.rs`**

Update `src/lib.rs` to its final form:

```rust
//! Runtime adapters bridging tasker-grammar operation traits to tasker-secure
//! resource handles.
//!
//! # Crate topology
//!
//! ```text
//! tasker-secure ←── tasker-runtime ──→ tasker-grammar
//! ```
//!
//! tasker-runtime depends on both but neither depends on it.
//!
//! # Module structure
//!
//! - [`adapters`] — `AdapterRegistry` and resource-specific adapters that
//!   implement grammar operation traits (`PersistableResource`, etc.) by
//!   wrapping tasker-secure handles. Feature-gated: `postgres`, `http`.
//! - [`pool_manager`] — `ResourcePoolManager` wrapping `ResourceRegistry`
//!   with lifecycle management, eviction, and admission control.
//! - [`sources`] — `ResourceDefinitionSource` trait and implementations for
//!   resolving resource definitions from configuration or encrypted files.
//! - [`provider`] — `RuntimeOperationProvider` implementing the grammar's
//!   `OperationProvider` trait by bridging pool manager + adapter registry.
//! - [`context`] — `CompositionExecutionContext` placeholder (Phase 3B).

pub mod adapters;
pub mod context;
pub mod pool_manager;
pub mod provider;
pub mod sources;

// Re-export primary types for convenience.
pub use adapters::AdapterRegistry;
pub use pool_manager::{PoolManagerConfig, ResourcePoolManager};
pub use provider::RuntimeOperationProvider;
pub use sources::ResourceDefinitionSource;
```

**Step 4: Verify full compilation**

Run: `cargo check -p tasker-runtime --all-features`
Expected: Compiles with no errors

**Step 5: Verify default features (no postgres/http)**

Run: `cargo check -p tasker-runtime`
Expected: Compiles with no errors

**Step 6: Commit**

```bash
git add crates/tasker-runtime/src/provider.rs crates/tasker-runtime/src/context/ crates/tasker-runtime/src/lib.rs
git commit -m "feat(TAS-373): add RuntimeOperationProvider and context placeholder"
```

---

### Task 7: CI scope detection update

**Files:**
- Modify: `.github/workflows/` — CI files that detect crate changes for conditional builds

**Step 1: Check if CI scope detection needs updating**

Look at the TAS-379 commit (`8e19107c`) for the pattern used when grammar/secure were added. The same pattern should include `tasker-runtime`.

Run: `grep -r "tasker-grammar\|tasker-secure" .github/workflows/ --include="*.yml" | head -20`

Review the output and add `tasker-runtime` alongside `tasker-grammar` and `tasker-secure` in any path filter or scope detection logic. These crates don't need FFI worker builds.

**Step 2: Update CI scope detection**

Apply the same pattern as TAS-379 — add `crates/tasker-runtime/**` to the list of paths that skip FFI worker builds but trigger Rust-only CI.

**Step 3: Verify CI config is valid**

Run: `gh workflow list` to confirm workflows are visible and parseable.

**Step 4: Commit**

```bash
git add .github/
git commit -m "ci(TAS-373): add tasker-runtime to CI scope detection"
```

---

### Task 8: Final verification and workspace check

**Step 1: Full workspace check**

Run: `cargo check --all-features --workspace`
Expected: Entire workspace compiles including tasker-runtime

**Step 2: Clippy**

Run: `cargo clippy -p tasker-runtime --all-features -- -D warnings`
Expected: Zero warnings

**Step 3: Format check**

Run: `cargo fmt -p tasker-runtime -- --check`
Expected: No formatting issues

**Step 4: Verify crate structure**

Run: `find crates/tasker-runtime -type f | sort`
Expected:
```
crates/tasker-runtime/Cargo.toml
crates/tasker-runtime/Makefile.toml
crates/tasker-runtime/src/adapters/http.rs
crates/tasker-runtime/src/adapters/mod.rs
crates/tasker-runtime/src/adapters/postgres.rs
crates/tasker-runtime/src/context/mod.rs
crates/tasker-runtime/src/lib.rs
crates/tasker-runtime/src/pool_manager/lifecycle.rs
crates/tasker-runtime/src/pool_manager/metrics.rs
crates/tasker-runtime/src/pool_manager/mod.rs
crates/tasker-runtime/src/provider.rs
crates/tasker-runtime/src/sources/mod.rs
crates/tasker-runtime/src/sources/sops.rs
crates/tasker-runtime/src/sources/static_config.rs
```
