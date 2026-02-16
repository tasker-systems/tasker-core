//! Global bridge state and napi-rs FFI implementation.
//!
//! TAS-290: Replaces C FFI + JSON serialization with napi-rs native objects.
//! Key differences from the koffi approach:
//! - Structured objects cross the FFI boundary directly (no JSON ser/de)
//! - Errors become JavaScript exceptions (no {success, error} envelope)
//! - No manual memory management (no free_rust_string)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use napi::bindgen_prelude::*;
use tokio::sync::RwLock;
use tracing::{error, info};
use uuid::Uuid;

use tasker_shared::events::domain_events::{DomainEvent, DomainEventPublisher};
use tasker_shared::messaging::StepExecutionResult as RustStepExecutionResult;
use tasker_shared::models::core::batch_worker::CheckpointYieldData;
use tasker_worker::worker::{
    services::CheckpointService, DomainEventCallback, FfiDispatchChannel, FfiDispatchChannelConfig,
    FfiStepEvent, StepEventPublisherRegistry,
};
use tasker_worker::{WorkerBootstrap, WorkerSystemHandle};
use tokio::sync::broadcast;

use crate::error::NapiFfiError;

// =============================================================================
// Global State (same pattern as existing TypeScript worker)
// =============================================================================

pub static WORKER_SYSTEM: Mutex<Option<NapiBridgeHandle>> = Mutex::new(None);

pub struct NapiBridgeHandle {
    pub system_handle: WorkerSystemHandle,
    pub ffi_dispatch_channel: Arc<FfiDispatchChannel>,
    #[expect(
        dead_code,
        reason = "Kept alive for domain event publishing infrastructure"
    )]
    pub domain_event_publisher: Arc<DomainEventPublisher>,
    pub in_process_event_receiver: Option<Arc<Mutex<broadcast::Receiver<DomainEvent>>>>,
    pub client: Option<Arc<tasker_worker::FfiClientBridge>>,
    pub runtime: tokio::runtime::Runtime,
    pub worker_id: String,
}

impl std::fmt::Debug for NapiBridgeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NapiBridgeHandle")
            .field("worker_id", &self.worker_id)
            .finish()
    }
}

// =============================================================================
// napi-rs Object Types (replace JSON DTOs)
// =============================================================================

/// Configuration for bootstrapping the worker.
#[napi(object)]
#[derive(Debug)]
pub struct BootstrapConfig {
    pub namespace: Option<String>,
    pub config_path: Option<String>,
}

/// Result of bootstrapping the worker.
#[napi(object)]
#[derive(Debug)]
pub struct BootstrapResult {
    pub success: bool,
    pub status: String,
    pub message: String,
    pub worker_id: Option<String>,
}

/// Worker status information.
#[napi(object)]
#[derive(Debug)]
pub struct WorkerStatus {
    pub success: bool,
    pub running: bool,
    pub worker_id: Option<String>,
    pub status: Option<String>,
    pub environment: Option<String>,
}

/// A step event dispatched to the TypeScript handler.
#[napi(object)]
#[derive(Debug)]
pub struct NapiStepEvent {
    pub event_id: String,
    pub task_uuid: String,
    pub step_uuid: String,
    pub correlation_id: String,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub task_correlation_id: String,
    pub parent_correlation_id: Option<String>,
    pub task: NapiTaskInfo,
    pub workflow_step: NapiWorkflowStep,
    pub step_definition: NapiStepDefinition,
    pub dependency_results: HashMap<String, NapiDependencyResult>,
}

#[napi(object)]
#[derive(Debug)]
pub struct NapiTaskInfo {
    pub task_uuid: String,
    pub named_task_uuid: String,
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub context: Option<serde_json::Value>,
    pub correlation_id: String,
    pub parent_correlation_id: Option<String>,
    pub complete: bool,
    pub priority: i32,
    pub initiator: Option<String>,
    pub source_system: Option<String>,
    pub reason: Option<String>,
    pub tags: Option<serde_json::Value>,
    pub identity_hash: String,
    pub created_at: String,
    pub updated_at: String,
    pub requested_at: String,
}

