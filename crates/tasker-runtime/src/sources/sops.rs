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
