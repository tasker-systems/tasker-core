//! Command handlers for the Tasker CLI
//!
//! Each module handles a specific command group, delegating to `tasker-client` for API operations.

pub(crate) mod auth;
pub(crate) mod config;
pub(crate) mod dlq;
pub(crate) mod docs;
pub(crate) mod system;
pub(crate) mod task;
pub(crate) mod worker;

pub(crate) use auth::handle_auth_command;
pub(crate) use config::handle_config_command;
pub(crate) use dlq::handle_dlq_command;
pub(crate) use docs::handle_docs_command;
pub(crate) use system::handle_system_command;
pub(crate) use task::handle_task_command;
pub(crate) use worker::handle_worker_command;
