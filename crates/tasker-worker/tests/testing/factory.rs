//! Worker testing factory for integration tests.
//!
//! Provides type-safe test data creation, factory patterns for common test
//! objects, and database-backed test data persistence.

use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tasker_pgmq::PgmqClient;
use tasker_shared::models::core::task_request::TaskRequest;
use tasker_shared::models::{NamedStep, NamedTask, TaskNamespace, WorkflowStep};
use uuid::Uuid;

/// Test-specific error type for factory operations
#[derive(Debug, thiserror::Error)]
pub enum TestFactoryError {
    #[error("Database operation failed: {0}")]
    DatabaseError(String),

    #[error("Test data creation failed: {0}")]
    CreationError(String),
}

/// Worker-specific testing factory for creating test data
///
/// Provides factory methods for creating test tasks, steps, and related
/// objects backed by a database pool.
#[derive(Clone, Debug)]
pub struct WorkerTestFactory {
    /// Pool for Tasker table operations (tasks, steps, namespaces)
    database_pool: Arc<PgPool>,
    /// Pool for PGMQ queue operations (TAS-78: may be separate database)
    pgmq_pool: Arc<PgPool>,
}

impl WorkerTestFactory {
    /// Create a new factory with database connection.
    ///
    /// Uses the same pool for both Tasker tables and PGMQ operations.
    pub fn new(database_pool: Arc<PgPool>) -> Self {
        Self {
            pgmq_pool: database_pool.clone(),
            database_pool,
        }
    }

    /// Create a test task request
    pub fn create_test_task_request(
        &self,
        namespace: &str,
        name: &str,
        test_id: &str,
    ) -> TaskRequest {
        TaskRequest::new(name.to_string(), namespace.to_string())
            .with_context(json!({
                "test": true,
                "test_id": test_id,
                "created_by": "worker_test_factory",
            }))
            .with_initiator("test_factory".to_string())
            .with_source_system("worker_testing".to_string())
            .with_reason(format!("Testing {} workflow", name))
            .with_tags(vec!["test".to_string(), "worker".to_string()])
            .with_priority(5)
    }

    /// Create test namespace and related infrastructure.
    ///
    /// Uses the Tasker pool for namespace creation and PGMQ pool for queue creation.
    pub async fn create_test_namespace(
        &self,
        name: &str,
    ) -> Result<TestNamespace, TestFactoryError> {
        let tasker_pool = self.database_pool.as_ref();

        let namespace = TaskNamespace::find_or_create(tasker_pool, name)
            .await
            .map_err(|e| {
                TestFactoryError::DatabaseError(format!("Namespace creation failed: {}", e))
            })?;

        let pgmq_client = PgmqClient::new_with_pool(self.pgmq_pool.as_ref().clone()).await;
        let queue_name = format!("worker_{}_queue", name);

        pgmq_client.create_queue(&queue_name).await.map_err(|e| {
            TestFactoryError::CreationError(format!("Queue creation failed: {}", e))
        })?;

        Ok(TestNamespace {
            namespace,
            queue_name,
        })
    }

    /// Create a complete test foundation (namespace + named task + named step)
    pub async fn create_test_foundation(
        &self,
        namespace_name: &str,
        task_name: &str,
        step_name: &str,
    ) -> Result<TestFoundation, TestFactoryError> {
        let pool = self.database_pool.as_ref();

        let test_namespace = self.create_test_namespace(namespace_name).await?;

        let named_task = NamedTask::find_or_create_by_name_version_namespace(
            pool,
            task_name,
            "0.1.0",
            test_namespace.namespace.task_namespace_uuid,
        )
        .await
        .map_err(|e| {
            TestFactoryError::DatabaseError(format!("Named task creation failed: {}", e))
        })?;

        let named_step = NamedStep::find_or_create_by_name_simple(pool, step_name)
            .await
            .map_err(|e| {
                TestFactoryError::DatabaseError(format!("Named step creation failed: {}", e))
            })?;

        Ok(TestFoundation {
            namespace: test_namespace,
            named_task,
            named_step,
        })
    }

