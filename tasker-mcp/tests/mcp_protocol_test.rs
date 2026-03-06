//! MCP protocol integration tests.
//!
//! Uses the real `TaskerMcpServer` from the library target to verify protocol
//! round-trips: tool discovery via `list_tools` and tool invocation via `call_tool`
//! for all 29 tools (8 Tier 1/profile + 15 Tier 2 connected + 6 Tier 3 write).

use rmcp::model::{CallToolRequestParams, ClientInfo};
use rmcp::service::{RoleClient, RunningService};
use rmcp::{ClientHandler, ServiceExt};
use tasker_client::ProfileManager;
use tasker_mcp::server::TaskerMcpServer;
use tasker_mcp::tier::EnabledTiers;

#[derive(Debug, Clone, Default)]
struct TestClient;

impl ClientHandler for TestClient {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

/// Helper: spin up an offline server/client pair (Tier 1 tools only).
async fn setup_offline() -> anyhow::Result<(
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

/// Helper: spin up a connected server with all tools registered but no reachable backend.
/// This allows testing Tier 2/3 tool error paths (connection_failed, offline_mode errors)
/// without tools being pruned from the router.
async fn setup_connected_no_server() -> anyhow::Result<(
    RunningService<RoleClient, TestClient>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
)> {
    let (server_transport, client_transport) = tokio::io::duplex(65536);

    let pm = ProfileManager::offline(); // No profiles, but not offline mode
    let server = TaskerMcpServer::with_profile_manager(pm, false, None);
    let server_handle = tokio::spawn(async move {
        let service = server.serve(server_transport).await?;
        service.waiting().await?;
        anyhow::Ok(())
    });

    let client = TestClient.serve(client_transport).await?;
    Ok((client, server_handle))
}

/// Helper: spin up a server with specific tiers enabled.
async fn setup_with_tiers(
    tiers: EnabledTiers,
) -> anyhow::Result<(
    RunningService<RoleClient, TestClient>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
)> {
    let (server_transport, client_transport) = tokio::io::duplex(65536);

    let pm = ProfileManager::offline();
    let server = TaskerMcpServer::with_profile_manager(pm, false, Some(tiers));
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
async fn test_list_tools_offline_returns_tier1() -> anyhow::Result<()> {
    let (client, server_handle) = setup_offline().await?;

    let tools = client.list_tools(None).await?;
    let mut names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();
    names.sort();

    assert_eq!(
        names,
        vec![
            "handler_generate",
            "schema_compare",
            "schema_diff",
            "schema_inspect",
            "template_generate",
            "template_inspect",
            "template_validate",
            "template_visualize",
        ]
    );
    assert_eq!(names.len(), 8, "Expected 8 Tier 1 tools in offline mode");

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_list_tools_connected_returns_all() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let tools = client.list_tools(None).await?;
    let mut names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();
    names.sort();

    assert_eq!(
        names,
        vec![
            "analytics_bottlenecks",
            "analytics_performance",
            "connection_status",
            "dlq_inspect",
            "dlq_list",
            "dlq_queue",
            "dlq_stats",
            "dlq_update",
            "handler_generate",
            "schema_compare",
            "schema_diff",
            "schema_inspect",
            "staleness_check",
            "step_audit",
            "step_complete",
            "step_inspect",
            "step_resolve",
            "step_retry",
            "system_config",
            "system_health",
            "task_cancel",
            "task_inspect",
            "task_list",
            "task_submit",
            "template_generate",
            "template_inspect",
            "template_inspect_remote",
            "template_list_remote",
            "template_validate",
            "template_visualize",
        ]
    );
    assert_eq!(
        names.len(),
        30,
        "Expected 30 tools: 8 Tier 1 + 1 profile + 15 Tier 2 connected + 6 Tier 3 write"
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_list_tools_tier_filtered() -> anyhow::Result<()> {
    let tiers = EnabledTiers::from_tier_strings(&["tier1".to_string(), "tier2".to_string()]);
    let (client, server_handle) = setup_with_tiers(tiers).await?;

    let tools = client.list_tools(None).await?;
    let names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();

    // 8 T1 + 1 connection_status + 15 T2 = 24
    assert_eq!(names.len(), 24, "Expected 24 tools (T1+profile+T2)");
    assert!(names.contains(&"template_validate"));
    assert!(names.contains(&"task_list"));
    assert!(names.contains(&"connection_status"));
    assert!(!names.contains(&"task_submit"));
    assert!(!names.contains(&"dlq_update"));

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

// ── template_validate ──

#[tokio::test]
async fn test_template_validate_valid() -> anyhow::Result<()> {
    let (client, server_handle) = setup_offline().await?;

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
    let (client, server_handle) = setup_offline().await?;

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
    let (client, server_handle) = setup_offline().await?;

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
    let (client, server_handle) = setup_offline().await?;

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
    let (client, server_handle) = setup_offline().await?;

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
    let (client, server_handle) = setup_offline().await?;

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
    let (client, server_handle) = setup_offline().await?;

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
    let (client, server_handle) = setup_offline().await?;

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

// ── schema_diff ──

#[tokio::test]
async fn test_schema_diff() -> anyhow::Result<()> {
    let (client, server_handle) = setup_offline().await?;

    let before_yaml = r#"
name: diff_test
namespace_name: test
version: "1.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
    result_schema:
      type: object
      required: [id, name]
      properties:
        id:
          type: string
        name:
          type: string
"#;
    let after_yaml = r#"
name: diff_test
namespace_name: test
version: "2.0.0"
steps:
  - name: step_a
    handler:
      callable: test.step_a
    result_schema:
      type: object
      required: [id]
      properties:
        id:
          type: string
        email:
          type: string
"#;

    let text = call_tool_text(
        &client,
        "schema_diff",
        serde_json::json!({
            "before_yaml": before_yaml,
            "after_yaml": after_yaml
        }),
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(parsed["compatibility"], "incompatible");
    let diffs = parsed["step_diffs"].as_array().unwrap();
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0]["step_name"], "step_a");
    assert_eq!(diffs[0]["status"], "modified");

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_schema_compare_step_not_found() -> anyhow::Result<()> {
    let (client, server_handle) = setup_offline().await?;

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

// ── Tier 2: Offline mode error tests ──
// These use setup_connected_no_server() so all tools are registered in the router.
// The tools return profile_not_found errors since the ProfileManager has no profiles.

#[tokio::test]
async fn test_task_list_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(&client, "task_list", serde_json::json!({})).await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    // With empty ProfileManager (not offline mode), this returns profile_not_found
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_task_inspect_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(
        &client,
        "task_inspect",
        serde_json::json!({ "task_uuid": "00000000-0000-0000-0000-000000000000" }),
    )
    .await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_dlq_list_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(&client, "dlq_list", serde_json::json!({})).await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_system_health_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(&client, "system_health", serde_json::json!({})).await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_analytics_performance_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(&client, "analytics_performance", serde_json::json!({})).await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_template_list_remote_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(&client, "template_list_remote", serde_json::json!({})).await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

// ── Tier 3: Offline mode error tests ──

#[tokio::test]
async fn test_task_submit_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(
        &client,
        "task_submit",
        serde_json::json!({
            "name": "test",
            "namespace": "default",
            "context": {},
            "confirm": true
        }),
    )
    .await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_task_cancel_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(
        &client,
        "task_cancel",
        serde_json::json!({ "task_uuid": "00000000-0000-0000-0000-000000000000" }),
    )
    .await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_step_retry_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(
        &client,
        "step_retry",
        serde_json::json!({
            "task_uuid": "00000000-0000-0000-0000-000000000000",
            "step_uuid": "00000000-0000-0000-0000-000000000000",
            "reason": "test",
            "reset_by": "test"
        }),
    )
    .await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_step_resolve_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(
        &client,
        "step_resolve",
        serde_json::json!({
            "task_uuid": "00000000-0000-0000-0000-000000000000",
            "step_uuid": "00000000-0000-0000-0000-000000000000",
            "reason": "test",
            "resolved_by": "test"
        }),
    )
    .await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_step_complete_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(
        &client,
        "step_complete",
        serde_json::json!({
            "task_uuid": "00000000-0000-0000-0000-000000000000",
            "step_uuid": "00000000-0000-0000-0000-000000000000",
            "result": {"value": 42},
            "reason": "test",
            "completed_by": "test"
        }),
    )
    .await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_dlq_update_offline() -> anyhow::Result<()> {
    let (client, server_handle) = setup_connected_no_server().await?;

    let text = call_tool_text(
        &client,
        "dlq_update",
        serde_json::json!({ "dlq_entry_uuid": "00000000-0000-0000-0000-000000000000" }),
    )
    .await?;
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["error"] == "profile_not_found" || parsed["error"] == "connection_failed",
        "Expected profile_not_found or connection_failed, got: {}",
        parsed["error"]
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

// ── Pruned tool returns protocol error ──

#[tokio::test]
async fn test_pruned_tool_returns_not_found() -> anyhow::Result<()> {
    // Offline mode prunes Tier 2/3 tools
    let (client, _server_handle) = setup_offline().await?;

    // Calling a pruned tool should return an MCP protocol error (not our JSON error)
    let result = client
        .call_tool(CallToolRequestParams {
            meta: None,
            name: "task_list".into(),
            arguments: Some(serde_json::Map::new()),
            task: None,
        })
        .await;

    // rmcp returns an error when calling an unknown tool
    assert!(result.is_err(), "Expected protocol error for pruned tool");

    client.cancel().await?;
    _server_handle.await??;
    Ok(())
}
