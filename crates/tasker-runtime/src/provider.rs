//! `RuntimeOperationProvider` — the production implementation of
//! `tasker_grammar::operations::OperationProvider`.
//!
//! Bridges the pool manager and adapter registry to provide grammar
//! capability executors with their operation trait objects.
//!
//! **Lifetime model:** One `RuntimeOperationProvider` per composition execution.
//! Created when a worker picks up a composition, dropped when execution completes.

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::{
    AcquirableResource, EmittableResource, OperationProvider, PersistableResource,
    ResourceOperationError,
};
use tasker_secure::ResourceError;

use crate::adapters::AdapterRegistry;
use crate::cache::AdapterCache;
use crate::pool_manager::ResourcePoolManager;
use crate::sources::ResourceHandleResolver;

/// Production implementation of `OperationProvider`.
///
/// When a grammar capability executor calls `get_persistable("orders-db")`,
/// this provider:
/// 1. Checks the per-composition [`AdapterCache`] for a cached adapter
/// 2. Asks the [`ResourcePoolManager`] to get or initialize the handle
/// 3. Asks the [`AdapterRegistry`] to wrap the handle in the right adapter
/// 4. Caches and returns the adapter as `Arc<dyn PersistableResource>`
///
/// The executor never sees handles, pools, or adapters — just the
/// operation trait it tested against `InMemoryOperations`.
#[derive(Debug)]
pub struct RuntimeOperationProvider {
    pool_manager: Arc<ResourcePoolManager>,
    adapter_registry: Arc<AdapterRegistry>,
    source: Option<Arc<dyn ResourceHandleResolver>>,
    cache: AdapterCache,
}

impl RuntimeOperationProvider {
    /// Create a new provider without a resource handle resolver.
    ///
    /// Resources must be pre-registered in the pool manager before
    /// composition execution starts.
    pub fn new(
        pool_manager: Arc<ResourcePoolManager>,
        adapter_registry: Arc<AdapterRegistry>,
    ) -> Self {
        Self {
            pool_manager,
            adapter_registry,
            source: None,
            cache: AdapterCache::new(),
        }
    }

    /// Create a new provider with a resource handle resolver for lazy initialization.
    ///
    /// When a resource is not found in the pool manager, the resolver will be
    /// called to initialize it on demand.
    pub fn with_source(
        pool_manager: Arc<ResourcePoolManager>,
        adapter_registry: Arc<AdapterRegistry>,
        source: Arc<dyn ResourceHandleResolver>,
    ) -> Self {
        Self {
            pool_manager,
            adapter_registry,
            source: Some(source),
            cache: AdapterCache::new(),
        }
    }
}

#[async_trait]
impl OperationProvider for RuntimeOperationProvider {
    async fn get_persistable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> {
        if let Some(adapter) = self.cache.get_persistable(resource_ref).await {
            return Ok(adapter);
        }

        let handle = self
            .pool_manager
            .get_or_initialize(resource_ref, self.source.as_deref())
            .await
            .map_err(map_resource_error)?;

        let adapter = self.adapter_registry.as_persistable(handle)?;
        self.cache
            .insert_persistable(resource_ref.to_string(), adapter.clone())
            .await;
        Ok(adapter)
    }

    async fn get_acquirable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError> {
        if let Some(adapter) = self.cache.get_acquirable(resource_ref).await {
            return Ok(adapter);
        }

        let handle = self
            .pool_manager
            .get_or_initialize(resource_ref, self.source.as_deref())
            .await
            .map_err(map_resource_error)?;

        let adapter = self.adapter_registry.as_acquirable(handle)?;
        self.cache
            .insert_acquirable(resource_ref.to_string(), adapter.clone())
            .await;
        Ok(adapter)
    }

