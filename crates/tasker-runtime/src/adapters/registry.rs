//! Adapter registry mapping resource types to adapter factories.
//!
//! Uses closure-based factories for extensibility without proliferating
//! named factory traits. The [`AdapterRegistry::standard`] constructor registers
//! built-in adapters; custom types can be added via `register_*` methods.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use tasker_grammar::operations::{
    AcquirableResource, EmittableResource, PersistableResource, ResourceOperationError,
};
use tasker_secure::{ResourceHandle, ResourceType};

type PersistFactory = Box<
    dyn Fn(Arc<dyn ResourceHandle>) -> Result<Arc<dyn PersistableResource>, ResourceOperationError>
        + Send
        + Sync,
>;
type AcquireFactory = Box<
    dyn Fn(Arc<dyn ResourceHandle>) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError>
        + Send
        + Sync,
>;
type EmitFactory = Box<
    dyn Fn(Arc<dyn ResourceHandle>) -> Result<Arc<dyn EmittableResource>, ResourceOperationError>
        + Send
        + Sync,
>;

/// Maps resource handles to the appropriate adapter implementation.
///
/// Registered at worker startup with available adapter factories.
/// When the `RuntimeOperationProvider` needs an operation trait object,
/// it asks the registry to wrap a handle in the right adapter.
pub struct AdapterRegistry {
    persist_factories: HashMap<ResourceType, PersistFactory>,
    acquire_factories: HashMap<ResourceType, AcquireFactory>,
    emit_factories: HashMap<ResourceType, EmitFactory>,
}

impl fmt::Debug for AdapterRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdapterRegistry")
            .field(
                "persist_types",
                &self.persist_factories.keys().collect::<Vec<_>>(),
            )
            .field(
                "acquire_types",
                &self.acquire_factories.keys().collect::<Vec<_>>(),
            )
            .field(
                "emit_types",
                &self.emit_factories.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl AdapterRegistry {
    /// Create an empty adapter registry with no factories registered.
    pub fn new() -> Self {
        Self {
            persist_factories: HashMap::new(),
            acquire_factories: HashMap::new(),
            emit_factories: HashMap::new(),
        }
    }

    /// Create a registry with all built-in adapters registered.
    ///
    /// Registers Postgres adapters (persist, acquire) when the `postgres`
    /// feature is enabled, and HTTP adapters (persist, acquire, emit) when
    /// the `http` feature is enabled.
    pub fn standard() -> Self {
        let mut registry = Self::new();

        #[cfg(feature = "postgres")]
        {
            use super::postgres::{PostgresAcquireAdapter, PostgresPersistAdapter};
            use tasker_secure::ResourceHandleExt;

            registry.register_persist(
                ResourceType::Postgres,
                Box::new(|handle| {
                    handle.as_postgres().ok_or_else(|| {
                        ResourceOperationError::ValidationFailed {
                            message: format!(
                                "Expected Postgres handle, got {:?}",
                                handle.resource_type()
                            ),
                        }
                    })?;
                    Ok(Arc::new(PostgresPersistAdapter::new(handle)))
                }),
            );

            registry.register_acquire(
                ResourceType::Postgres,
                Box::new(|handle| {
                    handle.as_postgres().ok_or_else(|| {
                        ResourceOperationError::ValidationFailed {
                            message: format!(
                                "Expected Postgres handle, got {:?}",
                                handle.resource_type()
                            ),
                        }
                    })?;
                    Ok(Arc::new(PostgresAcquireAdapter::new(handle)))
                }),
            );
        }

        #[cfg(feature = "http")]
        {
            use super::http::{HttpAcquireAdapter, HttpEmitAdapter, HttpPersistAdapter};
            use tasker_secure::ResourceHandleExt;

            registry.register_persist(
                ResourceType::Http,
                Box::new(|handle| {
                    handle
                        .as_http()
                        .ok_or_else(|| ResourceOperationError::ValidationFailed {
                            message: format!(
                                "Expected HTTP handle, got {:?}",
                                handle.resource_type()
                            ),
                        })?;
                    Ok(Arc::new(HttpPersistAdapter::new(handle)))
                }),
            );

            registry.register_acquire(
                ResourceType::Http,
                Box::new(|handle| {
                    handle
                        .as_http()
                        .ok_or_else(|| ResourceOperationError::ValidationFailed {
                            message: format!(
                                "Expected HTTP handle, got {:?}",
                                handle.resource_type()
                            ),
                        })?;
                    Ok(Arc::new(HttpAcquireAdapter::new(handle)))
                }),
            );

            registry.register_emit(
                ResourceType::Http,
                Box::new(|handle| {
                    handle
                        .as_http()
                        .ok_or_else(|| ResourceOperationError::ValidationFailed {
                            message: format!(
                                "Expected HTTP handle, got {:?}",
                                handle.resource_type()
                            ),
                        })?;
                    Ok(Arc::new(HttpEmitAdapter::new(handle)))
                }),
            );
        }

        registry
    }

    /// Register a factory for creating `PersistableResource` adapters from
    /// handles of the given resource type.
    pub fn register_persist(&mut self, resource_type: ResourceType, factory: PersistFactory) {
        self.persist_factories.insert(resource_type, factory);
    }

    /// Register a factory for creating `AcquirableResource` adapters from
    /// handles of the given resource type.
    pub fn register_acquire(&mut self, resource_type: ResourceType, factory: AcquireFactory) {
        self.acquire_factories.insert(resource_type, factory);
    }

    /// Register a factory for creating `EmittableResource` adapters from
    /// handles of the given resource type.
    pub fn register_emit(&mut self, resource_type: ResourceType, factory: EmitFactory) {
        self.emit_factories.insert(resource_type, factory);
    }

    /// Wrap a resource handle as a [`PersistableResource`].
    ///
    /// Returns an error if no persist factory is registered for the handle's
    /// resource type, or if the factory rejects the handle.
    pub fn as_persistable(
        &self,
        handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> {
        let factory = self
            .persist_factories
            .get(handle.resource_type())
            .ok_or_else(|| ResourceOperationError::ValidationFailed {
                message: format!(
                    "No persist adapter registered for resource type '{}'",
                    handle.resource_type()
                ),
            })?;
        factory(handle)
    }

    /// Wrap a resource handle as an [`AcquirableResource`].
    ///
    /// Returns an error if no acquire factory is registered for the handle's
    /// resource type, or if the factory rejects the handle.
    pub fn as_acquirable(
        &self,
        handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError> {
        let factory = self
            .acquire_factories
            .get(handle.resource_type())
            .ok_or_else(|| ResourceOperationError::ValidationFailed {
                message: format!(
                    "No acquire adapter registered for resource type '{}'",
                    handle.resource_type()
                ),
            })?;
        factory(handle)
    }

    /// Wrap a resource handle as an [`EmittableResource`].
    ///
    /// Returns an error if no emit factory is registered for the handle's
    /// resource type, or if the factory rejects the handle.
    pub fn as_emittable(
        &self,
        handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn EmittableResource>, ResourceOperationError> {
        let factory = self
            .emit_factories
            .get(handle.resource_type())
            .ok_or_else(|| ResourceOperationError::ValidationFailed {
                message: format!(
                    "No emit adapter registered for resource type '{}'",
                    handle.resource_type()
                ),
            })?;
        factory(handle)
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
