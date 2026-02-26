//! MCP ServerHandler implementation for Tasker.
//!
//! Provides the core MCP server that responds to the `initialize` handshake
//! with Tasker server info. Tool registration will be added in TAS-305.

use rmcp::handler::server::ServerHandler;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};

/// Tasker MCP server handler.
///
/// Currently a scaffold that returns server info on initialize.
/// Tool implementations will be added in the follow-up ticket (TAS-305).
#[derive(Debug)]
pub struct TaskerMcpServer;

impl TaskerMcpServer {
    pub fn new() -> Self {
        Self
    }
}

impl ServerHandler for TaskerMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities::default(),
            server_info: Implementation {
                name: "tasker-mcp".to_string(),
                title: Some("Tasker MCP Server".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some(
                    "MCP server exposing Tasker developer tooling: code generation, \
                     template parsing, and schema inspection"
                        .to_string(),
                ),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Tasker MCP server provides developer tooling for the Tasker \
                 workflow orchestration system. Tools for code generation, template \
                 management, and schema inspection will be available in future versions."
                    .to_string(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_info() {
        let server = TaskerMcpServer::new();
        let info = server.get_info();

        assert_eq!(info.server_info.name, "tasker-mcp");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.instructions.is_some());
    }

    #[test]
    fn test_server_uses_tasker_tooling() {
        // Verify that tasker-tooling is consumable from this crate
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = tasker_tooling::template_parser::parse_template_str(yaml).unwrap();
        assert_eq!(template.name, "codegen_test");

        // Verify schema inspection works
        let report = tasker_tooling::schema_inspector::inspect(&template);
        assert!(!report.steps.is_empty());
    }
}
