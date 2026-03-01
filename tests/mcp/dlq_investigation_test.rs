//! Technical Ops persona: DLQ investigation workflows.
//!
//! Fixture: retry_exhaustion_test.yaml (always-fail handler, forces DLQ entry)
//! Flow: seed failing task → wait for failure → exercise DLQ tools

use super::harness::McpTestHarness;

#[tokio::test]
async fn test_dlq_stats_and_queue() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Seed a task that will fail and eventually generate DLQ entries
    harness
        .seed_and_fail(
            "rust_e2e_retry",
            "retry_exhaustion_test",
            serde_json::json!({ "test_id": "dlq_stats_test" }),
        )
        .await?;

    // dlq_stats should return aggregated statistics
    let stats = harness
        .call_tool("dlq_stats", serde_json::json!({}))
        .await?;
    assert!(
        stats.get("error").is_none(),
        "dlq_stats should succeed: {:?}",
        stats
    );

    // dlq_queue should return prioritized investigation queue
    let queue = harness
        .call_tool("dlq_queue", serde_json::json!({}))
        .await?;
    assert!(
        queue.get("error").is_none(),
        "dlq_queue should succeed: {:?}",
        queue
    );

    harness.teardown().await?;
    Ok(())
}

#[tokio::test]
async fn test_dlq_list_and_inspect() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Seed a failing task to generate DLQ entries
    harness
        .seed_and_fail(
            "rust_e2e_retry",
            "retry_exhaustion_test",
            serde_json::json!({ "test_id": "dlq_list_test" }),
        )
        .await?;

    // dlq_list should return entries
    let list = harness.call_tool("dlq_list", serde_json::json!({})).await?;
    assert!(
        list.get("error").is_none(),
        "dlq_list should succeed: {:?}",
        list
    );

    // If entries are available, inspect the first one
    if let Some(entries) = list
        .get("entries")
        .and_then(|e| e.as_array())
        .filter(|a| !a.is_empty())
    {
        let entry_id = entries[0]
            .get("id")
            .or_else(|| entries[0].get("entry_id"))
            .and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            });

        if let Some(id) = entry_id {
            let detail = harness
                .call_tool("dlq_inspect", serde_json::json!({ "entry_id": id }))
                .await?;
            assert!(
                detail.get("error").is_none(),
                "dlq_inspect should succeed: {:?}",
                detail
            );
        }
    }

    harness.teardown().await?;
    Ok(())
}
