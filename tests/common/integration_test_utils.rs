//! Helper functions for integration tests
//!
// Note: Using #[allow(dead_code)] instead of #[expect] because test utility
// functions may be used by some test targets but not others, causing inconsistent
// lint behavior between different test compilation units.
#![allow(dead_code)]

use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

use tasker_client::OrchestrationApiClient;
use tasker_shared::models::core::task_request::TaskRequest;
use tasker_shared::models::orchestration::execution_status::ExecutionStatus;

/// Check if a task status string represents a terminal state machine state.
///
/// The task state machine has three terminal states: complete, error, cancelled.
/// The derived `execution_status` (from step aggregation) can report "all_complete"
/// before the task state machine has transitioned, so callers should check this
/// to avoid race conditions between step completion and task finalization.
fn is_task_state_terminal(status: &str) -> bool {
    matches!(status, "complete" | "error" | "cancelled")
}

/// Get timeout multiplier based on execution environment
///
/// Applies cumulative multipliers for slow environments:
/// - CI (GitHub Actions, etc.): 1.5x - shared CPU, higher contention
/// - Coverage instrumentation (TASKER_COVERAGE_MODE=1): 4x - instrumented debug builds
///
/// The CI multiplier can be overridden via TASKER_CI_TIMEOUT_MULTIPLIER env var.
///
/// These multiply together, so CI + coverage = 6x.
pub fn get_timeout_multiplier() -> u64 {
    let ci_multiplier = if std::env::var("CI").is_ok() {
        // Allow override via env var, default to 1.5x
        std::env::var("TASKER_CI_TIMEOUT_MULTIPLIER")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.5)
    } else {
        1.0
    };

    let coverage_multiplier: f64 = if std::env::var("TASKER_COVERAGE_MODE").is_ok() {
        4.0 // Coverage instrumentation adds significant overhead
    } else {
        1.0
    };

    // Ceiling ensures we never round down to a shorter timeout
    (ci_multiplier * coverage_multiplier).ceil() as u64
}

/// Get default base timeout for task completion (in seconds)
///
/// Returns a reasonable base timeout for general task completion polling.
/// The actual effective timeout will be scaled by `get_timeout_multiplier()`
/// inside `wait_for_task_completion` / `wait_for_task_failure`, so callers
/// should NOT pre-multiply this value.
pub fn get_task_completion_timeout() -> u64 {
    10
}

