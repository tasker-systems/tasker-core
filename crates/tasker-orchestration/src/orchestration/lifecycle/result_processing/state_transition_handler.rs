//! State Transition Handler
//!
//! TAS-41: Handles orchestration state transitions for EnqueuedForOrchestration steps.

use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, warn};
use uuid::Uuid;

use tasker_shared::errors::OrchestrationResult;
use tasker_shared::messaging::StepExecutionResult;
use tasker_shared::models::core::workflow_step::WorkflowStep;
use tasker_shared::state_machine::states::WorkflowStepState;
use tasker_shared::system_context::SystemContext;

/// Handles orchestration state transitions for steps
#[derive(Clone, Debug)]
pub struct StateTransitionHandler {
    context: Arc<SystemContext>,
}

impl StateTransitionHandler {
    pub fn new(context: Arc<SystemContext>) -> Self {
        Self { context }
    }

    /// TAS-41: Process orchestration state transitions for EnqueuedForOrchestration steps
    ///
    /// This method handles the transition of steps from EnqueuedForOrchestration state
    /// to their final states (Complete or Error) after orchestration metadata processing.
    /// This is critical for fixing the race condition where workers bypass orchestration.
    pub async fn process_state_transition(
        &self,
        step_uuid: &Uuid,
        original_status: &String,
        correlation_id: Uuid,
    ) -> OrchestrationResult<()> {
        // Load the current step to check its state
        let step = WorkflowStep::find_by_id(self.context.database_pool(), *step_uuid)
            .await
            .map_err(
                |e| tasker_shared::errors::OrchestrationError::DatabaseError {
                    operation: "load_step".to_string(),
                    reason: format!("Failed to load step {}: {}", step_uuid, e),
                },
            )?;

        let Some(step) = step else {
            warn!(
                correlation_id = %correlation_id,
                step_uuid = %step_uuid,
                "Step not found - may have been processed by another processor"
            );
            return Ok(());
        };

        // Get the current state using the step's state machine
        let current_state = step
            .get_current_state(self.context.database_pool())
            .await
            .map_err(
                |e| tasker_shared::errors::OrchestrationError::DatabaseError {
                    operation: "get_current_state".to_string(),
                    reason: format!("Failed to get current state for step {}: {}", step_uuid, e),
                },
            )?;

        // Only process if step is in EnqueuedForOrchestration state
        if let Some(state_str) = current_state {
            let step_state = WorkflowStepState::from_str(&state_str).map_err(|e| {
                tasker_shared::errors::OrchestrationError::from(format!(
                    "Invalid workflow step state: {}",
                    e
                ))
            })?;

            if matches!(
                step_state,
                WorkflowStepState::EnqueuedForOrchestration
                    | WorkflowStepState::EnqueuedAsErrorForOrchestration
            ) {
                debug!(
                    correlation_id = %correlation_id,
                    step_uuid = %step_uuid,
                    original_status = %original_status,
                    step_state = %step_state,
                    "Processing orchestration state transition for step in notification state"
                );

                // Create state machine for the step
                use tasker_shared::state_machine::StepStateMachine;
                let mut state_machine = StepStateMachine::new(step.clone(), self.context.clone());

                // Determine the final state based on step notification state and execution result
                let final_event = match step_state {
                    WorkflowStepState::EnqueuedForOrchestration => {
                        self.determine_success_event(&step, original_status)
                    }
                    WorkflowStepState::EnqueuedAsErrorForOrchestration => {
                        self.determine_error_event(&step, original_status, correlation_id)
                            .await
                    }
                    _ => unreachable!("Already matched above"),
                };

                // Execute the state transition
                let final_state = state_machine.transition(final_event).await.map_err(|e| {
                    tasker_shared::errors::OrchestrationError::StateTransitionFailed {
                        entity_type: "WorkflowStep".to_string(),
                        entity_uuid: *step_uuid,
                        reason: format!("Failed to transition step to final state: {}", e),
                    }
                })?;

                debug!(
                    correlation_id = %correlation_id,
                    step_uuid = %step_uuid,
                    final_state = %final_state,
                    "Successfully transitioned step from notification state to final state"
                );
            } else {
                debug!(
                    correlation_id = %correlation_id,
                    step_uuid = %step_uuid,
                    current_state = %step_state,
                    "Step not in EnqueuedForOrchestration or EnqueuedAsErrorForOrchestration state - skipping orchestration transition"
                );
            }
        } else {
            warn!(
                correlation_id = %correlation_id,
                step_uuid = %step_uuid,
                "Step has no current state - may be in inconsistent state"
            );
        }

        Ok(())
    }

