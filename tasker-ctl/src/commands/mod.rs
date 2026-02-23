//! Command handlers for the Tasker CLI
//!
//! Each module handles a specific command group, delegating to `tasker-client` for API operations.

pub(crate) mod auth;
pub(crate) mod config;
pub(crate) mod dlq;
pub(crate) mod docs;
pub(crate) mod generate;
pub(crate) mod init;
pub(crate) mod plugin;
pub(crate) mod remote;
pub(crate) mod system;
pub(crate) mod task;
pub(crate) mod template;
pub(crate) mod worker;

pub(crate) use auth::handle_auth_command;
pub(crate) use config::handle_config_command;
pub(crate) use dlq::handle_dlq_command;
pub(crate) use docs::handle_docs_command;
pub(crate) use generate::handle_generate_command;
pub(crate) use init::handle_init_command;
pub(crate) use plugin::handle_plugin_command;
pub(crate) use remote::handle_remote_command;
pub(crate) use system::handle_system_command;
pub(crate) use task::handle_task_command;
pub(crate) use template::handle_template_command;
pub(crate) use worker::handle_worker_command;
