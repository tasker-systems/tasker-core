//! C FFI bindings for tasker-core TypeScript/JavaScript worker
//!
//! This module provides the FFI interface between Rust and JavaScript runtimes
//! (Node.js, Bun, Deno), exposing worker functionality through a C-compatible API.
//!
//! # Runtime Support
//!
//! - **Node.js**: Via `ffi-napi` package
//! - **Bun**: Via built-in `bun:ffi`
//! - **Deno**: Via `Deno.dlopen`
//!
//! # Memory Management
//!
//! All strings returned from Rust are heap-allocated and must be freed by calling
//! `free_rust_string`. The caller is responsible for managing the lifetime of
//! returned pointers.
//!
//! # Thread Safety
//!
//! The FFI functions are designed to be called from a single thread. The internal
//! state is protected by a Mutex, but concurrent calls from multiple threads are
//! not recommended due to JavaScript's single-threaded nature.
//!
//! # Phases
//!
//! - **Phase 1 (TAS-101)**: FFI scaffolding, runtime detection, event polling
//! - **Phase 2 (TAS-102)**: Handler API and registry
//! - **Phase 3 (TAS-103)**: Specialized handlers
//! - **Phase 4 (TAS-104)**: Server and bootstrap
//! - **Phase 5 (TAS-105)**: Testing and examples
//! - **Phase 6 (TAS-106)**: Runtime optimizations
//! - **Phase 7 (TAS-107)**: Documentation

#![expect(
    dead_code,
    reason = "FFI module with functions exposed to TypeScript runtimes"
)]
#![allow(clippy::missing_safety_doc)]

use std::ffi::{c_char, c_int, CStr, CString};
use std::panic::{self, AssertUnwindSafe};
use std::ptr;

mod bridge;
mod conversions;
mod dto;
mod error;
mod ffi_logging;

// Re-export bridge functions for internal use
use bridge::WORKER_SYSTEM;

/// Returns the version of the tasker-worker-ts package.
///
/// # Safety
///
/// The returned pointer is a heap-allocated C string that must be freed
/// by calling `free_rust_string`.
#[no_mangle]
pub extern "C" fn get_version() -> *mut c_char {
    let version = env!("CARGO_PKG_VERSION");
    match CString::new(version) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Returns the Rust library version for debugging.
///
/// # Safety
///
/// The returned pointer is a heap-allocated C string that must be freed
/// by calling `free_rust_string`.
#[no_mangle]
pub extern "C" fn get_rust_version() -> *mut c_char {
    let version = format!(
        "tasker-worker-ts {} (rustc {})",
        env!("CARGO_PKG_VERSION"),
        env!("RUSTC_VERSION")
    );
    match CString::new(version) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Check if the FFI module is working correctly.
///
/// Returns 1 if the FFI layer is functional, 0 otherwise.
#[no_mangle]
pub extern "C" fn health_check() -> c_int {
    1
}

/// Check if the worker is currently running.
///
/// Returns 1 if running, 0 if not.
#[no_mangle]
pub extern "C" fn is_worker_running() -> c_int {
    match WORKER_SYSTEM.lock() {
        Ok(guard) => {
            if guard.is_some() {
                1
            } else {
                0
            }
        }
        Err(_) => 0,
    }
}

/// Bootstrap the worker with the given configuration.
///
/// TAS-173: Uses `catch_unwind` to prevent panics from crossing the FFI boundary.
///
/// # Parameters
///
/// - `config_json`: JSON string containing bootstrap configuration, or null for defaults
///
/// # Returns
///
/// JSON string containing the bootstrap result, or null on error.
/// The returned pointer must be freed with `free_rust_string`.
///
/// # Safety
///
/// - `config_json` must be a valid null-terminated C string if not null
/// - The returned pointer must be freed by calling `free_rust_string`
#[no_mangle]
pub unsafe extern "C" fn bootstrap_worker(config_json: *const c_char) -> *mut c_char {
    let config_str = if config_json.is_null() {
        None
    } else {
        // SAFETY: Caller guarantees config_json is a valid null-terminated C string
        match unsafe { CStr::from_ptr(config_json) }.to_str() {
            Ok(s) => Some(s),
            Err(_) => {
                return json_error("Invalid UTF-8 in config_json");
            }
        }
    };

    // TAS-173: Catch panics to prevent undefined behavior at FFI boundary
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        bridge::bootstrap_worker_internal(config_str)
    }));

    match result {
        Ok(Ok(json)) => match CString::new(json) {
            Ok(s) => s.into_raw(),
            Err(_) => json_error("Failed to create result string"),
        },
        Ok(Err(e)) => json_error(&format!("Bootstrap failed: {}", e)),
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                format!("Bootstrap panicked: {}", s)
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                format!("Bootstrap panicked: {}", s)
            } else {
                "Bootstrap panicked with unknown error".to_string()
            };
            tracing::error!("{}", msg);
            json_error(&msg)
        }
    }
}

