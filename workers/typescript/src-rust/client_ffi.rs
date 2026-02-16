//! # TypeScript Client FFI Functions
//!
//! TAS-231: Internal implementations for C FFI client operations.
//! All functions take/return JSON strings for TypeScript interop.

use crate::bridge::WORKER_SYSTEM;
use crate::error::TypeScriptFfiError;
use anyhow::Result;
use serde::Deserialize;
use tracing::error;

/// Helper: call a client method, returning JSON string result.
fn call_client<F>(op_name: &str, f: F) -> Result<String>
where
    F: FnOnce(
        &tasker_worker::FfiClientBridge,
    ) -> Result<serde_json::Value, tasker_worker::FfiClientError>,
{
    let guard = WORKER_SYSTEM
        .lock()
        .map_err(|_| TypeScriptFfiError::LockError)?;

    let handle = guard
        .as_ref()
        .ok_or(TypeScriptFfiError::WorkerNotInitialized)?;

    let client = handle.client.as_ref().ok_or_else(|| {
        TypeScriptFfiError::RuntimeError(
            "Client not initialized. Orchestration client may not be configured.".to_string(),
        )
    })?;

    match f(client) {
        Ok(value) => {
            let json = serde_json::json!({
                "success": true,
                "data": value
            });
            Ok(json.to_string())
        }
        Err(e) => {
            error!(op = op_name, error = %e, recoverable = e.is_recoverable, "Client operation failed");
            let json = serde_json::json!({
                "success": false,
                "error": e.message,
                "recoverable": e.is_recoverable
            });
            Ok(json.to_string())
        }
    }
}

/// Internal implementation of client_create_task.
pub fn client_create_task_internal(request_json: &str) -> Result<String> {
    // Use Deserializer to read exactly one JSON value, tolerating trailing bytes
    // that koffi's string marshalling may include in the buffer.
    let mut deserializer = serde_json::Deserializer::from_str(request_json);
    let task_request =
        tasker_shared::models::core::task_request::TaskRequest::deserialize(&mut deserializer)
            .map_err(|e| {
                TypeScriptFfiError::InvalidArgument(format!("Invalid task request JSON: {e}"))
            })?;

    call_client("create_task", move |client| {
        client.create_task(task_request)
    })
}

/// Internal implementation of client_get_task.
pub fn client_get_task_internal(task_uuid: &str) -> Result<String> {
    let uuid = uuid::Uuid::parse_str(task_uuid)
        .map_err(|e| TypeScriptFfiError::InvalidArgument(format!("Invalid UUID: {e}")))?;

    call_client("get_task", move |client| client.get_task(uuid))
}

/// Internal implementation of client_list_tasks.
pub fn client_list_tasks_internal(params_json: &str) -> Result<String> {
    // Use Deserializer to read exactly one JSON value, tolerating trailing bytes
    // that koffi's string marshalling may include in the buffer.
    let mut deserializer = serde_json::Deserializer::from_str(params_json);
    let params: serde_json::Value = serde_json::Value::deserialize(&mut deserializer)
        .map_err(|e| TypeScriptFfiError::InvalidArgument(format!("Invalid params JSON: {e}")))?;

    let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(50) as i32;
    let offset = params.get("offset").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let namespace = params.get("namespace").and_then(|v| v.as_str());
    let status = params.get("status").and_then(|v| v.as_str());

    // Need owned strings for the closure
    let namespace_owned = namespace.map(String::from);
    let status_owned = status.map(String::from);

    call_client("list_tasks", move |client| {
        client.list_tasks(
            limit,
            offset,
            namespace_owned.as_deref(),
            status_owned.as_deref(),
        )
    })
}

/// Internal implementation of client_cancel_task.
pub fn client_cancel_task_internal(task_uuid: &str) -> Result<String> {
    let uuid = uuid::Uuid::parse_str(task_uuid)
        .map_err(|e| TypeScriptFfiError::InvalidArgument(format!("Invalid UUID: {e}")))?;

    call_client("cancel_task", move |client| client.cancel_task(uuid))
}

/// Internal implementation of client_list_task_steps.
pub fn client_list_task_steps_internal(task_uuid: &str) -> Result<String> {
    let uuid = uuid::Uuid::parse_str(task_uuid)
        .map_err(|e| TypeScriptFfiError::InvalidArgument(format!("Invalid UUID: {e}")))?;

    call_client("list_task_steps", move |client| {
        client.list_task_steps(uuid)
    })
}

/// Internal implementation of client_get_step.
pub fn client_get_step_internal(task_uuid: &str, step_uuid: &str) -> Result<String> {
    let t_uuid = uuid::Uuid::parse_str(task_uuid)
        .map_err(|e| TypeScriptFfiError::InvalidArgument(format!("Invalid task UUID: {e}")))?;
    let s_uuid = uuid::Uuid::parse_str(step_uuid)
        .map_err(|e| TypeScriptFfiError::InvalidArgument(format!("Invalid step UUID: {e}")))?;

    call_client("get_step", move |client| client.get_step(t_uuid, s_uuid))
}

/// Internal implementation of client_get_step_audit_history.
pub fn client_get_step_audit_history_internal(task_uuid: &str, step_uuid: &str) -> Result<String> {
    let t_uuid = uuid::Uuid::parse_str(task_uuid)
        .map_err(|e| TypeScriptFfiError::InvalidArgument(format!("Invalid task UUID: {e}")))?;
    let s_uuid = uuid::Uuid::parse_str(step_uuid)
        .map_err(|e| TypeScriptFfiError::InvalidArgument(format!("Invalid step UUID: {e}")))?;

    call_client("get_step_audit_history", move |client| {
        client.get_step_audit_history(t_uuid, s_uuid)
    })
}

/// Internal implementation of client_health_check.
pub fn client_health_check_internal() -> Result<String> {
    call_client("health_check", |client| client.health_check())
}
