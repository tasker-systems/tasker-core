//! napi-rs FFI bindings for tasker-core TypeScript/JavaScript worker.
//!
//! TAS-290: Replaces C FFI (koffi) with napi-rs native bindings.
//!
//! Key improvements over the koffi approach:
//! - Native JavaScript objects cross the FFI boundary (no JSON serialization)
//! - Errors become JavaScript exceptions (no `{success, error}` envelope)
//! - No manual memory management (`free_rust_string` eliminated)
//! - Eliminates entire class of TAS-283 "trailing input" bugs

#[macro_use]
extern crate napi_derive;

mod bridge;
mod client_ffi;
mod error;
mod ffi_logging;

/// Returns the version of the tasker-ts package.
#[napi]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns the Rust library version for debugging.
#[napi]
pub fn get_rust_version() -> String {
    format!(
        "tasker-ts {} (rustc {})",
        env!("CARGO_PKG_VERSION"),
        env!("RUSTC_VERSION")
    )
}

/// Simple health check to verify the FFI layer is functional.
#[napi]
pub fn health_check() -> bool {
    true
}

// Re-export bridge functions at module level
pub use bridge::{
    bootstrap_worker, check_starvation_warnings, checkpoint_yield_step_event, cleanup_timeouts,
    complete_step_event, get_ffi_dispatch_metrics, get_worker_status, is_worker_running,
    poll_in_process_events, poll_step_events, stop_worker, transition_to_graceful_shutdown,
};

// Re-export client functions at module level
pub use client_ffi::{
    client_cancel_task, client_create_task, client_get_step, client_get_step_audit_history,
    client_get_task, client_health_check, client_list_task_steps, client_list_tasks,
};

// Re-export logging functions at module level
pub use ffi_logging::{log_debug, log_error, log_info, log_trace, log_warn};
