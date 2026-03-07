//! # OrchestrationEventSystem Integration Tests (TAS-63)
//!
//! Tests for the public API of OrchestrationEventSystem including construction,
//! getters, process_event, health_check, and component statistics.
//!
//! Uses real database pools with mock command handlers on the receiver end.
//!
//! Note: The `statistics()` method uses `block_in_place` which requires a
//! multi-threaded runtime. Since `sqlx::test` uses current-thread runtime,
//! these tests validate behavior through process_event return values and
//! component_statistics() instead.

use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;

use tasker_orchestration::orchestration::channels::ChannelFactory;
use tasker_orchestration::orchestration::commands::{
    OrchestrationCommand, StepProcessResult, TaskFinalizationResult, TaskInitializeResult,
};
use tasker_orchestration::orchestration::core::OrchestrationCore;
use tasker_orchestration::orchestration::event_systems::{
    OrchestrationEventSystem, OrchestrationEventSystemConfig,
};
use tasker_orchestration::orchestration::orchestration_queues::OrchestrationQueueEvent;
use tasker_shared::messaging::service::{MessageEvent, MessageId};
use tasker_shared::monitoring::ChannelMonitor;
use tasker_shared::system_context::SystemContext;
use tasker_shared::{DeploymentMode, EventDrivenSystem};

/// Helper to construct an OrchestrationEventSystem with test defaults.
/// Returns the system, command receiver (for mock handler), and system context.
async fn setup_event_system(
    pool: PgPool,
) -> Result<(
    OrchestrationEventSystem,
    tasker_orchestration::orchestration::channels::OrchestrationCommandReceiver,
    Arc<SystemContext>,
)> {
    let context = Arc::new(SystemContext::with_pool(pool).await?);
    let core = Arc::new(OrchestrationCore::new(context.clone()).await?);
    let (cmd_sender, cmd_receiver) = ChannelFactory::orchestration_command_channel(100);
    let monitor = ChannelMonitor::new("test_orchestration_cmd", 100);
    let config = OrchestrationEventSystemConfig::default();
    let system =
        OrchestrationEventSystem::new(config, context.clone(), core, cmd_sender, monitor).await?;
    Ok((system, cmd_receiver, context))
}

/// Spawn a mock command handler that responds to commands on the receiver.
fn spawn_mock_handler(
    mut receiver: tasker_orchestration::orchestration::channels::OrchestrationCommandReceiver,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(cmd) = receiver.recv().await {
            match cmd {
                OrchestrationCommand::ProcessStepResultFromMessageEvent { resp, .. } => {
                    let _ = resp.send(Ok(StepProcessResult::Success {
                        message: "test ok".into(),
                    }));
                }
                OrchestrationCommand::InitializeTaskFromMessageEvent { resp, .. } => {
                    let _ = resp.send(Ok(TaskInitializeResult::Success {
                        task_uuid: uuid::Uuid::new_v4(),
                        message: "test ok".into(),
                    }));
                }
                OrchestrationCommand::FinalizeTaskFromMessageEvent { resp, .. } => {
                    let _ = resp.send(Ok(TaskFinalizationResult::Success {
                        task_uuid: uuid::Uuid::new_v4(),
                        final_status: "complete".into(),
                        completion_time: None,
                    }));
                }
                _ => {}
            }
        }
    })
}

