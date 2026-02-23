//! # Decision Point Service Integration Tests (TAS-63)
//!
//! Tests for DecisionPointService using the approval_routing template:
//! - NoBranches outcome processing
//! - Single and multiple step creation
//! - Deferred convergence step creation
//! - Edge creation verification
//! - Error handling (task not found, invalid descendant)
//! - Integration with ViableStepDiscovery

use anyhow::Result;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use tasker_orchestration::orchestration::lifecycle::step_enqueuer_services::StepEnqueuerService;
use tasker_orchestration::orchestration::lifecycle::task_initialization::TaskInitializer;
use tasker_orchestration::orchestration::lifecycle::{
    DecisionPointProcessingError, DecisionPointService,
};
use tasker_orchestration::orchestration::viable_step_discovery::ViableStepDiscovery;
use tasker_shared::messaging::DecisionPointOutcome;
use tasker_shared::models::core::task_request::TaskRequest;
use tasker_shared::models::WorkflowStep;
use tasker_shared::registry::TaskTemplateRegistry;
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

/// Setup helper: creates DecisionPointService with template registration
async fn setup_decision_service(
    pool: PgPool,
) -> Result<(
    DecisionPointService,
    ViableStepDiscovery,
    TaskInitializer,
    Arc<SystemContext>,
)> {
    let registry = TaskTemplateRegistry::new(pool.clone());
    registry
        .discover_and_register_templates(&fixture_path())
        .await?;

    let system_context = Arc::new(SystemContext::with_pool(pool.clone()).await?);
    let step_enqueuer = Arc::new(StepEnqueuerService::new(system_context.clone()).await?);
    let task_initializer = TaskInitializer::new(system_context.clone(), step_enqueuer);
    let decision_service = DecisionPointService::new(system_context.clone());
    let discovery = ViableStepDiscovery::new(system_context.clone());

    Ok((
        decision_service,
        discovery,
        task_initializer,
        system_context,
    ))
}

/// Create a task from the approval_routing template
async fn create_approval_task(initializer: &TaskInitializer) -> Result<Uuid> {
    let request = TaskRequest {
        name: "approval_routing".to_string(),
        namespace: "conditional_approval_rust".to_string(),
        version: "1.0.0".to_string(),
        context: json!({
            "_test_run_id": Uuid::now_v7().to_string(),
            "amount": 500,
            "requester": "test_user"
        }),
        correlation_id: Uuid::now_v7(),
        parent_correlation_id: None,
        initiator: "decision_test".to_string(),
        source_system: "integration_test".to_string(),
        reason: "Testing DecisionPointService".to_string(),
        tags: vec!["test".to_string()],
        requested_at: chrono::Utc::now().naive_utc(),
        options: None,
        priority: Some(5),
        idempotency_key: None,
    };
    let result = initializer.create_task_from_request(request).await?;
    Ok(result.task_uuid)
}

/// Complete a step through the state machine lifecycle and persist results
async fn complete_step(
    pool: &PgPool,
    system_context: &Arc<SystemContext>,
    task_uuid: Uuid,
    step_name: &str,
    result_data: serde_json::Value,
) -> Result<Uuid> {
    let step = WorkflowStep::find_step_by_name(pool, task_uuid, step_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Step '{}' not found", step_name))?;
    let step_uuid = step.workflow_step_uuid;

    let mut sm = StepStateMachine::new(step, system_context.clone());
    sm.transition(StepEvent::Enqueue).await?;
    sm.transition(StepEvent::Start).await?;
    sm.transition(StepEvent::EnqueueForOrchestration(Some(
        result_data.clone(),
    )))
    .await?;
    sm.transition(StepEvent::Complete(None)).await?;

    // Persist results to workflow_steps.results column
    let mut updated_step = WorkflowStep::find_by_id(pool, step_uuid)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Step disappeared"))?;
    updated_step.mark_processed(pool, Some(result_data)).await?;

    Ok(step_uuid)
}

// ---------------------------------------------------------------------------
// Tests: process_decision_outcome
// ---------------------------------------------------------------------------

/// NoBranches outcome returns empty map, no new steps
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_decision_no_branches(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_decision_service(pool.clone()).await?;
    let task_uuid = create_approval_task(&initializer).await?;

    // Complete validate_request
    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "validate_request",
        json!({"valid": true}),
    )
    .await?;

    // Get the routing_decision step UUID and complete it
    let decision_step_uuid = complete_step(
        &pool,
        &ctx,
        task_uuid,
        "routing_decision",
        json!({"decision_point_outcome": {"type": "no_branches"}}),
    )
    .await?;

    // Process NoBranches outcome
    let result = service
        .process_decision_outcome(
            decision_step_uuid,
            task_uuid,
            DecisionPointOutcome::no_branches(),
        )
        .await?;

    assert!(result.is_empty(), "NoBranches should return empty mapping");

    Ok(())
}

