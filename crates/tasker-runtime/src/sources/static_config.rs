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
