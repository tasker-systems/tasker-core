//! Shared confirmation semantics and permission-aware error handling.
//!
//! Used by tasker-mcp (JSON preview/execute pattern) and tasker-ctl
//! (interactive confirmation / --yes flag).

use serde::Serialize;
use tasker_shared::types::permissions::Permission;

/// Phase of a two-phase confirmation flow.
///
/// Write tools follow a preview → execute pattern: the caller first invokes
/// without confirmation to see what *would* happen, then re-invokes with
/// confirmation to actually perform the mutation. This enum makes that
/// intent explicit at the control-flow level rather than branching on a
/// raw boolean.
///
/// # Usage
///
/// ```rust,ignore
/// match ConfirmationPhase::from_flag(params.confirm) {
///     ConfirmationPhase::Preview => { /* build and return preview */ }
///     ConfirmationPhase::Execute => { /* perform the mutation */ }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationPhase {
    /// Caller is asking "what would happen?" — return a preview, do not mutate.
    Preview,
    /// Caller has reviewed the preview and is authorizing execution.
    Execute,
}

impl ConfirmationPhase {
    /// Derive the phase from a `confirm` flag (the MCP/CLI parameter).
    pub fn from_flag(confirm: bool) -> Self {
        if confirm {
            Self::Execute
        } else {
            Self::Preview
        }
    }
}

/// Outcome of a write operation that requires confirmation.
#[derive(Debug, Serialize)]
#[serde(tag = "status")]
pub enum WriteOutcome<T: Serialize> {
    /// Preview of what would happen — caller should show this and re-invoke with confirm.
    #[serde(rename = "preview")]
    Preview {
        action: String,
        description: String,
        details: serde_json::Value,
        instruction: String,
    },
    /// Operation executed successfully.
    #[serde(rename = "executed")]
    Executed(T),
}

/// Build a preview response for a write operation.
pub fn build_preview(
    action: &str,
    description: &str,
    details: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "action": action,
        "status": "preview",
        "description": description,
        "details": details,
        "instruction": "Call this tool again with confirm: true to execute this action."
    })
}

/// Permission mapping: tool name → required Permission.
pub fn required_permission(tool_name: &str) -> Option<Permission> {
    match tool_name {
        // Tier 3 write tools
        "task_submit" => Some(Permission::TasksCreate),
        "task_cancel" => Some(Permission::TasksCancel),
        "step_retry" | "step_resolve" | "step_complete" => Some(Permission::StepsResolve),
        "dlq_update" => Some(Permission::DlqUpdate),
        // Tier 2 read tools
        "task_list" | "task_inspect" => Some(Permission::TasksRead),
        "step_inspect" | "step_audit" => Some(Permission::StepsRead),
        "dlq_list" | "dlq_inspect" | "dlq_stats" | "dlq_queue" | "staleness_check" => {
            Some(Permission::DlqRead)
        }
        "analytics_performance" | "analytics_bottlenecks" => Some(Permission::AnalyticsRead),
        "system_health" | "system_config" => Some(Permission::SystemConfigRead),
        "template_list_remote" | "template_inspect_remote" => Some(Permission::TemplatesRead),
        _ => None,
    }
}

/// Detect whether an error string indicates a 403 permission denied response.
pub fn is_permission_error(error: &str) -> bool {
    error.contains("HTTP 403") || error.contains("403 -")
}

/// Build a structured permission-denied error JSON string.
pub fn permission_denied_json(tool_name: &str, profile: &str) -> String {
    let perm = required_permission(tool_name);
    let perm_str = perm.map(|p| p.as_str()).unwrap_or("unknown");

    serde_json::json!({
        "error": "permission_denied",
        "tool": tool_name,
        "message": format!("Profile '{}' is not authorized for this operation.", profile),
        "required_permission": perm_str,
        "hint": "Check the JWT claims or API key scope configured for this profile in tasker-client.toml."
    })
    .to_string()
}

/// Handle an API error with permission-aware enrichment.
///
/// If the error indicates a 403, returns a structured permission_denied response.
/// Otherwise returns a generic api_error.
pub fn handle_api_error(error: &str, tool_name: &str, profile: &str) -> String {
    if is_permission_error(error) {
        permission_denied_json(tool_name, profile)
    } else {
        serde_json::json!({
            "error": "api_error",
            "message": error,
            "valid": false
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_permission_error() {
        assert!(is_permission_error("HTTP 403: Forbidden"));
        assert!(is_permission_error("API error: 403 - Forbidden"));
        assert!(!is_permission_error("HTTP 404: Not Found"));
        assert!(!is_permission_error("connection refused"));
    }

    #[test]
    fn test_permission_denied_json() {
        let json = permission_denied_json("task_submit", "production");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"], "permission_denied");
        assert_eq!(parsed["tool"], "task_submit");
        assert_eq!(parsed["required_permission"], "tasks:create");
        assert!(parsed["message"].as_str().unwrap().contains("production"));
    }

    #[test]
    fn test_required_permission_mapping() {
        // Write tools
        assert_eq!(
            required_permission("task_submit"),
            Some(Permission::TasksCreate)
        );
        assert_eq!(
            required_permission("task_cancel"),
            Some(Permission::TasksCancel)
        );
        assert_eq!(
            required_permission("step_retry"),
            Some(Permission::StepsResolve)
        );
        assert_eq!(
            required_permission("step_resolve"),
            Some(Permission::StepsResolve)
        );
        assert_eq!(
            required_permission("step_complete"),
            Some(Permission::StepsResolve)
        );
        assert_eq!(
            required_permission("dlq_update"),
            Some(Permission::DlqUpdate)
        );
        // Read tools
        assert_eq!(
            required_permission("task_list"),
            Some(Permission::TasksRead)
        );
        assert_eq!(
            required_permission("system_health"),
            Some(Permission::SystemConfigRead)
        );
        // Unknown
        assert_eq!(required_permission("unknown_tool"), None);
    }

    #[test]
    fn test_handle_api_error_403() {
        let result = handle_api_error("HTTP 403: Forbidden", "task_submit", "prod");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "permission_denied");
        assert_eq!(parsed["required_permission"], "tasks:create");
    }

    #[test]
    fn test_handle_api_error_non_403() {
        let result = handle_api_error("connection refused", "task_list", "test");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "api_error");
        assert_eq!(parsed["message"], "connection refused");
    }

    #[test]
    fn test_build_preview() {
        let preview = build_preview(
            "task_submit",
            "Submit task 'order_processing' to namespace 'ecommerce'",
            serde_json::json!({"name": "order_processing"}),
        );
        assert_eq!(preview["status"], "preview");
        assert_eq!(preview["action"], "task_submit");
        assert!(preview["instruction"]
            .as_str()
            .unwrap()
            .contains("confirm: true"));
    }
}
