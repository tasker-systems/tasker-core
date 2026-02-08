//! # Python Client FFI Functions
//!
//! TAS-231: Exposes orchestration client operations to Python via PyO3.
//! Uses pythonize/depythonize for type conversions.

use crate::bridge::with_worker_system;
use crate::error::PythonFfiError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use tracing::error;

/// Helper: call a client method, converting the result to a Python object.
fn call_client<F>(op_name: &str, f: F) -> PyResult<Py<PyAny>>
where
    F: FnOnce(
        &tasker_worker::FfiClientBridge,
    ) -> Result<serde_json::Value, tasker_worker::FfiClientError>,
{
    with_worker_system(|handle| {
        let client = handle
            .client
            .as_ref()
            .ok_or(PythonFfiError::RuntimeError(
                "Client not initialized. Orchestration client may not be configured.".to_string(),
            ))?;

        match f(client) {
            Ok(value) => Python::attach(|py| {
                let bound = pythonize::pythonize(py, &value).map_err(|e| {
                    PythonFfiError::ConversionError(format!(
                        "Failed to convert {op_name} response: {e}"
                    ))
                })?;
                Ok(bound.unbind())
            }),
            Err(e) => {
                error!(op = op_name, error = %e, recoverable = e.is_recoverable, "Client operation failed");
                Err(PythonFfiError::RuntimeError(format!(
                    "{op_name} failed: {e}"
                )))
            }
        }
    })
    .map_err(PyErr::from)
}

/// Create a new task via the orchestration API.
///
/// Args:
///     request: dict with keys `name`, `namespace`, `version`, and optional `context`
///
/// Returns:
///     dict: Task response from the orchestration API
#[pyfunction]
pub fn client_create_task(request: &Bound<'_, PyDict>) -> PyResult<Py<PyAny>> {
    let task_request: tasker_shared::models::core::task_request::TaskRequest =
        pythonize::depythonize(request)
            .map_err(|e| PythonFfiError::InvalidArgument(format!("Invalid task request: {e}")))?;

    call_client("create_task", move |client| {
        client.create_task(task_request)
    })
}

/// Get a task by UUID.
///
/// Args:
///     task_uuid: UUID string of the task
///
/// Returns:
///     dict: Task response from the orchestration API
#[pyfunction]
pub fn client_get_task(task_uuid: String) -> PyResult<Py<PyAny>> {
    let uuid = uuid::Uuid::parse_str(&task_uuid)
        .map_err(|e| PythonFfiError::InvalidArgument(format!("Invalid UUID: {e}")))?;

    call_client("get_task", move |client| client.get_task(uuid))
}

/// List tasks with pagination and optional filters.
///
/// Args:
///     limit: Maximum number of tasks to return (default 50)
///     offset: Offset for pagination (default 0)
///     namespace: Optional namespace filter
///     status: Optional status filter
///
/// Returns:
///     dict: Task list response with pagination info
#[pyfunction]
#[pyo3(signature = (limit=50, offset=0, namespace=None, status=None))]
pub fn client_list_tasks(
    limit: i32,
    offset: i32,
    namespace: Option<String>,
    status: Option<String>,
) -> PyResult<Py<PyAny>> {
    call_client("list_tasks", move |client| {
        client.list_tasks(limit, offset, namespace.as_deref(), status.as_deref())
    })
}

/// Cancel a task.
///
/// Args:
///     task_uuid: UUID string of the task to cancel
///
/// Returns:
///     dict: `{"cancelled": true}` on success
#[pyfunction]
pub fn client_cancel_task(task_uuid: String) -> PyResult<Py<PyAny>> {
    let uuid = uuid::Uuid::parse_str(&task_uuid)
        .map_err(|e| PythonFfiError::InvalidArgument(format!("Invalid UUID: {e}")))?;

    call_client("cancel_task", move |client| client.cancel_task(uuid))
}

/// List workflow steps for a task.
///
/// Args:
///     task_uuid: UUID string of the task
///
/// Returns:
///     list[dict]: Step responses
#[pyfunction]
pub fn client_list_task_steps(task_uuid: String) -> PyResult<Py<PyAny>> {
    let uuid = uuid::Uuid::parse_str(&task_uuid)
        .map_err(|e| PythonFfiError::InvalidArgument(format!("Invalid UUID: {e}")))?;

    call_client("list_task_steps", move |client| {
        client.list_task_steps(uuid)
    })
}

/// Get a specific workflow step.
///
/// Args:
///     task_uuid: UUID string of the task
///     step_uuid: UUID string of the step
///
/// Returns:
///     dict: Step response
#[pyfunction]
pub fn client_get_step(task_uuid: String, step_uuid: String) -> PyResult<Py<PyAny>> {
    let t_uuid = uuid::Uuid::parse_str(&task_uuid)
        .map_err(|e| PythonFfiError::InvalidArgument(format!("Invalid task UUID: {e}")))?;
    let s_uuid = uuid::Uuid::parse_str(&step_uuid)
        .map_err(|e| PythonFfiError::InvalidArgument(format!("Invalid step UUID: {e}")))?;

    call_client("get_step", move |client| client.get_step(t_uuid, s_uuid))
}

/// Get audit history for a workflow step.
///
/// Args:
///     task_uuid: UUID string of the task
///     step_uuid: UUID string of the step
///
/// Returns:
///     list[dict]: Step audit history entries
#[pyfunction]
pub fn client_get_step_audit_history(task_uuid: String, step_uuid: String) -> PyResult<Py<PyAny>> {
    let t_uuid = uuid::Uuid::parse_str(&task_uuid)
        .map_err(|e| PythonFfiError::InvalidArgument(format!("Invalid task UUID: {e}")))?;
    let s_uuid = uuid::Uuid::parse_str(&step_uuid)
        .map_err(|e| PythonFfiError::InvalidArgument(format!("Invalid step UUID: {e}")))?;

    call_client("get_step_audit_history", move |client| {
        client.get_step_audit_history(t_uuid, s_uuid)
    })
}

/// Check if the orchestration API is healthy.
///
/// Returns:
///     dict: `{"healthy": true}` on success
#[pyfunction]
pub fn client_health_check() -> PyResult<Py<PyAny>> {
    call_client("health_check", |client| client.health_check())
}