    /// Determine the event for success pathway (EnqueuedForOrchestration)
    fn determine_success_event(
        &self,
        step: &WorkflowStep,
        original_status: &String,
    ) -> tasker_shared::state_machine::events::StepEvent {
        use tasker_shared::state_machine::events::StepEvent;

        // Deserialize StepExecutionResult to determine final state
        if let Some(results_json) = &step.results {
            match serde_json::from_value::<StepExecutionResult>(results_json.clone()) {
                Ok(step_execution_result) => {
                    if step_execution_result.success {
                        StepEvent::Complete(step.results.clone())
                    } else {
                        // Handle case where success path contains failure
                        let error_message = step_execution_result
                            .error
                            .map(|e| e.message)
                            .unwrap_or_else(|| "Unknown error".to_string());
                        StepEvent::Fail(format!("Step failed: {}", error_message))
                    }
                }
                Err(_) => {
                    // Fallback to original status parsing for backward compatibility
                    if Self::is_success_status(original_status) {
                        StepEvent::Complete(step.results.clone())
                    } else {
                        StepEvent::Fail(format!("Step failed with status: {}", original_status))
                    }
                }
            }
        } else {
            // No results available - use status
            if Self::is_success_status(original_status) {
                StepEvent::Complete(None)
            } else {
                StepEvent::Fail(format!("Step failed with status: {}", original_status))
            }
        }
    }

    /// Determine the event for error pathway (EnqueuedAsErrorForOrchestration)
    async fn determine_error_event(
        &self,
        step: &WorkflowStep,
        original_status: &String,
        correlation_id: Uuid,
    ) -> tasker_shared::state_machine::events::StepEvent {
        use tasker_shared::state_machine::events::StepEvent;

        // Determine if step should retry or move to permanent error
        let should_retry = self.should_retry_step(step, correlation_id).await;

        if should_retry {
            // Transition to WaitingForRetry (backoff already calculated)
            debug!(
                correlation_id = %correlation_id,
                step_uuid = %step.workflow_step_uuid,
                "Transitioning to WaitingForRetry state"
            );
            let error_message = self.extract_error_message(step, original_status);
            StepEvent::WaitForRetry(format!("{} - retryable", error_message))
        } else {
            // Transition to Error (permanent failure or max retries)
            debug!(
                correlation_id = %correlation_id,
                step_uuid = %step.workflow_step_uuid,
                "Transitioning to Error state (permanent or max retries)"
            );
            let error_message = self.extract_error_message(step, original_status);
            StepEvent::Fail(error_message)
        }
    }

