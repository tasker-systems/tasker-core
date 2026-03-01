//! SRE / Platform Engineer persona: system health and monitoring workflows.
//!
//! No fixture seeding needed — queries system state directly.

use super::harness::McpTestHarness;

#[tokio::test]
async fn test_connection_and_health() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // connection_status should show the test profile
    let status = harness
        .call_tool("connection_status", serde_json::json!({}))
        .await?;
    assert_eq!(status["mode"], "connected");
    assert!(status["profiles"].is_array());
    let profiles = status["profiles"].as_array().unwrap();
    assert!(
        !profiles.is_empty(),
        "Should have at least one profile configured"
    );

    // system_health should return component status
    let health = harness
        .call_tool("system_health", serde_json::json!({}))
        .await?;
    assert!(
        health.get("error").is_none(),
        "system_health should succeed: {:?}",
        health
    );

    harness.teardown().await?;
    Ok(())
}

#[tokio::test]
async fn test_system_config() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    let config = harness
        .call_tool("system_config", serde_json::json!({}))
        .await?;
    assert!(
        config.get("error").is_none(),
        "system_config should succeed: {:?}",
        config
    );

    harness.teardown().await?;
    Ok(())
}

#[tokio::test]
async fn test_staleness_monitoring() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    let staleness = harness
        .call_tool("staleness_check", serde_json::json!({}))
        .await?;
    assert!(
        staleness.get("error").is_none(),
        "staleness_check should succeed: {:?}",
        staleness
    );

    harness.teardown().await?;
    Ok(())
}

#[tokio::test]
async fn test_analytics_overview() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // analytics_performance should return metrics
    let perf = harness
        .call_tool("analytics_performance", serde_json::json!({}))
        .await?;
    assert!(
        perf.get("error").is_none(),
        "analytics_performance should succeed: {:?}",
        perf
    );

    // analytics_bottlenecks should return analysis
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
