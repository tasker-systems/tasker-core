//! Parameter and response structs for all MCP tools.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tasker_tooling::template_generator::{FieldSpec, StepSpec, TemplateSpec};

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

// ── use_environment ──

/// Parameters for the `use_environment` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UseEnvironmentParams {
    /// Profile name to switch to.
    #[schemars(description = "Profile name to switch to (e.g., 'default', 'staging', 'grpc')")]
    pub profile: String,
    /// Whether to probe health after switching (default: true).
    #[schemars(description = "Whether to probe health after switching (default: true)")]
    #[serde(default)]
    pub probe_health: Option<bool>,
}