    /// Check if step should retry based on metadata and retry limits
    async fn should_retry_step(&self, step: &WorkflowStep, correlation_id: Uuid) -> bool {
        if let Some(results_json) = &step.results {
            match serde_json::from_value::<StepExecutionResult>(results_json.clone()) {
                Ok(step_execution_result) => {
                    // Check if error is marked as non-retryable in metadata
                    let retryable_from_metadata = step_execution_result.metadata.retryable;

                    if !retryable_from_metadata {
                        debug!(
                            correlation_id = %correlation_id,
                            step_uuid = %step.workflow_step_uuid,
                            "Error marked as non-retryable by worker"
                        );
                        return false;
                    }

                    // Check retry limits from template
                    let max_attempts = step.max_attempts.unwrap_or(0);
                    let current_attempts = step.attempts.unwrap_or(0);

                    if current_attempts >= max_attempts {
                        debug!(
                            correlation_id = %correlation_id,
                            step_uuid = %step.workflow_step_uuid,
                            current_attempts = current_attempts,
                            max_attempts = max_attempts,
                            "Step has exceeded retry limit from template"
                        );
                        false
                    } else {
                        debug!(
                            correlation_id = %correlation_id,
                            step_uuid = %step.workflow_step_uuid,
                            current_attempts = current_attempts,
                            max_attempts = max_attempts,
                            "Step is retryable with attempts remaining"
                        );
                        true
                    }
                }
                Err(_) => {
                    // Can't deserialize results - default to checking retry limit only
                    let max_attempts = step.max_attempts.unwrap_or(0);
                    let current_attempts = step.attempts.unwrap_or(0);
                    current_attempts < max_attempts
                }
            }
        } else {
            // No results - check retry limit only
            let max_attempts = step.max_attempts.unwrap_or(0);
            let current_attempts = step.attempts.unwrap_or(0);
            current_attempts < max_attempts
        }
    }

    /// Extract error message from step results or status
    fn extract_error_message(&self, step: &WorkflowStep, original_status: &String) -> String {
        if let Some(results_json) = &step.results {
            match serde_json::from_value::<StepExecutionResult>(results_json.clone()) {
                Ok(step_execution_result) => step_execution_result
                    .error
                    .map(|e| e.message)
                    .unwrap_or_else(|| "Step execution failed".to_string()),
                Err(_) => format!("Step failed with status: {}", original_status),
            }
        } else {
            format!("Step failed with status: {}", original_status)
        }
    }

