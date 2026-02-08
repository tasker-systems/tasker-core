//! # Ruby Client FFI Functions
//!
//! TAS-231: Exposes orchestration client operations to Ruby via Magnus.
//! Uses serde_magnus for type conversions.

use crate::bridge::WORKER_SYSTEM;
use magnus::{prelude::*, Error, ExceptionClass, Ruby, Value};
use tracing::error;

/// Helper to get RuntimeError exception class
fn runtime_error_class() -> ExceptionClass {
    Ruby::get()
        .expect("Ruby runtime should be available")
        .exception_runtime_error()
}

/// Helper to get ArgumentError exception class
fn arg_error_class() -> ExceptionClass {
    Ruby::get()
        .expect("Ruby runtime should be available")
        .exception_arg_error()
}

/// Helper: call a client method, converting the result to a Ruby value.
fn call_client<F>(op_name: &str, f: F) -> Result<Value, Error>
where
    F: FnOnce(
        &tasker_worker::FfiClientBridge,
    ) -> Result<serde_json::Value, tasker_worker::FfiClientError>,
{
    let ruby = Ruby::get().map_err(|e| {
        Error::new(
            runtime_error_class(),
            format!("Failed to get Ruby runtime: {e}"),
        )
    })?;

    let handle_guard = WORKER_SYSTEM.lock().map_err(|e| {
        error!("Failed to acquire worker system lock: {}", e);
        Error::new(runtime_error_class(), "Lock acquisition failed")
    })?;

    let handle = handle_guard
        .as_ref()
        .ok_or_else(|| Error::new(runtime_error_class(), "Worker system not running"))?;

    let client = handle.client.as_ref().ok_or_else(|| {
        Error::new(
            runtime_error_class(),
            "Client not initialized. Orchestration client may not be configured.",
        )
    })?;

    match f(client) {
        Ok(value) => serde_magnus::serialize(&ruby, &value).map_err(|e| {
            Error::new(
                runtime_error_class(),
                format!("Failed to convert {op_name} response: {e}"),
            )
        }),
        Err(e) => {
            error!(op = op_name, error = %e, recoverable = e.is_recoverable, "Client operation failed");
            Err(Error::new(
                runtime_error_class(),
                format!("{op_name} failed: {e}"),
            ))
        }
    }
}

/// Create a new task via the orchestration API.
pub fn client_create_task(request_hash: Value) -> Result<Value, Error> {
    let ruby = Ruby::get().map_err(|e| {
        Error::new(
            runtime_error_class(),
            format!("Failed to get Ruby runtime: {e}"),
        )
    })?;
    let task_request: tasker_shared::models::core::task_request::TaskRequest =
        serde_magnus::deserialize(&ruby, request_hash)
            .map_err(|e| Error::new(arg_error_class(), format!("Invalid task request: {e}")))?;

    call_client("create_task", move |client| {
        client.create_task(task_request)
    })
}

/// Get a task by UUID.
pub fn client_get_task(task_uuid: String) -> Result<Value, Error> {
    let uuid = uuid::Uuid::parse_str(&task_uuid)
        .map_err(|e| Error::new(arg_error_class(), format!("Invalid UUID: {e}")))?;

    call_client("get_task", move |client| client.get_task(uuid))
}

/// List tasks with pagination and optional filters.
pub fn client_list_tasks(
    limit: i32,
    offset: i32,
    namespace: Option<String>,
    status: Option<String>,
) -> Result<Value, Error> {
    call_client("list_tasks", move |client| {
        client.list_tasks(limit, offset, namespace.as_deref(), status.as_deref())
    })
}

/// Cancel a task.
pub fn client_cancel_task(task_uuid: String) -> Result<Value, Error> {
    let uuid = uuid::Uuid::parse_str(&task_uuid)
        .map_err(|e| Error::new(arg_error_class(), format!("Invalid UUID: {e}")))?;

    call_client("cancel_task", move |client| client.cancel_task(uuid))
}

/// List workflow steps for a task.
pub fn client_list_task_steps(task_uuid: String) -> Result<Value, Error> {
    let uuid = uuid::Uuid::parse_str(&task_uuid)
        .map_err(|e| Error::new(arg_error_class(), format!("Invalid UUID: {e}")))?;

    call_client("list_task_steps", move |client| {
        client.list_task_steps(uuid)
    })
}

/// Get a specific workflow step.
pub fn client_get_step(task_uuid: String, step_uuid: String) -> Result<Value, Error> {
    let t_uuid = uuid::Uuid::parse_str(&task_uuid)
        .map_err(|e| Error::new(arg_error_class(), format!("Invalid task UUID: {e}")))?;
    let s_uuid = uuid::Uuid::parse_str(&step_uuid)
        .map_err(|e| Error::new(arg_error_class(), format!("Invalid step UUID: {e}")))?;

    call_client("get_step", move |client| client.get_step(t_uuid, s_uuid))
}

/// Get audit history for a workflow step.
pub fn client_get_step_audit_history(task_uuid: String, step_uuid: String) -> Result<Value, Error> {
    let t_uuid = uuid::Uuid::parse_str(&task_uuid)
        .map_err(|e| Error::new(arg_error_class(), format!("Invalid task UUID: {e}")))?;
    let s_uuid = uuid::Uuid::parse_str(&step_uuid)
        .map_err(|e| Error::new(arg_error_class(), format!("Invalid step UUID: {e}")))?;

    call_client("get_step_audit_history", move |client| {
        client.get_step_audit_history(t_uuid, s_uuid)
    })
}

/// Check if the orchestration API is healthy.
pub fn client_health_check() -> Result<Value, Error> {
    call_client("health_check", |client| client.health_check())
}

/// Initialize the client FFI module with all client methods.
pub fn init_client_ffi(module: &magnus::RModule) -> Result<(), Error> {
    use magnus::function;

    module.define_singleton_method("client_create_task", function!(client_create_task, 1))?;
    module.define_singleton_method("client_get_task", function!(client_get_task, 1))?;
    module.define_singleton_method("client_list_tasks", function!(client_list_tasks, 4))?;
    module.define_singleton_method("client_cancel_task", function!(client_cancel_task, 1))?;
    module.define_singleton_method(
        "client_list_task_steps",
        function!(client_list_task_steps, 1),
    )?;
    module.define_singleton_method("client_get_step", function!(client_get_step, 2))?;
    module.define_singleton_method(
        "client_get_step_audit_history",
        function!(client_get_step_audit_history, 2),
    )?;
    module.define_singleton_method("client_health_check", function!(client_health_check, 0))?;

    Ok(())
}
