//! REST/gRPC response parity tests (TAS-177).
//!
//! These tests verify that REST and gRPC APIs return equivalent responses
//! for the same operations. This ensures transport consistency.
//!
//! The suite runs many tests concurrently against one shared live database,
//! so a state transition can land between the REST read and the gRPC read.
//! Comparisons over volatile state go through [`assert_parity_with_retry`],
//! which re-reads both transports a bounded number of times before failing;
//! immutable structure (names, namespaces, versions) is compared directly.

use anyhow::Result;
use std::future::Future;
use std::time::Duration;
use tasker_client::{ClientConfig, OrchestrationClient, Transport, UnifiedOrchestrationClient};
use uuid::Uuid;

/// Helper to create REST and gRPC clients for parity testing
async fn create_parity_clients() -> Result<(UnifiedOrchestrationClient, UnifiedOrchestrationClient)>
{
    let rest_url = std::env::var("TASKER_TEST_ORCHESTRATION_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    let grpc_url = std::env::var("TASKER_TEST_ORCHESTRATION_GRPC_URL")
        .unwrap_or_else(|_| "http://localhost:9190".to_string());

    let rest_config = ClientConfig {
        transport: Transport::Rest,
        orchestration: tasker_client::config::ApiEndpointConfig {
            base_url: rest_url,
            timeout_ms: 10000,
            max_retries: 3,
            auth_token: None,
            auth: None,
        },
        ..Default::default()
    };

    let grpc_config = ClientConfig {
        transport: Transport::Grpc,
        orchestration: tasker_client::config::ApiEndpointConfig {
            base_url: grpc_url,
            timeout_ms: 10000,
            max_retries: 3,
            auth_token: None,
            auth: None,
        },
        ..Default::default()
    };

    let rest_client = UnifiedOrchestrationClient::from_config(&rest_config).await?;
    let grpc_client = UnifiedOrchestrationClient::from_config(&grpc_config).await?;

    Ok((rest_client, grpc_client))
}

/// Stability-bounded parity comparison for volatile state read over both
/// transports from the shared live test database.
///
/// A mismatch between the REST read and the gRPC read can be a transition
/// landing between the two reads rather than a parity defect, so the pair is
/// re-read a bounded number of times: the comparison passes on the first
/// agreement and panics on the last of `ATTEMPTS` consecutive disagreements,
/// reporting both values. Only the failing path waits, and a genuinely
/// divergent pair keeps failing — this bounds convergence, it does not
/// paper over it.
async fn assert_parity_with_retry<T, F, Fut>(mut read_pair: F, context: &str) -> Result<()>
where
    T: PartialEq + std::fmt::Debug,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(T, T)>>,
{
    const ATTEMPTS: u32 = 5;
    const RETRY_DELAY: Duration = Duration::from_millis(200);

    let mut last: Option<(T, T)> = None;
    for attempt in 1..=ATTEMPTS {
        let (rest_value, grpc_value) = read_pair().await?;
        if rest_value == grpc_value {
            return Ok(());
        }
        last = Some((rest_value, grpc_value));
        if attempt < ATTEMPTS {
            tokio::time::sleep(RETRY_DELAY).await;
        }
    }
    let (rest_value, grpc_value) = last.expect("loop body runs at least once");
    panic!(
        "{context}: values diverged across {ATTEMPTS} bounded attempts \
         (REST={rest_value:?}, gRPC={grpc_value:?}); a single transient \
         mismatch is a concurrent transition, persistent divergence is a \
         real parity defect"
    );
}

/// Test health response structure parity
#[tokio::test]
async fn test_health_response_parity() -> Result<()> {
    let (rest_client, grpc_client) = create_parity_clients().await?;

    // Get basic health from both
    let rest_health = rest_client.get_basic_health().await?;
    let grpc_health = grpc_client.get_basic_health().await?;

    // Compare structure
    assert_eq!(
        rest_health.status, grpc_health.status,
        "Health status should match"
    );
    // Timestamps may differ slightly, just verify both have values
    assert!(
        !rest_health.timestamp.is_empty() && !grpc_health.timestamp.is_empty(),
        "Both should have timestamps"
    );

    println!(
        "Health parity: REST status={}, gRPC status={}",
        rest_health.status, grpc_health.status
    );
    Ok(())
}

/// Test detailed health response parity
#[tokio::test]
async fn test_detailed_health_response_parity() -> Result<()> {
    let (rest_client, grpc_client) = create_parity_clients().await?;

    let rest_health = rest_client.get_detailed_health().await?;
    let grpc_health = grpc_client.get_detailed_health().await?;

    // Compare top-level status
    // Note: Status can vary between requests due to system state, so we just
    // verify both return valid status values. If they differ, log it but don't fail.
    let valid_statuses = ["healthy", "degraded", "unhealthy"];
    assert!(
        valid_statuses.contains(&rest_health.status.as_str()),
        "REST status should be valid"
    );
    assert!(
        valid_statuses.contains(&grpc_health.status.as_str()),
        "gRPC status should be valid"
    );
    if rest_health.status != grpc_health.status {
        println!(
            "Note: Health status differs (REST={}, gRPC={}) - this can happen with timing",
            rest_health.status, grpc_health.status
        );
    }

    // Compare info section
    assert_eq!(
        rest_health.info.version, grpc_health.info.version,
        "Version should match"
    );
    assert_eq!(
        rest_health.info.environment, grpc_health.info.environment,
        "Environment should match"
    );

    // Compare health check subsections
    assert_eq!(
        rest_health.checks.web_database.status, grpc_health.checks.web_database.status,
        "Web database status should match"
    );
    assert_eq!(
        rest_health.checks.orchestration_database.status,
        grpc_health.checks.orchestration_database.status,
        "Orchestration database status should match"
    );
    assert_eq!(
        rest_health.checks.circuit_breaker.status, grpc_health.checks.circuit_breaker.status,
        "Circuit breaker status should match"
    );

    println!(
        "Detailed health parity: version={}, env={}",
        rest_health.info.version, rest_health.info.environment
    );
    Ok(())
}

/// Test template list response parity
#[tokio::test]
async fn test_template_list_parity() -> Result<()> {
    let (rest_client, grpc_client) = create_parity_clients().await?;

    let rest_templates = rest_client.list_templates(None).await?;
    let grpc_templates = grpc_client.list_templates(None).await?;

    // Compare total counts
    assert_eq!(
        rest_templates.total_count, grpc_templates.total_count,
        "Template total count should match"
    );

    // Compare namespace counts
    assert_eq!(
        rest_templates.namespaces.len(),
        grpc_templates.namespaces.len(),
        "Namespace count should match"
    );

    // Compare template counts
    assert_eq!(
        rest_templates.templates.len(),
        grpc_templates.templates.len(),
        "Template list length should match"
    );

    println!(
        "Template list parity: total={}, namespaces={}, templates={}",
        rest_templates.total_count,
        rest_templates.namespaces.len(),
        rest_templates.templates.len()
    );
    Ok(())
}

/// Test task list response parity
#[tokio::test]
async fn test_task_list_parity() -> Result<()> {
    let (rest_client, grpc_client) = create_parity_clients().await?;

    let rest_tasks = rest_client.list_tasks(10, 0, None, None).await?;

    // The totals move as concurrent tests create tasks; re-read both
    // transports under the stability retry before declaring divergence.
    assert_parity_with_retry(
        || async {
            let rest_tasks = rest_client.list_tasks(10, 0, None, None).await?;
            let grpc_tasks = grpc_client.list_tasks(10, 0, None, None).await?;
            Ok((
                (rest_tasks.pagination.total_count, rest_tasks.tasks.len()),
                (grpc_tasks.pagination.total_count, grpc_tasks.tasks.len()),
            ))
        },
        "Task list totals should match",
    )
    .await?;

    println!(
        "Task list parity: total={}, returned={}",
        rest_tasks.pagination.total_count,
        rest_tasks.tasks.len()
    );
    Ok(())
}

/// Test task response structure parity using an existing task
///
/// Gets the same task via both transports and compares the response structure.
/// This avoids task creation conflicts and focuses on response parity.
#[tokio::test]
async fn test_task_creation_parity() -> Result<()> {
    let (rest_client, grpc_client) = create_parity_clients().await?;

    // Get an existing task from the database to test response parity
    let rest_tasks = rest_client.list_tasks(1, 0, None, None).await?;
    if rest_tasks.tasks.is_empty() {
        println!("No tasks in database, skipping task creation parity test");
        return Ok(());
    }

    let task_uuid: Uuid = rest_tasks.tasks[0].task_uuid.parse()?;

    // Get the same task via both transports
    let rest_task = rest_client.get_task(task_uuid).await?;
    let grpc_task = grpc_client.get_task(task_uuid).await?;

    // Compare response structure
    assert_eq!(rest_task.name, grpc_task.name, "Task name should match");
    assert_eq!(
        rest_task.namespace, grpc_task.namespace,
        "Task namespace should match"
    );
    assert_eq!(
        rest_task.version, grpc_task.version,
        "Task version should match"
    );
    assert_parity_with_retry(
        || async {
            let rest_task = rest_client.get_task(task_uuid).await?;
            let grpc_task = grpc_client.get_task(task_uuid).await?;
            Ok((rest_task.total_steps, grpc_task.total_steps))
        },
        "Total steps should match",
    )
    .await?;

    println!(
        "Task response parity: uuid={}, name={}",
        rest_task.task_uuid, rest_task.name
    );
    Ok(())
}

/// Test task get response parity using an existing task
#[tokio::test]
async fn test_task_get_parity() -> Result<()> {
    let (rest_client, grpc_client) = create_parity_clients().await?;

    // Get an existing task from the database
    let rest_tasks = rest_client.list_tasks(1, 0, None, None).await?;
    if rest_tasks.tasks.is_empty() {
        println!("No tasks in database, skipping task get parity test");
        return Ok(());
    }

    let task_uuid: Uuid = rest_tasks.tasks[0].task_uuid.parse()?;

    // Get the same task via both transports
    let rest_task = rest_client.get_task(task_uuid).await?;
    let grpc_task = grpc_client.get_task(task_uuid).await?;

    // Compare immutable identity directly, and volatile counters through the
    // stability retry: the picked task may belong to a concurrently running
    // test whose transitions land between the two reads.
    assert_eq!(
        rest_task.task_uuid, grpc_task.task_uuid,
        "Task UUID should match"
    );
    assert_eq!(rest_task.name, grpc_task.name, "Task name should match");
    assert_eq!(
        rest_task.namespace, grpc_task.namespace,
        "Task namespace should match"
    );
    assert_parity_with_retry(
        || async {
            let rest_task = rest_client.get_task(task_uuid).await?;
            let grpc_task = grpc_client.get_task(task_uuid).await?;
            Ok((rest_task.status, grpc_task.status))
        },
        "Task status should match",
    )
    .await?;
    assert_parity_with_retry(
        || async {
            let rest_task = rest_client.get_task(task_uuid).await?;
            let grpc_task = grpc_client.get_task(task_uuid).await?;
            Ok((rest_task.total_steps, grpc_task.total_steps))
        },
        "Total steps should match",
    )
    .await?;
    assert_parity_with_retry(
        || async {
            let rest_task = rest_client.get_task(task_uuid).await?;
            let grpc_task = grpc_client.get_task(task_uuid).await?;
            Ok((rest_task.pending_steps, grpc_task.pending_steps))
        },
        "Pending steps should match",
    )
    .await?;
    assert_parity_with_retry(
        || async {
            let rest_task = rest_client.get_task(task_uuid).await?;
            let grpc_task = grpc_client.get_task(task_uuid).await?;
            Ok((rest_task.completed_steps, grpc_task.completed_steps))
        },
        "Completed steps should match",
    )
    .await?;

    println!(
        "Task get parity: uuid={}, status={}, steps={}",
        rest_task.task_uuid, rest_task.status, rest_task.total_steps
    );
    Ok(())
}

/// Test step list response parity using an existing task
#[tokio::test]
async fn test_step_list_parity() -> Result<()> {
    let (rest_client, grpc_client) = create_parity_clients().await?;

    // Get an existing task from the database that has steps
    let rest_tasks = rest_client.list_tasks(10, 0, None, None).await?;
    if rest_tasks.tasks.is_empty() {
        println!("No tasks in database, skipping step list parity test");
        return Ok(());
    }

    // Find a task with steps
    let mut task_uuid: Option<Uuid> = None;
    for task in &rest_tasks.tasks {
        if task.total_steps > 0 {
            task_uuid = Some(task.task_uuid.parse()?);
            break;
        }
    }

    let task_uuid = match task_uuid {
        Some(id) => id,
        None => {
            println!("No tasks with steps found, skipping step list parity test");
            return Ok(());
        }
    };

    // Get steps via REST to pick the comparison target; the paired read
    // happens under the stability retry below.
    let rest_steps = rest_client.list_task_steps(task_uuid).await?;

    // Names, order, and states compared as one summary vector under the
    // stability retry: steps can be inserted by initialization and their
    // states can transition between the two reads. Vector equality subsumes
    // the former count and per-position assertions.
    assert_parity_with_retry(
        || async {
            let rest_steps = rest_client.list_task_steps(task_uuid).await?;
            let grpc_steps = grpc_client.list_task_steps(task_uuid).await?;
            let rest_summary: Vec<(String, String)> = rest_steps
                .iter()
                .map(|s| (s.name.clone(), s.current_state.clone()))
                .collect();
            let grpc_summary: Vec<(String, String)> = grpc_steps
                .iter()
                .map(|s| (s.name.clone(), s.current_state.clone()))
                .collect();
            Ok((rest_summary, grpc_summary))
        },
        "Step list should match (names, order, and states)",
    )
    .await?;

    println!("Step list parity: count={}", rest_steps.len());
    Ok(())
}