#[napi(object)]
#[derive(Debug)]
pub struct NapiWorkflowStep {
    pub workflow_step_uuid: String,
    pub task_uuid: String,
    pub named_step_uuid: String,
    pub name: String,
    pub template_step_name: String,
    pub retryable: bool,
    pub max_attempts: i32,
    pub attempts: i32,
    pub in_process: bool,
    pub processed: bool,
    pub inputs: Option<serde_json::Value>,
    pub results: Option<serde_json::Value>,
    pub backoff_request_seconds: Option<i32>,
    pub processed_at: Option<String>,
    pub last_attempted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub checkpoint: Option<serde_json::Value>,
}

#[napi(object)]
#[derive(Debug)]
pub struct NapiStepDefinition {
    pub name: String,
    pub description: Option<String>,
    pub handler_callable: String,
    pub handler_method: Option<String>,
    pub handler_resolver: Option<String>,
    pub handler_initialization: serde_json::Value,
    pub system_dependency: Option<String>,
    pub dependencies: Vec<String>,
    pub timeout_seconds: Option<i64>,
    pub retry_retryable: bool,
    pub retry_max_attempts: u32,
    pub retry_backoff: String,
    pub retry_backoff_base_ms: Option<i64>,
    pub retry_max_backoff_ms: Option<i64>,
}

#[napi(object)]
#[derive(Debug)]
pub struct NapiDependencyResult {
    pub step_uuid: String,
    pub success: bool,
    pub result: serde_json::Value,
    pub status: String,
    pub error_message: Option<String>,
    pub error_type: Option<String>,
    pub error_retryable: Option<bool>,
}

/// Result of completing a step event.
#[napi(object)]
#[derive(Debug)]
pub struct NapiStepResult {
    pub step_uuid: String,
    pub success: bool,
    pub result: serde_json::Value,
    pub status: String,
    pub error_message: Option<String>,
    pub error_type: Option<String>,
    pub error_retryable: Option<bool>,
    pub error_status_code: Option<u16>,
}

/// FFI dispatch metrics.
#[napi(object)]
#[derive(Debug)]
pub struct NapiDispatchMetrics {
    pub pending_count: u32,
    pub starvation_detected: bool,
    pub starving_event_count: u32,
    pub oldest_pending_age_ms: Option<f64>,
    pub newest_pending_age_ms: Option<f64>,
    pub oldest_event_id: Option<String>,
}

/// Domain event from in-process event bus (fast path).
#[napi(object)]
#[derive(Debug)]
pub struct NapiDomainEvent {
    pub event_id: String,
    pub event_name: String,
    pub event_version: String,
    pub metadata: NapiDomainEventMetadata,
    pub payload: serde_json::Value,
}

/// Metadata attached to domain events.
#[napi(object)]
#[derive(Debug)]
pub struct NapiDomainEventMetadata {
    pub task_uuid: String,
    pub step_uuid: Option<String>,
    pub step_name: Option<String>,
    pub namespace: String,
    pub correlation_id: String,
    pub fired_at: String,
    pub fired_by: Option<String>,
}

/// Checkpoint yield data for batch processing (TAS-125).
#[napi(object)]
#[derive(Debug)]
pub struct NapiCheckpointYieldData {
    pub step_uuid: String,
    pub cursor: serde_json::Value,
    pub items_processed: i64,
    pub accumulated_results: Option<serde_json::Value>,
}

// =============================================================================
// Conversion helpers: internal Rust types → napi objects
// =============================================================================

