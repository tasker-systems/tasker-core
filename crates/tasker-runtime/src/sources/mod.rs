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
    async fn resolve(&self, name: &str) -> Option<ResourceDefinition>;

    /// List all resource names known to this source.
    async fn list_names(&self) -> Vec<String>;
}
