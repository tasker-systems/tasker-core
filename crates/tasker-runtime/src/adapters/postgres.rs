//! PostgreSQL adapters for persist and acquire operations.
//!
//! Wraps `tasker_secure::resource::postgres::PostgresHandle` and implements
//! `PersistableResource` (SQL INSERT/UPSERT) and `AcquirableResource` (SQL SELECT).

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::{
    AcquirableResource, AcquireConstraints, AcquireResult, PersistConstraints, PersistResult,
    PersistableResource, ResourceOperationError,
};
use tasker_secure::resource::postgres::PostgresHandle;

/// Adapts a `PostgresHandle` for structured write operations.
#[derive(Debug)]
pub struct PostgresPersistAdapter {
    #[expect(dead_code, reason = "used in TAS-375 implementation")]
    handle: Arc<PostgresHandle>,
}

impl PostgresPersistAdapter {
    /// Create a new persist adapter wrapping the given handle.
    pub fn new(handle: Arc<PostgresHandle>) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl PersistableResource for PostgresPersistAdapter {
    async fn persist(
        &self,
        _entity: &str,
        _data: serde_json::Value,
        _constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        unimplemented!("TAS-375: PostgresPersistAdapter::persist")
    }
}

/// Adapts a `PostgresHandle` for structured read operations.
#[derive(Debug)]
pub struct PostgresAcquireAdapter {
    #[expect(dead_code, reason = "used in TAS-375 implementation")]
    handle: Arc<PostgresHandle>,
}

impl PostgresAcquireAdapter {
    /// Create a new acquire adapter wrapping the given handle.
    pub fn new(handle: Arc<PostgresHandle>) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl AcquirableResource for PostgresAcquireAdapter {
    async fn acquire(
        &self,
        _entity: &str,
        _params: serde_json::Value,
        _constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        unimplemented!("TAS-375: PostgresAcquireAdapter::acquire")
    }
}