/// Get the current worker status.
///
/// # Returns
///
/// JSON string containing worker status, or null on error.
/// The returned pointer must be freed with `free_rust_string`.
#[no_mangle]
pub extern "C" fn get_worker_status() -> *mut c_char {
    match panic::catch_unwind(bridge::get_worker_status_internal) {
        Ok(Ok(result)) => match CString::new(result) {
            Ok(s) => s.into_raw(),
            Err(_) => json_error("Failed to create status string"),
        },
        Ok(Err(e)) => json_error(&format!("Failed to get status: {}", e)),
        Err(_) => json_error("get_worker_status panicked unexpectedly"),
    }
}

/// Stop the worker gracefully.
///
/// # Returns
///
/// JSON string containing the stop result, or null on error.
/// The returned pointer must be freed with `free_rust_string`.
#[no_mangle]
pub extern "C" fn stop_worker() -> *mut c_char {
    match panic::catch_unwind(bridge::stop_worker_internal) {
        Ok(Ok(result)) => match CString::new(result) {
            Ok(s) => s.into_raw(),
            Err(_) => json_error("Failed to create result string"),
        },
        Ok(Err(e)) => json_error(&format!("Failed to stop worker: {}", e)),
        Err(_) => json_error("stop_worker panicked unexpectedly"),
    }
}

/// Transition the worker to graceful shutdown mode.
///
/// # Returns
///
/// JSON string containing the transition result, or null on error.
/// The returned pointer must be freed with `free_rust_string`.
#[no_mangle]
pub extern "C" fn transition_to_graceful_shutdown() -> *mut c_char {
    match panic::catch_unwind(bridge::transition_to_graceful_shutdown_internal) {
        Ok(Ok(result)) => match CString::new(result) {
            Ok(s) => s.into_raw(),
            Err(_) => json_error("Failed to create result string"),
        },
        Ok(Err(e)) => json_error(&format!("Failed to transition: {}", e)),
        Err(_) => json_error("transition_to_graceful_shutdown panicked unexpectedly"),
    }
}

/// Poll for pending step events.
///
/// # Returns
///
/// JSON string containing a step event, or null if no events are available.
/// The returned pointer must be freed with `free_rust_string`.
#[no_mangle]
pub extern "C" fn poll_step_events() -> *mut c_char {
    match panic::catch_unwind(bridge::poll_step_events_internal) {
        Ok(Ok(Some(result))) => match CString::new(result) {
            Ok(s) => s.into_raw(),
            Err(_) => ptr::null_mut(),
        },
        Ok(Ok(None)) => ptr::null_mut(),
        Ok(Err(e)) => {
            tracing::error!("Failed to poll step events: {}", e);
            ptr::null_mut()
        }
        Err(_) => {
            tracing::error!("poll_step_events panicked unexpectedly");
            ptr::null_mut()
        }
    }
}

/// Poll for in-process domain events (fast path).
///
/// This is used for real-time notifications that don't require
/// guaranteed delivery (e.g., metrics updates, logging, notifications).
///
/// # Returns
///
/// JSON string containing a domain event, or null if no events are available.
/// The returned pointer must be freed with `free_rust_string`.
///
/// # Event Structure
///
/// ```json
/// {
///   "eventId": "uuid-string",
///   "eventName": "payment.processed",
///   "eventVersion": "1.0.0",
///   "metadata": {
///     "taskUuid": "uuid-string",
///     "stepUuid": "uuid-string",
///     "stepName": "process_payment",
///     "namespace": "payments",
///     "correlationId": "uuid-string",
///     "firedAt": "2024-01-01T00:00:00Z",
///     "firedBy": "step_execution"
///   },
///   "payload": { ... }
/// }
/// ```
#[no_mangle]
pub extern "C" fn poll_in_process_events() -> *mut c_char {
    match panic::catch_unwind(bridge::poll_in_process_events_internal) {
        Ok(Ok(Some(result))) => match CString::new(result) {
            Ok(s) => s.into_raw(),
            Err(_) => ptr::null_mut(),
        },
        Ok(Ok(None)) => ptr::null_mut(),
        Ok(Err(e)) => {
            tracing::error!("Failed to poll in-process events: {}", e);
            ptr::null_mut()
        }
        Err(_) => {
            tracing::error!("poll_in_process_events panicked unexpectedly");
            ptr::null_mut()
        }
    }
}

