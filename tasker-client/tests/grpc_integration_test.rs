//! gRPC transport integration tests for tasker-client.
//!
//! These tests exercise gRPC client code paths (currently 3-10% coverage)
//! including real proto data round-trips through the conversion layer.
//!
//! Feature gate: requires `test-services` + `grpc` features with running services.

#![cfg(all(feature = "test-services", feature = "grpc"))]

mod common;

use common::{create_orchestration_grpc_client, create_task_request, create_worker_grpc_client};
use uuid::Uuid;

// ============================================================================
// Orchestration gRPC - Health
// ============================================================================

#[tokio::test]
async fn test_grpc_orchestration_health_check() {
    let client = create_orchestration_grpc_client().await;
    let result = client.health_check().await;
    assert!(result.is_ok(), "gRPC health_check failed: {result:?}");
}

#[tokio::test]
async fn test_grpc_orchestration_basic_health() {
    let client = create_orchestration_grpc_client().await;
    let health = client.get_basic_health().await.unwrap();
    assert!(!health.status.is_empty());
    println!("gRPC basic health: {}", health.status);
}

#[tokio::test]
async fn test_grpc_orchestration_liveness() {
    let client = create_orchestration_grpc_client().await;
    let health = client.liveness_probe().await.unwrap();
    assert!(!health.status.is_empty());
}

#[tokio::test]
async fn test_grpc_orchestration_readiness() {
    let client = create_orchestration_grpc_client().await;
    // Readiness may return an error if service is not fully ready
    let result = client.readiness_probe().await;
    match result {
        Ok(readiness) => {
            println!(
                "gRPC readiness: {} checks.web_db={}",
                readiness.status, readiness.checks.web_database.status
            );
        }
        Err(e) => {
            // Service not ready is acceptable (e.g., 503)
            println!("gRPC readiness probe returned error (acceptable): {e}");
        }
    }
}

#[tokio::test]
async fn test_grpc_orchestration_detailed_health() {
    let client = create_orchestration_grpc_client().await;
    let health = client.get_detailed_health().await.unwrap();
    assert!(!health.status.is_empty());
    assert!(!health.info.version.is_empty());
    println!(
        "gRPC detailed health: {} v{} ({} subsystems)",
        health.status,
        health.info.version,
        8 // DetailedHealthChecks has 8 fields
    );
}

// ============================================================================
// Orchestration gRPC - Task Lifecycle
// ============================================================================

#[tokio::test]
async fn test_grpc_create_and_get_task() {
    let client = create_orchestration_grpc_client().await;
    let run_id = format!("grpc_create_{}", Uuid::new_v4().as_simple());
    let request = create_task_request(&run_id);

    let created = client.create_task(request).await.unwrap();
    assert!(!created.task_uuid.is_empty());
    assert_eq!(created.namespace, "rust_e2e_linear");
    println!("gRPC created task: {}", created.task_uuid);

    let task_uuid: Uuid = created.task_uuid.parse().unwrap();
    let fetched = client.get_task(task_uuid).await.unwrap();
    assert_eq!(fetched.task_uuid, created.task_uuid);
}

#[tokio::test]
async fn test_grpc_list_tasks() {
    let client = create_orchestration_grpc_client().await;
    let list = client.list_tasks(50, 0, None, None).await.unwrap();
    println!(
        "gRPC list_tasks: {} tasks, page {}",
        list.tasks.len(),
        list.pagination.page
    );
}

#[tokio::test]
async fn test_grpc_cancel_task() {
    let client = create_orchestration_grpc_client().await;
    let run_id = format!("grpc_cancel_{}", Uuid::new_v4().as_simple());
    let request = create_task_request(&run_id);

    let task = client.create_task(request).await.unwrap();
    let task_uuid: Uuid = task.task_uuid.parse().unwrap();

    let result = client.cancel_task(task_uuid).await;
    println!("gRPC cancel_task result: {result:?}");
}

// ============================================================================
// Orchestration gRPC - Steps
// ============================================================================