fn convert_step_event(event: &FfiStepEvent) -> NapiStepEvent {
    let payload = &event.execution_event.payload;
    let tss = &payload.task_sequence_step;
    let task = &tss.task;
    let step = &tss.workflow_step;
    let step_def = &tss.step_definition;

    NapiStepEvent {
        event_id: event.event_id.to_string(),
        task_uuid: event.task_uuid.to_string(),
        step_uuid: event.step_uuid.to_string(),
        correlation_id: event.correlation_id.to_string(),
        trace_id: event.trace_id.clone(),
        span_id: event.span_id.clone(),
        task_correlation_id: task.task.correlation_id.to_string(),
        parent_correlation_id: task.task.parent_correlation_id.map(|id| id.to_string()),
        task: NapiTaskInfo {
            task_uuid: task.task.task_uuid.to_string(),
            named_task_uuid: task.task.named_task_uuid.to_string(),
            name: task.task_name.clone(),
            namespace: task.namespace_name.clone(),
            version: task.task_version.clone(),
            context: task.task.context.clone(),
            correlation_id: task.task.correlation_id.to_string(),
            parent_correlation_id: task.task.parent_correlation_id.map(|id| id.to_string()),
            complete: task.task.complete,
            priority: task.task.priority,
            initiator: task.task.initiator.clone(),
            source_system: task.task.source_system.clone(),
            reason: task.task.reason.clone(),
            tags: task.task.tags.clone(),
            identity_hash: task.task.identity_hash.clone(),
            created_at: task.task.created_at.to_string(),
            updated_at: task.task.updated_at.to_string(),
            requested_at: task.task.requested_at.to_string(),
        },
        workflow_step: NapiWorkflowStep {
            workflow_step_uuid: step.workflow_step_uuid.to_string(),
            task_uuid: step.task_uuid.to_string(),
            named_step_uuid: step.named_step_uuid.to_string(),
            name: step.name.clone(),
            template_step_name: step.template_step_name.clone(),
            retryable: step.retryable,
            max_attempts: step.max_attempts.unwrap_or(1),
            attempts: step.attempts.unwrap_or(0),
            in_process: step.in_process,
            processed: step.processed,
            inputs: step.inputs.clone(),
            results: step.results.clone(),
            backoff_request_seconds: step.backoff_request_seconds,
            processed_at: step.processed_at.map(|t| t.to_string()),
            last_attempted_at: step.last_attempted_at.map(|t| t.to_string()),
            created_at: step.created_at.to_string(),
            updated_at: step.updated_at.to_string(),
            checkpoint: step.checkpoint.clone(),
        },
        step_definition: NapiStepDefinition {
            name: step_def.name.clone(),
            description: step_def.description.clone(),
            handler_callable: step_def.handler.callable.clone(),
            handler_method: step_def.handler.method.clone(),
            handler_resolver: step_def.handler.resolver.clone(),
            handler_initialization: serde_json::to_value(&step_def.handler.initialization)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
            system_dependency: step_def.system_dependency.clone(),
            dependencies: step_def.dependencies.clone(),
            timeout_seconds: step_def.timeout_seconds.map(|v| v as i64),
            retry_retryable: step_def.retry.retryable,
            retry_max_attempts: step_def.retry.max_attempts,
            retry_backoff: format!("{:?}", step_def.retry.backoff).to_lowercase(),
            retry_backoff_base_ms: step_def.retry.backoff_base_ms.map(|v| v as i64),
            retry_max_backoff_ms: step_def.retry.max_backoff_ms.map(|v| v as i64),
        },
        dependency_results: tss
            .dependency_results
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    NapiDependencyResult {
                        step_uuid: v.step_uuid.to_string(),
                        success: v.success,
                        result: v.result.clone(),
                        status: v.status.clone(),
                        error_message: v.error.as_ref().map(|e| e.message.clone()),
                        error_type: v.error.as_ref().and_then(|e| e.error_type.clone()),
                        error_retryable: v.error.as_ref().map(|e| e.retryable),
                    },
                )
            })
            .collect(),
    }
}

fn convert_napi_result_to_rust(result: NapiStepResult) -> RustStepExecutionResult {
    let error = if !result.success {
        Some(tasker_shared::messaging::StepExecutionError {
            message: result
                .error_message
                .unwrap_or_else(|| "Unknown error".to_string()),
            error_type: result.error_type,
            retryable: result.error_retryable.unwrap_or(false),
            status_code: result.error_status_code,
            backtrace: None,
            context: std::collections::HashMap::new(),
        })
    } else {
        None
    };

    RustStepExecutionResult {
        step_uuid: Uuid::parse_str(&result.step_uuid).unwrap_or_else(|_| Uuid::nil()),
        success: result.success,
        result: result.result,
        metadata: tasker_shared::messaging::StepExecutionMetadata::default(),
        status: result.status,
        error,
        orchestration_metadata: None,
    }
}

