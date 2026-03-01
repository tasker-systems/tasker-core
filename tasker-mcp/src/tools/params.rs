//! Parameter and response structs for all MCP tools.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tasker_sdk::template_generator::{FieldSpec, StepSpec, TemplateSpec};

// ── template_validate ──

/// Parameters for the `template_validate` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemplateValidateParams {
    /// Task template YAML content to validate.
    #[schemars(description = "Task template YAML content to validate")]
    pub template_yaml: String,
}

// ── template_inspect ──

/// Parameters for the `template_inspect` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemplateInspectParams {
    /// Task template YAML content to inspect.
    #[schemars(description = "Task template YAML content to inspect")]
    pub template_yaml: String,
}

/// Response for the `template_inspect` tool.
#[derive(Debug, Serialize)]
pub struct TemplateInspectResponse {
    pub name: String,
    pub namespace: String,
    pub version: String,
    pub description: Option<String>,
    pub step_count: usize,
    pub has_input_schema: bool,
    pub execution_order: Vec<String>,
    pub root_steps: Vec<String>,
    pub leaf_steps: Vec<String>,
    pub steps: Vec<StepInspection>,
}

/// Inspection detail for a single step.
#[derive(Debug, Serialize)]
pub struct StepInspection {
    pub name: String,
    pub description: Option<String>,
    pub handler_callable: String,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
    pub has_result_schema: bool,
    pub result_field_count: Option<usize>,
}

// ── template_generate ──

/// Parameters for the `template_generate` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemplateGenerateParams {
    /// Task name.
    #[schemars(description = "Task name (e.g., 'order_processing')")]
    pub name: String,
    /// Namespace for organization.
    #[schemars(description = "Namespace (e.g., 'ecommerce')")]
    pub namespace: String,
    /// Semantic version (defaults to "1.0.0").
    #[schemars(description = "Semantic version (defaults to '1.0.0')")]
    pub version: Option<String>,
    /// Human-readable description.
    #[schemars(description = "Human-readable description of the task")]
    pub description: Option<String>,
    /// Step definitions.
    #[schemars(description = "Step definitions for the workflow")]
    pub steps: Vec<StepSpecParam>,
}

/// Step specification for template generation.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StepSpecParam {
    /// Step name.
    #[schemars(description = "Step name (e.g., 'validate_order')")]
    pub name: String,
    /// Step description.
    #[schemars(description = "Human-readable description of the step")]
    pub description: Option<String>,
    /// Handler callable (auto-generated as `{namespace}.{name}` if omitted).
    #[schemars(description = "Handler callable (auto-generated if omitted)")]
    pub handler: Option<String>,
    /// Dependencies on other steps.
    #[schemars(description = "Names of steps this step depends on")]
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Output fields that form the result_schema.
    #[schemars(description = "Output fields for the step's result_schema")]
    #[serde(default)]
    pub outputs: Vec<FieldSpecParam>,
}

/// Field specification for template generation.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FieldSpecParam {
    /// Field name.
    #[schemars(description = "Field name")]
    pub name: String,
    /// Field type: string, integer, number, boolean, array:T, object.
    #[schemars(description = "Field type: string, integer, number, boolean, array:<type>, object")]
    pub field_type: String,
    /// Whether this field is required (defaults to true).
    #[schemars(description = "Whether this field is required (defaults to true)")]
    #[serde(default = "default_true")]
    pub required: bool,
    /// Field description.
    #[schemars(description = "Human-readable description of the field")]
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

impl From<TemplateGenerateParams> for TemplateSpec {
    fn from(p: TemplateGenerateParams) -> Self {
        TemplateSpec {
            name: p.name,
            namespace: p.namespace,
            version: p.version,
            description: p.description,
            steps: p.steps.into_iter().map(StepSpec::from).collect(),
        }
    }
}

impl From<StepSpecParam> for StepSpec {
    fn from(p: StepSpecParam) -> Self {
        StepSpec {
            name: p.name,
            description: p.description,
            handler: p.handler,
            depends_on: p.depends_on,
            outputs: p.outputs.into_iter().map(FieldSpec::from).collect(),
        }
    }
}

impl From<FieldSpecParam> for FieldSpec {
    fn from(p: FieldSpecParam) -> Self {
        FieldSpec {
            name: p.name,
            field_type: p.field_type,
            required: p.required,
            description: p.description,
        }
    }
}

// ── handler_generate ──

