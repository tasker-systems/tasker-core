//! Resource definition sources for runtime resource resolution.
//!
//! Provides the `ResourceDefinitionSource` trait and implementations
//! for resolving resource definitions from configuration files,
//! encrypted SOPS files, or other backends.

pub mod static_config;

#[cfg(feature = "sops")]
pub mod sops;

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::ResourceOperationError;
use tasker_secure::{ResourceDefinition, ResourceHandle};

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
    async fn resolve(&self, name: &str) -> Option<ResourceDefinition>;

    /// List all resource names known to this source.
    async fn list_names(&self) -> Vec<String>;
}

/// Resolves a resource reference to a live [`ResourceHandle`].
///
/// This is the extension point for TAS-376 (ResourceDefinitionSource implementations).
/// Distinct from [`ResourceDefinitionSource`], which returns configuration descriptors
/// (`ResourceDefinition`). This trait operates at a higher level — given a resource
/// reference string, it returns an initialized, ready-to-use handle.
///
/// In practice, a TAS-376 implementation would use a `ResourceDefinitionSource` internally
/// to look up the definition, then initialize the handle from it.
#[async_trait]
pub trait ResourceHandleResolver: Send + Sync + std::fmt::Debug {
    /// Resolve a resource reference to a live handle.
    async fn resolve(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceOperationError>;
}
