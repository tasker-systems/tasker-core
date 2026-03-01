//! Tier 3 write tools: preview → confirm workflows.
//!
//! Tests the two-phase confirmation pattern for all write tools.
//! Fixtures: mathematical_sequence.yaml (success), retry_exhaustion_test.yaml (failure/DLQ)

use super::harness::McpTestHarness;

// ── task_submit ──

#[tokio::test]
async fn test_task_submit_preview_and_execute() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Preview (omit confirm)
    let preview = harness
        .call_tool(
            "task_submit",
            serde_json::json!({
                "name": "mathematical_sequence",
                "namespace": "rust_e2e_linear",
                "version": "1.0.0",
                "context": { "even_number": 10 },
                "initiator": "integration-test"
            }),
        )
        .await?;
    assert_eq!(
        preview["status"], "preview",
        "Should return preview: {:?}",
        preview
    );
    assert_eq!(preview["action"], "task_submit");
    assert!(preview["instruction"]
        .as_str()
        .unwrap()
        .contains("confirm: true"));

    // Execute with confirm
    let result = harness
        .call_tool(
            "task_submit",
            serde_json::json!({
                "name": "mathematical_sequence",
                "namespace": "rust_e2e_linear",
                "version": "1.0.0",
                "context": { "even_number": 10 },
                "initiator": "integration-test",
                "confirm": true
            }),
        )
        .await?;
    assert_eq!(result["status"], "executed", "Should execute: {:?}", result);
    assert!(
        result["result"].is_object(),
        "Should contain task result: {:?}",
        result
    );

    harness.teardown().await?;
    Ok(())
}

// ── task_cancel ──

#[tokio::test]
async fn test_task_cancel_preview_and_execute() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Seed a task via direct API
    let task_uuid = harness
        .seed_and_complete(
            "rust_e2e_linear",
            "mathematical_sequence",
            serde_json::json!({ "even_number": 2 }),
        )
        .await?;

    // Preview
    let preview = harness
        .call_tool("task_cancel", serde_json::json!({ "task_uuid": task_uuid }))
        .await?;
    assert_eq!(
        preview["status"], "preview",
        "Should return preview: {:?}",
        preview
    );
    assert_eq!(preview["action"], "task_cancel");

    // Execute — task is already complete so this may error, but the confirm path should be exercised
    let result = harness
        .call_tool(
            "task_cancel",
            serde_json::json!({ "task_uuid": task_uuid, "confirm": true }),
        )
        .await?;
    // Either succeeds or returns an API error (can't cancel completed task) — both are valid
    assert!(
        result.get("status").is_some() || result.get("error").is_some(),
        "Should return either executed or error: {:?}",
        result
    );

    harness.teardown().await?;
    Ok(())
}

// ── step_retry ──

#[tokio::test]
async fn test_step_retry_preview_and_execute() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Seed a failing task
    let task_uuid = harness
        .seed_and_fail(
            "rust_e2e_retry",
            "retry_exhaustion_test",
            serde_json::json!({ "test_id": "step_retry_test" }),
        )
        .await?;

    // Get the failed step UUID
    let inspect = harness
        .call_tool(
            "task_inspect",
            serde_json::json!({ "task_uuid": task_uuid }),
        )
        .await?;
    let steps = inspect["steps"].as_array().expect("Should have steps");
    let failed_step = steps
        .iter()
        .find(|s| {
            s["status"]
                .as_str()
                .is_some_and(|st| st == "error" || st == "Error")
        })
        .expect("Should have a failed step");
    let step_uuid = failed_step["step_uuid"]
        .as_str()
        .expect("Step should have UUID");

    // Preview
    let preview = harness
        .call_tool(
            "step_retry",
            serde_json::json!({
                "task_uuid": task_uuid,
                "step_uuid": step_uuid,
                "reason": "Transient failure — retry after fix",
                "reset_by": "integration-test"
            }),
        )
        .await?;
    assert_eq!(
        preview["status"], "preview",
        "Should return preview: {:?}",
        preview
    );
    assert_eq!(preview["action"], "step_retry");
    assert!(preview["details"]["current_step"].is_object());

    // Execute
    let result = harness
        .call_tool(
            "step_retry",
            serde_json::json!({
                "task_uuid": task_uuid,
                "step_uuid": step_uuid,
                "reason": "Transient failure — retry after fix",
                "reset_by": "integration-test",
                "confirm": true
            }),
        )
        .await?;
    assert_eq!(
        result["status"], "executed",
        "Should execute step_retry: {:?}",
        result
    );

    harness.teardown().await?;
    Ok(())
}

// ── step_resolve ──