    /// Check if status indicates success
    fn is_success_status(status: &str) -> bool {
        let lower = status.to_lowercase();
        lower.contains("success") || lower == "complete" || lower == "completed"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_state_transition_handler_creation(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        // Verify it's created (basic smoke test)
        assert!(Arc::strong_count(&handler.context) >= 1);
        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_state_transition_handler_clone(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context.clone());

        let cloned = handler.clone();

        // Verify both share the same Arc
        assert_eq!(Arc::as_ptr(&handler.context), Arc::as_ptr(&cloned.context));
        Ok(())
    }

    #[test]
    fn test_is_success_status() {
        // Test success status detection
        assert!(StateTransitionHandler::is_success_status("success"));
        assert!(StateTransitionHandler::is_success_status("SUCCESS"));
        assert!(StateTransitionHandler::is_success_status("complete"));
        assert!(StateTransitionHandler::is_success_status("completed"));
        assert!(StateTransitionHandler::is_success_status("Success"));

        // Test non-success statuses
        assert!(!StateTransitionHandler::is_success_status("error"));
        assert!(!StateTransitionHandler::is_success_status("failed"));
        assert!(!StateTransitionHandler::is_success_status("timeout"));
    }

    #[test]
    fn test_is_success_status_partial_match() {
        // Test that "success" is matched even in longer strings
        assert!(StateTransitionHandler::is_success_status(
            "successful_execution"
        ));
        assert!(StateTransitionHandler::is_success_status(
            "operation_success"
        ));
    }

    // ── extract_error_message tests ──

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_extract_error_message_from_results(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        // Create a WorkflowStep with error results
        let step_result = StepExecutionResult::failure(
            Uuid::now_v7(),
            "Payment gateway timeout".to_string(),
            Some("GATEWAY_TIMEOUT".to_string()),
            Some("TimeoutError".to_string()),
            true,
            5000,
            None,
        );
        let results_json = serde_json::to_value(&step_result)?;

        let step = WorkflowStep {
            workflow_step_uuid: Uuid::now_v7(),
            task_uuid: Uuid::now_v7(),
            named_step_uuid: Uuid::now_v7(),
            retryable: true,
            max_attempts: Some(3),
            in_process: false,
            processed: false,
            processed_at: None,
            attempts: Some(1),
            last_attempted_at: None,
            backoff_request_seconds: None,
            inputs: None,
            results: Some(results_json),
            checkpoint: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let original_status = "error".to_string();
        let message = handler.extract_error_message(&step, &original_status);

        assert_eq!(message, "Payment gateway timeout");
        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_extract_error_message_no_results_uses_status(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        let step = WorkflowStep {
            workflow_step_uuid: Uuid::now_v7(),
            task_uuid: Uuid::now_v7(),
            named_step_uuid: Uuid::now_v7(),
            retryable: true,
            max_attempts: Some(3),
            in_process: false,
            processed: false,
            processed_at: None,
            attempts: Some(1),
            last_attempted_at: None,
            backoff_request_seconds: None,
            inputs: None,
            results: None,
            checkpoint: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let original_status = "timeout".to_string();
        let message = handler.extract_error_message(&step, &original_status);

        assert_eq!(message, "Step failed with status: timeout");
        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_extract_error_message_empty_error(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        // Success result has no error field
        let step_result = StepExecutionResult::success(
            Uuid::now_v7(),
            serde_json::json!({"data": "value"}),
            100,
            None,
        );
        let results_json = serde_json::to_value(&step_result)?;

        let step = WorkflowStep {
            workflow_step_uuid: Uuid::now_v7(),
            task_uuid: Uuid::now_v7(),
            named_step_uuid: Uuid::now_v7(),
            retryable: true,
            max_attempts: Some(3),
            in_process: false,
            processed: false,
            processed_at: None,
            attempts: Some(1),
            last_attempted_at: None,
            backoff_request_seconds: None,
            inputs: None,
            results: Some(results_json),
            checkpoint: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let original_status = "error".to_string();
        let message = handler.extract_error_message(&step, &original_status);

        // No error in results → fallback message
        assert_eq!(message, "Step execution failed");
        Ok(())
    }

    // ── process_state_transition tests ──

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_process_state_transition_nonexistent_step(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        let nonexistent_uuid = Uuid::new_v4();
        let correlation_id = Uuid::new_v4();
        let status = "completed".to_string();

        // Nonexistent step → Ok (warning logged, no error)
        let result = handler
            .process_state_transition(&nonexistent_uuid, &status, correlation_id)
            .await;
        assert!(result.is_ok());
        Ok(())
    }

    // ── should_retry_step tests ──

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_should_retry_step_within_limits(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        // Step with retryable result and attempts remaining
        let step_result = StepExecutionResult::failure(
            Uuid::now_v7(),
            "transient error".to_string(),
            None,
            None,
            true, // retryable
            100,
            None,
        );
        let results_json = serde_json::to_value(&step_result)?;

        let step = WorkflowStep {
            workflow_step_uuid: Uuid::now_v7(),
            task_uuid: Uuid::now_v7(),
            named_step_uuid: Uuid::now_v7(),
            retryable: true,
            max_attempts: Some(5),
            in_process: false,
            processed: false,
            processed_at: None,
            attempts: Some(2), // 2 < 5
            last_attempted_at: None,
            backoff_request_seconds: None,
            inputs: None,
            results: Some(results_json),
            checkpoint: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let correlation_id = Uuid::new_v4();
        let should_retry = handler.should_retry_step(&step, correlation_id).await;
        assert!(should_retry);
        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_should_retry_step_at_max_attempts(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        let step_result = StepExecutionResult::failure(
            Uuid::now_v7(),
            "error".to_string(),
            None,
            None,
            true,
            100,
            None,
        );
        let results_json = serde_json::to_value(&step_result)?;

        let step = WorkflowStep {
            workflow_step_uuid: Uuid::now_v7(),
            task_uuid: Uuid::now_v7(),
            named_step_uuid: Uuid::now_v7(),
            retryable: true,
            max_attempts: Some(3),
            in_process: false,
            processed: false,
            processed_at: None,
            attempts: Some(3), // 3 >= 3
            last_attempted_at: None,
            backoff_request_seconds: None,
            inputs: None,
            results: Some(results_json),
            checkpoint: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let correlation_id = Uuid::new_v4();
        let should_retry = handler.should_retry_step(&step, correlation_id).await;
        assert!(!should_retry);
        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_should_retry_step_non_retryable_from_metadata(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        // Failure marked as non-retryable by worker
        let step_result = StepExecutionResult::failure(
            Uuid::now_v7(),
            "permanent error".to_string(),
            None,
            None,
            false, // NOT retryable
            100,
            None,
        );
        let results_json = serde_json::to_value(&step_result)?;

        let step = WorkflowStep {
            workflow_step_uuid: Uuid::now_v7(),
            task_uuid: Uuid::now_v7(),
            named_step_uuid: Uuid::now_v7(),
            retryable: true,
            max_attempts: Some(5),
            in_process: false,
            processed: false,
            processed_at: None,
            attempts: Some(1), // Plenty of attempts remaining
            last_attempted_at: None,
            backoff_request_seconds: None,
            inputs: None,
            results: Some(results_json),
            checkpoint: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let correlation_id = Uuid::new_v4();
        let should_retry = handler.should_retry_step(&step, correlation_id).await;
        assert!(!should_retry); // Non-retryable overrides attempt count
        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_should_retry_step_no_results(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        let step = WorkflowStep {
            workflow_step_uuid: Uuid::now_v7(),
            task_uuid: Uuid::now_v7(),
            named_step_uuid: Uuid::now_v7(),
            retryable: true,
            max_attempts: Some(5),
            in_process: false,
            processed: false,
            processed_at: None,
            attempts: Some(2),
            last_attempted_at: None,
            backoff_request_seconds: None,
            inputs: None,
            results: None, // No results
            checkpoint: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let correlation_id = Uuid::new_v4();
        let should_retry = handler.should_retry_step(&step, correlation_id).await;
        assert!(should_retry); // Falls back to retry limit check: 2 < 5
        Ok(())
    }

    // ── determine_success_event tests ──

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_determine_success_event_with_success_result(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use tasker_shared::state_machine::events::StepEvent;

        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        let step_result = StepExecutionResult::success(
            Uuid::now_v7(),
            serde_json::json!({"data": "processed"}),
            100,
            None,
        );
        let results_json = serde_json::to_value(&step_result)?;

        let step = WorkflowStep {
            workflow_step_uuid: Uuid::now_v7(),
            task_uuid: Uuid::now_v7(),
            named_step_uuid: Uuid::now_v7(),
            retryable: true,
            max_attempts: Some(3),
            in_process: false,
            processed: false,
            processed_at: None,
            attempts: Some(1),
            last_attempted_at: None,
            backoff_request_seconds: None,
            inputs: None,
            results: Some(results_json),
            checkpoint: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let status = "completed".to_string();
        let event = handler.determine_success_event(&step, &status);

        assert!(matches!(event, StepEvent::Complete(_)));
        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_determine_success_event_with_failure_in_results(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use tasker_shared::state_machine::events::StepEvent;

        let context = Arc::new(SystemContext::with_pool(pool).await?);
        let handler = StateTransitionHandler::new(context);

        let step_result = StepExecutionResult::failure(
            Uuid::now_v7(),
            "data validation failed".to_string(),
            None,
            None,
            false,
            100,
            None,
        );
        let results_json = serde_json::to_value(&step_result)?;

        let step = WorkflowStep {
            workflow_step_uuid: Uuid::now_v7(),
            task_uuid: Uuid::now_v7(),
            named_step_uuid: Uuid::now_v7(),
            retryable: true,
            max_attempts: Some(3),
            in_process: false,
            processed: false,
            processed_at: None,
            attempts: Some(1),
            last_attempted_at: None,
            backoff_request_seconds: None,
            inputs: None,
            results: Some(results_json),
            checkpoint: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let status = "completed".to_string();
        let event = handler.determine_success_event(&step, &status);

        // Success path but result says failure → Fail event
        assert!(matches!(event, StepEvent::Fail(_)));
        Ok(())
    }
}