    /// Create test step message for worker processing
    pub async fn create_test_step_message(
        &self,
        task_uuid: Uuid,
        step_uuid: Uuid,
        step_name: &str,
        payload: serde_json::Value,
    ) -> Result<TestStepMessage, TestFactoryError> {
        Ok(TestStepMessage {
            step_uuid,
            task_uuid,
            step_name: step_name.to_string(),
            payload,
            created_at: chrono::Utc::now(),
        })
    }

    /// Create multiple test workflow steps for a task
    pub async fn create_test_workflow_steps(
        &self,
        task_uuid: Uuid,
        step_configs: Vec<TestStepConfig>,
    ) -> Result<Vec<WorkflowStep>, TestFactoryError> {
        let pool = self.database_pool.as_ref();
        let mut created_steps = Vec::new();

        for config in step_configs {
            let named_step = NamedStep::find_or_create_by_name_simple(pool, &config.name)
                .await
                .map_err(|e| {
                    TestFactoryError::DatabaseError(format!("Named step creation failed: {}", e))
                })?;

            let new_step = tasker_shared::models::core::workflow_step::NewWorkflowStep {
                task_uuid,
                named_step_uuid: named_step.named_step_uuid,
                retryable: Some(config.retryable),
                max_attempts: Some(config.max_attempts),
                inputs: Some(config.inputs),
            };

            let step = WorkflowStep::create(pool, new_step).await.map_err(|e| {
                TestFactoryError::DatabaseError(format!("Workflow step creation failed: {}", e))
            })?;

            created_steps.push(step);
        }

        Ok(created_steps)
    }
}

/// Test namespace with associated queue
#[derive(Debug, Clone)]
pub struct TestNamespace {
    pub namespace: TaskNamespace,
    #[expect(dead_code, reason = "Dummy Testing Struct")]
    pub queue_name: String,
}

/// Complete test foundation with namespace, named task, and named step
#[derive(Debug, Clone)]
pub struct TestFoundation {
    pub namespace: TestNamespace,
    pub named_task: NamedTask,
    #[expect(dead_code, reason = "Dummy Testing Struct")]
    pub named_step: NamedStep,
}

/// Test step configuration for creating workflow steps
#[derive(Debug, Clone)]
pub struct TestStepConfig {
    pub name: String,
    pub inputs: serde_json::Value,
    pub retryable: bool,
    pub max_attempts: i32,
    pub skippable: bool,
}

impl Default for TestStepConfig {
    fn default() -> Self {
        Self {
            name: "test_step".to_string(),
            inputs: json!({}),
            retryable: true,
            max_attempts: 3,
            skippable: false,
        }
    }
}

/// Test step message structure
#[derive(Debug, Clone)]
pub struct TestStepMessage {
    pub step_uuid: Uuid,
    pub task_uuid: Uuid,
    pub step_name: String,
    pub payload: serde_json::Value,
    #[expect(dead_code, reason = "Dummy Testing Struct")]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Common test data builder for worker tests
#[derive(Debug)]
pub struct WorkerTestData {
    pub namespace_name: String,
    pub task_name: String,
    pub test_id: String,
    pub foundation: Option<TestFoundation>,
}

impl WorkerTestData {
    /// Create a new test data builder
    pub fn new(test_id: &str) -> Self {
        Self {
            namespace_name: format!("test_ns_{}", test_id),
            task_name: format!("test_task_{}", test_id),
            test_id: test_id.to_string(),
            foundation: None,
        }
    }

    /// Build test data with factory
    pub async fn build_with_factory(
        self,
        factory: &WorkerTestFactory,
    ) -> Result<Self, TestFactoryError> {
        let foundation = factory
            .create_test_foundation(
                &self.namespace_name,
                &self.task_name,
                &format!("step_{}", self.test_id),
            )
            .await?;

        Ok(Self {
            foundation: Some(foundation),
            ..self
        })
    }

    /// Get the test foundation
    pub fn foundation(&self) -> Option<&TestFoundation> {
        self.foundation.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_config_defaults() {
        let config = TestStepConfig::default();

        assert_eq!(config.name, "test_step");
        assert!(config.retryable);
        assert_eq!(config.max_attempts, 3);
        assert!(!config.skippable);
    }
}