fn convert_domain_event(event: &DomainEvent) -> NapiDomainEvent {
    NapiDomainEvent {
        event_id: event.event_id.to_string(),
        event_name: event.event_name.clone(),
        event_version: event.event_version.clone(),
        metadata: NapiDomainEventMetadata {
            task_uuid: event.metadata.task_uuid.to_string(),
            step_uuid: event.metadata.step_uuid.map(|id| id.to_string()),
            step_name: event.metadata.step_name.clone(),
            namespace: event.metadata.namespace.clone(),
            correlation_id: event.metadata.correlation_id.to_string(),
            fired_at: event.metadata.fired_at.to_string(),
            fired_by: Some(event.metadata.fired_by.clone()),
        },
        payload: serde_json::to_value(&event.payload).unwrap_or(serde_json::Value::Null),
    }
}

fn convert_dispatch_metrics(
    metrics: &tasker_worker::worker::FfiDispatchMetrics,
) -> NapiDispatchMetrics {
    NapiDispatchMetrics {
        pending_count: metrics.pending_count as u32,
        starvation_detected: metrics.starvation_detected,
        starving_event_count: metrics.starving_event_count as u32,
        oldest_pending_age_ms: metrics.oldest_pending_age_ms.map(|v| v as f64),
        newest_pending_age_ms: metrics.newest_pending_age_ms.map(|v| v as f64),
        oldest_event_id: metrics.oldest_event_id.map(|id| id.to_string()),
    }
}

// =============================================================================
// napi-rs FFI Functions
// =============================================================================

/// Bootstrap the worker system.
#[napi]
pub fn bootstrap_worker(config: Option<BootstrapConfig>) -> Result<BootstrapResult> {
    let mut guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    // Check if already running
    if guard.is_some() {
        return Ok(BootstrapResult {
            success: true,
            status: "already_running".to_string(),
            message: "Worker is already running".to_string(),
            worker_id: guard.as_ref().map(|h| h.worker_id.clone()),
        });
    }

    if let Some(ref cfg) = config {
        info!(?cfg, "Bootstrap config received via napi-rs");
    }

    let worker_id = Uuid::new_v4();
    let worker_id_str = format!("typescript-worker-{}", worker_id);

    let runtime = tokio::runtime::Runtime::new().map_err(|e| {
        error!("Failed to create tokio runtime: {}", e);
        napi::Error::from(NapiFfiError::RuntimeError(format!(
            "Runtime creation failed: {}",
            e
        )))
    })?;

    runtime.block_on(async {
        tasker_shared::logging::init_tracing();
    });

    let mut system_handle = runtime
        .block_on(async { WorkerBootstrap::bootstrap().await })
        .map_err(|e| {
            error!("Failed to bootstrap worker system: {}", e);
            napi::Error::from(NapiFfiError::BootstrapFailed(e.to_string()))
        })?;

    info!("Worker system bootstrapped successfully via napi-rs");

    let (domain_event_publisher, domain_event_callback) = runtime.block_on(async {
        let worker_core = system_handle.worker_core.lock().await;
        let message_client = worker_core.context.message_client.clone();
        let publisher = Arc::new(DomainEventPublisher::new(message_client));
        let event_router = worker_core.event_router().ok_or_else(|| {
            napi::Error::from(NapiFfiError::BootstrapFailed(
                "EventRouter not available".to_string(),
            ))
        })?;
        let step_event_registry =
            StepEventPublisherRegistry::with_event_router(publisher.clone(), event_router);
        let registry = Arc::new(RwLock::new(step_event_registry));
        let callback = Arc::new(DomainEventCallback::new(registry));
        Ok::<_, napi::Error>((publisher, callback))
    })?;

    let ffi_dispatch_channel = if let Some(dispatch_handles) = system_handle.take_dispatch_handles()
    {
        let config_ffi = FfiDispatchChannelConfig::new(runtime.handle().clone())
            .with_service_id(worker_id_str.clone())
            .with_completion_timeout(std::time::Duration::from_secs(30));

        let db_pool = runtime.block_on(async {
            let worker_core = system_handle.worker_core.lock().await;
            worker_core.context.database_pool().clone()
        });

        let checkpoint_service = CheckpointService::new(db_pool);

        let channel = FfiDispatchChannel::new(
            dispatch_handles.dispatch_receiver,
            dispatch_handles.completion_sender,
            config_ffi,
            domain_event_callback,
        )
        .with_checkpoint_support(checkpoint_service, dispatch_handles.dispatch_sender);

        Arc::new(channel)
    } else {
        return Err(napi::Error::from(NapiFfiError::BootstrapFailed(
            "Dispatch handles not available".to_string(),
        )));
    };

    let in_process_event_receiver = runtime.block_on(async {
        let worker_core = system_handle.worker_core.lock().await;
        let bus = worker_core.in_process_event_bus();
        let bus_guard = bus.write().await;
        bus_guard.subscribe_ffi()
    });

    let ffi_client = runtime.block_on(async {
        let worker_core = system_handle.worker_core.lock().await;
        tasker_worker::create_ffi_client_bridge(&worker_core, runtime.handle().clone()).await
    });

    *guard = Some(NapiBridgeHandle {
        system_handle,
        ffi_dispatch_channel,
        domain_event_publisher,
        in_process_event_receiver: Some(Arc::new(Mutex::new(in_process_event_receiver))),
        client: ffi_client,
        runtime,
        worker_id: worker_id_str.clone(),
    });

    Ok(BootstrapResult {
        success: true,
        status: "started".to_string(),
        message: "TypeScript worker system started successfully via napi-rs".to_string(),
        worker_id: Some(worker_id_str),
    })
}

