//! Operation trait definitions for grammar capability executors.

use std::sync::Arc;

use async_trait::async_trait;

use super::error::ResourceOperationError;
use super::types::*;

/// A resource that can accept structured write operations.
///
/// Grammar capability executors (`PersistExecutor`) call through this trait.
/// Implementations live in tasker-runtime as adapters wrapping tasker-secure
/// handles. Test implementations live here as `InMemoryOperations`.
#[async_trait]
pub trait PersistableResource: Send + Sync {
    /// Execute a write operation.
    async fn persist(
        &self,
        entity: &str,
        data: serde_json::Value,
        constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError>;
}

/// A resource that can serve structured read operations.
///
/// Grammar capability executors (`AcquireExecutor`) call through this trait.
#[async_trait]
pub trait AcquirableResource: Send + Sync {
    /// Execute a read operation.
    async fn acquire(
        &self,
        entity: &str,
        params: serde_json::Value,
        constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError>;
}

/// A resource that can accept event/message publication.
///
/// Grammar capability executors (`EmitExecutor`) call through this trait.
#[async_trait]
pub trait EmittableResource: Send + Sync {
    /// Publish an event or message.
    async fn emit(
        &self,
        topic: &str,
        payload: serde_json::Value,
        metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError>;
}

/// The interface that grammar capability executors use to obtain
/// operation trait objects for named resources.
///
/// Implemented by tasker-runtime's `RuntimeOperationProvider`,
/// which resolves `resource_ref` names through the `ResourcePoolManager`
/// and wraps handles in the appropriate adapters.
///
/// Implemented by `InMemoryOperationProvider` for grammar-level testing.
#[async_trait]
pub trait OperationProvider: Send + Sync {
    /// Get a `PersistableResource` for a named resource.
    async fn get_persistable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError>;

    /// Get an `AcquirableResource` for a named resource.
    async fn get_acquirable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError>;

    /// Get an `EmittableResource` for a named resource.
    async fn get_emittable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn EmittableResource>, ResourceOperationError>;
}
