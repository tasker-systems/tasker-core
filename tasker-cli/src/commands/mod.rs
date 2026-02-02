//! Command handlers for the Tasker CLI
//!
//! Each module handles a specific command group, delegating to `tasker-client` for API operations.

pub mod auth;
pub mod config;
pub mod dlq;
pub mod docs;
pub mod system;
pub mod task;
pub mod worker;

pub use auth::handle_auth_command;
pub use config::handle_config_command;
pub use dlq::handle_dlq_command;
pub use docs::handle_docs_command;
pub use system::handle_system_command;
pub use task::handle_task_command;
pub use worker::handle_worker_command;
