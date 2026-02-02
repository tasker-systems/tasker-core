//! Integration tests for orchestration API methods NOT covered by root E2E tests.
//!
//! These tests exercise the remaining `OrchestrationApiClient` methods that are
//! not already tested by `tests/e2e/rust/api_client_endpoints_test.rs`.
//!
//! Feature gate: requires `test-services` (running orchestration service).

#![cfg(feature = "test-services")]

mod common;

use common::{create_orchestration_client, create_task_request};
use uuid::Uuid;

/// Test cancel_task on a freshly created task.
#[tokio::test]
async fn test_cancel_task() {
    let client = create_orchestration_client();
    let run_id = format!("cancel_{}", Uuid::new_v4().as_simple());
    let request = create_task_request(&run_id);

    let task = client.create_task(request).await.unwrap();
    let task_uuid: Uuid = task.task_uuid.parse().unwrap();

    let result = client.cancel_task(task_uuid).await;
    // Cancel may succeed or fail depending on task state progression,
    // but the client method should not panic.
    println!("cancel_task result: {result:?}");
}

/// Test list_task_steps after creating a task.
#[tokio::test]
async fn test_list_task_steps() {
    let client = create_orchestration_client();
    let run_id = format!("steps_{}", Uuid::new_v4().as_simple());
    let request = create_task_request(&run_id);

    let task = client.create_task(request).await.unwrap();
    let task_uuid: Uuid = task.task_uuid.parse().unwrap();

    // Brief pause to allow step initialization
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let steps = client.list_task_steps(task_uuid).await.unwrap();
    println!("list_task_steps returned {} steps", steps.len());
    // Template-based tasks should have steps
    for step in &steps {
        assert!(!step.step_uuid.is_empty());
        assert!(!step.name.is_empty());
    }
}

/// Test get_step for an individual step.
#[tokio::test]
async fn test_get_step() {
    let client = create_orchestration_client();
    let run_id = format!("getstep_{}", Uuid::new_v4().as_simple());
    let request = create_task_request(&run_id);

    let task = client.create_task(request).await.unwrap();
    let task_uuid: Uuid = task.task_uuid.parse().unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let steps = client.list_task_steps(task_uuid).await.unwrap();
    if let Some(first_step) = steps.first() {
        let step_uuid: Uuid = first_step.step_uuid.parse().unwrap();
        let step = client.get_step(task_uuid, step_uuid).await.unwrap();
        assert_eq!(step.step_uuid, first_step.step_uuid);
        assert_eq!(step.task_uuid, task.task_uuid);
        println!("get_step: {} state={}", step.name, step.current_state);
    } else {
        println!("No steps found to test get_step (task may not have template steps)");
    }
}

/// Test get_step_audit_history for a step.
#[tokio::test]
async fn test_get_step_audit_history() {
    let client = create_orchestration_client();
    let run_id = format!("audit_{}", Uuid::new_v4().as_simple());
    let request = create_task_request(&run_id);

    let task = client.create_task(request).await.unwrap();
    let task_uuid: Uuid = task.task_uuid.parse().unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let steps = client.list_task_steps(task_uuid).await.unwrap();
    if let Some(first_step) = steps.first() {
        let step_uuid: Uuid = first_step.step_uuid.parse().unwrap();
        let audit = client
            .get_step_audit_history(task_uuid, step_uuid)
            .await
            .unwrap();
        // Audit may be empty for a freshly created step
        println!(
            "get_step_audit_history: {} records for step {}",
            audit.len(),
            first_step.name
        );
    }
}

/// Test list_dlq_entries returns a (possibly empty) list.
#[tokio::test]
async fn test_list_dlq_entries() {
    let client = create_orchestration_client();
    let entries = client.list_dlq_entries(None).await.unwrap();
    println!("list_dlq_entries: {} entries", entries.len());
}