/// CreateSteps with single step creates that step and returns its UUID
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_decision_single_step(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_decision_service(pool.clone()).await?;
    let task_uuid = create_approval_task(&initializer).await?;

    // Complete prerequisites
    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "validate_request",
        json!({"valid": true}),
    )
    .await?;
    let decision_step_uuid = complete_step(
        &pool,
        &ctx,
        task_uuid,
        "routing_decision",
        json!({"decision_point_outcome": {"type": "create_steps", "step_names": ["auto_approve"]}}),
    )
    .await?;

    let result = service
        .process_decision_outcome(
            decision_step_uuid,
            task_uuid,
            DecisionPointOutcome::create_steps(vec!["auto_approve".to_string()]),
        )
        .await?;

    // auto_approve + finalize_approval (deferred convergence) should be created
    assert!(
        result.contains_key("auto_approve"),
        "auto_approve should be in result mapping"
    );
    // Verify the created step exists in the database
    let created_step =
        WorkflowStep::find_by_id(&pool, *result.get("auto_approve").unwrap()).await?;
    assert!(created_step.is_some(), "Created step should exist in DB");

    Ok(())
}

/// CreateSteps with multiple steps creates all specified steps
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_decision_multiple_steps(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_decision_service(pool.clone()).await?;
    let task_uuid = create_approval_task(&initializer).await?;

    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "validate_request",
        json!({"valid": true}),
    )
    .await?;
    let decision_step_uuid = complete_step(
        &pool,
        &ctx,
        task_uuid,
        "routing_decision",
        json!({"decision_point_outcome": {"type": "create_steps", "step_names": ["manager_approval", "finance_review"]}}),
    )
    .await?;

    let result = service
        .process_decision_outcome(
            decision_step_uuid,
            task_uuid,
            DecisionPointOutcome::create_steps(vec![
                "manager_approval".to_string(),
                "finance_review".to_string(),
            ]),
        )
        .await?;

    assert!(
        result.contains_key("manager_approval"),
        "manager_approval should be created"
    );
    assert!(
        result.contains_key("finance_review"),
        "finance_review should be created"
    );

    Ok(())
}

/// Deferred convergence step (finalize_approval) is created when dependencies intersect
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_decision_creates_convergence(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_decision_service(pool.clone()).await?;
    let task_uuid = create_approval_task(&initializer).await?;

    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "validate_request",
        json!({"valid": true}),
    )
    .await?;
    let decision_step_uuid = complete_step(
        &pool,
        &ctx,
        task_uuid,
        "routing_decision",
        json!({"decision_point_outcome": {"type": "create_steps", "step_names": ["auto_approve"]}}),
    )
    .await?;

    let result = service
        .process_decision_outcome(
            decision_step_uuid,
            task_uuid,
            DecisionPointOutcome::create_steps(vec!["auto_approve".to_string()]),
        )
        .await?;

    // finalize_approval depends on [auto_approve, manager_approval, finance_review]
    // Since auto_approve is being created, finalize_approval should also be created
    assert!(
        result.contains_key("finalize_approval"),
        "Deferred convergence step finalize_approval should be created when dependency intersects"
    );

    Ok(())
}