    async fn get_emittable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn EmittableResource>, ResourceOperationError> {
        if let Some(adapter) = self.cache.get_emittable(resource_ref).await {
            return Ok(adapter);
        }

        let handle = self
            .pool_manager
            .get_or_initialize(resource_ref, self.source.as_deref())
            .await
            .map_err(map_resource_error)?;

        let adapter = self.adapter_registry.as_emittable(handle)?;
        self.cache
            .insert_emittable(resource_ref.to_string(), adapter.clone())
            .await;
        Ok(adapter)
    }
}

/// Map a `ResourceError` (tasker-secure domain) to a `ResourceOperationError`
/// (tasker-grammar domain).
pub(crate) fn map_resource_error(err: ResourceError) -> ResourceOperationError {
    match err {
        ResourceError::ResourceNotFound { name } => {
            ResourceOperationError::EntityNotFound { entity: name }
        }
        ResourceError::InitializationFailed { name, message } => {
            ResourceOperationError::Unavailable {
                message: format!("Resource '{name}' initialization failed: {message}"),
            }
        }
        ResourceError::HealthCheckFailed { name, message } => ResourceOperationError::Unavailable {
            message: format!("Resource '{name}' health check failed: {message}"),
        },
        ResourceError::CredentialRefreshFailed { name, message } => {
            ResourceOperationError::Unavailable {
                message: format!("Resource '{name}' credential refresh failed: {message}"),
            }
        }
        ResourceError::WrongResourceType {
            name,
            expected,
            actual,
        } => ResourceOperationError::ValidationFailed {
            message: format!("Resource '{name}' type mismatch: expected {expected}, got {actual}"),
        },
        ResourceError::MissingConfigKey { resource, key } => {
            ResourceOperationError::ValidationFailed {
                message: format!("Resource '{resource}' missing required config key: '{key}'"),
            }
        }
        ResourceError::SecretResolution { resource, source } => {
            ResourceOperationError::Unavailable {
                message: format!("Resource '{resource}' secret resolution failed: {source}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_resource_not_found() {
        let err = ResourceError::ResourceNotFound {
            name: "db1".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(
            matches!(mapped, ResourceOperationError::EntityNotFound { entity } if entity == "db1")
        );
    }

    #[test]
    fn map_initialization_failed() {
        let err = ResourceError::InitializationFailed {
            name: "db1".to_string(),
            message: "connection refused".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(
            matches!(mapped, ResourceOperationError::Unavailable { message } if message.contains("connection refused"))
        );
    }

    #[test]
    fn map_wrong_resource_type() {
        let err = ResourceError::WrongResourceType {
            name: "db1".to_string(),
            expected: "Postgres".to_string(),
            actual: "Http".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(
            matches!(mapped, ResourceOperationError::ValidationFailed { message } if message.contains("type mismatch"))
        );
    }

    #[test]
    fn map_missing_config_key() {
        let err = ResourceError::MissingConfigKey {
            resource: "db1".to_string(),
            key: "host".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(
            matches!(mapped, ResourceOperationError::ValidationFailed { message } if message.contains("host"))
        );
    }

    #[test]
    fn map_health_check_failed() {
        let err = ResourceError::HealthCheckFailed {
            name: "db1".to_string(),
            message: "timeout".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(
            matches!(mapped, ResourceOperationError::Unavailable { message } if message.contains("timeout"))
        );
    }

    #[test]
    fn map_credential_refresh_failed() {
        let err = ResourceError::CredentialRefreshFailed {
            name: "db1".to_string(),
            message: "expired".to_string(),
        };
        let mapped = map_resource_error(err);
        assert!(
            matches!(mapped, ResourceOperationError::Unavailable { message } if message.contains("expired"))
        );
    }

    #[test]
    fn map_secret_resolution() {
        let err = ResourceError::SecretResolution {
            resource: "db1".to_string(),
            source: tasker_secure::SecretsError::ProviderUnavailable {
                message: "vault sealed".to_string(),
            },
        };
        let mapped = map_resource_error(err);
        assert!(
            matches!(mapped, ResourceOperationError::Unavailable { message } if message.contains("vault sealed"))
        );
    }
}