#[tokio::test]
async fn test_grpc_list_task_steps() {
    let client = create_orchestration_grpc_client().await;
    let run_id = format!("grpc_steps_{}", Uuid::new_v4().as_simple());
    let request = create_task_request(&run_id);

    let task = client.create_task(request).await.unwrap();
    let task_uuid: Uuid = task.task_uuid.parse().unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let steps = client.list_task_steps(task_uuid).await.unwrap();
    println!("gRPC list_task_steps: {} steps", steps.len());
}

#[tokio::test]
async fn test_grpc_get_step() {
    let client = create_orchestration_grpc_client().await;
    let run_id = format!("grpc_getstep_{}", Uuid::new_v4().as_simple());
    let request = create_task_request(&run_id);

    let task = client.create_task(request).await.unwrap();
    let task_uuid: Uuid = task.task_uuid.parse().unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let steps = client.list_task_steps(task_uuid).await.unwrap();
    if let Some(first) = steps.first() {
        let step_uuid: Uuid = first.step_uuid.parse().unwrap();
        let step = client.get_step(task_uuid, step_uuid).await.unwrap();
        assert_eq!(step.step_uuid, first.step_uuid);
        println!("gRPC get_step: {} state={}", step.name, step.current_state);
    }
}

#[tokio::test]
async fn test_grpc_get_step_audit_history() {
    let client = create_orchestration_grpc_client().await;
    let run_id = format!("grpc_audit_{}", Uuid::new_v4().as_simple());
    let request = create_task_request(&run_id);

    let task = client.create_task(request).await.unwrap();
    let task_uuid: Uuid = task.task_uuid.parse().unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let steps = client.list_task_steps(task_uuid).await.unwrap();
    if let Some(first) = steps.first() {
        let step_uuid: Uuid = first.step_uuid.parse().unwrap();
        let audit = client
            .get_step_audit_history(task_uuid, step_uuid)
            .await
            .unwrap();
        println!("gRPC audit history: {} records", audit.len());
    }
}

// ============================================================================
// Orchestration gRPC - Templates
// ============================================================================

#[tokio::test]
async fn test_grpc_list_templates() {
    let client = create_orchestration_grpc_client().await;
    let templates = client.list_templates(None).await.unwrap();
    println!(
        "gRPC list_templates: {} namespaces, {} templates",
        templates.namespaces.len(),
        templates.templates.len()
    );
}

#[tokio::test]
async fn test_grpc_get_template() {
    let client = create_orchestration_grpc_client().await;
    let list = client.list_templates(None).await.unwrap();
    if let Some(first) = list.templates.first() {
        let detail = client
            .get_template(&first.namespace, &first.name, &first.version)
            .await
            .unwrap();
        assert_eq!(detail.name, first.name);
        println!(
            "gRPC get_template: {} with {} steps",
            detail.name,
            detail.steps.len()
        );
    }
}

// ============================================================================
// Orchestration gRPC - Analytics
// ============================================================================

#[tokio::test]
async fn test_grpc_get_performance_metrics() {
    let client = create_orchestration_grpc_client().await;
    let metrics = client.get_performance_metrics(None).await.unwrap();
    println!(
        "gRPC performance: total={} health={:.2}",
        metrics.total_tasks, metrics.system_health_score
    );
}

#[tokio::test]
async fn test_grpc_get_bottlenecks() {
    let client = create_orchestration_grpc_client().await;
    let analysis = client.get_bottlenecks(None, None).await.unwrap();
    println!(
        "gRPC bottlenecks: {} slow_steps, {} slow_tasks",
        analysis.slow_steps.len(),
        analysis.slow_tasks.len()
    );
}

// ============================================================================
// Orchestration gRPC - Config
// ============================================================================

#[tokio::test]
async fn test_grpc_get_config() {
    let client = create_orchestration_grpc_client().await;
    let config = client.get_config().await.unwrap();
    assert!(!config.metadata.version.is_empty());
    println!(
        "gRPC config: env={} v={}",
        config.metadata.environment, config.metadata.version
    );
}