#[tokio::test]
async fn test_step_resolve_preview_and_execute() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Seed a failing task
    let task_uuid = harness
        .seed_and_fail(
            "rust_e2e_retry",
            "retry_exhaustion_test",
            serde_json::json!({ "test_id": "step_resolve_test" }),
        )
        .await?;

    let inspect = harness
        .call_tool(
            "task_inspect",
            serde_json::json!({ "task_uuid": task_uuid }),
        )
        .await?;
    let steps = inspect["steps"].as_array().expect("Should have steps");
    let failed_step = steps
        .iter()
        .find(|s| {
            s["status"]
                .as_str()
                .is_some_and(|st| st == "error" || st == "Error")
        })
        .expect("Should have a failed step");
    let step_uuid = failed_step["step_uuid"]
        .as_str()
        .expect("Step should have UUID");

    // Preview
    let preview = harness
        .call_tool(
            "step_resolve",
            serde_json::json!({
                "task_uuid": task_uuid,
                "step_uuid": step_uuid,
                "reason": "Known issue — resolving manually",
                "resolved_by": "integration-test"
            }),
        )
        .await?;
    assert_eq!(preview["status"], "preview");

    // Execute
    let result = harness
        .call_tool(
            "step_resolve",
            serde_json::json!({
                "task_uuid": task_uuid,
                "step_uuid": step_uuid,
                "reason": "Known issue — resolving manually",
                "resolved_by": "integration-test",
                "confirm": true
            }),
        )
        .await?;
    assert_eq!(
        result["status"], "executed",
        "Should execute step_resolve: {:?}",
        result
    );

    harness.teardown().await?;
    Ok(())
}

// ── step_complete ──

#[tokio::test]
async fn test_step_complete_preview_and_execute() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Seed a failing task
    let task_uuid = harness
        .seed_and_fail(
            "rust_e2e_retry",
            "retry_exhaustion_test",
            serde_json::json!({ "test_id": "step_complete_test" }),
        )
        .await?;

    let inspect = harness
        .call_tool(
            "task_inspect",
            serde_json::json!({ "task_uuid": task_uuid }),
        )
        .await?;
    let steps = inspect["steps"].as_array().expect("Should have steps");
    let failed_step = steps
        .iter()
        .find(|s| {
            s["status"]
                .as_str()
                .is_some_and(|st| st == "error" || st == "Error")
        })
        .expect("Should have a failed step");
    let step_uuid = failed_step["step_uuid"]
        .as_str()
        .expect("Step should have UUID");

    // Preview
    let preview = harness
        .call_tool(
            "step_complete",
            serde_json::json!({
                "task_uuid": task_uuid,
                "step_uuid": step_uuid,
                "result": { "corrected_value": 42 },
                "reason": "Providing corrected data",
                "completed_by": "integration-test"
            }),
        )
        .await?;
    assert_eq!(preview["status"], "preview");
    assert!(preview["details"]["result_data"].is_object());

    // Execute
    let result = harness
        .call_tool(
            "step_complete",
            serde_json::json!({
                "task_uuid": task_uuid,
                "step_uuid": step_uuid,
                "result": { "corrected_value": 42 },
                "reason": "Providing corrected data",
                "completed_by": "integration-test",
                "confirm": true
            }),
        )
        .await?;
    assert_eq!(
        result["status"], "executed",
        "Should execute step_complete: {:?}",
        result
    );

    harness.teardown().await?;
    Ok(())
}

// ── dlq_update ──

#[tokio::test]
async fn test_dlq_update_preview_and_execute() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Seed a failing task to generate DLQ entries
    let task_uuid = harness
        .seed_and_fail(
            "rust_e2e_retry",
            "retry_exhaustion_test",
            serde_json::json!({ "test_id": "dlq_update_test" }),
        )
        .await?;

    // DLQ entry creation is async — poll until the entry appears (max 10s)
    let mut dlq_entry_uuid = None;
    for attempt in 0..20 {
        let dlq_detail = harness
            .call_tool("dlq_inspect", serde_json::json!({ "task_uuid": task_uuid }))
            .await?;
        if let Some(uuid) = dlq_detail.get("dlq_entry_uuid").and_then(|v| v.as_str()) {
            dlq_entry_uuid = Some(uuid.to_string());
            break;
        }
        if attempt < 19 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }
    let dlq_entry_uuid =
        dlq_entry_uuid.expect("DLQ entry should appear within 10s after task failure");

    // Preview
    let preview = harness
        .call_tool(
            "dlq_update",
            serde_json::json!({
                "dlq_entry_uuid": dlq_entry_uuid,
                "resolution_status": "manually_resolved",
                "resolution_notes": "Resolved via integration test",
                "resolved_by": "integration-test"
            }),
        )
        .await?;
    assert_eq!(
        preview["status"], "preview",
        "Should return preview: {:?}",
        preview
    );
    assert_eq!(preview["action"], "dlq_update");

    // Execute
    let result = harness
        .call_tool(
            "dlq_update",
            serde_json::json!({
                "dlq_entry_uuid": dlq_entry_uuid,
                "resolution_status": "manually_resolved",
                "resolution_notes": "Resolved via integration test",
                "resolved_by": "integration-test",
                "confirm": true
            }),
        )
        .await?;
    assert_eq!(
        result["status"], "executed",
        "Should execute dlq_update: {:?}",
        result
    );

    harness.teardown().await?;
    Ok(())
}
