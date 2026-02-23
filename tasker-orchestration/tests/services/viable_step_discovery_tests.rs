//! # Viable Step Discovery Integration Tests (TAS-63)
//!
//! Tests for ViableStepDiscovery using real task templates and database:
//! - Linear workflow progression (mathematical_sequence)
//! - Diamond parallelism and convergence (diamond_pattern)
//! - Dependency level calculation
//! - Task execution context loading
//! - Task readiness summary
//! - Step execution request building

use anyhow::Result;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use tasker_orchestration::orchestration::lifecycle::step_enqueuer_services::StepEnqueuerService;
use tasker_orchestration::orchestration::lifecycle::task_initialization::TaskInitializer;
use tasker_orchestration::orchestration::viable_step_discovery::ViableStepDiscovery;
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

/// Setup helper: creates ViableStepDiscovery with template registration
async fn setup_discovery(
    pool: PgPool,
) -> Result<(ViableStepDiscovery, TaskInitializer, Arc<SystemContext>)> {
    let registry = TaskTemplateRegistry::new(pool.clone());
    registry
        .discover_and_register_templates(&fixture_path())
        .await?;

    let system_context = Arc::new(SystemContext::with_pool(pool.clone()).await?);
    let step_enqueuer = Arc::new(StepEnqueuerService::new(system_context.clone()).await?);
    let task_initializer = TaskInitializer::new(system_context.clone(), step_enqueuer);
    let discovery = ViableStepDiscovery::new(system_context.clone());

    Ok((discovery, task_initializer, system_context))
}

/// Create a task from the mathematical_sequence template (4 linear steps)
async fn create_linear_task(initializer: &TaskInitializer) -> Result<Uuid> {
    let request = TaskRequest {
        name: "mathematical_sequence".to_string(),
        namespace: "rust_e2e_linear".to_string(),
        version: "1.0.0".to_string(),
        context: json!({
            "_test_run_id": Uuid::now_v7().to_string(),
            "input": 6
        }),
        correlation_id: Uuid::now_v7(),
        parent_correlation_id: None,
        initiator: "discovery_test".to_string(),
        source_system: "integration_test".to_string(),
        reason: "Testing ViableStepDiscovery".to_string(),
        tags: vec!["test".to_string()],
        requested_at: chrono::Utc::now().naive_utc(),
        options: None,
        priority: Some(5),
        idempotency_key: None,
    };
    let result = initializer.create_task_from_request(request).await?;
    Ok(result.task_uuid)
}

/// Create a task from the diamond_pattern template (4 steps with parallelism)
async fn create_diamond_task(initializer: &TaskInitializer) -> Result<Uuid> {
    let request = TaskRequest {
        name: "diamond_pattern".to_string(),
        namespace: "rust_e2e_diamond".to_string(),
        version: "1.0.0".to_string(),
        context: json!({
            "_test_run_id": Uuid::now_v7().to_string(),
            "input": 6
        }),
        correlation_id: Uuid::now_v7(),
        parent_correlation_id: None,
        initiator: "discovery_test".to_string(),
        source_system: "integration_test".to_string(),
        reason: "Testing ViableStepDiscovery diamond".to_string(),
        tags: vec!["test".to_string()],
        requested_at: chrono::Utc::now().naive_utc(),
        options: None,
        priority: Some(5),
        idempotency_key: None,
    };
    let result = initializer.create_task_from_request(request).await?;
    Ok(result.task_uuid)
}

/// Complete a step through the full state machine lifecycle:
/// Pending → Enqueued → InProgress → EnqueuedForOrchestration → Complete
/// Also marks the step as processed with results stored in the workflow_steps table.
async fn complete_step(
    pool: &PgPool,
    system_context: &Arc<SystemContext>,
    task_uuid: Uuid,
    step_name: &str,
    result_data: serde_json::Value,
) -> Result<()> {
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

    // Also persist results to the workflow_steps.results column
    let mut updated_step = WorkflowStep::find_by_id(pool, step_uuid)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Step disappeared"))?;
    updated_step.mark_processed(pool, Some(result_data)).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests: find_viable_steps
// ---------------------------------------------------------------------------

/// After initialization, only the first step (no dependencies) should be viable
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_find_viable_steps_linear_initial(pool: PgPool) -> Result<()> {
    let (discovery, initializer, _ctx) = setup_discovery(pool).await?;
    let task_uuid = create_linear_task(&initializer).await?;

    let viable = discovery.find_viable_steps(task_uuid).await?;

    assert_eq!(
        viable.len(),
        1,
        "Only first step should be viable initially"
    );
    assert_eq!(viable[0].name, "linear_step_1");
    assert!(viable[0].dependencies_satisfied);

    Ok(())
}

/// After completing step 1, step 2 becomes viable
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_find_viable_steps_linear_after_completion(pool: PgPool) -> Result<()> {
    let (discovery, initializer, ctx) = setup_discovery(pool.clone()).await?;
    let task_uuid = create_linear_task(&initializer).await?;

    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "linear_step_1",
        json!({"result": 36}),
    )
    .await?;

    let viable = discovery.find_viable_steps(task_uuid).await?;

    assert_eq!(
        viable.len(),
        1,
        "Only step 2 should be viable after step 1 completes"
    );
    assert_eq!(viable[0].name, "linear_step_2");

    Ok(())
}

