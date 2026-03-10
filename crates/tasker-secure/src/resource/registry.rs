//! Thread-safe registry for managing live infrastructure resource handles.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::error::ResourceError;
use super::handle::ResourceHandle;
use super::types::ResourceSummary;
use crate::secrets::SecretsProvider;

/// A thread-safe registry of live [`ResourceHandle`] instances.
///
/// The registry owns a shared [`SecretsProvider`] that is passed to handles
/// during credential refresh. Handles are stored behind an `Arc` so they can
/// be shared across concurrent tasks.
///
/// # Thread safety
///
/// Uses [`tokio::sync::RwLock`] for the resource map so that `get` and
/// `list_resources` can proceed concurrently, while `register` takes an
/// exclusive write lock.
pub struct ResourceRegistry {
    secrets: Arc<dyn SecretsProvider>,
    resources: RwLock<HashMap<String, Arc<dyn ResourceHandle>>>,
}

impl fmt::Debug for ResourceRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResourceRegistry")
            .field("resource_count", &"<locked>")
            .finish()
    }
}

impl ResourceRegistry {
    /// Create a new, empty registry backed by the given secrets provider.
    pub fn new(secrets: Arc<dyn SecretsProvider>) -> Self {
        Self {
            secrets,
            resources: RwLock::new(HashMap::new()),
        }
    }

    /// Register a resource handle under the given name.
    ///
    /// If a handle with the same name already exists it is replaced.
    pub async fn register(&self, name: &str, handle: Arc<dyn ResourceHandle>) {
        let mut map = self.resources.write().await;
        map.insert(name.to_string(), handle);
    }

    /// Look up a resource handle by name.
    ///
    /// Uses `try_read()` to avoid blocking — returns `None` if the lock is
    /// currently held by a writer **or** if the name is not registered.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ResourceHandle>> {
        let map = self.resources.try_read().ok()?;
        map.get(name).cloned()
    }

    /// Remove a resource handle by name, returning it if it existed.
    ///
    /// Takes an exclusive write lock. The returned handle can still be
    /// used by any code that already holds an `Arc` to it — removal
    /// only prevents future lookups.
    pub async fn remove(&self, name: &str) -> Option<Arc<dyn ResourceHandle>> {
        let mut map = self.resources.write().await;
        map.remove(name)
    }

    /// Return a lightweight summary for every registered resource.
    ///
    /// Uses `try_read()` to avoid blocking. If the lock cannot be acquired
    /// (e.g., a concurrent `register` call) an empty list is returned.
    ///
    /// The `healthy` field defaults to `true` because a synchronous method
    /// cannot call the async `health_check`. Use `refresh_resource` or
    /// call `health_check` on individual handles for accurate status.
    ///
    /// **Security**: summaries contain only name, type, and healthy flag —
    /// never host, port, credentials, or secret paths.
    pub fn list_resources(&self) -> Vec<ResourceSummary> {
        let Some(map) = self.resources.try_read().ok() else {
            return Vec::new();
        };
        map.values()
            .map(|handle| ResourceSummary {
                name: handle.resource_name().to_string(),
                resource_type: handle.resource_type().clone(),
                healthy: true,
            })
            .collect()
    }

    /// Refresh credentials for a single resource by name.
    ///
    /// Looks up the handle, then calls its [`ResourceHandle::refresh_credentials`]
    /// method with the registry's secrets provider.
    pub async fn refresh_resource(&self, name: &str) -> Result<(), ResourceError> {
        let handle = {
            let map = self.resources.read().await;
            map.get(name).cloned()
        };

        match handle {
            Some(h) => h.refresh_credentials(self.secrets.as_ref()).await,
            None => Err(ResourceError::ResourceNotFound {
                name: name.to_string(),
            }),
        }
    }

    /// Access the underlying secrets provider.
    pub fn secrets(&self) -> &dyn SecretsProvider {
        self.secrets.as_ref()
    }
}