/// Parameters for the `handler_generate` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct HandlerGenerateParams {
    /// Task template YAML content.
    #[schemars(description = "Task template YAML content")]
    pub template_yaml: String,
    /// Target language: python, ruby, typescript, or rust.
    #[schemars(description = "Target language: python, ruby, typescript, or rust")]
    pub language: String,
    /// Optional step name to generate code for (generates all if omitted).
    #[schemars(description = "Optional step name to generate code for (all steps if omitted)")]
    pub step_filter: Option<String>,
    /// When true (default), handlers import generated types and use typed return values.
    /// When false, generates independent types/handlers/tests without import wiring.
    #[schemars(
        description = "When true (default), handlers import generated types and use typed return values. When false, generates independent types/handlers/tests."
    )]
    pub scaffold: Option<bool>,
}

/// Response for the `handler_generate` tool.
#[derive(Debug, Serialize)]
pub struct HandlerGenerateResponse {
    pub language: String,
    pub types: String,
    pub handlers: String,
    pub tests: String,
    /// Handler registry bridge (Rust only — wraps plain functions as `StepHandler` trait objects)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_registry: Option<String>,
}

// ── schema_inspect ──

/// Parameters for the `schema_inspect` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SchemaInspectParams {
    /// Task template YAML content.
    #[schemars(description = "Task template YAML content")]
    pub template_yaml: String,
    /// Optional step name to inspect (inspects all if omitted).
    #[schemars(description = "Optional step name to inspect (all steps if omitted)")]
    pub step_filter: Option<String>,
}

/// Response for the `schema_inspect` tool.
#[derive(Debug, Serialize)]
pub struct SchemaInspectResponse {
    pub template_name: String,
    pub has_input_schema: bool,
    pub steps: Vec<StepSchemaDetail>,
}

/// Schema detail for a single step.
#[derive(Debug, Serialize)]
pub struct StepSchemaDetail {
    pub name: String,
    pub has_result_schema: bool,
    pub fields: Vec<FieldDetail>,
    pub consumed_by: Vec<String>,
}

/// Detail for a single field in a schema.
#[derive(Debug, Serialize)]
pub struct FieldDetail {
    pub name: String,
    pub field_type: String,
    pub required: bool,
    pub description: Option<String>,
}

// ── schema_compare ──

/// Parameters for the `schema_compare` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SchemaCompareParams {
    /// Task template YAML content.
    #[schemars(description = "Task template YAML content containing both steps")]
    pub template_yaml: String,
    /// Name of the producer step.
    #[schemars(description = "Name of the producer step (outputs data)")]
    pub producer_step: String,
    /// Name of the consumer step.
    #[schemars(description = "Name of the consumer step (consumes data)")]
    pub consumer_step: String,
}

// ── schema_diff ──

/// Parameters for the `schema_diff` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SchemaDiffParams {
    /// Older version of the task template YAML.
    #[schemars(description = "Older version of the task template YAML (before changes)")]
    pub before_yaml: String,
    /// Newer version of the task template YAML.
    #[schemars(description = "Newer version of the task template YAML (after changes)")]
    pub after_yaml: String,
    /// Optional step name to diff (diffs all steps if omitted).
    #[schemars(description = "Optional step name to diff (all steps if omitted)")]
    pub step_filter: Option<String>,
}

// ── connection_status ──

/// Parameters for the `connection_status` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConnectionStatusParams {
    /// Set to true to refresh health probes for all profiles.
    #[schemars(description = "Set to true to refresh health probes for all profiles")]
    #[serde(default)]
    pub refresh: Option<bool>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 2: Connected Read-Only Tools
// ═══════════════════════════════════════════════════════════════════════════

// ── Task & Step Inspection ──

/// Parameters for the `task_list` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskListParams {
    /// Optional profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Filter by namespace.
    #[schemars(description = "Filter tasks by namespace")]
    pub namespace: Option<String>,
    /// Filter by status (e.g., 'pending', 'complete', 'error').
    #[schemars(description = "Filter tasks by status (e.g., 'pending', 'complete', 'error')")]
    pub status: Option<String>,
    /// Maximum number of tasks to return (default 20, max 100).
    #[schemars(description = "Maximum number of tasks to return (default 20, max 100)")]
    pub limit: Option<i32>,
    /// Offset for pagination (default 0).
    #[schemars(description = "Offset for pagination (default 0)")]
    pub offset: Option<i32>,
}

/// Parameters for the `task_inspect` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskInspectParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Task UUID to inspect.
    #[schemars(description = "Task UUID to inspect")]
    pub task_uuid: String,
}

/// Parameters for the `step_inspect` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StepInspectToolParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Task UUID the step belongs to.
    #[schemars(description = "Task UUID the step belongs to")]
    pub task_uuid: String,
    /// Step UUID to inspect.
    #[schemars(description = "Step UUID to inspect")]
    pub step_uuid: String,
}

