//! # Step Result Hydrator
//!
//! Hydrates step execution results from PGMQ messages containing minimal StepMessage payloads.
//!
//! ## Purpose
//!
//! Workers submit lightweight StepMessage (task_uuid + step_uuid + correlation_id) to the orchestration queue.
//! This hydrator performs database lookup to retrieve the full StepExecutionResult from the
//! WorkflowStep.results JSONB column, enabling efficient message queue operations while maintaining
//! rich execution data.
//!
//! ## Process
//!
//! 1. Parse StepMessage from PGMQ message
//! 2. Database lookup for WorkflowStep by step_uuid
//! 3. Validate results exist in JSONB column
//! 4. Deserialize StepExecutionResult from JSONB
//! 5. Return fully hydrated result for processing

use pgmq::Message as PgmqMessage;
use std::sync::Arc;
use tasker_shared::messaging::message::StepMessage;
use tasker_shared::messaging::service::QueuedMessage;
use tasker_shared::messaging::StepExecutionResult;
use tasker_shared::models::WorkflowStep;
use tasker_shared::system_context::SystemContext;
use tasker_shared::{TaskerError, TaskerResult};
use tracing::{debug, error, info};

/// Hydrates full StepExecutionResult from lightweight StepMessage
///
/// This service performs database-driven hydration, converting minimal queue messages
/// into rich execution results for orchestration processing.
///
/// ## Example
///
/// ```rust,no_run
/// use tasker_orchestration::orchestration::hydration::StepResultHydrator;
/// use std::sync::Arc;
///
/// # async fn example(context: Arc<tasker_shared::system_context::SystemContext>, message: pgmq::Message) -> tasker_shared::TaskerResult<()> {
/// let hydrator = StepResultHydrator::new(context);
/// let result = hydrator.hydrate_from_message(&message).await?;
/// // result is now ready for orchestration processing
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct StepResultHydrator {
    context: Arc<SystemContext>,
}

impl StepResultHydrator {
    /// Create a new StepResultHydrator
    ///
    /// # Arguments
    ///
    /// * `context` - System context providing database access
    pub fn new(context: Arc<SystemContext>) -> Self {
        Self { context }
    }

