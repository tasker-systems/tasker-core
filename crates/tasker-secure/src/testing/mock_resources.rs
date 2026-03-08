//! In-memory resource handle and test fixtures for testing.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::resource::{ResourceError, ResourceHandle, ResourceRegistry, ResourceType};
use crate::secrets::SecretsProvider;

use super::InMemorySecretsProvider;

/// An in-memory [`ResourceHandle`] for testing resource registry interactions.
///
/// Provides fixture data for simulating `acquire` operations, and capture
/// vectors for inspecting `persist` and `emit` calls made during tests.
#[derive(Debug)]
pub struct InMemoryResourceHandle {
    name: String,
    resource_type: ResourceType,
    fixture_data: HashMap<String, Value>,
    persisted: Arc<Mutex<Vec<Value>>>,
    emitted: Arc<Mutex<Vec<Value>>>,
}

impl InMemoryResourceHandle {
    /// Create a new handle with no fixture data and empty capture vectors.
    pub fn new(name: &str, resource_type: ResourceType) -> Self {
        Self {
            name: name.to_string(),
            resource_type,
            fixture_data: HashMap::new(),
            persisted: Arc::new(Mutex::new(Vec::new())),
            emitted: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a handle pre-loaded with fixture data for acquire-style tests.
    pub fn with_fixtures(
        name: &str,
        resource_type: ResourceType,
        fixture_data: HashMap<String, Value>,
    ) -> Self {
        Self {
            name: name.to_string(),
            resource_type,
            fixture_data,
            persisted: Arc::new(Mutex::new(Vec::new())),
            emitted: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Look up a fixture value by key (simulates an acquire operation).
    pub fn get_fixture(&self, key: &str) -> Option<&Value> {
        self.fixture_data.get(key)
    }

    /// Record a value as "persisted" for later inspection.
    pub fn capture_persist(&self, value: Value) {
        self.persisted
            .lock()
            .expect("persisted mutex poisoned")
            .push(value);
    }

    /// Record a value as "emitted" for later inspection.
    pub fn capture_emit(&self, value: Value) {
        self.emitted
            .lock()
            .expect("emitted mutex poisoned")
            .push(value);
    }

    /// Return a cloned snapshot of all persisted values.
    pub fn persisted(&self) -> Vec<Value> {
        self.persisted
            .lock()
            .expect("persisted mutex poisoned")
            .clone()
    }

    /// Return a cloned snapshot of all emitted values.
    pub fn emitted(&self) -> Vec<Value> {
        self.emitted.lock().expect("emitted mutex poisoned").clone()
    }

    /// Clear both persisted and emitted capture vectors.
    pub fn clear_captures(&self) {
        self.persisted
            .lock()
            .expect("persisted mutex poisoned")
            .clear();
        self.emitted.lock().expect("emitted mutex poisoned").clear();
    }
}

#[async_trait::async_trait]
impl ResourceHandle for InMemoryResourceHandle {
    fn resource_name(&self) -> &str {
        &self.name
    }

    fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }

    async fn refresh_credentials(
        &self,
        _secrets: &dyn SecretsProvider,
    ) -> Result<(), ResourceError> {
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ResourceError> {
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A description of a resource fixture for use with [`test_registry_with_fixtures`].
#[derive(Debug, Clone)]
pub struct ResourceFixture {
    /// The resource name.
    pub name: String,
    /// The resource type.
    pub resource_type: ResourceType,
    /// Fixture data to pre-load.
    pub data: HashMap<String, Value>,
}

/// Create a [`ResourceRegistry`] populated with [`InMemoryResourceHandle`]s
/// from the given fixtures.
///
/// Uses an empty [`InMemorySecretsProvider`] as the backing secrets provider.
pub async fn test_registry_with_fixtures(fixtures: Vec<ResourceFixture>) -> ResourceRegistry {
    let secrets: Arc<dyn SecretsProvider> = Arc::new(InMemorySecretsProvider::new(HashMap::new()));
    let registry = ResourceRegistry::new(secrets);

    for fixture in fixtures {
        let handle = Arc::new(InMemoryResourceHandle::with_fixtures(
            &fixture.name,
            fixture.resource_type,
            fixture.data,
        ));
        registry.register(&fixture.name, handle).await;
    }

    registry
}