/// Check if the worker is currently running.
#[napi]
pub fn is_worker_running() -> Result<bool> {
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;
    Ok(guard.is_some())
}

/// Get worker status.
#[napi]
pub fn get_worker_status() -> Result<WorkerStatus> {
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    match &*guard {
        Some(handle) => {
            let status = handle
                .runtime
                .block_on(async { handle.system_handle.status().await })
                .map_err(|e| {
                    napi::Error::from(NapiFfiError::RuntimeError(format!(
                        "Failed to get status: {}",
                        e
                    )))
                })?;

            Ok(WorkerStatus {
                success: true,
                running: status.running,
                worker_id: Some(handle.worker_id.clone()),
                status: Some(format!("{:?}", status.worker_core_status)),
                environment: Some(status.environment),
            })
        }
        None => Ok(WorkerStatus {
            success: true,
            running: false,
            worker_id: None,
            status: Some("stopped".to_string()),
            environment: None,
        }),
    }
}

/// Stop the worker system.
#[napi]
pub fn stop_worker() -> Result<WorkerStatus> {
    let mut guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    match guard.as_mut() {
        Some(handle) => {
            let worker_id = handle.worker_id.clone();
            handle
                .runtime
                .block_on(handle.system_handle.stop())
                .map_err(|e| {
                    napi::Error::from(NapiFfiError::RuntimeError(format!("Failed to stop: {}", e)))
                })?;

            *guard = None;

            Ok(WorkerStatus {
                success: true,
                running: false,
                worker_id: Some(worker_id),
                status: Some("stopped".to_string()),
                environment: None,
            })
        }
        None => Ok(WorkerStatus {
            success: true,
            running: false,
            worker_id: None,
            status: Some("not_running".to_string()),
            environment: None,
        }),
    }
}

/// Transition the worker to graceful shutdown mode.
#[napi]
pub fn transition_to_graceful_shutdown() -> Result<WorkerStatus> {
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    match &*guard {
        Some(handle) => {
            handle.runtime.block_on(async {
                let mut worker_core = handle.system_handle.worker_core.lock().await;
                worker_core.stop().await.map_err(|e| {
                    napi::Error::from(NapiFfiError::RuntimeError(format!(
                        "Graceful shutdown failed: {}",
                        e
                    )))
                })
            })?;

            Ok(WorkerStatus {
                success: true,
                running: true,
                worker_id: Some(handle.worker_id.clone()),
                status: Some("transitioning".to_string()),
                environment: None,
            })
        }
        None => Err(napi::Error::from(NapiFfiError::WorkerNotInitialized)),
    }
}

/// Poll for a step event to dispatch.
#[napi]
pub fn poll_step_events() -> Result<Option<NapiStepEvent>> {
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    let handle = guard
        .as_ref()
        .ok_or_else(|| napi::Error::from(NapiFfiError::WorkerNotInitialized))?;

    match handle.ffi_dispatch_channel.poll() {
        Some(event) => Ok(Some(convert_step_event(&event))),
        None => Ok(None),
    }
}