    /// Hydrate full StepExecutionResult from PGMQ message
    ///
    /// Performs the complete hydration process:
    /// 1. Parse StepMessage from message payload
    /// 2. Look up WorkflowStep in database
    /// 3. Extract and validate results JSONB
    /// 4. Deserialize into StepExecutionResult
    ///
    /// # Arguments
    ///
    /// * `message` - PGMQ message containing StepMessage payload
    ///
    /// # Returns
    ///
    /// Fully hydrated `StepExecutionResult` ready for orchestration processing
    ///
    /// # Errors
    ///
    /// - `ValidationError`: Invalid message format or missing data
    /// - `DatabaseError`: Database lookup failure
    pub async fn hydrate_from_message(
        &self,
        message: &PgmqMessage,
    ) -> TaskerResult<StepExecutionResult> {
        debug!(
            msg_id = message.msg_id,
            message_size = message.message.to_string().len(),
            "HYDRATOR: Starting step result hydration"
        );

        // Step 1: Parse StepMessage (task_uuid, step_uuid, correlation_id)
        let step_message: StepMessage =
            serde_json::from_value(message.message.clone()).map_err(|e| {
                error!(
                    msg_id = message.msg_id,
                    error = %e,
                    "HYDRATOR: Failed to parse StepMessage"
                );
                TaskerError::ValidationError(format!("Invalid StepMessage format: {e}"))
            })?;

        debug!(
            msg_id = message.msg_id,
            step_uuid = %step_message.step_uuid,
            task_uuid = %step_message.task_uuid,
            "HYDRATOR: Successfully parsed StepMessage"
        );

        // Step 2: Database lookup for WorkflowStep
        debug!(
            step_uuid = %step_message.step_uuid,
            "HYDRATOR: Looking up WorkflowStep in database"
        );

        let workflow_step =
            WorkflowStep::find_by_id(self.context.database_pool(), step_message.step_uuid)
                .await
                .map_err(|e| {
                    error!(
                        step_uuid = %step_message.step_uuid,
                        error = %e,
                        "HYDRATOR: Database lookup failed for WorkflowStep"
                    );
                    TaskerError::DatabaseError(format!("Failed to lookup step: {e}"))
                })?
                .ok_or_else(|| {
                    error!(
                        step_uuid = %step_message.step_uuid,
                        "HYDRATOR: WorkflowStep not found in database"
                    );
                    TaskerError::ValidationError(format!(
                        "WorkflowStep not found for step_uuid: {}",
                        step_message.step_uuid
                    ))
                })?;

        debug!(
            step_uuid = %step_message.step_uuid,
            task_uuid = %workflow_step.task_uuid,
            has_results = workflow_step.results.is_some(),
            "HYDRATOR: Successfully retrieved WorkflowStep from database"
        );

        // Step 3: Validate results exist
        let results_json = workflow_step.results.ok_or_else(|| {
            error!(
                step_uuid = %step_message.step_uuid,
                task_uuid = %workflow_step.task_uuid,
                "HYDRATOR: No results found in WorkflowStep.results JSONB column"
            );
            TaskerError::ValidationError(format!(
                "No results found for step_uuid: {}",
                step_message.step_uuid
            ))
        })?;

        debug!(
            step_uuid = %step_message.step_uuid,
            results_size = results_json.to_string().len(),
            "HYDRATOR: Deserializing StepExecutionResult from JSONB"
        );

        // Step 4: Deserialize StepExecutionResult from results JSONB column
        let step_execution_result: StepExecutionResult =
            serde_json::from_value(results_json.clone()).map_err(|e| {
                error!(
                    step_uuid = %step_message.step_uuid,
                    task_uuid = %workflow_step.task_uuid,
                    error = %e,
                    "HYDRATOR: Failed to deserialize StepExecutionResult from results JSONB"
                );
                TaskerError::ValidationError(format!(
                    "Failed to deserialize StepExecutionResult from results JSONB: {e}"
                ))
            })?;

        info!(
            step_uuid = %step_message.step_uuid,
            task_uuid = %workflow_step.task_uuid,
            status = %step_execution_result.status,
            execution_time_ms = step_execution_result.metadata.execution_time_ms,
            "HYDRATOR: Successfully hydrated StepExecutionResult"
        );

        Ok(step_execution_result)
    }

    /// TAS-133: Hydrate full StepExecutionResult from provider-agnostic QueuedMessage
    ///
    /// This is the provider-agnostic version of hydrate_from_message, working with
    /// `QueuedMessage<serde_json::Value>` instead of PGMQ-specific `PgmqMessage`.
    pub async fn hydrate_from_queued_message(
        &self,
        message: &QueuedMessage<serde_json::Value>,
    ) -> TaskerResult<StepExecutionResult> {
        debug!(
            handle = ?message.handle,
            "HYDRATOR: Starting step result hydration from QueuedMessage"
        );

        // Step 1: Parse StepMessage (task_uuid, step_uuid, correlation_id)
        let step_message: StepMessage =
            serde_json::from_value(message.message.clone()).map_err(|e| {
                error!(
                    handle = ?message.handle,
                    error = %e,
                    "HYDRATOR: Failed to parse StepMessage"
                );
                TaskerError::ValidationError(format!("Invalid StepMessage format: {e}"))
            })?;

        debug!(
            handle = ?message.handle,
            step_uuid = %step_message.step_uuid,
            task_uuid = %step_message.task_uuid,
            "HYDRATOR: Successfully parsed StepMessage"
        );

        // Steps 2-4: Shared hydration logic
        self.hydrate_from_step_message(&step_message).await
    }

