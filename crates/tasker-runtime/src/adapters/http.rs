//! HTTP adapters for persist, acquire, and emit operations.
//!
//! Wraps `tasker_secure::resource::http::HttpHandle` and implements
//! `PersistableResource` (POST/PUT), `AcquirableResource` (GET),
//! and `EmittableResource` (POST webhook).

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::{
    AcquirableResource, AcquireConstraints, AcquireResult, EmitMetadata, EmitResult,
    EmittableResource, PersistConstraints, PersistResult, PersistableResource,
    ResourceOperationError,
};
use tasker_secure::resource::http::HttpHandle;

/// Adapts an `HttpHandle` for structured write operations (POST/PUT).
#[derive(Debug)]
pub struct HttpPersistAdapter {
    handle: Arc<HttpHandle>,
}

impl HttpPersistAdapter {
    /// Create a new persist adapter wrapping the given handle.
    pub fn new(handle: Arc<HttpHandle>) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl PersistableResource for HttpPersistAdapter {
    async fn persist(
        &self,
        _entity: &str,
        _data: serde_json::Value,
        _constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        unimplemented!("TAS-375: HttpPersistAdapter::persist")
    }
}

/// Adapts an `HttpHandle` for structured read operations (GET).
#[derive(Debug)]
pub struct HttpAcquireAdapter {
    handle: Arc<HttpHandle>,
}

impl HttpAcquireAdapter {
    /// Create a new acquire adapter wrapping the given handle.
    pub fn new(handle: Arc<HttpHandle>) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl AcquirableResource for HttpAcquireAdapter {
    async fn acquire(
        &self,
        _entity: &str,
        _params: serde_json::Value,
        _constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        unimplemented!("TAS-375: HttpAcquireAdapter::acquire")
    }
}

/// Adapts an `HttpHandle` for event emission (POST webhook).
#[derive(Debug)]
pub struct HttpEmitAdapter {
    handle: Arc<HttpHandle>,
}

impl HttpEmitAdapter {
    /// Create a new emit adapter wrapping the given handle.
    pub fn new(handle: Arc<HttpHandle>) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl EmittableResource for HttpEmitAdapter {
    async fn emit(
        &self,
        _topic: &str,
        _payload: serde_json::Value,
        _metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError> {
        unimplemented!("TAS-375: HttpEmitAdapter::emit")
    }
}
