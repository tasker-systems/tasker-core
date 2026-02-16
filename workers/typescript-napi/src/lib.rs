//! # napi-rs FFI Spike for TypeScript Worker
//!
//! Research spike to evaluate napi-rs as a replacement for koffi + C FFI.
//! See RESEARCH.md for findings.
//!
//! ## Why napi-rs?
//!
//! The current koffi approach requires:
//! - Manual JSON serialization on every FFI call
//! - Manual memory management (free_rust_string)
//! - C string handling that causes TAS-283 "trailing input" bugs
//!
//! napi-rs provides native JavaScript object mapping (like pyo3/magnus),
//! eliminating the entire class of bugs.

#![allow(clippy::module_inception)]

#[macro_use]
extern crate napi_derive;

mod bridge;
mod client_ffi;
mod error;

/// Returns the version of the tasker-ts-napi package.
///
/// This is the simplest possible FFI function — no state, no serialization.
/// If this fails, napi-rs itself isn't working.
#[napi]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Returns the Rust library version for debugging.
#[napi]
pub fn get_rust_version() -> String {
    format!(
        "tasker-ts-napi {} (napi-rs spike)",
        env!("CARGO_PKG_VERSION")
    )
}

/// Simple health check to verify the FFI layer is functional.
#[napi]
pub fn health_check() -> bool {
    true
}

// Re-export bridge functions at module level
pub use bridge::{
    bootstrap_worker, complete_step_event, get_worker_status, poll_step_events, stop_worker,
};

// Re-export client functions at module level
pub use client_ffi::{
    client_cancel_task, client_create_task, client_get_task, client_health_check,
    client_list_task_steps, client_list_tasks,
};
