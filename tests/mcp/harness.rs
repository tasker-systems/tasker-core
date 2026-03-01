//! Test harness for connected MCP integration tests.
//!
//! Combines `IntegrationTestManager` (service health, API clients) with an MCP
//! server/client pair via in-memory duplex transport. Task seeding uses the shared
//! `create_task_request` + `wait_for_task_completion` utilities.

#![allow(dead_code)]

use anyhow::Result;
use rmcp::model::{CallToolRequestParams, ClientInfo};
use rmcp::service::{RoleClient, RunningService};
use rmcp::{ClientHandler, ServiceExt};
use serde_json::Value;
use tokio::task::JoinHandle;

use tasker_client::ProfileManager;
use tasker_mcp::server::TaskerMcpServer;

use crate::common::integration_test_manager::IntegrationTestManager;
use crate::common::integration_test_utils::{
    create_task_request, wait_for_task_completion, wait_for_task_failure,
};

#[derive(Debug, Clone, Default)]
pub(super) struct TestClient;

impl ClientHandler for TestClient {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

/// Connected MCP test harness.
///
/// Wraps an MCP server/client pair (duplex transport) backed by the shared
/// `IntegrationTestManager` for service validation and direct API access.
pub struct McpTestHarness {
    pub mcp_client: RunningService<RoleClient, TestClient>,
    pub manager: IntegrationTestManager,
    server_handle: JoinHandle<Result<()>>,
}

impl McpTestHarness {
    /// Set up: validate services via IntegrationTestManager, then spin up an MCP
    /// server connected to the same orchestration endpoint via ProfileManager.
    pub async fn setup() -> Result<Self> {
        let manager = IntegrationTestManager::setup().await?;

        // Build a ProfileManager with a single "test" profile pointing at the
        // same orchestration server the IntegrationTestManager validated.
        let toml_content = format!(
            r#"
[profile.test]
description = "Connected integration test"
transport = "rest"

[profile.test.orchestration]
base_url = "{}"

[profile.test.worker]
base_url = "{}"
"#,
            manager.orchestration_url,
            manager
                .worker_url
                .as_deref()
                .unwrap_or("http://localhost:8081")
        );
        let profile_file: tasker_client::config::ProfileConfigFile = toml::from_str(&toml_content)?;
        let pm = ProfileManager::from_profile_file_for_test(profile_file);

        // Create MCP server in connected mode
        let server = TaskerMcpServer::with_profile_manager(pm, false);
        let (server_transport, client_transport) = tokio::io::duplex(65536);

        let server_handle = tokio::spawn(async move {
            let service = server.serve(server_transport).await?;
            service.waiting().await?;
            anyhow::Ok(())
        });

        let mcp_client = TestClient.serve(client_transport).await?;

        Ok(Self {
            mcp_client,
            manager,
            server_handle,
        })
    }

    /// Call an MCP tool and parse the text response as JSON.
    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        let result = self
            .mcp_client
            .call_tool(CallToolRequestParams {
                meta: None,
                name: name.to_string().into(),
                arguments: Some(args.as_object().unwrap().clone()),
                task: None,
            })
            .await?;

        let text = result
            .content
            .first()
            .and_then(|c| c.raw.as_text())
            .map(|t| t.text.clone())
            .ok_or_else(|| anyhow::anyhow!("No text content in tool response"))?;

        let parsed: Value = serde_json::from_str(&text)?;
        Ok(parsed)
    }

    /// Seed a task via the direct API client, wait for completion, return UUID.
    pub async fn seed_and_complete(
        &self,
        namespace: &str,
        name: &str,
        context: Value,
    ) -> Result<String> {
        let request = create_task_request(namespace, name, context);
        let response = self
            .manager
            .orchestration_client
            .create_task(request)
            .await?;
        let task_uuid = response.task_uuid.clone();

        wait_for_task_completion(&self.manager.orchestration_client, &task_uuid, 15).await?;
        Ok(task_uuid)
    }

    /// Seed a task via the direct API client, wait for failure, return UUID.
    pub async fn seed_and_fail(
        &self,
        namespace: &str,
        name: &str,
        context: Value,
    ) -> Result<String> {
        let request = create_task_request(namespace, name, context);
        let response = self
            .manager
            .orchestration_client
            .create_task(request)
            .await?;
        let task_uuid = response.task_uuid.clone();

        wait_for_task_failure(&self.manager.orchestration_client, &task_uuid, 30).await?;
        Ok(task_uuid)
    }

    /// Tear down: cancel MCP client and join server handle.
    pub async fn teardown(self) -> Result<()> {
        self.mcp_client.cancel().await?;
        self.server_handle.await??;
        Ok(())
    }
}