    /// Internal: Hydrate StepExecutionResult from parsed StepMessage
    ///
    /// This internal method contains the shared logic used by both
    /// `hydrate_from_message` and `hydrate_from_queued_message`.
    async fn hydrate_from_step_message(
        &self,
        step_message: &StepMessage,
    ) -> TaskerResult<StepExecutionResult> {
        // Step 2: Database lookup for WorkflowStep
        debug!(
            step_uuid = %step_message.step_uuid,
            "HYDRATOR: Looking up WorkflowStep in database"
        );

        let workflow_step =
            WorkflowStep::find_by_id(self.context.database_pool(), step_message.step_uuid)
                .await
                .map_err(|e| {
                    error!(
                        step_uuid = %step_message.step_uuid,
                        error = %e,
                        "HYDRATOR: Database lookup failed for WorkflowStep"
                    );
                    TaskerError::DatabaseError(format!("Failed to lookup step: {e}"))
                })?
                .ok_or_else(|| {
                    error!(
                        step_uuid = %step_message.step_uuid,
                        "HYDRATOR: WorkflowStep not found in database"
                    );
                    TaskerError::ValidationError(format!(
                        "WorkflowStep not found for step_uuid: {}",
                        step_message.step_uuid
                    ))
                })?;

        debug!(
            step_uuid = %step_message.step_uuid,
            task_uuid = %workflow_step.task_uuid,
            has_results = workflow_step.results.is_some(),
            "HYDRATOR: Successfully retrieved WorkflowStep from database"
        );

        // Step 3: Validate results exist
        let results_json = workflow_step.results.ok_or_else(|| {
            error!(
                step_uuid = %step_message.step_uuid,
                task_uuid = %workflow_step.task_uuid,
                "HYDRATOR: No results found in WorkflowStep.results JSONB column"
            );
            TaskerError::ValidationError(format!(
                "No results found for step_uuid: {}",
                step_message.step_uuid
            ))
        })?;

        debug!(
            step_uuid = %step_message.step_uuid,
            results_size = results_json.to_string().len(),
            "HYDRATOR: Deserializing StepExecutionResult from JSONB"
        );

        // Step 4: Deserialize StepExecutionResult from results JSONB column
        let step_execution_result: StepExecutionResult =
            serde_json::from_value(results_json.clone()).map_err(|e| {
                error!(
                    step_uuid = %step_message.step_uuid,
                    task_uuid = %workflow_step.task_uuid,
                    error = %e,
                    "HYDRATOR: Failed to deserialize StepExecutionResult from results JSONB"
                );
                TaskerError::ValidationError(format!(
                    "Failed to deserialize StepExecutionResult from results JSONB: {e}"
                ))
            })?;

        info!(
            step_uuid = %step_message.step_uuid,
            task_uuid = %workflow_step.task_uuid,
            status = %step_execution_result.status,
            execution_time_ms = step_execution_result.metadata.execution_time_ms,
            "HYDRATOR: Successfully hydrated StepExecutionResult"
        );

        Ok(step_execution_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Arc;
    use tasker_shared::messaging::message::StepMessage;
    use tasker_shared::messaging::service::{MessageHandle, MessageMetadata};
    use tasker_shared::system_context::SystemContext;
    use uuid::Uuid;

    fn create_pgmq_message(payload: serde_json::Value) -> PgmqMessage {
        PgmqMessage {
            msg_id: 1,
            message: payload,
            vt: Utc::now(),
            read_ct: 1,
            enqueued_at: Utc::now(),
        }
    }

    fn create_queued_message(payload: serde_json::Value) -> QueuedMessage<serde_json::Value> {
        QueuedMessage::with_handle(
            payload,
            MessageHandle::Pgmq {
                msg_id: 1,
                queue_name: "test_orchestration_queue".to_string(),
            },
            MessageMetadata::new(1, Utc::now()),
        )
    }

    /// Create test infrastructure: namespace, named_task, task, named_step, workflow_step.
    /// Returns (task_uuid, step_uuid, correlation_id).
    async fn setup_test_step(
        pool: &sqlx::PgPool,
        results: Option<serde_json::Value>,
    ) -> Result<(Uuid, Uuid, Uuid), Box<dyn std::error::Error>> {
        let task_uuid = Uuid::now_v7();
        let step_uuid = Uuid::now_v7();
        let named_task_uuid = Uuid::now_v7();
        let named_step_uuid = Uuid::now_v7();
        let correlation_id = Uuid::now_v7();
        let step_name = format!("hydration_step_{}", step_uuid);

        // Create namespace (find or create)
        sqlx::query(
            "INSERT INTO tasker.task_namespaces (name, description, created_at, updated_at) \
             VALUES ('hydration_test_ns', 'Test namespace', NOW(), NOW()) \
             ON CONFLICT (name) DO NOTHING",
        )
        .execute(pool)
        .await?;

        let namespace_id: (Uuid,) = sqlx::query_as(
            "SELECT task_namespace_uuid FROM tasker.task_namespaces WHERE name = 'hydration_test_ns'",
        )
        .fetch_one(pool)
        .await?;

        // Create named task
        sqlx::query(
            "INSERT INTO tasker.named_tasks (named_task_uuid, task_namespace_uuid, name, description, version, created_at, updated_at) \
             VALUES ($1, $2, 'hydration_test_task', 'Test task', 1, NOW(), NOW()) \
             ON CONFLICT (task_namespace_uuid, name, version) DO UPDATE SET named_task_uuid = $1",
        )
        .bind(named_task_uuid)
        .bind(namespace_id.0)
        .execute(pool)
        .await?;

        // Create task
        sqlx::query(
            "INSERT INTO tasker.tasks (task_uuid, named_task_uuid, complete, requested_at, identity_hash, priority, created_at, updated_at, correlation_id) \
             VALUES ($1, $2, false, NOW(), $3, 0, NOW(), NOW(), $4)",
        )
        .bind(task_uuid)
        .bind(named_task_uuid)
        .bind(format!("hydration_test_{}", task_uuid))
        .bind(correlation_id)
        .execute(pool)
        .await?;

        // Create named step
        sqlx::query(
            "INSERT INTO tasker.named_steps (named_step_uuid, name, description, created_at, updated_at) \
             VALUES ($1, $2, 'Test step', NOW(), NOW()) \
             ON CONFLICT (name) DO UPDATE SET named_step_uuid = $1",
        )
        .bind(named_step_uuid)
        .bind(&step_name)
        .execute(pool)
        .await?;

        // Create workflow step with optional results
        sqlx::query(
            "INSERT INTO tasker.workflow_steps (workflow_step_uuid, task_uuid, named_step_uuid, retryable, in_process, processed, results, created_at, updated_at) \
             VALUES ($1, $2, $3, true, false, false, $4, NOW(), NOW())",
        )
        .bind(step_uuid)
        .bind(task_uuid)
        .bind(named_step_uuid)
        .bind(results)
        .execute(pool)
        .await?;

        Ok((task_uuid, step_uuid, correlation_id))
    }

    // --- hydrate_from_message tests ---

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_hydrate_from_message_success(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let step_result = StepExecutionResult {
            step_uuid: Uuid::now_v7(),
            success: true,
            result: json!({"output": "test_data"}),
            status: "completed".to_string(),
            ..Default::default()
        };
        let results_json = serde_json::to_value(&step_result)?;

        let (task_uuid, step_uuid, correlation_id) =
            setup_test_step(&pool, Some(results_json)).await?;

        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let hydrator = StepResultHydrator::new(context);

        let step_msg = StepMessage::new(task_uuid, step_uuid, correlation_id);
        let message = create_pgmq_message(serde_json::to_value(&step_msg)?);

        let result = hydrator.hydrate_from_message(&message).await;
        assert!(result.is_ok(), "Hydration should succeed: {:?}", result.err());

        let execution_result = result.unwrap();
        assert!(execution_result.success);
        assert_eq!(execution_result.status, "completed");
        assert_eq!(execution_result.result, json!({"output": "test_data"}));

        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_hydrate_from_message_step_not_found(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let hydrator = StepResultHydrator::new(context);

        let step_msg = StepMessage::new(Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let message = create_pgmq_message(serde_json::to_value(&step_msg)?);

        let result = hydrator.hydrate_from_message(&message).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "Error should indicate step not found: {err}"
        );

        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_hydrate_from_message_no_results(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create step without results (None)
        let (task_uuid, step_uuid, correlation_id) = setup_test_step(&pool, None).await?;

        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let hydrator = StepResultHydrator::new(context);

        let step_msg = StepMessage::new(task_uuid, step_uuid, correlation_id);
        let message = create_pgmq_message(serde_json::to_value(&step_msg)?);

        let result = hydrator.hydrate_from_message(&message).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("No results found"),
            "Error should indicate no results: {err}"
        );

        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_hydrate_from_message_invalid_results_json(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create step with results that are not a JSON object (array instead)
        // StepExecutionResult requires an object, so a string/array/number should fail
        let invalid_results = json!("this is not an object");
        let (task_uuid, step_uuid, correlation_id) =
            setup_test_step(&pool, Some(invalid_results)).await?;

        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let hydrator = StepResultHydrator::new(context);

        let step_msg = StepMessage::new(task_uuid, step_uuid, correlation_id);
        let message = create_pgmq_message(serde_json::to_value(&step_msg)?);

        let result = hydrator.hydrate_from_message(&message).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("deserialize") || err.to_string().contains("StepExecutionResult"),
            "Error should indicate deserialization failure: {err}"
        );

        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_hydrate_from_message_invalid_message_format(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let hydrator = StepResultHydrator::new(context);

        // Message that can't be parsed as StepMessage
        let message = create_pgmq_message(json!({"invalid": "format"}));

        let result = hydrator.hydrate_from_message(&message).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Invalid StepMessage"),
            "Error should indicate invalid format: {err}"
        );

        Ok(())
    }

    // --- hydrate_from_queued_message tests ---

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_hydrate_from_queued_message_success(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let step_result = StepExecutionResult {
            step_uuid: Uuid::now_v7(),
            success: false,
            result: json!({"error": "timeout"}),
            status: "error".to_string(),
            ..Default::default()
        };
        let results_json = serde_json::to_value(&step_result)?;

        let (task_uuid, step_uuid, correlation_id) =
            setup_test_step(&pool, Some(results_json)).await?;

        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let hydrator = StepResultHydrator::new(context);

        let step_msg = StepMessage::new(task_uuid, step_uuid, correlation_id);
        let message = create_queued_message(serde_json::to_value(&step_msg)?);

        let result = hydrator.hydrate_from_queued_message(&message).await;
        assert!(result.is_ok(), "Hydration should succeed: {:?}", result.err());

        let execution_result = result.unwrap();
        assert!(!execution_result.success);
        assert_eq!(execution_result.status, "error");

        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_hydrate_from_queued_message_step_not_found(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let hydrator = StepResultHydrator::new(context);

        let step_msg = StepMessage::new(Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let message = create_queued_message(serde_json::to_value(&step_msg)?);

        let result = hydrator.hydrate_from_queued_message(&message).await;
        assert!(result.is_err());

        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_hydrate_from_queued_message_invalid_format(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let hydrator = StepResultHydrator::new(context);

        let message = create_queued_message(json!({"not": "a step message"}));

        let result = hydrator.hydrate_from_queued_message(&message).await;
        assert!(result.is_err());

        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_hydrate_from_queued_message_no_results(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (task_uuid, step_uuid, correlation_id) = setup_test_step(&pool, None).await?;

        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let hydrator = StepResultHydrator::new(context);

        let step_msg = StepMessage::new(task_uuid, step_uuid, correlation_id);
        let message = create_queued_message(serde_json::to_value(&step_msg)?);

        let result = hydrator.hydrate_from_queued_message(&message).await;
        assert!(result.is_err());

        Ok(())
    }

    // --- Construction and trait tests ---

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_hydrator_debug_impl(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let hydrator = StepResultHydrator::new(context);

        let debug_str = format!("{:?}", hydrator);
        assert!(
            debug_str.contains("StepResultHydrator"),
            "Debug should contain struct name: {debug_str}"
        );

        Ok(())
    }
}