// =============================================================================
// Construction and getter tests
// =============================================================================

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_construction_and_getters(pool: PgPool) -> Result<()> {
    let (system, _receiver, _context) = setup_event_system(pool).await?;

    assert_eq!(system.system_id(), "orchestration-event-system".to_string());
    assert_eq!(system.deployment_mode(), DeploymentMode::Hybrid);
    assert!(!system.is_running());
    assert!(system.uptime().is_none());

    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_health_check_fails_when_not_running(pool: PgPool) -> Result<()> {
    let (system, _receiver, _context) = setup_event_system(pool).await?;

    let result = system.health_check().await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_msg = format!("{:?}", err);
    assert!(
        err_msg.contains("not running"),
        "Error should mention not running: {}",
        err_msg
    );

    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_config_returns_expected_values(pool: PgPool) -> Result<()> {
    let (system, _receiver, _context) = setup_event_system(pool).await?;

    let config = system.config();
    assert_eq!(config.deployment_mode, DeploymentMode::Hybrid);
    assert_eq!(config.system_id, "orchestration-event-system");

    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_component_statistics_initially_empty(pool: PgPool) -> Result<()> {
    let (system, _receiver, _context) = setup_event_system(pool).await?;

    let comp_stats = system.component_statistics().await;
    assert!(comp_stats.fallback_poller_stats.is_none());
    assert!(comp_stats.queue_listener_stats.is_none());
    assert!(comp_stats.system_uptime.is_none());
    assert_eq!(comp_stats.deployment_mode, DeploymentMode::Hybrid);

    Ok(())
}

// =============================================================================
// process_event tests with mock command handler
// =============================================================================

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_event_step_result(pool: PgPool) -> Result<()> {
    let (system, receiver, _context) = setup_event_system(pool).await?;
    let handler = spawn_mock_handler(receiver);

    let event = OrchestrationQueueEvent::StepResult(MessageEvent::new(
        "orchestration_step_results",
        "orchestration",
        MessageId::from(100i64),
    ));

    let result = system.process_event(event).await;
    assert!(result.is_ok());

    handler.abort();
    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_event_task_request(pool: PgPool) -> Result<()> {
    let (system, receiver, _context) = setup_event_system(pool).await?;
    let handler = spawn_mock_handler(receiver);

    let event = OrchestrationQueueEvent::TaskRequest(MessageEvent::new(
        "orchestration_task_requests",
        "orchestration",
        MessageId::from(200i64),
    ));

    let result = system.process_event(event).await;
    assert!(result.is_ok());

    handler.abort();
    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_event_task_finalization(pool: PgPool) -> Result<()> {
    let (system, receiver, _context) = setup_event_system(pool).await?;
    let handler = spawn_mock_handler(receiver);

    let event = OrchestrationQueueEvent::TaskFinalization(MessageEvent::new(
        "orchestration_task_finalizations",
        "orchestration",
        MessageId::from(300i64),
    ));

    let result = system.process_event(event).await;
    assert!(result.is_ok());

    handler.abort();
    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_event_unknown_succeeds(pool: PgPool) -> Result<()> {
    let (system, _receiver, _context) = setup_event_system(pool).await?;

    let event = OrchestrationQueueEvent::Unknown {
        queue_name: "unknown_queue".to_string(),
        payload: "{}".to_string(),
    };

    // Unknown events are handled without error
    let result = system.process_event(event).await;
    assert!(result.is_ok());

    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_event_multiple_succeeds(pool: PgPool) -> Result<()> {
    let (system, receiver, _context) = setup_event_system(pool).await?;
    let handler = spawn_mock_handler(receiver);

    // Process multiple events to ensure repeated calls work correctly
    for i in 0..3 {
        let event = OrchestrationQueueEvent::StepResult(MessageEvent::new(
            "orchestration_step_results",
            "orchestration",
            MessageId::from(i as i64),
        ));
        let result = system.process_event(event).await;
        assert!(result.is_ok(), "Event {} should succeed", i);
    }

    handler.abort();
    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_event_mixed_types(pool: PgPool) -> Result<()> {
    let (system, receiver, _context) = setup_event_system(pool).await?;
    let handler = spawn_mock_handler(receiver);

    // Process one of each event type
    let step_event = OrchestrationQueueEvent::StepResult(MessageEvent::new(
        "orchestration_step_results",
        "orchestration",
        MessageId::from(1i64),
    ));
    assert!(system.process_event(step_event).await.is_ok());

    let task_event = OrchestrationQueueEvent::TaskRequest(MessageEvent::new(
        "orchestration_task_requests",
        "orchestration",
        MessageId::from(2i64),
    ));
    assert!(system.process_event(task_event).await.is_ok());

    let finalize_event = OrchestrationQueueEvent::TaskFinalization(MessageEvent::new(
        "orchestration_task_finalizations",
        "orchestration",
        MessageId::from(3i64),
    ));
    assert!(system.process_event(finalize_event).await.is_ok());

    let unknown_event = OrchestrationQueueEvent::Unknown {
        queue_name: "test".to_string(),
        payload: "{}".to_string(),
    };
    assert!(system.process_event(unknown_event).await.is_ok());

    handler.abort();
    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_stop_when_not_running(pool: PgPool) -> Result<()> {
    let (mut system, _receiver, _context) = setup_event_system(pool).await?;

    // stop() when not running should succeed without panic
    let result = system.stop().await;
    assert!(result.is_ok());

    Ok(())
}

// =============================================================================
// Channel closed error path
// =============================================================================

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_event_with_closed_channel(pool: PgPool) -> Result<()> {
    let (system, receiver, _context) = setup_event_system(pool).await?;

    // Drop the receiver to close the channel
    drop(receiver);

    let event = OrchestrationQueueEvent::StepResult(MessageEvent::new(
        "orchestration_step_results",
        "orchestration",
        MessageId::from(1i64),
    ));

    // process_event should return an error when the channel is closed
    let result = system.process_event(event).await;
    assert!(
        result.is_err(),
        "process_event should fail with closed channel"
    );

    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_event_task_request_closed_channel(pool: PgPool) -> Result<()> {
    let (system, receiver, _context) = setup_event_system(pool).await?;
    drop(receiver);

    let event = OrchestrationQueueEvent::TaskRequest(MessageEvent::new(
        "orchestration_task_requests",
        "orchestration",
        MessageId::from(1i64),
    ));

    let result = system.process_event(event).await;
    assert!(
        result.is_err(),
        "TaskRequest should fail with closed channel"
    );

    Ok(())
}

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_process_event_finalization_closed_channel(pool: PgPool) -> Result<()> {
    let (system, receiver, _context) = setup_event_system(pool).await?;
    drop(receiver);

    let event = OrchestrationQueueEvent::TaskFinalization(MessageEvent::new(
        "orchestration_task_finalizations",
        "orchestration",
        MessageId::from(1i64),
    ));

    let result = system.process_event(event).await;
    assert!(
        result.is_err(),
        "TaskFinalization should fail with closed channel"
    );

    Ok(())
}

// =============================================================================
// Component statistics after process_event
// =============================================================================

#[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
async fn test_component_statistics_unchanged_after_process_event(pool: PgPool) -> Result<()> {
    let (system, receiver, _context) = setup_event_system(pool).await?;
    let handler = spawn_mock_handler(receiver);

    let event = OrchestrationQueueEvent::StepResult(MessageEvent::new(
        "orchestration_step_results",
        "orchestration",
        MessageId::from(1i64),
    ));
    system.process_event(event).await?;

    // Component statistics should still show no poller/listener
    // since we never called start()
    let comp_stats = system.component_statistics().await;
    assert!(comp_stats.fallback_poller_stats.is_none());
    assert!(comp_stats.queue_listener_stats.is_none());
    // system_uptime is still None because we never called start()
    assert!(comp_stats.system_uptime.is_none());

    handler.abort();
    Ok(())
}
