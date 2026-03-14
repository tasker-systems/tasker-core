//! Runtime adapters bridging tasker-grammar operation traits to tasker-secure
//! resource handles.
//!
//! # Crate topology
//!
//! ```text
//! tasker-secure <── tasker-runtime ──> tasker-grammar
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
mod cache;
pub mod context;
pub mod pool_manager;
pub mod provider;
pub mod sources;

// Re-export primary types for convenience.
pub use adapters::AdapterRegistry;
pub use pool_manager::{
    PoolManagerConfig, PoolManagerMetrics, PoolManagerMetricsSnapshot, ResourceAccessMetrics,
    ResourcePoolManager,
};
pub use provider::RuntimeOperationProvider;
pub use sources::{ResourceDefinitionSource, ResourceHandleResolver};
