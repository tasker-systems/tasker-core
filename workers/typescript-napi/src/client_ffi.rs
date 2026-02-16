//! # Client FFI Functions (napi-rs)
//!
//! This is the critical module for the spike. The `client_create_task` function
//! is the exact function that fails with "trailing input" in the koffi approach.
//!
//! With napi-rs:
//! - TaskRequest crosses as a native JS object (no JSON serialization)
//! - The entire class of trailing-bytes-in-C-string bugs is eliminated
//! - Errors become JavaScript exceptions (no JSON envelope)

use napi::bindgen_prelude::*;
use tracing::error;

use crate::bridge::WORKER_SYSTEM;
use crate::error::NapiFfiError;

// =============================================================================
// napi-rs Object Types for Client API
// =============================================================================

/// Task creation request — THE critical type.
///
/// In koffi, this was serialized to JSON, passed as a C string, and parsed
/// with `serde_json::Deserializer::from_str` to tolerate trailing bytes.
/// That workaround STILL fails (TAS-283).
///
/// In napi-rs, this is a native JS object. No serialization. No trailing bytes.
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
/// In koffi, all client operations returned JSON: {"success": true, "data": {...}}
/// In napi-rs, success returns the data directly, failure throws an exception.
/// We keep the envelope for now to match the existing TypeScript API surface.
#[napi(object)]
#[derive(Debug)]
pub struct NapiClientResult {
    pub success: bool,
    /// The response data as a JS object (task info, step list, etc.)
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

/// Create a task — **THE definitive TAS-283 test**.
///
/// In koffi: `client_create_task(json_string) -> json_string`
///   → Fails with "trailing input" due to C string marshalling
///
/// In napi-rs: `clientCreateTask(request: NapiTaskRequest) -> NapiClientResult`
///   → Native object, no serialization, no trailing bytes
#[napi]
pub fn client_create_task(request: NapiTaskRequest) -> Result<NapiClientResult> {
    // Convert napi object to internal TaskRequest — no JSON parsing!
    let correlation_id = match &request.correlation_id {
        Some(id) => uuid::Uuid::parse_str(id).map_err(|e| {
            napi::Error::from(NapiFfiError::InvalidArgument(format!(
                "Invalid correlation_id: {}",
                e
            )))
        })?,
        None => uuid::Uuid::new_v4(),
    };

    let parent_correlation_id = match &request.parent_correlation_id {
        Some(id) => Some(uuid::Uuid::parse_str(id).map_err(|e| {
            napi::Error::from(NapiFfiError::InvalidArgument(format!(
                "Invalid parent_correlation_id: {}",
                e
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
        napi::Error::from(NapiFfiError::InvalidArgument(format!(
            "Invalid UUID: {}",
            e
        )))
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
        napi::Error::from(NapiFfiError::InvalidArgument(format!(
            "Invalid UUID: {}",
            e
        )))
    })?;

    call_client("cancel_task", move |client| client.cancel_task(uuid))
}

/// List steps for a task.
#[napi]
pub fn client_list_task_steps(task_uuid: String) -> Result<NapiClientResult> {
    let uuid = uuid::Uuid::parse_str(&task_uuid).map_err(|e| {
        napi::Error::from(NapiFfiError::InvalidArgument(format!(
            "Invalid UUID: {}",
            e
        )))
    })?;

    call_client("list_task_steps", move |client| {
        client.list_task_steps(uuid)
    })
}

/// Health check against the orchestration API.
#[napi]
pub fn client_health_check() -> Result<NapiClientResult> {
    call_client("health_check", |client| client.health_check())
}