/// In diamond pattern, after start completes, both branches become viable
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_find_viable_steps_diamond_parallel(pool: PgPool) -> Result<()> {
    let (discovery, initializer, ctx) = setup_discovery(pool.clone()).await?;
    let task_uuid = create_diamond_task(&initializer).await?;

    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "diamond_start",
        json!({"result": 36}),
    )
    .await?;

    let viable = discovery.find_viable_steps(task_uuid).await?;

    assert_eq!(
        viable.len(),
        2,
        "Both branches should be viable after start completes"
    );
    let names: Vec<&str> = viable.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"diamond_branch_b"));
    assert!(names.contains(&"diamond_branch_c"));

    Ok(())
}

/// Convergence step only becomes viable after both branches complete
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_find_viable_steps_diamond_convergence(pool: PgPool) -> Result<()> {
    let (discovery, initializer, ctx) = setup_discovery(pool.clone()).await?;
    let task_uuid = create_diamond_task(&initializer).await?;

    // Complete start
    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "diamond_start",
        json!({"result": 36}),
    )
    .await?;

    // Complete only one branch
    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "diamond_branch_b",
        json!({"result": 1296}),
    )
    .await?;

    let viable = discovery.find_viable_steps(task_uuid).await?;
    let names: Vec<&str> = viable.iter().map(|s| s.name.as_str()).collect();
    assert!(
        !names.contains(&"diamond_end"),
        "Convergence step should NOT be viable with only one branch complete"
    );
    assert!(
        names.contains(&"diamond_branch_c"),
        "Remaining branch should be viable"
    );

    // Complete second branch
    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "diamond_branch_c",
        json!({"result": 1296}),
    )
    .await?;

    let viable = discovery.find_viable_steps(task_uuid).await?;
    assert_eq!(viable.len(), 1);
    assert_eq!(
        viable[0].name, "diamond_end",
        "Convergence step should be viable after both branches complete"
    );

    Ok(())
}

