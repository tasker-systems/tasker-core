//! # Batch Processing Service Integration Tests (TAS-63)
//!
//! Tests for BatchProcessingService using the large_dataset_processor template:
//! - NoBatches placeholder worker creation
//! - CreateBatches with multiple cursors
//! - Convergence step creation
//! - Edge wiring (batchable → workers → convergence)
//! - Idempotency on duplicate calls
//! - Worker naming conventions
//! - Integration with ViableStepDiscovery

use anyhow::Result;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use tasker_orchestration::orchestration::lifecycle::step_enqueuer_services::StepEnqueuerService;
use tasker_orchestration::orchestration::lifecycle::task_initialization::TaskInitializer;
use tasker_orchestration::orchestration::lifecycle::{
    BatchProcessingError, BatchProcessingService,
};
use tasker_orchestration::orchestration::viable_step_discovery::ViableStepDiscovery;
use tasker_shared::messaging::execution_types::{
    BatchProcessingOutcome, CursorConfig, StepExecutionResult,
};
use tasker_shared::models::core::task_request::TaskRequest;
use tasker_shared::models::WorkflowStep;
use tasker_shared::registry::TaskHandlerRegistry;
use tasker_shared::state_machine::{StepEvent, StepStateMachine};
use tasker_shared::system_context::SystemContext;

/// Get the path to task template fixtures in the workspace root
fn fixture_path() -> String {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&manifest_dir)
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("tests/fixtures/task_templates/rust")
        .to_string_lossy()
        .to_string()
}

/// Setup helper: creates BatchProcessingService with template registration
async fn setup_batch_service(
    pool: PgPool,
) -> Result<(
    BatchProcessingService,
    ViableStepDiscovery,
    TaskInitializer,
    Arc<SystemContext>,
)> {
    let registry = TaskHandlerRegistry::new(pool.clone());
    registry
        .discover_and_register_templates(&fixture_path())
        .await?;

    let system_context = Arc::new(SystemContext::with_pool(pool.clone()).await?);
    let step_enqueuer = Arc::new(StepEnqueuerService::new(system_context.clone()).await?);
    let task_initializer = TaskInitializer::new(system_context.clone(), step_enqueuer);
    let batch_service = BatchProcessingService::new(system_context.clone());
    let discovery = ViableStepDiscovery::new(system_context.clone());

    Ok((batch_service, discovery, task_initializer, system_context))
}

/// Create a task from the large_dataset_processor template
async fn create_batch_task(initializer: &TaskInitializer) -> Result<Uuid> {
    let request = TaskRequest {
        name: "large_dataset_processor".to_string(),
        namespace: "data_processing".to_string(),
        version: "1.0.0".to_string(),
        context: json!({
            "_test_run_id": Uuid::now_v7().to_string(),
            "dataset_size": 5000,
            "dataset_name": "users",
            "processing_mode": "parallel"
        }),
        correlation_id: Uuid::now_v7(),
        parent_correlation_id: None,
        initiator: "batch_test".to_string(),
        source_system: "integration_test".to_string(),
        reason: "Testing BatchProcessingService".to_string(),
        tags: vec!["test".to_string()],
        requested_at: chrono::Utc::now().naive_utc(),
        options: None,
        priority: Some(5),
        idempotency_key: None,
    };
    let result = initializer.create_task_from_request(request).await?;
    Ok(result.task_uuid)
}