/// Complete a step event with the given result.
///
/// # Parameters
///
/// - `event_id`: UUID string of the event to complete
/// - `result_json`: JSON string containing the step execution result
///
/// # Returns
///
/// 1 on success, 0 on failure.
///
/// # Safety
///
/// Both parameters must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn complete_step_event(
    event_id: *const c_char,
    result_json: *const c_char,
) -> c_int {
    if event_id.is_null() || result_json.is_null() {
        tracing::error!(
            "complete_step_event: null pointer received (event_id={}, result_json={})",
            event_id.is_null(),
            result_json.is_null()
        );
        return 0;
    }

    // SAFETY: Caller guarantees event_id is a valid null-terminated C string
    let event_id_str = match unsafe { CStr::from_ptr(event_id) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("complete_step_event: invalid UTF-8 in event_id: {}", e);
            return 0;
        }
    };

    // SAFETY: Caller guarantees result_json is a valid null-terminated C string
    let result_str = match unsafe { CStr::from_ptr(result_json) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("complete_step_event: invalid UTF-8 in result_json: {}", e);
            return 0;
        }
    };

    tracing::info!(
        event_id = %event_id_str,
        result_json_len = result_str.len(),
        "complete_step_event: FFI call received"
    );

    let event_id_owned = event_id_str.to_string();
    let result_owned = result_str.to_string();

    match panic::catch_unwind(AssertUnwindSafe(|| {
        bridge::complete_step_event_internal(&event_id_owned, &result_owned)
    })) {
        Ok(Ok(true)) => {
            tracing::info!(event_id = %event_id_str, "complete_step_event: SUCCESS");
            1
        }
        Ok(Ok(false)) => {
            tracing::warn!(event_id = %event_id_str, "complete_step_event: returned false (event not in pending)");
            0
        }
        Ok(Err(e)) => {
            tracing::error!(event_id = %event_id_str, error = %e, "complete_step_event: internal error");
            0
        }
        Err(_) => {
            tracing::error!(event_id = %event_id_str, "complete_step_event panicked unexpectedly");
            0
        }
    }
}

/// Yield a checkpoint for batch processing (TAS-125).
///
/// Signals a checkpoint yield, persisting the checkpoint data and causing
/// the step to be re-dispatched for continued processing. Unlike
/// `complete_step_event`, this does NOT complete the step.
///
/// # Parameters
///
/// - `event_id`: UUID string of the event
/// - `checkpoint_json`: JSON string containing the checkpoint data with fields:
///   - `step_uuid`: UUID of the step being checkpointed
///   - `cursor`: Current cursor position (where to resume)
///   - `items_processed`: Number of items successfully processed so far
///   - `accumulated_results`: Optional partial results to carry forward
///
/// # Returns
///
/// 1 on success (checkpoint persisted and step re-dispatched), 0 on failure.
///
/// # Safety
///
/// Both parameters must be valid null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn checkpoint_yield_step_event(
    event_id: *const c_char,
    checkpoint_json: *const c_char,
) -> c_int {
    if event_id.is_null() || checkpoint_json.is_null() {
        tracing::error!(
            "checkpoint_yield_step_event: null pointer received (event_id={}, checkpoint_json={})",
            event_id.is_null(),
            checkpoint_json.is_null()
        );
        return 0;
    }

    // SAFETY: Caller guarantees event_id is a valid null-terminated C string
    let event_id_str = match unsafe { CStr::from_ptr(event_id) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                "checkpoint_yield_step_event: invalid UTF-8 in event_id: {}",
                e
            );
            return 0;
        }
    };

    // SAFETY: Caller guarantees checkpoint_json is a valid null-terminated C string
    let checkpoint_str = match unsafe { CStr::from_ptr(checkpoint_json) }.to_str() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                "checkpoint_yield_step_event: invalid UTF-8 in checkpoint_json: {}",
                e
            );
            return 0;
        }
    };

    tracing::info!(
        event_id = %event_id_str,
        checkpoint_json_len = checkpoint_str.len(),
        "checkpoint_yield_step_event: FFI call received"
    );

    let event_id_owned = event_id_str.to_string();
    let checkpoint_owned = checkpoint_str.to_string();

    match panic::catch_unwind(AssertUnwindSafe(|| {
        bridge::checkpoint_yield_step_event_internal(&event_id_owned, &checkpoint_owned)
    })) {
        Ok(Ok(true)) => {
            tracing::info!(event_id = %event_id_str, "checkpoint_yield_step_event: SUCCESS");
            1
        }
        Ok(Ok(false)) => {
            tracing::warn!(event_id = %event_id_str, "checkpoint_yield_step_event: returned false (checkpoint support not configured or event not found)");
            0
        }
        Ok(Err(e)) => {
            tracing::error!(event_id = %event_id_str, error = %e, "checkpoint_yield_step_event: internal error");
            0
        }
        Err(_) => {
            tracing::error!(event_id = %event_id_str, "checkpoint_yield_step_event panicked unexpectedly");
            0
        }
    }
}