/// Parameters for the `step_audit` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StepAuditParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Task UUID the step belongs to.
    #[schemars(description = "Task UUID the step belongs to")]
    pub task_uuid: String,
    /// Step UUID to get audit history for.
    #[schemars(description = "Step UUID to get audit history for")]
    pub step_uuid: String,
}

// ── DLQ Investigation ──

/// Parameters for the `dlq_list` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DlqListToolParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Filter by resolution status (e.g., 'pending_investigation', 'resolved', 'retry_scheduled').
    #[schemars(
        description = "Filter by resolution status (e.g., 'pending_investigation', 'resolved', 'retry_scheduled')"
    )]
    pub resolution_status: Option<String>,
    /// Maximum number of entries to return (default 20).
    #[schemars(description = "Maximum number of entries to return (default 20)")]
    pub limit: Option<i64>,
}

/// Parameters for the `dlq_inspect` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DlqInspectToolParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Task UUID of the DLQ entry to inspect.
    #[schemars(description = "Task UUID of the DLQ entry to inspect")]
    pub task_uuid: String,
}

/// Parameters for the `dlq_stats` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DlqStatsToolParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
}

/// Parameters for the `dlq_queue` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DlqQueueToolParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Maximum number of entries in the prioritized queue (default 10).
    #[schemars(description = "Maximum number of entries in the prioritized queue (default 10)")]
    pub limit: Option<i64>,
}

/// Parameters for the `staleness_check` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StalenessCheckParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Maximum number of tasks to check (default 20).
    #[schemars(description = "Maximum number of tasks to check (default 20)")]
    pub limit: Option<i64>,
}

// ── Analytics ──

/// Parameters for the `analytics_performance` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnalyticsPerformanceParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Number of hours to look back for metrics (e.g., 24 for last day).
    #[schemars(description = "Number of hours to look back for metrics (e.g., 24 for last day)")]
    pub hours: Option<u32>,
}

/// Parameters for the `analytics_bottlenecks` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnalyticsBottlenecksParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Maximum number of bottlenecks to return.
    #[schemars(description = "Maximum number of bottlenecks to return")]
    pub limit: Option<i32>,
    /// Minimum number of executions to consider a step for bottleneck analysis.
    #[schemars(
        description = "Minimum number of executions to consider a step for bottleneck analysis"
    )]
    pub min_executions: Option<i32>,
}

// ── System ──

/// Parameters for the `system_health` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SystemHealthParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
}

/// Parameters for the `system_config` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SystemConfigParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
}

// ── Remote Templates ──

/// Parameters for the `template_list_remote` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemplateListRemoteParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Filter templates by namespace.
    #[schemars(description = "Filter templates by namespace")]
    pub namespace: Option<String>,
}

/// Parameters for the `template_inspect_remote` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TemplateInspectRemoteParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Template namespace.
    #[schemars(description = "Template namespace")]
    pub namespace: String,
    /// Template name.
    #[schemars(description = "Template name")]
    pub name: String,
    /// Template version.
    #[schemars(description = "Template version (e.g., '1.0.0')")]
    pub version: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Tier 3: Write Tools with Confirmation
// ═══════════════════════════════════════════════════════════════════════════

/// Parameters for the `task_submit` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskSubmitParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Task name matching a registered template.
    #[schemars(
        description = "Task name matching a registered template (e.g., 'order_processing')"
    )]
    pub name: String,
    /// Task namespace.
    #[schemars(description = "Task namespace (e.g., 'ecommerce')")]
    pub namespace: String,
    /// Template version (defaults to '0.1.0').
    #[schemars(description = "Template version (defaults to '0.1.0')")]
    #[serde(default = "default_version")]
    pub version: Option<String>,
    /// Context data for task execution.
    #[schemars(description = "Context data (JSON object) for task execution")]
    pub context: serde_json::Value,
    /// Who or what initiated this task.
    #[schemars(description = "Who or what initiated this task (defaults to 'mcp-agent')")]
    pub initiator: Option<String>,
    /// Source system identifier.
    #[schemars(description = "Source system identifier (defaults to 'tasker-mcp')")]
    pub source_system: Option<String>,
    /// Reason for task submission.
    #[schemars(description = "Reason for task submission")]
    pub reason: Option<String>,
    /// Task priority (-100 to 100, higher = more urgent).
    #[schemars(description = "Task priority (-100 to 100, higher = more urgent)")]
    pub priority: Option<i32>,
    /// Tags for categorization.
    #[schemars(description = "Tags for categorization and filtering")]
    #[serde(default)]
    pub tags: Vec<String>,
    /// Set to true to execute the submission (omit or false for preview).
    #[schemars(
        description = "Set to true to execute the submission. Omit or set false to preview what will happen."
    )]
    #[serde(default)]
    pub confirm: bool,
}