/// Poll for in-process domain events (fast path).
#[napi]
pub fn poll_in_process_events() -> Result<Option<NapiDomainEvent>> {
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    let handle = guard
        .as_ref()
        .ok_or_else(|| napi::Error::from(NapiFfiError::WorkerNotInitialized))?;

    let receiver = match &handle.in_process_event_receiver {
        Some(r) => r,
        None => return Ok(None),
    };

    let mut receiver_guard = receiver
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    match receiver_guard.try_recv() {
        Ok(event) => Ok(Some(convert_domain_event(&event))),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty) => Ok(None),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(count)) => {
            tracing::warn!(
                count,
                "In-process event receiver lagged, some events dropped"
            );
            Ok(None)
        }
        Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
            tracing::warn!("In-process event channel closed");
            Ok(None)
        }
    }
}

/// Complete a step event with the handler's result.
#[napi]
pub fn complete_step_event(event_id: String, result: NapiStepResult) -> Result<bool> {
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    let handle = guard
        .as_ref()
        .ok_or_else(|| napi::Error::from(NapiFfiError::WorkerNotInitialized))?;

    let event_uuid = Uuid::parse_str(&event_id).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!(
            "Invalid event ID: {}",
            e
        )))
    })?;

    let rust_result = convert_napi_result_to_rust(result);
    Ok(handle
        .ffi_dispatch_channel
        .complete(event_uuid, rust_result))
}

/// Yield a checkpoint for batch processing (TAS-125).
#[napi]
pub fn checkpoint_yield_step_event(
    event_id: String,
    checkpoint: NapiCheckpointYieldData,
) -> Result<bool> {
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    let handle = guard
        .as_ref()
        .ok_or_else(|| napi::Error::from(NapiFfiError::WorkerNotInitialized))?;

    let event_uuid = Uuid::parse_str(&event_id).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!(
            "Invalid event ID: {}",
            e
        )))
    })?;

    let step_uuid = Uuid::parse_str(&checkpoint.step_uuid).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!(
            "Invalid step UUID: {}",
            e
        )))
    })?;

    let checkpoint_data = CheckpointYieldData {
        step_uuid,
        cursor: checkpoint.cursor,
        items_processed: checkpoint.items_processed as u64,
        accumulated_results: checkpoint.accumulated_results.and_then(|v| {
            v.as_object()
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        }),
    };

    info!(
        event_id = %event_id,
        step_uuid = %checkpoint_data.step_uuid,
        items_processed = checkpoint_data.items_processed,
        "Checkpoint yield received via napi-rs FFI"
    );

    Ok(handle
        .ffi_dispatch_channel
        .checkpoint_yield(event_uuid, checkpoint_data))
}

/// Get FFI dispatch metrics.
#[napi]
pub fn get_ffi_dispatch_metrics() -> Result<NapiDispatchMetrics> {
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    let handle = guard
        .as_ref()
        .ok_or_else(|| napi::Error::from(NapiFfiError::WorkerNotInitialized))?;

    let metrics = handle.ffi_dispatch_channel.metrics();
    Ok(convert_dispatch_metrics(&metrics))
}

/// Check for and log starvation warnings.
#[napi]
pub fn check_starvation_warnings() -> Result<()> {
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    let handle = guard
        .as_ref()
        .ok_or_else(|| napi::Error::from(NapiFfiError::WorkerNotInitialized))?;

    handle.ffi_dispatch_channel.check_starvation_warnings();
    Ok(())
}

/// Cleanup timed-out events.
#[napi]
pub fn cleanup_timeouts() -> Result<()> {
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    let handle = guard
        .as_ref()
        .ok_or_else(|| napi::Error::from(NapiFfiError::WorkerNotInitialized))?;

    handle.ffi_dispatch_channel.cleanup_timeouts();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_not_initialized() {
        *WORKER_SYSTEM.lock().unwrap() = None;
        let result = get_worker_status().unwrap();
        assert!(!result.running);
    }

    #[test]
    fn test_is_worker_running_not_started() {
        *WORKER_SYSTEM.lock().unwrap() = None;
        assert!(!is_worker_running().unwrap());
    }
}