/// Get FFI dispatch metrics.
///
/// # Returns
///
/// JSON string containing dispatch metrics, or null on error.
/// The returned pointer must be freed with `free_rust_string`.
#[no_mangle]
pub extern "C" fn get_ffi_dispatch_metrics() -> *mut c_char {
    match panic::catch_unwind(bridge::get_ffi_dispatch_metrics_internal) {
        Ok(Ok(result)) => match CString::new(result) {
            Ok(s) => s.into_raw(),
            Err(_) => json_error("Failed to create metrics string"),
        },
        Ok(Err(e)) => json_error(&format!("Failed to get metrics: {}", e)),
        Err(_) => json_error("get_ffi_dispatch_metrics panicked unexpectedly"),
    }
}

/// Check for and log starvation warnings.
#[no_mangle]
pub extern "C" fn check_starvation_warnings() {
    let _ = panic::catch_unwind(|| {
        if let Err(e) = bridge::check_starvation_warnings_internal() {
            tracing::error!("Failed to check starvation warnings: {}", e);
        }
    });
}

/// Cleanup timed-out events.
#[no_mangle]
pub extern "C" fn cleanup_timeouts() {
    let _ = panic::catch_unwind(|| {
        if let Err(e) = bridge::cleanup_timeouts_internal() {
            tracing::error!("Failed to cleanup timeouts: {}", e);
        }
    });
}

/// Log an error message.
///
/// # Parameters
///
/// - `message`: The error message to log
/// - `fields_json`: Optional JSON string with additional fields, or null
///
/// # Safety
///
/// `message` must be a valid null-terminated C string.
/// `fields_json` must be a valid null-terminated C string or null.
#[no_mangle]
pub unsafe extern "C" fn log_error(message: *const c_char, fields_json: *const c_char) {
    // SAFETY: Caller guarantees message and fields_json are valid C strings or null
    unsafe { ffi_logging::log_at_level(tracing::Level::ERROR, message, fields_json) };
}

/// Log a warning message.
#[no_mangle]
pub unsafe extern "C" fn log_warn(message: *const c_char, fields_json: *const c_char) {
    // SAFETY: Caller guarantees message and fields_json are valid C strings or null
    unsafe { ffi_logging::log_at_level(tracing::Level::WARN, message, fields_json) };
}

/// Log an info message.
#[no_mangle]
pub unsafe extern "C" fn log_info(message: *const c_char, fields_json: *const c_char) {
    // SAFETY: Caller guarantees message and fields_json are valid C strings or null
    unsafe { ffi_logging::log_at_level(tracing::Level::INFO, message, fields_json) };
}

/// Log a debug message.
#[no_mangle]
pub unsafe extern "C" fn log_debug(message: *const c_char, fields_json: *const c_char) {
    // SAFETY: Caller guarantees message and fields_json are valid C strings or null
    unsafe { ffi_logging::log_at_level(tracing::Level::DEBUG, message, fields_json) };
}

/// Log a trace message.
#[no_mangle]
pub unsafe extern "C" fn log_trace(message: *const c_char, fields_json: *const c_char) {
    // SAFETY: Caller guarantees message and fields_json are valid C strings or null
    unsafe { ffi_logging::log_at_level(tracing::Level::TRACE, message, fields_json) };
}

/// Free a string that was allocated by Rust.
///
/// # Safety
///
/// `ptr` must be a pointer that was returned by one of the FFI functions
/// in this module, or null.
#[no_mangle]
pub unsafe extern "C" fn free_rust_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        // SAFETY: We're taking ownership back of a CString we created.
        // The caller guarantees ptr was returned by one of our FFI functions.
        drop(unsafe { CString::from_raw(ptr) });
    }
}

/// Helper to create a JSON error response.
fn json_error(message: &str) -> *mut c_char {
    let error = serde_json::json!({
        "success": false,
        "error": message
    });
    match CString::new(error.to_string()) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_version() {
        let version_ptr = get_version();
        assert!(!version_ptr.is_null());

        // SAFETY: version_ptr was returned by get_version() and is a valid C string
        unsafe {
            let version = CStr::from_ptr(version_ptr).to_str().unwrap();
            assert!(!version.is_empty());
            assert!(version.contains('.'));
            free_rust_string(version_ptr);
        }
    }

    #[test]
    fn test_get_rust_version() {
        let version_ptr = get_rust_version();
        assert!(!version_ptr.is_null());

        // SAFETY: version_ptr was returned by get_rust_version() and is a valid C string
        unsafe {
            let version = CStr::from_ptr(version_ptr).to_str().unwrap();
            assert!(version.contains("tasker-worker-ts"));
            assert!(version.contains("rustc"));
            free_rust_string(version_ptr);
        }
    }

    #[test]
    fn test_health_check() {
        assert_eq!(health_check(), 1);
    }

    #[test]
    fn test_is_worker_running_not_started() {
        assert_eq!(is_worker_running(), 0);
    }
}