fn default_version() -> Option<String> {
    Some("0.1.0".to_string())
}

/// Parameters for the `task_cancel` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TaskCancelParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Task UUID to cancel.
    #[schemars(description = "Task UUID to cancel")]
    pub task_uuid: String,
    /// Set to true to execute the cancellation (omit or false for preview).
    #[schemars(
        description = "Set to true to execute the cancellation. Omit or set false to preview what will happen."
    )]
    #[serde(default)]
    pub confirm: bool,
}

/// Parameters for the `step_retry` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StepRetryParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Task UUID the step belongs to.
    #[schemars(description = "Task UUID the step belongs to")]
    pub task_uuid: String,
    /// Step UUID to retry.
    #[schemars(description = "Step UUID to reset for retry")]
    pub step_uuid: String,
    /// Reason for the retry.
    #[schemars(description = "Human-readable reason for resetting this step for retry")]
    pub reason: String,
    /// Operator performing the reset.
    #[schemars(description = "Identifier of the operator performing the reset")]
    pub reset_by: String,
    /// Set to true to execute the retry (omit or false for preview).
    #[schemars(
        description = "Set to true to execute the retry. Omit or set false to preview what will happen."
    )]
    #[serde(default)]
    pub confirm: bool,
}

/// Parameters for the `step_resolve` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StepResolveParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Task UUID the step belongs to.
    #[schemars(description = "Task UUID the step belongs to")]
    pub task_uuid: String,
    /// Step UUID to resolve.
    #[schemars(description = "Step UUID to mark as manually resolved")]
    pub step_uuid: String,
    /// Reason for manual resolution.
    #[schemars(description = "Human-readable reason for manual resolution")]
    pub reason: String,
    /// Operator performing the resolution.
    #[schemars(description = "Identifier of the operator performing the resolution")]
    pub resolved_by: String,
    /// Set to true to execute the resolution (omit or false for preview).
    #[schemars(
        description = "Set to true to execute the resolution. Omit or set false to preview what will happen."
    )]
    #[serde(default)]
    pub confirm: bool,
}

/// Parameters for the `step_complete` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StepCompleteParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// Task UUID the step belongs to.
    #[schemars(description = "Task UUID the step belongs to")]
    pub task_uuid: String,
    /// Step UUID to complete.
    #[schemars(description = "Step UUID to manually complete")]
    pub step_uuid: String,
    /// Result data for the step (business data that downstream steps expect).
    #[schemars(
        description = "Result data (JSON object) for the step. Must match what downstream steps expect."
    )]
    pub result: serde_json::Value,
    /// Optional metadata for observability.
    #[schemars(description = "Optional metadata for observability and tracking")]
    pub metadata: Option<serde_json::Value>,
    /// Reason for manual completion.
    #[schemars(description = "Human-readable reason for manual completion")]
    pub reason: String,
    /// Operator performing the completion.
    #[schemars(description = "Identifier of the operator performing the completion")]
    pub completed_by: String,
    /// Set to true to execute the completion (omit or false for preview).
    #[schemars(
        description = "Set to true to execute the completion. Omit or set false to preview what will happen."
    )]
    #[serde(default)]
    pub confirm: bool,
}

/// Parameters for the `dlq_update` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DlqUpdateParams {
    /// Profile name to target a specific environment (uses active profile if omitted).
    #[schemars(
        description = "Profile name to target a specific environment (uses active profile if omitted)"
    )]
    pub profile: Option<String>,
    /// DLQ entry UUID to update.
    #[schemars(description = "DLQ entry UUID to update")]
    pub dlq_entry_uuid: String,
    /// New resolution status (e.g., 'manually_resolved', 'permanently_failed', 'cancelled').
    #[schemars(
        description = "New resolution status: 'pending', 'manually_resolved', 'permanently_failed', 'cancelled'"
    )]
    pub resolution_status: Option<String>,
    /// Investigation notes.
    #[schemars(description = "Investigation notes documenting what was found and done")]
    pub resolution_notes: Option<String>,
    /// Who resolved the investigation.
    #[schemars(description = "Identifier of who resolved the investigation")]
    pub resolved_by: Option<String>,
    /// Additional metadata.
    #[schemars(description = "Additional metadata (JSON object) for the investigation")]
    pub metadata: Option<serde_json::Value>,
    /// Set to true to execute the update (omit or false for preview).
    #[schemars(
        description = "Set to true to execute the update. Omit or set false to preview what will happen."
    )]
    #[serde(default)]
    pub confirm: bool,
}