/// Returns empty vec for a nonexistent task
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_find_viable_steps_empty_for_nonexistent_task(pool: PgPool) -> Result<()> {
    let (discovery, _initializer, _ctx) = setup_discovery(pool).await?;

    let viable = discovery.find_viable_steps(Uuid::now_v7()).await?;
    assert!(
        viable.is_empty(),
        "Should return empty for nonexistent task"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests: get_dependency_levels
// ---------------------------------------------------------------------------

/// Linear workflow: 4 steps at levels 0, 1, 2, 3
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_get_dependency_levels_linear(pool: PgPool) -> Result<()> {
    let (discovery, initializer, _ctx) = setup_discovery(pool.clone()).await?;
    let task_uuid = create_linear_task(&initializer).await?;

    let levels = discovery.get_dependency_levels(task_uuid).await?;

    assert_eq!(levels.len(), 4, "Linear workflow should have 4 steps");

    // Find steps by name to verify levels
    let steps = WorkflowStep::list_by_task(&pool, task_uuid).await?;
    let mut level_values: Vec<i32> = steps
        .iter()
        .filter_map(|s| levels.get(&s.workflow_step_uuid).copied())
        .collect();
    level_values.sort();
    assert_eq!(level_values, vec![0, 1, 2, 3], "Should have levels 0-3");

    Ok(())
}

/// Diamond workflow: start=0, branches=1, end=2
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_get_dependency_levels_diamond(pool: PgPool) -> Result<()> {
    let (discovery, initializer, _ctx) = setup_discovery(pool.clone()).await?;
    let task_uuid = create_diamond_task(&initializer).await?;

    let levels = discovery.get_dependency_levels(task_uuid).await?;

    assert_eq!(levels.len(), 4, "Diamond workflow should have 4 steps");

    let mut level_values: Vec<i32> = levels.values().copied().collect();
    level_values.sort();
    // Start=0, two branches=1, end=2
    assert_eq!(level_values, vec![0, 1, 1, 2], "Diamond: 0,1,1,2");

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests: get_execution_context
// ---------------------------------------------------------------------------

/// Initial context: total_steps present, completed=0, ready>=1
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_get_execution_context_initial(pool: PgPool) -> Result<()> {
    let (discovery, initializer, _ctx) = setup_discovery(pool).await?;
    let task_uuid = create_linear_task(&initializer).await?;

    let context = discovery.get_execution_context(task_uuid).await?;

    let ctx = context.expect("Context should exist for valid task");
    assert_eq!(ctx.total_steps, 4, "Linear task has 4 steps");
    assert_eq!(ctx.completed_steps, 0, "No steps completed yet");
    assert!(ctx.ready_steps >= 1, "At least one step should be ready");

    Ok(())
}

/// After completing a step, completed count increments
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_get_execution_context_after_progress(pool: PgPool) -> Result<()> {
    let (discovery, initializer, ctx) = setup_discovery(pool.clone()).await?;
    let task_uuid = create_linear_task(&initializer).await?;

    complete_step(
        &pool,
        &ctx,
        task_uuid,
        "linear_step_1",
        json!({"result": 36}),
    )
    .await?;

    let context = discovery.get_execution_context(task_uuid).await?;
    let exec_ctx = context.expect("Context should exist");
    assert_eq!(exec_ctx.completed_steps, 1, "One step should be completed");

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests: get_task_readiness_summary
// ---------------------------------------------------------------------------

/// Readiness summary: correct categorization of ready, blocked, complete
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_get_task_readiness_summary(pool: PgPool) -> Result<()> {
    let (discovery, initializer, _ctx) = setup_discovery(pool).await?;
    let task_uuid = create_diamond_task(&initializer).await?;

    let summary = discovery.get_task_readiness_summary(task_uuid).await?;

    assert_eq!(summary.total_steps, 4, "Diamond has 4 steps");
    assert_eq!(summary.complete_steps, 0, "No steps complete initially");
    // diamond_start is ready (no deps), others are blocked
    assert!(
        summary.ready_steps >= 1,
        "At least diamond_start should be ready"
    );
    assert!(!summary.is_complete());
    assert!(!summary.has_failures());

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests: build_step_execution_requests
// ---------------------------------------------------------------------------

/// Build execution requests for initial viable steps
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_build_step_execution_requests(pool: PgPool) -> Result<()> {
    let (discovery, initializer, _ctx) = setup_discovery(pool.clone()).await?;
    let task_uuid = create_linear_task(&initializer).await?;

    let viable = discovery.find_viable_steps(task_uuid).await?;
    assert_eq!(viable.len(), 1);

    let registry = TaskTemplateRegistry::new(pool);
    let requests = discovery
        .build_step_execution_requests(task_uuid, &viable, &registry)
        .await?;

    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    assert_eq!(req.step_name, "linear_step_1");
    assert!(!req.handler_class.is_empty(), "Handler class should be set");
    assert_eq!(req.task_uuid, task_uuid);
    // Task context should contain our input
    assert_eq!(req.task_context.get("input"), Some(&json!(6)));
    // First step has no previous results
    assert!(req.previous_results.is_empty());

    Ok(())
}

/// After step 1 completes, step 2's request includes step 1's results as dependencies
#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_build_step_execution_requests_with_previous_results(pool: PgPool) -> Result<()> {
    let (discovery, initializer, ctx) = setup_discovery(pool.clone()).await?;
    let task_uuid = create_linear_task(&initializer).await?;

    // Complete step 1 with a result
    let step1_result = json!({"result": 36, "computed_value": 36});
    complete_step(&pool, &ctx, task_uuid, "linear_step_1", step1_result).await?;

    // Find viable steps (should be step 2)
    let viable = discovery.find_viable_steps(task_uuid).await?;
    assert_eq!(viable.len(), 1);
    assert_eq!(viable[0].name, "linear_step_2");

    let registry = TaskTemplateRegistry::new(pool);
    let requests = discovery
        .build_step_execution_requests(task_uuid, &viable, &registry)
        .await?;

    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    // Step 2 should have step 1's results as previous_results
    assert!(
        !req.previous_results.is_empty(),
        "Step 2 should have previous results from step 1"
    );
    assert!(
        req.previous_results.contains_key("linear_step_1"),
        "Previous results should contain linear_step_1"
    );

    Ok(())
}