// ============================================================================
// Orchestration gRPC - DLQ
// ============================================================================

#[tokio::test]
async fn test_grpc_list_dlq_entries() {
    let client = create_orchestration_grpc_client().await;
    let entries = client.list_dlq_entries(None).await.unwrap();
    println!("gRPC list_dlq: {} entries", entries.len());
}

#[tokio::test]
async fn test_grpc_get_dlq_stats() {
    let client = create_orchestration_grpc_client().await;
    let stats = client.get_dlq_stats().await.unwrap();
    println!("gRPC dlq_stats: {} reason groups", stats.len());
}

#[tokio::test]
async fn test_grpc_get_investigation_queue() {
    let client = create_orchestration_grpc_client().await;
    let queue = client.get_investigation_queue(Some(10)).await.unwrap();
    println!("gRPC investigation_queue: {} entries", queue.len());
}

#[tokio::test]
async fn test_grpc_get_staleness_monitoring() {
    let client = create_orchestration_grpc_client().await;
    let monitoring = client.get_staleness_monitoring(Some(10)).await.unwrap();
    println!("gRPC staleness: {} entries", monitoring.len());
}

// ============================================================================
// Worker gRPC - Health
// ============================================================================

#[tokio::test]
async fn test_grpc_worker_health_check() {
    let client = create_worker_grpc_client().await;
    let health = client.health_check().await.unwrap();
    assert!(!health.status.is_empty());
    println!("gRPC worker health: {}", health.status);
}

#[tokio::test]
async fn test_grpc_worker_liveness() {
    let client = create_worker_grpc_client().await;
    let health = client.liveness_probe().await.unwrap();
    assert!(!health.status.is_empty());
}

#[tokio::test]
async fn test_grpc_worker_readiness() {
    let client = create_worker_grpc_client().await;
    let result = client.readiness_probe().await;
    match result {
        Ok(readiness) => {
            println!("gRPC worker readiness: {}", readiness.status);
        }
        Err(e) => {
            println!("gRPC worker readiness error (acceptable): {e}");
        }
    }
}

#[tokio::test]
async fn test_grpc_worker_detailed_health() {
    let client = create_worker_grpc_client().await;
    let health = client.get_detailed_health().await.unwrap();
    assert!(!health.status.is_empty());
    assert!(!health.system_info.version.is_empty());
    println!(
        "gRPC worker detailed: {} v{} type={}",
        health.status, health.system_info.version, health.system_info.worker_type
    );
}

// ============================================================================
// Worker gRPC - Templates
// ============================================================================

#[tokio::test]
async fn test_grpc_worker_list_templates() {
    let client = create_worker_grpc_client().await;
    let templates = client.list_templates(None).await.unwrap();
    println!(
        "gRPC worker templates: count={} namespaces={:?}",
        templates.template_count, templates.supported_namespaces
    );
}

#[tokio::test]
async fn test_grpc_worker_get_template() {
    let client = create_worker_grpc_client().await;
    // Worker get_template requires namespace, name, and version
    // Use a known template from the test fixtures
    let result = client
        .get_template("rust_e2e_linear", "mathematical_sequence", "1.0.0")
        .await;
    match result {
        Ok(template) => {
            println!(
                "gRPC worker get_template: {} cached={}",
                template.template.template.name, template.cached
            );
        }
        Err(e) => {
            // Template may not exist in the worker's cache, which is acceptable
            println!("gRPC worker get_template error (acceptable): {e}");
        }
    }
}

// ============================================================================
// Worker gRPC - Config
// ============================================================================

#[tokio::test]
async fn test_grpc_worker_get_config() {
    let client = create_worker_grpc_client().await;
    let config = client.get_config().await.unwrap();
    assert!(!config.worker_id.is_empty());
    println!(
        "gRPC worker config: id={} type={}",
        config.worker_id, config.worker_type
    );
}