/// Complete a step through the state machine lifecycle and return the step
async fn complete_step_and_get(
    pool: &PgPool,
    system_context: &Arc<SystemContext>,
    task_uuid: Uuid,
    step_name: &str,
    result_data: serde_json::Value,
) -> Result<WorkflowStep> {
    let step = WorkflowStep::find_step_by_name(pool, task_uuid, step_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Step '{}' not found", step_name))?;

    let mut sm = StepStateMachine::new(step.clone(), system_context.clone());
    sm.transition(StepEvent::Enqueue).await?;
    sm.transition(StepEvent::Start).await?;
    sm.transition(StepEvent::EnqueueForOrchestration(Some(
        result_data.clone(),
    )))
    .await?;
    sm.transition(StepEvent::Complete(None)).await?;

    // Re-fetch and mark as processed so the step result is persisted
    let mut updated = WorkflowStep::find_by_id(pool, step.workflow_step_uuid)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Step disappeared after completion"))?;
    updated.mark_processed(pool, Some(result_data)).await?;

    Ok(updated)
}

/// Build a StepExecutionResult with a BatchProcessingOutcome embedded
fn make_batch_result(step_uuid: Uuid, outcome: &BatchProcessingOutcome) -> StepExecutionResult {
    let outcome_json = serde_json::to_value(outcome).expect("outcome should serialize");
    StepExecutionResult {
        step_uuid,
        success: true,
        result: json!({"batch_processing_outcome": outcome_json}),
        status: "completed".to_string(),
        ..Default::default()
    }
}

/// Helper to build cursor configs for testing
fn make_cursor_configs(count: usize, batch_size: u32) -> Vec<CursorConfig> {
    (0..count)
        .map(|i| CursorConfig {
            batch_id: format!("{:03}", i + 1),
            start_cursor: json!(i as u32 * batch_size),
            end_cursor: json!((i as u32 + 1) * batch_size),
            batch_size,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests: NoBatches outcome
// ---------------------------------------------------------------------------

/// NoBatches creates a placeholder worker
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_batchable_no_batches(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_batch_service(pool.clone()).await?;
    let task_uuid = create_batch_task(&initializer).await?;

    // Complete analyze_dataset with NoBatches outcome
    let batchable_step = complete_step_and_get(
        &pool,
        &ctx,
        task_uuid,
        "analyze_dataset",
        json!({"batch_processing_outcome": {"type": "no_batches"}}),
    )
    .await?;

    let step_result = make_batch_result(
        batchable_step.workflow_step_uuid,
        &BatchProcessingOutcome::NoBatches,
    );

    let outcome = service
        .process_batchable_step(task_uuid, &batchable_step, &step_result)
        .await?;

    assert!(
        matches!(outcome, BatchProcessingOutcome::NoBatches),
        "Should return NoBatches"
    );

    // Verify a placeholder worker was created (should have batch_dependency edge)
    let worker_edges = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM tasker.workflow_step_edges
        WHERE from_step_uuid = $1 AND name = 'batch_dependency'
        "#,
        batchable_step.workflow_step_uuid
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(worker_edges, 1, "One placeholder worker edge should exist");

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests: CreateBatches outcome
// ---------------------------------------------------------------------------

/// CreateBatches with 3 cursors creates 3 workers
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_batchable_create_batches(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_batch_service(pool.clone()).await?;
    let task_uuid = create_batch_task(&initializer).await?;

    let cursors = make_cursor_configs(3, 1000);
    let batchable_step = complete_step_and_get(
        &pool,
        &ctx,
        task_uuid,
        "analyze_dataset",
        json!({"batch_processing_outcome": {
            "type": "create_batches",
            "worker_template_name": "process_batch",
            "worker_count": 3,
            "cursor_configs": cursors,
            "total_items": 3000
        }}),
    )
    .await?;

    let outcome_data = BatchProcessingOutcome::CreateBatches {
        worker_template_name: "process_batch".to_string(),
        worker_count: 3,
        cursor_configs: make_cursor_configs(3, 1000),
        total_items: 3000,
    };
    let step_result = make_batch_result(batchable_step.workflow_step_uuid, &outcome_data);

    let outcome = service
        .process_batchable_step(task_uuid, &batchable_step, &step_result)
        .await?;

    match outcome {
        BatchProcessingOutcome::CreateBatches { worker_count, .. } => {
            assert_eq!(worker_count, 3, "Should report 3 workers");
        }
        other => panic!("Expected CreateBatches, got {:?}", other),
    }

    // Verify 3 batch_dependency edges from analyze_dataset
    let worker_edges = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM tasker.workflow_step_edges
        WHERE from_step_uuid = $1 AND name = 'batch_dependency'
        "#,
        batchable_step.workflow_step_uuid
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(worker_edges, 3, "Three worker edges should exist");

    Ok(())
}

/// Convergence step (aggregate_results) is created for batch workflows
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_batch_creates_convergence_step(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_batch_service(pool.clone()).await?;
    let task_uuid = create_batch_task(&initializer).await?;

    let batchable_step = complete_step_and_get(
        &pool,
        &ctx,
        task_uuid,
        "analyze_dataset",
        json!({"batch_processing_outcome": {"type": "no_batches"}}),
    )
    .await?;

    let step_result = make_batch_result(
        batchable_step.workflow_step_uuid,
        &BatchProcessingOutcome::NoBatches,
    );

    service
        .process_batchable_step(task_uuid, &batchable_step, &step_result)
        .await?;

    // Verify aggregate_results was created as a convergence step
    let convergence_step =
        WorkflowStep::find_step_by_name(&pool, task_uuid, "aggregate_results").await?;

    assert!(
        convergence_step.is_some(),
        "aggregate_results convergence step should be created"
    );

    Ok(())
}

/// Worker-to-convergence edges are properly wired
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_batch_worker_edges_created(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_batch_service(pool.clone()).await?;
    let task_uuid = create_batch_task(&initializer).await?;

    let cursors = make_cursor_configs(2, 1000);
    let batchable_step = complete_step_and_get(
        &pool,
        &ctx,
        task_uuid,
        "analyze_dataset",
        json!({"batch_processing_outcome": {
            "type": "create_batches",
            "worker_template_name": "process_batch",
            "worker_count": 2,
            "cursor_configs": cursors,
            "total_items": 2000
        }}),
    )
    .await?;

    let outcome_data = BatchProcessingOutcome::CreateBatches {
        worker_template_name: "process_batch".to_string(),
        worker_count: 2,
        cursor_configs: make_cursor_configs(2, 1000),
        total_items: 2000,
    };
    let step_result = make_batch_result(batchable_step.workflow_step_uuid, &outcome_data);

    service
        .process_batchable_step(task_uuid, &batchable_step, &step_result)
        .await?;

    // Verify worker_to_convergence edges exist
    let convergence_edges = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM tasker.workflow_step_edges
        WHERE name = 'worker_to_convergence'
          AND to_step_uuid IN (
              SELECT ws.workflow_step_uuid
              FROM tasker.workflow_steps ws
              JOIN tasker.named_steps ns ON ws.named_step_uuid = ns.named_step_uuid
              WHERE ws.task_uuid = $1 AND ns.name = 'aggregate_results'
          )
        "#,
        task_uuid
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(
        convergence_edges, 2,
        "Two worker_to_convergence edges should point to aggregate_results"
    );

    Ok(())
}

/// Idempotency: second call returns same outcome without creating duplicates
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_batch_idempotency(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_batch_service(pool.clone()).await?;
    let task_uuid = create_batch_task(&initializer).await?;

    let batchable_step = complete_step_and_get(
        &pool,
        &ctx,
        task_uuid,
        "analyze_dataset",
        json!({"batch_processing_outcome": {"type": "no_batches"}}),
    )
    .await?;

    let step_result = make_batch_result(
        batchable_step.workflow_step_uuid,
        &BatchProcessingOutcome::NoBatches,
    );

    // First call
    service
        .process_batchable_step(task_uuid, &batchable_step, &step_result)
        .await?;

    // Second call should succeed idempotently
    let result2 = service
        .process_batchable_step(task_uuid, &batchable_step, &step_result)
        .await?;

    assert!(
        matches!(result2, BatchProcessingOutcome::NoBatches),
        "Idempotent call should return same outcome"
    );

    // Verify still only 1 worker edge (not duplicated)
    let edge_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM tasker.workflow_step_edges
        WHERE from_step_uuid = $1 AND name = 'batch_dependency'
        "#,
        batchable_step.workflow_step_uuid
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(
        edge_count, 1,
        "Idempotent call should not create duplicate workers"
    );

    Ok(())
}

/// Workers are named following the {template}_{batch_id} convention
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_batch_worker_names(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_batch_service(pool.clone()).await?;
    let task_uuid = create_batch_task(&initializer).await?;

    let cursors = make_cursor_configs(2, 1000);
    let batchable_step = complete_step_and_get(
        &pool,
        &ctx,
        task_uuid,
        "analyze_dataset",
        json!({"batch_processing_outcome": {
            "type": "create_batches",
            "worker_template_name": "process_batch",
            "worker_count": 2,
            "cursor_configs": cursors,
            "total_items": 2000
        }}),
    )
    .await?;

    let outcome_data = BatchProcessingOutcome::CreateBatches {
        worker_template_name: "process_batch".to_string(),
        worker_count: 2,
        cursor_configs: make_cursor_configs(2, 1000),
        total_items: 2000,
    };
    let step_result = make_batch_result(batchable_step.workflow_step_uuid, &outcome_data);

    service
        .process_batchable_step(task_uuid, &batchable_step, &step_result)
        .await?;

    // Verify worker names follow convention: process_batch_{batch_id}
    let worker_step_1 =
        WorkflowStep::find_step_by_name(&pool, task_uuid, "process_batch_001").await?;
    let worker_step_2 =
        WorkflowStep::find_step_by_name(&pool, task_uuid, "process_batch_002").await?;

    assert!(
        worker_step_1.is_some(),
        "Worker process_batch_001 should exist"
    );
    assert!(
        worker_step_2.is_some(),
        "Worker process_batch_002 should exist"
    );

    Ok(())
}

/// After batch processing, workers become viable steps
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_batch_workers_become_viable(pool: PgPool) -> Result<()> {
    let (service, discovery, initializer, ctx) = setup_batch_service(pool.clone()).await?;
    let task_uuid = create_batch_task(&initializer).await?;

    let cursors = make_cursor_configs(2, 1000);
    let batchable_step = complete_step_and_get(
        &pool,
        &ctx,
        task_uuid,
        "analyze_dataset",
        json!({"batch_processing_outcome": {
            "type": "create_batches",
            "worker_template_name": "process_batch",
            "worker_count": 2,
            "cursor_configs": cursors,
            "total_items": 2000
        }}),
    )
    .await?;

    let outcome_data = BatchProcessingOutcome::CreateBatches {
        worker_template_name: "process_batch".to_string(),
        worker_count: 2,
        cursor_configs: make_cursor_configs(2, 1000),
        total_items: 2000,
    };
    let step_result = make_batch_result(batchable_step.workflow_step_uuid, &outcome_data);

    service
        .process_batchable_step(task_uuid, &batchable_step, &step_result)
        .await?;

    // Workers should be discoverable as viable steps
    let viable = discovery.find_viable_steps(task_uuid).await?;
    let viable_names: Vec<&str> = viable.iter().map(|s| s.name.as_str()).collect();

    assert!(
        viable_names.contains(&"process_batch_001"),
        "Worker process_batch_001 should be viable. Found: {:?}",
        viable_names
    );
    assert!(
        viable_names.contains(&"process_batch_002"),
        "Worker process_batch_002 should be viable. Found: {:?}",
        viable_names
    );

    Ok(())
}

/// Non-batchable step returns error
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_batch_processing_error_invalid_result(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_batch_service(pool.clone()).await?;
    let task_uuid = create_batch_task(&initializer).await?;

    let batchable_step = complete_step_and_get(
        &pool,
        &ctx,
        task_uuid,
        "analyze_dataset",
        json!({"some_other_result": true}),
    )
    .await?;

    // Create a step result WITHOUT batch_processing_outcome
    let step_result = StepExecutionResult {
        step_uuid: batchable_step.workflow_step_uuid,
        success: true,
        result: json!({"no_batch_outcome_here": true}),
        status: "completed".to_string(),
        ..Default::default()
    };

    let result = service
        .process_batchable_step(task_uuid, &batchable_step, &step_result)
        .await;

    assert!(
        result.is_err(),
        "Should fail when result lacks batch_processing_outcome"
    );
    match result.unwrap_err() {
        BatchProcessingError::ResultParsing(msg) => {
            assert!(
                msg.contains("batch processing outcome"),
                "Error should mention missing outcome: {}",
                msg
            );
        }
        other => panic!("Expected ResultParsing, got {:?}", other),
    }

    Ok(())
}

/// Worker step initialization contains cursor data
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_batch_worker_inputs_contain_cursor(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_batch_service(pool.clone()).await?;
    let task_uuid = create_batch_task(&initializer).await?;

    let cursors = make_cursor_configs(1, 500);
    let batchable_step = complete_step_and_get(
        &pool,
        &ctx,
        task_uuid,
        "analyze_dataset",
        json!({"batch_processing_outcome": {
            "type": "create_batches",
            "worker_template_name": "process_batch",
            "worker_count": 1,
            "cursor_configs": cursors,
            "total_items": 500
        }}),
    )
    .await?;

    let outcome_data = BatchProcessingOutcome::CreateBatches {
        worker_template_name: "process_batch".to_string(),
        worker_count: 1,
        cursor_configs: make_cursor_configs(1, 500),
        total_items: 500,
    };
    let step_result = make_batch_result(batchable_step.workflow_step_uuid, &outcome_data);

    service
        .process_batchable_step(task_uuid, &batchable_step, &step_result)
        .await?;

    // Fetch the created worker and check its inputs contain cursor data
    let worker = WorkflowStep::find_step_by_name(&pool, task_uuid, "process_batch_001")
        .await?
        .expect("Worker should exist");

    let inputs = worker.inputs.expect("Worker should have inputs");
    assert!(
        inputs.get("cursor").is_some(),
        "Worker inputs should contain cursor configuration. Got: {}",
        inputs
    );

    Ok(())
}