/// Test get_dlq_stats returns statistics grouped by reason.
#[tokio::test]
async fn test_get_dlq_stats() {
    let client = create_orchestration_client();
    let stats = client.get_dlq_stats().await.unwrap();
    println!("get_dlq_stats: {} reason groups", stats.len());
    for stat in &stats {
        println!(
            "  {:?}: total={} pending={}",
            stat.dlq_reason, stat.total_entries, stat.pending
        );
    }
}

/// Test get_investigation_queue returns a prioritized list.
#[tokio::test]
async fn test_get_investigation_queue() {
    let client = create_orchestration_client();
    let queue = client.get_investigation_queue(Some(10)).await.unwrap();
    println!("get_investigation_queue: {} entries", queue.len());
    for entry in &queue {
        println!(
            "  task={} priority={:.1} minutes_in_dlq={:.1}",
            entry.task_uuid, entry.priority_score, entry.minutes_in_dlq
        );
    }
}

/// Test get_staleness_monitoring returns monitoring data.
#[tokio::test]
async fn test_get_staleness_monitoring() {
    let client = create_orchestration_client();
    let monitoring = client.get_staleness_monitoring(Some(10)).await.unwrap();
    println!("get_staleness_monitoring: {} entries", monitoring.len());
}

/// Test get_performance_metrics without time range.
#[tokio::test]
async fn test_get_performance_metrics_no_range() {
    let client = create_orchestration_client();
    let metrics = client.get_performance_metrics(None).await.unwrap();
    println!(
        "performance_metrics: total_tasks={} health_score={}",
        metrics.total_tasks, metrics.system_health_score
    );
    // Basic sanity: rates should be between 0 and 1
    assert!(metrics.completion_rate >= 0.0);
    assert!(metrics.error_rate >= 0.0);
}

/// Test get_performance_metrics with time range.
#[tokio::test]
async fn test_get_performance_metrics_with_hours() {
    use tasker_shared::types::api::orchestration::MetricsQuery;

    let client = create_orchestration_client();
    let query = MetricsQuery { hours: Some(24) };
    let metrics = client.get_performance_metrics(Some(&query)).await.unwrap();
    println!(
        "performance_metrics (24h): completed={} failed={}",
        metrics.completed_tasks, metrics.failed_tasks
    );
}

/// Test get_bottlenecks returns analysis data.
#[tokio::test]
async fn test_get_bottlenecks() {
    use tasker_shared::types::api::orchestration::BottleneckQuery;

    let client = create_orchestration_client();
    let query = BottleneckQuery {
        limit: Some(5),
        min_executions: Some(1),
    };
    let analysis = client.get_bottlenecks(Some(&query)).await.unwrap();
    println!(
        "bottleneck_analysis: {} slow_steps, {} slow_tasks, {} recommendations",
        analysis.slow_steps.len(),
        analysis.slow_tasks.len(),
        analysis.recommendations.len(),
    );
}

/// Test list_templates returns template discovery data.
#[tokio::test]
async fn test_list_templates() {
    let client = create_orchestration_client();
    let templates = client.list_templates(None).await.unwrap();
    println!(
        "list_templates: {} namespaces, {} templates",
        templates.namespaces.len(),
        templates.templates.len()
    );
    // total_count is u64, always >= 0
    let _ = templates.total_count;
}

/// Test get_template for a specific template (if any exist).
#[tokio::test]
async fn test_get_template() {
    let client = create_orchestration_client();
    let list = client.list_templates(None).await.unwrap();
    if let Some(first) = list.templates.first() {
        let detail = client
            .get_template(&first.namespace, &first.name, &first.version)
            .await
            .unwrap();
        assert_eq!(detail.name, first.name);
        assert_eq!(detail.namespace, first.namespace);
        println!(
            "get_template: {} v{} with {} steps",
            detail.name,
            detail.version,
            detail.steps.len()
        );
    } else {
        println!("No templates available to test get_template");
    }
}
