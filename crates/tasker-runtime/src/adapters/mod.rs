//! Adapter registry and resource-specific adapter implementations.
//!
//! Each adapter wraps a `tasker_secure::ResourceHandle` and implements
//! the corresponding grammar operation trait (`PersistableResource`,
//! `AcquirableResource`, `EmittableResource`).

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "http")]
pub mod http;

#[cfg(feature = "postgres")]
pub mod sql_gen;

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
    pub fn as_persistable(
        &self,
        _handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> {
        unimplemented!("TAS-375: AdapterRegistry::as_persistable")
    }

    /// Wrap a resource handle as an `AcquirableResource`.
    pub fn as_acquirable(
        &self,
        _handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError> {
        unimplemented!("TAS-375: AdapterRegistry::as_acquirable")
    }

    /// Wrap a resource handle as an `EmittableResource`.
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
