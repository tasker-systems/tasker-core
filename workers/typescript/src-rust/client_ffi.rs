//! # Client FFI Functions (napi-rs)
//!
//! TAS-290: Replaces C FFI client operations with napi-rs native objects.
//! TaskRequest crosses as a native JS object (no JSON serialization),
//! eliminating the TAS-283 "trailing input" class of bugs.

use napi::bindgen_prelude::*;
use tracing::error;

use crate::bridge::WORKER_SYSTEM;
use crate::error::NapiFfiError;

// =============================================================================
// napi-rs Object Types for Client API
// =============================================================================

/// Task creation request — crosses FFI as native JS object.
///
/// In koffi, this was serialized to JSON and parsed with
/// `serde_json::Deserializer::from_str` to tolerate trailing bytes.
/// In napi-rs, no serialization needed. No trailing bytes. No TAS-283.
#[napi(object)]
#[derive(Debug)]
pub struct NapiTaskRequest {
    pub name: String,
    pub namespace: String,
    pub version: String,
    /// Native JS object — napi-rs handles this via serde-json feature
    pub context: serde_json::Value,
    pub initiator: String,
    pub source_system: String,
    pub reason: String,
    pub tags: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub correlation_id: Option<String>,
    pub parent_correlation_id: Option<String>,
    pub idempotency_key: Option<String>,
}

/// Client operation result.
///
/// Maintains the existing TypeScript API surface with success/data/error envelope.
#[napi(object)]
#[derive(Debug)]
pub struct NapiClientResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub recoverable: Option<bool>,
}

/// Parameters for listing tasks.
#[napi(object)]
#[derive(Debug)]
pub struct NapiListTasksParams {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub namespace: Option<String>,
    pub status: Option<String>,
}

// =============================================================================
// Helper: Call client method with error handling
// =============================================================================

fn call_client<F>(op_name: &str, f: F) -> Result<NapiClientResult>
where
    F: FnOnce(
        &tasker_worker::FfiClientBridge,
    ) -> std::result::Result<serde_json::Value, tasker_worker::FfiClientError>,
{
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| napi::Error::from(NapiFfiError::LockError))?;

    let handle = guard
        .as_ref()
        .ok_or_else(|| napi::Error::from(NapiFfiError::WorkerNotInitialized))?;

    let client = handle.client.as_ref().ok_or_else(|| {
        napi::Error::from(NapiFfiError::RuntimeError(
            "Client not initialized. Orchestration client may not be configured.".to_string(),
        ))
    })?;

    match f(client) {
        Ok(value) => Ok(NapiClientResult {
            success: true,
            data: Some(value),
            error: None,
            recoverable: None,
        }),
        Err(e) => {
            error!(op = op_name, error = %e, recoverable = e.is_recoverable, "Client operation failed");
            Ok(NapiClientResult {
                success: false,
                data: None,
                error: Some(e.message.clone()),
                recoverable: Some(e.is_recoverable),
            })
        }
    }
}

// =============================================================================
// napi-rs Client FFI Functions
// =============================================================================

/// Create a task via the orchestration API.
///
/// Native object crossing — no JSON serialization, no TAS-283 trailing input.
#[napi]
pub fn client_create_task(request: NapiTaskRequest) -> Result<NapiClientResult> {
    let correlation_id = match &request.correlation_id {
        Some(id) => uuid::Uuid::parse_str(id).map_err(|e| {
            napi::Error::from(NapiFfiError::InvalidArgument(format!(
                "Invalid correlation_id: {e}"
            )))
        })?,
        None => uuid::Uuid::new_v4(),
    };

    let parent_correlation_id = match &request.parent_correlation_id {
        Some(id) => Some(uuid::Uuid::parse_str(id).map_err(|e| {
            napi::Error::from(NapiFfiError::InvalidArgument(format!(
                "Invalid parent_correlation_id: {e}"
            )))
        })?),
        None => None,
    };

    let task_request = tasker_shared::models::core::task_request::TaskRequest {
        name: request.name,
        namespace: request.namespace,
        version: request.version,
        context: request.context,
        initiator: request.initiator,
        source_system: request.source_system,
        reason: request.reason,
        tags: request.tags.unwrap_or_default(),
        requested_at: chrono::Utc::now().naive_utc(),
        options: None,
        priority: request.priority,
        correlation_id,
        parent_correlation_id,
        idempotency_key: request.idempotency_key,
    };

    call_client("create_task", move |client| {
        client.create_task(task_request)
    })
}

/// Get a task by UUID.
#[napi]
pub fn client_get_task(task_uuid: String) -> Result<NapiClientResult> {
    let uuid = uuid::Uuid::parse_str(&task_uuid).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!("Invalid UUID: {e}")))
    })?;

    call_client("get_task", move |client| client.get_task(uuid))
}

/// List tasks with optional filters.
#[napi]
pub fn client_list_tasks(params: NapiListTasksParams) -> Result<NapiClientResult> {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    let namespace = params.namespace;
    let status = params.status;

    call_client("list_tasks", move |client| {
        client.list_tasks(limit, offset, namespace.as_deref(), status.as_deref())
    })
}

/// Cancel a task by UUID.
#[napi]
pub fn client_cancel_task(task_uuid: String) -> Result<NapiClientResult> {
    let uuid = uuid::Uuid::parse_str(&task_uuid).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!("Invalid UUID: {e}")))
    })?;

    call_client("cancel_task", move |client| client.cancel_task(uuid))
}

/// List steps for a task.
#[napi]
pub fn client_list_task_steps(task_uuid: String) -> Result<NapiClientResult> {
    let uuid = uuid::Uuid::parse_str(&task_uuid).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!("Invalid UUID: {e}")))
    })?;

    call_client("list_task_steps", move |client| {
        client.list_task_steps(uuid)
    })
}

/// Get a specific workflow step.
#[napi]
pub fn client_get_step(task_uuid: String, step_uuid: String) -> Result<NapiClientResult> {
    let t_uuid = uuid::Uuid::parse_str(&task_uuid).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!(
            "Invalid task UUID: {e}"
        )))
    })?;
    let s_uuid = uuid::Uuid::parse_str(&step_uuid).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!(
            "Invalid step UUID: {e}"
        )))
    })?;

    call_client("get_step", move |client| client.get_step(t_uuid, s_uuid))
}

/// Get audit history for a workflow step.
#[napi]
pub fn client_get_step_audit_history(
    task_uuid: String,
    step_uuid: String,
) -> Result<NapiClientResult> {
    let t_uuid = uuid::Uuid::parse_str(&task_uuid).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!(
            "Invalid task UUID: {e}"
        )))
    })?;
    let s_uuid = uuid::Uuid::parse_str(&step_uuid).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!(
            "Invalid step UUID: {e}"
        )))
    })?;

    call_client("get_step_audit_history", move |client| {
        client.get_step_audit_history(t_uuid, s_uuid)
    })
}

/// Health check against the orchestration API.
#[napi]
pub fn client_health_check() -> Result<NapiClientResult> {
    call_client("health_check", |client| client.health_check())
}
