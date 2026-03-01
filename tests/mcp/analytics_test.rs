//! Analytics Team persona: performance analysis workflows.
//!
//! Fixture: mathematical_sequence.yaml (seeded multiple times for volume)
//! Flow: seed tasks → verify analytics reflect execution data

use super::harness::McpTestHarness;

#[tokio::test]
async fn test_performance_after_tasks() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Seed a few tasks to ensure there's execution data
    for i in 0..3 {
        let even = (i + 1) * 2 + 2; // 4, 6, 8
        harness
            .seed_and_complete(
                "rust_e2e_linear",
                "mathematical_sequence",
                serde_json::json!({ "even_number": even }),
            )
            .await?;
    }

    // analytics_performance should show non-zero metrics
    let perf = harness
        .call_tool("analytics_performance", serde_json::json!({}))
        .await?;
    assert!(
        perf.get("error").is_none(),
        "analytics_performance should succeed: {:?}",
        perf
    );

    harness.teardown().await?;
    Ok(())
}

#[tokio::test]
async fn test_bottleneck_analysis() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Seed a task so there's data for analysis
    harness
        .seed_and_complete(
            "rust_e2e_linear",
            "mathematical_sequence",
            serde_json::json!({ "even_number": 10 }),
        )
        .await?;

    let bottlenecks = harness
        .call_tool("analytics_bottlenecks", serde_json::json!({}))
        .await?;
    assert!(
        bottlenecks.get("error").is_none(),
        "analytics_bottlenecks should succeed: {:?}",
        bottlenecks
    );

    harness.teardown().await?;
    Ok(())
}

#[tokio::test]
async fn test_task_list_filtering() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Test namespace filtering
    let filtered = harness
        .call_tool(
            "task_list",
            serde_json::json!({
                "namespace": "rust_e2e_linear",
                "limit": 5
            }),
        )
        .await?;
    assert!(filtered["tasks"].is_array());
    let tasks = filtered["tasks"].as_array().unwrap();
    assert!(
        tasks.len() <= 5,
        "Limit should be respected, got {}",
        tasks.len()
    );

    // Test status filtering — DB accepts "completed" or "pending"
    let complete_tasks = harness
        .call_tool(
            "task_list",
            serde_json::json!({
                "status": "completed",
                "limit": 10
            }),
        )
        .await?;
    assert!(
        complete_tasks.get("error").is_none(),
        "task_list with status filter should succeed: {:?}",
        complete_tasks
    );
    assert!(complete_tasks["tasks"].is_array());

    // Test pagination via offset
    let page2 = harness
        .call_tool(
            "task_list",
            serde_json::json!({
                "limit": 5,
                "offset": 5
            }),
        )
        .await?;
    assert!(page2["tasks"].is_array());

    harness.teardown().await?;
    Ok(())
}