/// TaskNotFound error for nonexistent task
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_decision_task_not_found(pool: PgPool) -> Result<()> {
    let (service, _discovery, _initializer, _ctx) = setup_decision_service(pool).await?;

    let nonexistent_task = Uuid::now_v7();
    let result = service
        .process_decision_outcome(
            Uuid::now_v7(),
            nonexistent_task,
            DecisionPointOutcome::create_steps(vec!["auto_approve".to_string()]),
        )
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        DecisionPointProcessingError::TaskNotFound(uuid) => {
            assert_eq!(uuid, nonexistent_task);
        }
        other => panic!("Expected TaskNotFound, got {:?}", other),
    }

    Ok(())
}

/// Invalid descendant step name returns error
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_decision_invalid_descendant(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_decision_service(pool.clone()).await?;
    let task_uuid = create_approval_task(&initializer).await?;

    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "validate_request",
        json!({"valid": true}),
    )
    .await?;
    let decision_step_uuid = complete_step(
        &pool,
        &ctx,
        task_uuid,
        "routing_decision",
        json!({"decision_point_outcome": {"type": "create_steps", "step_names": ["nonexistent_step"]}}),
    )
    .await?;

    let result = service
        .process_decision_outcome(
            decision_step_uuid,
            task_uuid,
            DecisionPointOutcome::create_steps(vec!["nonexistent_step".to_string()]),
        )
        .await;

    assert!(result.is_err(), "Should fail for non-descendant step");
    match result.unwrap_err() {
        DecisionPointProcessingError::InvalidDescendant { step_name, .. } => {
            assert_eq!(step_name, "nonexistent_step");
        }
        other => panic!("Expected InvalidDescendant, got {:?}", other),
    }

    Ok(())
}

/// Created steps become discoverable by ViableStepDiscovery
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_decision_created_steps_become_viable(pool: PgPool) -> Result<()> {
    let (service, discovery, initializer, ctx) = setup_decision_service(pool.clone()).await?;
    let task_uuid = create_approval_task(&initializer).await?;

    // Complete prerequisites
    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "validate_request",
        json!({"valid": true}),
    )
    .await?;
    let decision_step_uuid = complete_step(
        &pool,
        &ctx,
        task_uuid,
        "routing_decision",
        json!({"decision_point_outcome": {"type": "create_steps", "step_names": ["auto_approve"]}}),
    )
    .await?;

    // Create steps from decision
    let step_mapping = service
        .process_decision_outcome(
            decision_step_uuid,
            task_uuid,
            DecisionPointOutcome::create_steps(vec!["auto_approve".to_string()]),
        )
        .await?;

    assert!(!step_mapping.is_empty(), "Steps should have been created");

    // Now verify that auto_approve is discoverable as a viable step
    let viable = discovery.find_viable_steps(task_uuid).await?;
    let viable_names: Vec<&str> = viable.iter().map(|s| s.name.as_str()).collect();

    assert!(
        viable_names.contains(&"auto_approve"),
        "Dynamically created auto_approve should be viable. Found: {:?}",
        viable_names
    );

    Ok(())
}

/// Edges from decision step to created steps are verified in DB
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_decision_creates_edges(pool: PgPool) -> Result<()> {
    let (service, _discovery, initializer, ctx) = setup_decision_service(pool.clone()).await?;
    let task_uuid = create_approval_task(&initializer).await?;

    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "validate_request",
        json!({"valid": true}),
    )
    .await?;
    let decision_step_uuid = complete_step(
        &pool,
        &ctx,
        task_uuid,
        "routing_decision",
        json!({"decision_point_outcome": {"type": "create_steps", "step_names": ["auto_approve"]}}),
    )
    .await?;

    let step_mapping = service
        .process_decision_outcome(
            decision_step_uuid,
            task_uuid,
            DecisionPointOutcome::create_steps(vec!["auto_approve".to_string()]),
        )
        .await?;

    // Verify edge exists from routing_decision to auto_approve
    let auto_approve_uuid = step_mapping.get("auto_approve").unwrap();
    let edge_exists = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM tasker.workflow_step_edges
            WHERE from_step_uuid = $1 AND to_step_uuid = $2
        ) AS "exists!"
        "#,
        decision_step_uuid,
        *auto_approve_uuid
    )
    .fetch_one(&pool)
    .await?;

    assert!(
        edge_exists,
        "Edge should exist from decision step to auto_approve"
    );

    Ok(())
}
