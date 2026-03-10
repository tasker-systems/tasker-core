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
