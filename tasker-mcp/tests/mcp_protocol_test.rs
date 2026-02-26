//! MCP protocol integration test.
//!
//! Verifies that the server correctly handles the MCP protocol round-trip:
//! tool discovery via `list_tools` and tool invocation via `call_tool`.

use rmcp::model::{CallToolRequestParams, ClientInfo};
use rmcp::{ClientHandler, ServiceExt};

// Re-create a minimal server for protocol testing since the binary crate
// doesn't export a library target.
mod test_server {
    use rmcp::handler::server::router::tool::ToolRouter;
    use rmcp::handler::server::wrapper::Parameters;
    use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
    use rmcp::{tool, tool_handler, tool_router, ServerHandler};
    use schemars::JsonSchema;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, JsonSchema)]
    pub struct ValidateParams {
        #[schemars(description = "YAML content")]
        pub template_yaml: String,
    }

    #[derive(Debug, Clone)]
    pub struct TaskerMcpServer {
        tool_router: ToolRouter<Self>,
    }

    impl TaskerMcpServer {
        pub fn new() -> Self {
            Self {
                tool_router: Self::tool_router(),
            }
        }
    }

    #[tool_handler(router = self.tool_router)]
    impl ServerHandler for TaskerMcpServer {
        fn get_info(&self) -> ServerInfo {
            ServerInfo {
                protocol_version: ProtocolVersion::V_2025_03_26,
                capabilities: ServerCapabilities::builder().enable_tools().build(),
                server_info: Implementation {
                    name: "tasker-mcp".to_string(),
                    title: Some("Tasker MCP Server".to_string()),
                    version: "test".to_string(),
                    description: None,
                    icons: None,
                    website_url: None,
                },
                instructions: None,
            }
        }
    }

    #[tool_router(router = tool_router)]
    impl TaskerMcpServer {
        #[tool(
            name = "template_validate",
            description = "Validate a task template YAML"
        )]
        pub async fn template_validate(
            &self,
            Parameters(params): Parameters<ValidateParams>,
        ) -> String {
            match tasker_tooling::template_parser::parse_template_str(&params.template_yaml) {
                Ok(template) => {
                    let report = tasker_tooling::template_validator::validate(&template);
                    serde_json::to_string(&report).unwrap()
                }
                Err(e) => format!(r#"{{"error":"yaml_parse_error","message":"{}"}}"#, e),
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct DummyClient;

impl ClientHandler for DummyClient {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

#[tokio::test]
async fn test_mcp_protocol_list_tools() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = test_server::TaskerMcpServer::new();
    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await?;
        service.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClient.serve(client_transport).await?;

    let tools = client.list_tools(None).await?;
    let tool_names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(
        tool_names.contains(&"template_validate"),
        "Expected template_validate in tool list, got: {:?}",
        tool_names
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_mcp_protocol_call_tool() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = test_server::TaskerMcpServer::new();
    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await?;
        service.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClient.serve(client_transport).await?;

    let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
    let result = client
        .call_tool(CallToolRequestParams {
            meta: None,
            name: "template_validate".into(),
            arguments: Some(
                serde_json::json!({ "template_yaml": yaml })
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
            task: None,
        })
        .await?;

    let text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.as_str())
        .expect("Expected text content");

    let parsed: serde_json::Value = serde_json::from_str(text)?;
    assert_eq!(parsed["valid"], true);
    assert_eq!(parsed["step_count"], 5);

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
