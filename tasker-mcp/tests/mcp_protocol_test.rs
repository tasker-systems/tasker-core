//! MCP protocol integration tests.
//!
//! Uses the real `TaskerMcpServer` from the library target to verify protocol
//! round-trips: tool discovery via `list_tools` and tool invocation via `call_tool`
//! for all 6 tools.

use rmcp::model::{CallToolRequestParams, ClientInfo};
use rmcp::service::{RoleClient, RunningService};
use rmcp::{ClientHandler, ServiceExt};
use tasker_mcp::server::TaskerMcpServer;

#[derive(Debug, Clone, Default)]
struct TestClient;

impl ClientHandler for TestClient {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

/// Helper: spin up a server/client pair connected via in-memory duplex.
async fn setup() -> anyhow::Result<(
    RunningService<RoleClient, TestClient>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
)> {
    let (server_transport, client_transport) = tokio::io::duplex(65536);

    let server = TaskerMcpServer::new();
    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await?;
        service.waiting().await?;
        anyhow::Ok(())
    });

    let client = TestClient.serve(client_transport).await?;
    Ok((client, server_handle))
}

/// Helper: call a tool and extract the text content from the response.
async fn call_tool_text(
    client: &RunningService<RoleClient, TestClient>,
    name: impl Into<std::borrow::Cow<'static, str>>,
    args: serde_json::Value,
) -> anyhow::Result<String> {
    let result = client
        .call_tool(CallToolRequestParams {
            meta: None,
            name: name.into(),
            arguments: Some(args.as_object().unwrap().clone()),
            task: None,
        })
        .await?;

    let text = result
        .content
        .first()
        .and_then(|c| c.raw.as_text())
        .map(|t| t.text.clone())
        .expect("Expected text content in tool response");

    Ok(text)
}

fn codegen_yaml() -> &'static str {
    include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml")
}

// ── Discovery ──

#[tokio::test]
async fn test_list_tools_returns_all_six() -> anyhow::Result<()> {
    let (client, server_handle) = setup().await?;

    let tools = client.list_tools(None).await?;
    let mut names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();
    names.sort();

    assert_eq!(
        names,
        vec![
            "handler_generate",
            "schema_compare",
            "schema_inspect",
            "template_generate",
            "template_inspect",
            "template_validate",
        ]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

// ── template_validate ──

#[tokio::test]
async fn test_template_validate_valid() -> anyhow::Result<()> {
    let (client, server_handle) = setup().await?;

    let text = call_tool_text(
        &client,
        "template_validate",
        serde_json::json!({ "template_yaml": codegen_yaml() }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(parsed["valid"], true);
    assert_eq!(parsed["step_count"], 5);

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_template_validate_invalid_yaml() -> anyhow::Result<()> {
    let (client, server_handle) = setup().await?;

    let text = call_tool_text(
        &client,
        "template_validate",
        serde_json::json!({ "template_yaml": "not: [valid: yaml" }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(parsed["error"], "yaml_parse_error");

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

// ── template_inspect ──

#[tokio::test]
async fn test_template_inspect() -> anyhow::Result<()> {
    let (client, server_handle) = setup().await?;

    let text = call_tool_text(
        &client,
        "template_inspect",
        serde_json::json!({ "template_yaml": codegen_yaml() }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(parsed["name"], "codegen_test");
    assert_eq!(parsed["step_count"], 5);
    assert_eq!(parsed["execution_order"].as_array().unwrap().len(), 5);
    assert!(parsed["root_steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "validate_order"));

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

// ── template_generate ──

#[tokio::test]
async fn test_template_generate() -> anyhow::Result<()> {
    let (client, server_handle) = setup().await?;

    let text = call_tool_text(
        &client,
        "template_generate",
        serde_json::json!({
            "name": "test_task",
            "namespace": "ns",
            "description": "A test task",
            "steps": [{
                "name": "step_one",
                "depends_on": [],
                "outputs": [{
                    "name": "result",
                    "field_type": "string",
                    "required": true
                }]
            }]
        }),
    )
    .await?;

    assert!(text.contains("test_task"));
    assert!(text.contains("ns.step_one"));

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

// ── handler_generate ──

#[tokio::test]
async fn test_handler_generate_scaffold() -> anyhow::Result<()> {
    let (client, server_handle) = setup().await?;

    let text = call_tool_text(
        &client,
        "handler_generate",
        serde_json::json!({
            "template_yaml": codegen_yaml(),
            "language": "python",
            "step_filter": "validate_order",
            "scaffold": true
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(parsed["language"], "python");
    assert!(parsed["types"].as_str().unwrap().contains("class"));
    assert!(parsed["handlers"].as_str().unwrap().contains("def"));
    assert!(parsed["handlers"]
        .as_str()
        .unwrap()
        .contains("from .models import"));

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_handler_generate_invalid_language() -> anyhow::Result<()> {
    let (client, server_handle) = setup().await?;

    let text = call_tool_text(
        &client,
        "handler_generate",
        serde_json::json!({
            "template_yaml": codegen_yaml(),
            "language": "cobol"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(parsed["error"], "invalid_language");

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

// ── schema_inspect ──

#[tokio::test]
async fn test_schema_inspect() -> anyhow::Result<()> {
    let (client, server_handle) = setup().await?;

    let text = call_tool_text(
        &client,
        "schema_inspect",
        serde_json::json!({
            "template_yaml": codegen_yaml(),
            "step_filter": "validate_order"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(parsed["template_name"], "codegen_test");
    let steps = parsed["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1);
    assert!(steps[0]["has_result_schema"].as_bool().unwrap());
    assert!(!steps[0]["fields"].as_array().unwrap().is_empty());

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

// ── schema_compare ──

#[tokio::test]
async fn test_schema_compare() -> anyhow::Result<()> {
    let (client, server_handle) = setup().await?;

    let text = call_tool_text(
        &client,
        "schema_compare",
        serde_json::json!({
            "template_yaml": codegen_yaml(),
            "producer_step": "validate_order",
            "consumer_step": "enrich_order"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(parsed["compatibility"].is_string());
    assert!(parsed["findings"].is_array());

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_schema_compare_step_not_found() -> anyhow::Result<()> {
    let (client, server_handle) = setup().await?;

    let text = call_tool_text(
        &client,
        "schema_compare",
        serde_json::json!({
            "template_yaml": codegen_yaml(),
            "producer_step": "nonexistent",
            "consumer_step": "enrich_order"
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(parsed["error"], "step_not_found");

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