/// Helper to create a TaskRequest matching CLI usage
///
/// TAS-154: Injects a unique test_run_id into the context to ensure each test run
/// produces a unique identity hash. This prevents conflicts from duplicate identity
/// hashes across test runs when using the STRICT identity strategy (default).
pub fn create_task_request(
    namespace: &str,
    name: &str,
    input_context: serde_json::Value,
) -> TaskRequest {
    // TAS-154: Inject unique test_run_id to ensure unique identity hash per test run
    let mut context = match input_context {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    context.insert(
        "_test_run_id".to_string(),
        serde_json::Value::String(Uuid::now_v7().to_string()),
    );

    TaskRequest {
        namespace: namespace.to_string(),
        name: name.to_string(),
        version: "1.0.0".to_string(),
        context: serde_json::Value::Object(context),
        correlation_id: Uuid::now_v7(),
        parent_correlation_id: None,
        initiator: "integration-test".to_string(),
        source_system: "test".to_string(),
        reason: "Integration test execution".to_string(),
        tags: vec!["integration-test".to_string()],

        requested_at: chrono::Utc::now().naive_utc(),
        options: None,
        priority: Some(5),
        idempotency_key: None,
    }
}

/// Wait for task completion by polling task status
///
/// The provided `max_wait_seconds` is scaled by `get_timeout_multiplier()`
/// to account for CI and coverage overhead. This means hardcoded timeouts
/// in tests automatically get extended in slower environments.
pub async fn wait_for_task_completion(
    client: &OrchestrationApiClient,
    task_uuid: &str,
    max_wait_seconds: u64,
) -> Result<()> {
    let start_time = std::time::Instant::now();
    let effective_timeout = max_wait_seconds * get_timeout_multiplier();
    let max_duration = Duration::from_secs(effective_timeout);

    println!(
        "⏳ Waiting for task {} to complete (max {}s, base {}s)...",
        task_uuid, effective_timeout, max_wait_seconds
    );

    while start_time.elapsed() < max_duration {
        let uuid = Uuid::parse_str(task_uuid)?;
        match client.get_task(uuid).await {
            Ok(task_response) => {
                let execution_status = task_response.execution_status_typed();
                println!(
                    "   Task execution status: {} ({})",
                    task_response.execution_status, task_response.status
                );

                match execution_status {
                    ExecutionStatus::AllComplete => {
                        // Steps are done, but the task state machine may still
                        // be transitioning (steps_in_process → evaluating_results
                        // → complete). Wait for the task status to catch up.
                        if is_task_state_terminal(&task_response.status) {
                            println!("✅ Task completed successfully!");
                            return Ok(());
                        }
                        // State machine still catching up, poll quickly
                        sleep(Duration::from_millis(250)).await;
                    }
                    ExecutionStatus::BlockedByFailures => {
                        if is_task_state_terminal(&task_response.status) {
                            return Err(anyhow::anyhow!(
                                "Task blocked by failures that cannot be retried: {}",
                                task_response.execution_status
                            ));
                        }
                        sleep(Duration::from_millis(250)).await;
                    }
                    ExecutionStatus::HasReadySteps
                    | ExecutionStatus::Processing
                    | ExecutionStatus::WaitingForDependencies => {
                        // Still processing, continue polling
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }
            Err(e) => {
                println!("   Error polling task status: {}", e);
                sleep(Duration::from_secs(2)).await;
            }
        }
    }

    Err(anyhow::anyhow!(
        "Task did not complete within {}s (base {}s)",
        effective_timeout,
        max_wait_seconds
    ))
}

/// Wait for task to fail (reach error or blocked state)
///
/// This is useful for testing error scenarios where we expect the task to fail.
/// Returns Ok(()) when task reaches BlockedByFailures state.
///
/// The provided `max_wait_seconds` is scaled by `get_timeout_multiplier()`
/// to account for CI and coverage overhead.
pub async fn wait_for_task_failure(
    client: &OrchestrationApiClient,
    task_uuid: &str,
    max_wait_seconds: u64,
) -> Result<()> {
    let start_time = std::time::Instant::now();
    let effective_timeout = max_wait_seconds * get_timeout_multiplier();
    let max_duration = Duration::from_secs(effective_timeout);

    println!(
        "⏳ Waiting for task {} to fail (max {}s, base {}s)...",
        task_uuid, effective_timeout, max_wait_seconds
    );

    while start_time.elapsed() < max_duration {
        let uuid = Uuid::parse_str(task_uuid)?;
        match client.get_task(uuid).await {
            Ok(task_response) => {
                let execution_status = task_response.execution_status_typed();
                println!(
                    "   Task execution status: {} ({})",
                    task_response.execution_status, task_response.status
                );

                match execution_status {
                    ExecutionStatus::BlockedByFailures => {
                        // Wait for the task state machine to finish transitioning
                        // before returning, so assertions on task.status are safe.
                        if is_task_state_terminal(&task_response.status) {
                            println!("✅ Task failed as expected (blocked by failures)!");
                            return Ok(());
                        }
                        sleep(Duration::from_millis(250)).await;
                    }
                    ExecutionStatus::AllComplete => {
                        if is_task_state_terminal(&task_response.status) {
                            return Err(anyhow::anyhow!(
                                "Task completed successfully but was expected to fail"
                            ));
                        }
                        sleep(Duration::from_millis(250)).await;
                    }
                    ExecutionStatus::HasReadySteps
                    | ExecutionStatus::Processing
                    | ExecutionStatus::WaitingForDependencies => {
                        // Still processing, continue polling
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            }
            Err(e) => {
                println!("   Error polling task status: {}", e);
                sleep(Duration::from_secs(1)).await;
            }
        }
    }

    Err(anyhow::anyhow!(
        "Task did not fail within {}s (base {}s)",
        effective_timeout,
        max_wait_seconds
    ))
}
