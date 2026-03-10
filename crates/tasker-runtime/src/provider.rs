//! `RuntimeOperationProvider` — the production implementation of
//! `tasker_grammar::operations::OperationProvider`.
//!
//! Bridges the pool manager and adapter registry to provide grammar
//! capability executors with their operation trait objects.

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::{
    AcquirableResource, EmittableResource, OperationProvider, PersistableResource,
    ResourceOperationError,
};

use crate::adapters::AdapterRegistry;
use crate::pool_manager::ResourcePoolManager;

/// Production implementation of `OperationProvider`.
///
/// When a grammar capability executor calls `get_persistable("orders-db")`,
/// this provider:
/// 1. Asks the `ResourcePoolManager` to get or initialize the handle
/// 2. Asks the `AdapterRegistry` to wrap the handle in the right adapter
/// 3. Returns the adapter as `Arc<dyn PersistableResource>`
///
/// The executor never sees handles, pools, or adapters — just the
/// operation trait it tested against `InMemoryOperations`.
#[derive(Debug)]
pub struct RuntimeOperationProvider {
    #[expect(dead_code, reason = "used in TAS-377 implementation")]
    pool_manager: Arc<ResourcePoolManager>,
    #[expect(dead_code, reason = "used in TAS-377 implementation")]
    adapter_registry: Arc<AdapterRegistry>,
}

impl RuntimeOperationProvider {
    /// Create a new runtime operation provider.
    pub fn new(
        pool_manager: Arc<ResourcePoolManager>,
        adapter_registry: Arc<AdapterRegistry>,
    ) -> Self {
        Self {
            pool_manager,
            adapter_registry,
        }
    }
}

#[async_trait]
impl OperationProvider for RuntimeOperationProvider {
    async fn get_persistable(
        &self,
        _resource_ref: &str,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> {
        unimplemented!("TAS-377: RuntimeOperationProvider::get_persistable")
    }

    async fn get_acquirable(
        &self,
        _resource_ref: &str,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError> {
        unimplemented!("TAS-377: RuntimeOperationProvider::get_acquirable")
    }

    async fn get_emittable(
        &self,
        _resource_ref: &str,
    ) -> Result<Arc<dyn EmittableResource>, ResourceOperationError> {
        unimplemented!("TAS-377: RuntimeOperationProvider::get_emittable")
    }
}
