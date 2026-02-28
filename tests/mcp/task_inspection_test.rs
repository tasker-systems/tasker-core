//! Software Engineer persona: task and step inspection workflows.
//!
//! Fixture: mathematical_sequence.yaml (linear 4-step Rust handler, reliable success)
//! Flow: seed task → wait for completion → inspect task/steps/templates

use super::harness::McpTestHarness;

#[tokio::test]
async fn test_task_list_and_inspect() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // Seed a task and wait for it to complete
    let task_uuid = harness
        .seed_and_complete(
            "rust_e2e_linear",
            "mathematical_sequence",
            serde_json::json!({ "even_number": 4 }),
        )
        .await?;

    // task_list should include the seeded task
    let list_result = harness
        .call_tool(
            "task_list",
            serde_json::json!({ "namespace": "rust_e2e_linear", "limit": 10 }),
        )
        .await?;
    assert!(list_result["tasks"].is_array());
    let tasks = list_result["tasks"].as_array().unwrap();
    assert!(
        !tasks.is_empty(),
        "Expected at least one task in rust_e2e_linear namespace"
    );

    // task_inspect returns { "task": { ... }, "steps": [ ... ] }
    let inspect_result = harness
        .call_tool(
            "task_inspect",
            serde_json::json!({ "task_uuid": task_uuid }),
        )
        .await?;
    assert!(
        inspect_result.get("error").is_none(),
        "task_inspect should succeed: {:?}",
        inspect_result
    );
    assert!(inspect_result["steps"].is_array());
    let steps = inspect_result["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 4, "mathematical_sequence has 4 steps");

    harness.teardown().await?;
    Ok(())
}

#[tokio::test]
async fn test_step_inspect_and_audit() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    let task_uuid = harness
        .seed_and_complete(
            "rust_e2e_linear",
            "mathematical_sequence",
            serde_json::json!({ "even_number": 6 }),
        )
        .await?;

    // Get the task to find a step UUID
    let inspect_result = harness
        .call_tool(
            "task_inspect",
            serde_json::json!({ "task_uuid": task_uuid }),
        )
        .await?;
    let steps = inspect_result["steps"].as_array().unwrap();
    let first_step_uuid = steps[0]["step_uuid"]
        .as_str()
        .expect("Step should have a step_uuid");

    // step_inspect requires task_uuid + step_uuid
    let step_result = harness
        .call_tool(
            "step_inspect",
            serde_json::json!({ "task_uuid": task_uuid, "step_uuid": first_step_uuid }),
        )
        .await?;
    assert!(
        step_result.get("error").is_none(),
        "step_inspect should succeed: {:?}",
        step_result
    );

    // step_audit requires task_uuid + step_uuid
    let audit_result = harness
        .call_tool(
            "step_audit",
            serde_json::json!({ "task_uuid": task_uuid, "step_uuid": first_step_uuid }),
        )
        .await?;
    assert!(
        audit_result.get("error").is_none(),
        "step_audit should succeed: {:?}",
        audit_result
    );

    harness.teardown().await?;
    Ok(())
}

#[tokio::test]
async fn test_template_remote_operations() -> anyhow::Result<()> {
    let harness = McpTestHarness::setup().await?;

    // template_list_remote should list registered templates
    let list_result = harness
        .call_tool("template_list_remote", serde_json::json!({}))
        .await?;
    assert!(
        list_result.get("error").is_none(),
        "template_list_remote should succeed: {:?}",
        list_result
    );

    // The response is a direct array of templates from the API
    let templates = list_result
        .as_array()
        .or_else(|| list_result.get("templates").and_then(|t| t.as_array()))
        .expect("template_list_remote should return a list of templates");
    assert!(
        !templates.is_empty(),
        "Server should have at least one registered template"
    );

    // Find a template to inspect — needs namespace, name, version
    let first_template = &templates[0];
    let namespace = first_template["namespace"]
        .as_str()
        .expect("Template should have a namespace");
    let name = first_template["name"]
        .as_str()
        .expect("Template should have a name");
    let version = first_template["version"]
        .as_str()
        .expect("Template should have a version");

    // template_inspect_remote requires namespace + name + version
    let inspect_result = harness
        .call_tool(
            "template_inspect_remote",
            serde_json::json!({ "namespace": namespace, "name": name, "version": version }),
        )
        .await?;
    assert!(
        inspect_result.get("error").is_none(),
        "template_inspect_remote should succeed for '{}/{}@{}': {:?}",
        namespace,
        name,
        version,
        inspect_result
    );

    harness.teardown().await?;
    Ok(())
}
