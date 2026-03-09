//! In-memory test implementations of operation traits.
//!
//! Provides fixture data for acquire operations and capture lists
//! for persist and emit operations. Used by capability executor tests
//! to verify the full orchestration pipeline with zero I/O.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::error::ResourceOperationError;
use super::traits::*;
use super::types::*;

/// Captured persist operation for test assertions.
#[derive(Debug, Clone)]
pub struct CapturedPersist {
    pub entity: String,
    pub data: serde_json::Value,
    pub constraints: PersistConstraints,
}

/// Captured emit operation for test assertions.
#[derive(Debug, Clone)]
pub struct CapturedEmit {
    pub topic: String,
    pub payload: serde_json::Value,
    pub metadata: EmitMetadata,
}

/// In-memory implementation of all operation traits for grammar testing.
///
/// Provides canned responses for acquire operations and capture lists
/// for persist and emit operations.
#[derive(Debug, Clone)]
pub struct InMemoryOperations {
    /// Canned responses for acquire operations, keyed by entity name
    fixture_data: HashMap<String, Vec<serde_json::Value>>,
    /// Captured persist operations for test assertions
    persisted: Arc<Mutex<Vec<CapturedPersist>>>,
    /// Captured emit operations for test assertions
    emitted: Arc<Mutex<Vec<CapturedEmit>>>,
}

impl InMemoryOperations {
    /// Create a new `InMemoryOperations` with the given fixture data.
    pub fn new(fixture_data: HashMap<String, Vec<serde_json::Value>>) -> Self {
        Self {
            fixture_data,
            persisted: Arc::new(Mutex::new(Vec::new())),
            emitted: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get all captured persist operations.
    pub async fn captured_persists(&self) -> Vec<CapturedPersist> {
        self.persisted.lock().await.clone()
    }

    /// Get all captured emit operations.
    pub async fn captured_emits(&self) -> Vec<CapturedEmit> {
        self.emitted.lock().await.clone()
    }
}

#[async_trait]
impl PersistableResource for InMemoryOperations {
    async fn persist(
        &self,
        entity: &str,
        data: serde_json::Value,
        constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        self.persisted.lock().await.push(CapturedPersist {
            entity: entity.to_string(),
            data: data.clone(),
            constraints: constraints.clone(),
        });
        Ok(PersistResult {
            data,
            affected_count: Some(1),
        })
    }
}

#[async_trait]
impl AcquirableResource for InMemoryOperations {
    async fn acquire(
        &self,
        entity: &str,
        _params: serde_json::Value,
        constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        let records =
            self.fixture_data
                .get(entity)
                .ok_or(ResourceOperationError::EntityNotFound {
                    entity: entity.to_string(),
                })?;
        let total_count = Some(records.len() as u64);
        let data = serde_json::Value::Array(
            records
                .iter()
                .take(constraints.limit.unwrap_or(u64::MAX) as usize)
                .cloned()
                .collect(),
        );
        Ok(AcquireResult { data, total_count })
    }
}

#[async_trait]
impl EmittableResource for InMemoryOperations {
    async fn emit(
        &self,
        topic: &str,
        payload: serde_json::Value,
        metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError> {
        self.emitted.lock().await.push(CapturedEmit {
            topic: topic.to_string(),
            payload,
            metadata: metadata.clone(),
        });
        Ok(EmitResult {
            data: serde_json::json!({"message_id": "test-msg-001"}),
            confirmed: true,
        })
    }
}

/// In-memory `OperationProvider` for grammar testing.
///
/// Returns the wrapped `InMemoryOperations` for any resource_ref,
/// enabling executor tests to run without real resources.
#[derive(Debug)]
pub struct InMemoryOperationProvider {
    ops: Arc<InMemoryOperations>,
}

impl InMemoryOperationProvider {
    /// Create a provider wrapping the given operations.
    pub fn new(ops: Arc<InMemoryOperations>) -> Self {
        Self { ops }
    }
}

#[async_trait]
impl OperationProvider for InMemoryOperationProvider {
    async fn get_persistable(
        &self,
        _resource_ref: &str,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> {
        Ok(self.ops.clone() as Arc<dyn PersistableResource>)
    }

    async fn get_acquirable(
        &self,
        _resource_ref: &str,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError> {
        Ok(self.ops.clone() as Arc<dyn AcquirableResource>)
    }

    async fn get_emittable(
        &self,
        _resource_ref: &str,
    ) -> Result<Arc<dyn EmittableResource>, ResourceOperationError> {
        Ok(self.ops.clone() as Arc<dyn EmittableResource>)
    }
}

/// Convenience constructor for test contexts with in-memory operations.
pub fn test_operations_with_fixtures(
    fixtures: HashMap<String, Vec<serde_json::Value>>,
) -> InMemoryOperations {
    InMemoryOperations::new(fixtures)
}
