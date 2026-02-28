//! MCP ServerHandler implementation for Tasker.
//!
//! Provides the MCP server with developer tooling tools (Tier 1, always available)
//! and profile management tools (available when profiles are configured):
//!
//! **Tier 1 — Developer Tooling (offline)**
//! - `template_validate` — Validate a task template for structural correctness
//! - `template_inspect` — Inspect template DAG structure and step details
//! - `template_generate` — Generate task template YAML from a structured spec
//! - `handler_generate` — Generate typed handler code for a template
//! - `schema_inspect` — Inspect result_schema field details per step
//! - `schema_compare` — Compare producer/consumer schema compatibility
//! - `schema_diff` — Detect field-level changes between two template versions
//!
//! **Profile Management (when profiles configured)**
//! - `connection_status` — Show profile health and available capabilities
//! - `use_environment` — Switch active profile/environment

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use tokio::sync::RwLock;

use tasker_client::ProfileManager;
use tasker_tooling::codegen::{self, TargetLanguage};
use tasker_tooling::schema_comparator;
use tasker_tooling::schema_diff;
use tasker_tooling::schema_inspector;
use tasker_tooling::template_generator;
use tasker_tooling::template_parser::parse_template_str;
use tasker_tooling::template_validator;

use crate::tools::*;

/// Tasker MCP server handler with developer tooling and profile management.
#[derive(Debug, Clone)]
pub struct TaskerMcpServer {
    tool_router: ToolRouter<Self>,
    profile_manager: Arc<RwLock<ProfileManager>>,
    offline: bool,
}

impl Default for TaskerMcpServer {
    fn default() -> Self {
        let pm = ProfileManager::load().unwrap_or_else(|_| ProfileManager::offline());
        Self::with_profile_manager(pm, false)
    }
}

impl TaskerMcpServer {
    /// Create a server with profile management.
    pub fn with_profile_manager(profile_manager: ProfileManager, offline: bool) -> Self {
        Self {
            tool_router: Self::tool_router(),
            profile_manager: Arc::new(RwLock::new(profile_manager)),
            offline,
        }
    }

    /// Create a server in offline mode (Tier 1 developer tools only).
    pub fn offline() -> Self {
        Self::with_profile_manager(ProfileManager::offline(), true)
    }

    /// Create a server with no-arg constructor for backward compatibility in tests.
    pub fn new() -> Self {
        Self::offline()
    }

    /// Get a reference to the profile manager.
    pub fn profile_manager(&self) -> &Arc<RwLock<ProfileManager>> {
        &self.profile_manager
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TaskerMcpServer {
    fn get_info(&self) -> ServerInfo {
        let instructions = if self.offline {
            "Tasker is a workflow orchestration system. Running in OFFLINE mode — \
             only developer tooling tools are available (template validation, code generation, \
             schema inspection). Connect to a Tasker instance for task management tools.\n\
             Workflow: template_generate → template_validate → handler_generate (unified types+handlers+tests)\n\
             handler_generate defaults to scaffold mode: handlers import generated types with typed returns.\n\
             When debugging: template_inspect → schema_inspect → schema_compare\n\
             For versioning: schema_diff compares before/after template YAML to detect breaking field changes"
                .to_string()
        } else {
            "Tasker is a workflow orchestration system. You help developers create and validate \
             task templates (workflow definitions) and generate typed handler code.\n\
             Workflow: template_generate → template_validate → handler_generate (unified types+handlers+tests)\n\
             handler_generate defaults to scaffold mode: handlers import generated types with typed returns.\n\
             When debugging: template_inspect → schema_inspect → schema_compare\n\
             For versioning: schema_diff compares before/after template YAML to detect breaking field changes\n\
             Profile management: connection_status to check environment health, use_environment to switch profiles"
                .to_string()
        };

        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "tasker-mcp".to_string(),
                title: Some("Tasker MCP Server".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some(
                    "MCP server exposing Tasker developer tooling: template validation, \
                     code generation, schema inspection, and workflow analysis"
                        .to_string(),
                ),
                icons: None,
                website_url: None,
            },
            instructions: Some(instructions),
        }
    }
}

#[tool_router(router = tool_router)]
impl TaskerMcpServer {
    // ── Tier 1: Developer Tooling (always available) ──

    /// Validate a task template YAML for structural correctness, dependency cycles,
    /// and best-practice warnings.
    #[tool(
        name = "template_validate",
        description = "Validate a task template YAML for structural correctness, dependency cycles, and best-practice warnings. Returns validation findings with severity levels (error/warning/info)."
    )]
    pub async fn template_validate(
        &self,
        Parameters(params): Parameters<TemplateValidateParams>,
    ) -> String {
        match parse_template_str(&params.template_yaml) {
            Ok(template) => {
                let report = template_validator::validate(&template);
                serde_json::to_string_pretty(&report)
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            Err(e) => error_json("yaml_parse_error", &e.to_string()),
        }
    }

    /// Inspect a task template's DAG structure: execution order, root/leaf steps,
    /// dependencies, and per-step details.
    #[tool(
        name = "template_inspect",
        description = "Inspect a task template's DAG structure: execution order, root/leaf steps, dependencies, and per-step details including handler callables and schema presence."
    )]
    pub async fn template_inspect(
        &self,
        Parameters(params): Parameters<TemplateInspectParams>,
    ) -> String {
        match parse_template_str(&params.template_yaml) {
            Ok(template) => {
                let schema_report = schema_inspector::inspect(&template);

                // Build dependency maps
                let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
                for step in &template.steps {
                    for dep in &step.dependencies {
                        dependents.entry(dep.as_str()).or_default().push(&step.name);
                    }
                }

                let root_steps: Vec<String> = template
                    .steps
                    .iter()
                    .filter(|s| s.dependencies.is_empty())
                    .map(|s| s.name.clone())
                    .collect();

                let depended_on: HashSet<&str> = template
                    .steps
                    .iter()
                    .flat_map(|s| s.dependencies.iter().map(|d| d.as_str()))
                    .collect();
                let leaf_steps: Vec<String> = template
                    .steps
                    .iter()
                    .filter(|s| !depended_on.contains(s.name.as_str()))
                    .map(|s| s.name.clone())
                    .collect();

                // Topological sort for execution order
                let execution_order = topological_sort(&template);

                let steps: Vec<StepInspection> = template
                    .steps
                    .iter()
                    .map(|step| {
                        let schema_info = schema_report.steps.iter().find(|s| s.name == step.name);
                        StepInspection {
                            name: step.name.clone(),
                            description: step.description.clone(),
                            handler_callable: step.handler.callable.clone(),
                            dependencies: step.dependencies.clone(),
                            dependents: dependents
                                .get(step.name.as_str())
                                .map(|d| d.iter().map(|s| s.to_string()).collect())
                                .unwrap_or_default(),
                            has_result_schema: schema_info
                                .map(|s| s.has_result_schema)
                                .unwrap_or(false),
                            result_field_count: schema_info.and_then(|s| s.property_count),
                        }
                    })
                    .collect();

                let response = TemplateInspectResponse {
                    name: template.name.clone(),
                    namespace: template.namespace_name.clone(),
                    version: template.version.clone(),
                    description: template.description.clone(),
                    step_count: template.steps.len(),
                    has_input_schema: template.input_schema.is_some(),
                    execution_order,
                    root_steps,
                    leaf_steps,
                    steps,
                };

                serde_json::to_string_pretty(&response)
                    .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
            }
            Err(e) => error_json("yaml_parse_error", &e.to_string()),
        }
    }

    /// Generate a task template YAML from a structured specification with step
    /// definitions and output field types.
    #[tool(
        name = "template_generate",
        description = "Generate a task template YAML from a structured specification. Provide task name, namespace, steps with dependencies, and output field definitions. Returns valid template YAML."
    )]
    pub async fn template_generate(
        &self,
        Parameters(params): Parameters<TemplateGenerateParams>,
    ) -> String {
        let spec: tasker_tooling::template_generator::TemplateSpec = params.into();
        match template_generator::generate_yaml(&spec) {
            Ok(yaml) => yaml,
            Err(e) => error_json("generation_error", &e.to_string()),
        }
    }

    /// Generate typed handler code (types, handlers, tests) for a task template
    /// in the specified language.
    #[tool(
        name = "handler_generate",
        description = "Generate typed handler code for a task template. Returns types, handler scaffolds, and test files for the specified language (python, ruby, typescript, rust). By default uses scaffold mode where handlers import generated types."
    )]
    pub async fn handler_generate(
        &self,
        Parameters(params): Parameters<HandlerGenerateParams>,
    ) -> String {
        let template = match parse_template_str(&params.template_yaml) {
            Ok(t) => t,
            Err(e) => return error_json("yaml_parse_error", &e.to_string()),
        };

        let language: TargetLanguage = match params.language.parse() {
            Ok(l) => l,
            Err(e) => return error_json("invalid_language", &e.to_string()),
        };

        let step_filter = params.step_filter.as_deref();
        let use_scaffold = params.scaffold.unwrap_or(true);

        if use_scaffold {
            let scaffold_output =
                match codegen::scaffold::generate_scaffold(&template, language, step_filter) {
                    Ok(o) => o,
                    Err(e) => return error_json("codegen_error", &e.to_string()),
                };

            let response = HandlerGenerateResponse {
                language: language.to_string(),
                types: scaffold_output.types,
                handlers: scaffold_output.handlers,
                tests: scaffold_output.tests,
                handler_registry: scaffold_output.handler_registry,
            };

            serde_json::to_string_pretty(&response)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        } else {
            let types = match codegen::generate_types(&template, language, step_filter) {
                Ok(t) => t,
                Err(e) => return error_json("codegen_error", &format!("types: {e}")),
            };

            let handlers = match codegen::generate_handlers(&template, language, step_filter) {
                Ok(h) => h,
                Err(e) => return error_json("codegen_error", &format!("handlers: {e}")),
            };

            let tests = match codegen::generate_tests(&template, language, step_filter) {
                Ok(t) => t,
                Err(e) => return error_json("codegen_error", &format!("tests: {e}")),
            };

            let response = HandlerGenerateResponse {
                language: language.to_string(),
                types,
                handlers,
                tests,
                handler_registry: None,
            };

            serde_json::to_string_pretty(&response)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
    }

    /// Inspect result_schema definitions across template steps with field-level
    /// detail including types, required status, and consumer relationships.
    #[tool(
        name = "schema_inspect",
        description = "Inspect result_schema definitions across template steps. Returns field-level detail including types, required status, and which downstream steps consume each step's output."
    )]
    pub async fn schema_inspect(
        &self,
        Parameters(params): Parameters<SchemaInspectParams>,
    ) -> String {
        let template = match parse_template_str(&params.template_yaml) {
            Ok(t) => t,
            Err(e) => return error_json("yaml_parse_error", &e.to_string()),
        };

        // Build consumed_by map
        let mut consumed_by: HashMap<&str, Vec<&str>> = HashMap::new();
        for step in &template.steps {
            for dep in &step.dependencies {
                consumed_by
                    .entry(dep.as_str())
                    .or_default()
                    .push(&step.name);
            }
        }

        let steps: Vec<StepSchemaDetail> = template
            .steps
            .iter()
            .filter(|s| {
                params
                    .step_filter
                    .as_ref()
                    .is_none_or(|filter| s.name == *filter)
            })
            .map(|step| {
                let fields = step
                    .result_schema
                    .as_ref()
                    .and_then(|schema| codegen::schema::extract_types(&step.name, schema).ok())
                    .map(|type_defs| {
                        // Get the root type (last in dependency order)
                        type_defs
                            .last()
                            .map(|td| {
                                td.fields
                                    .iter()
                                    .map(|f| FieldDetail {
                                        name: f.name.clone(),
                                        field_type: f.field_type.json_schema_type(),
                                        required: f.required,
                                        description: f.description.clone(),
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();

                StepSchemaDetail {
                    name: step.name.clone(),
                    has_result_schema: step.result_schema.is_some(),
                    fields,
                    consumed_by: consumed_by
                        .get(step.name.as_str())
                        .map(|c| c.iter().map(|s| s.to_string()).collect())
                        .unwrap_or_default(),
                }
            })
            .collect();

        let response = SchemaInspectResponse {
            template_name: template.name.clone(),
            has_input_schema: template.input_schema.is_some(),
            steps,
        };

        serde_json::to_string_pretty(&response)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
    }

    /// Compare the result_schema of a producer step against a consumer step
    /// to check compatibility.
    #[tool(
        name = "schema_compare",
        description = "Compare the result_schema of a producer step against a consumer step to check data contract compatibility. Reports missing required fields, type mismatches, and extra fields."
    )]
    pub async fn schema_compare(
        &self,
        Parameters(params): Parameters<SchemaCompareParams>,
    ) -> String {
        let template = match parse_template_str(&params.template_yaml) {
            Ok(t) => t,
            Err(e) => return error_json("yaml_parse_error", &e.to_string()),
        };

        let producer = template
            .steps
            .iter()
            .find(|s| s.name == params.producer_step);
        let consumer = template
            .steps
            .iter()
            .find(|s| s.name == params.consumer_step);

        let Some(producer) = producer else {
            return error_json(
                "step_not_found",
                &format!("Producer step '{}' not found", params.producer_step),
            );
        };
        let Some(consumer) = consumer else {
            return error_json(
                "step_not_found",
                &format!("Consumer step '{}' not found", params.consumer_step),
            );
        };

        let empty_schema = serde_json::json!({"type": "object"});
        let producer_schema = producer.result_schema.as_ref().unwrap_or(&empty_schema);
        let consumer_schema = consumer.result_schema.as_ref().unwrap_or(&empty_schema);

        let report = schema_comparator::compare_schemas(
            &params.producer_step,
            producer_schema,
            &params.consumer_step,
            consumer_schema,
        );

        serde_json::to_string_pretty(&report)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
    }

    /// Diff two versions of a task template to detect field-level changes in
    /// result_schema definitions (additions, removals, type changes, required status).
    #[tool(
        name = "schema_diff",
        description = "Compare two versions of the same task template to detect field-level changes. Reports field additions, removals, type changes, and required/optional status changes with breaking-change analysis."
    )]
    pub async fn schema_diff(&self, Parameters(params): Parameters<SchemaDiffParams>) -> String {
        let before = match parse_template_str(&params.before_yaml) {
            Ok(t) => t,
            Err(e) => return error_json("yaml_parse_error", &format!("before_yaml: {e}")),
        };
        let after = match parse_template_str(&params.after_yaml) {
            Ok(t) => t,
            Err(e) => return error_json("yaml_parse_error", &format!("after_yaml: {e}")),
        };

        let report = schema_diff::diff_templates(&before, &after, params.step_filter.as_deref());

        serde_json::to_string_pretty(&report)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
    }

    // ── Profile Management ──

    /// Show connection status of all configured Tasker profiles.
    #[tool(
        name = "connection_status",
        description = "Show connection status of all configured Tasker profiles. Returns profile names, endpoints, transport type, health status, and which profile is active. Use this to verify connectivity before running task management operations. Pass refresh=true to re-probe all endpoints."
    )]
    pub async fn connection_status(
        &self,
        Parameters(params): Parameters<ConnectionStatusParams>,
    ) -> String {
        if self.offline {
            return serde_json::json!({
                "mode": "offline",
                "message": "Running in offline mode. No profiles are configured. Only developer tooling tools (template_*, handler_*, schema_*) are available.",
                "profiles": []
            })
            .to_string();
        }

        let mut pm = self.profile_manager.write().await;

        if params.refresh.unwrap_or(false) {
            pm.probe_all_health().await;
        }

        let profiles = pm.list_profiles();
        let active = pm.active_profile_name().to_string();

        serde_json::to_string_pretty(&serde_json::json!({
            "mode": "connected",
            "active_profile": active,
            "profiles": profiles,
        }))
        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
    }

    /// Switch the active Tasker profile to a different environment.
    #[tool(
        name = "use_environment",
        description = "Switch the active Tasker profile to a different environment (e.g., 'staging', 'local-dev', 'grpc'). After switching, probes health by default. Returns the new active profile's connection details."
    )]
    pub async fn use_environment(
        &self,
        Parameters(params): Parameters<UseEnvironmentParams>,
    ) -> String {
        if self.offline {
            return error_json(
                "offline_mode",
                "Cannot switch environments in offline mode. Restart without --offline flag.",
            );
        }

        let mut pm = self.profile_manager.write().await;

        if let Err(e) = pm.switch_profile(&params.profile) {
            return error_json("profile_not_found", &e.to_string());
        }

        // Probe health after switching (default: true)
        if params.probe_health.unwrap_or(true) {
            let _ = pm.probe_active_health().await;
        }

        let profiles = pm.list_profiles();
        let active_summary = profiles.into_iter().find(|p| p.is_active);

        serde_json::to_string_pretty(&serde_json::json!({
            "switched_to": params.profile,
            "profile": active_summary,
        }))
        .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
    }
}

/// Build a structured error JSON string that LLMs can parse.
fn error_json(error_code: &str, message: &str) -> String {
    serde_json::json!({
        "error": error_code,
        "message": message,
        "valid": false
    })
    .to_string()
}

/// Simple topological sort via Kahn's algorithm.
fn topological_sort(
    template: &tasker_shared::models::core::task_template::TaskTemplate,
) -> Vec<String> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for step in &template.steps {
        in_degree.entry(&step.name).or_insert(0);
        for dep in &step.dependencies {
            adj.entry(dep.as_str()).or_default().push(&step.name);
            *in_degree.entry(&step.name).or_insert(0) += 1;
        }
    }

    let mut queue: std::collections::VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&name, _)| name)
        .collect();

    // Sort the initial queue for deterministic output
    let mut sorted_queue: Vec<&str> = queue.drain(..).collect();
    sorted_queue.sort();
    queue.extend(sorted_queue);

    let mut result = Vec::new();
    while let Some(node) = queue.pop_front() {
        result.push(node.to_string());
        if let Some(neighbors) = adj.get(node) {
            let mut next_batch = Vec::new();
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        next_batch.push(neighbor);
                    }
                }
            }
            next_batch.sort();
            queue.extend(next_batch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_info_offline() {
        let server = TaskerMcpServer::new();
        let info = server.get_info();

        assert_eq!(info.server_info.name, "tasker-mcp");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        let instructions = info.instructions.unwrap();
        assert!(instructions.contains("OFFLINE"));
        assert!(instructions.contains("template_generate"));
    }

    #[test]
    fn test_server_info_connected() {
        let pm = ProfileManager::offline(); // Empty but not in offline mode
        let server = TaskerMcpServer::with_profile_manager(pm, false);
        let info = server.get_info();

        let instructions = info.instructions.unwrap();
        assert!(!instructions.contains("OFFLINE"));
        assert!(instructions.contains("connection_status"));
        assert!(instructions.contains("use_environment"));
    }

    #[test]
    fn test_server_uses_tasker_tooling() {
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = tasker_tooling::template_parser::parse_template_str(yaml).unwrap();
        assert_eq!(template.name, "codegen_test");

        let report = tasker_tooling::schema_inspector::inspect(&template);
        assert!(!report.steps.is_empty());
    }

    #[tokio::test]
    async fn test_template_validate_valid() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .template_validate(Parameters(TemplateValidateParams {
                template_yaml: yaml.to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["valid"], true);
        assert_eq!(parsed["step_count"], 5);
    }

    #[tokio::test]
    async fn test_template_validate_cycle() {
        let server = TaskerMcpServer::new();
        let yaml = r#"
name: cycle_test
namespace_name: test
version: "1.0.0"
steps:
  - name: a
    handler:
      callable: test.a
    depends_on: [b]
  - name: b
    handler:
      callable: test.b
    depends_on: [a]
"#;
        let result = server
            .template_validate(Parameters(TemplateValidateParams {
                template_yaml: yaml.to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["valid"], false);
        assert_eq!(parsed["has_cycles"], true);
    }

    #[tokio::test]
    async fn test_template_validate_invalid_yaml() {
        let server = TaskerMcpServer::new();
        let result = server
            .template_validate(Parameters(TemplateValidateParams {
                template_yaml: "not: [valid: yaml".to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "yaml_parse_error");
    }

    #[tokio::test]
    async fn test_template_inspect() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .template_inspect(Parameters(TemplateInspectParams {
                template_yaml: yaml.to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "codegen_test");
        assert_eq!(parsed["step_count"], 5);
        assert!(parsed["execution_order"].as_array().unwrap().len() == 5);
        assert!(parsed["root_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "validate_order"));
    }

    #[tokio::test]
    async fn test_template_generate() {
        let server = TaskerMcpServer::new();
        let result = server
            .template_generate(Parameters(TemplateGenerateParams {
                name: "test_task".into(),
                namespace: "ns".into(),
                version: None,
                description: Some("Test".into()),
                steps: vec![StepSpecParam {
                    name: "step_one".into(),
                    description: None,
                    handler: None,
                    depends_on: vec![],
                    outputs: vec![FieldSpecParam {
                        name: "result".into(),
                        field_type: "string".into(),
                        required: true,
                        description: None,
                    }],
                }],
            }))
            .await;
        assert!(result.contains("test_task"));
        assert!(result.contains("ns.step_one"));
    }

    #[tokio::test]
    async fn test_handler_generate() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .handler_generate(Parameters(HandlerGenerateParams {
                template_yaml: yaml.to_string(),
                language: "python".into(),
                step_filter: Some("validate_order".into()),
                scaffold: Some(true),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["language"], "python");
        assert!(parsed["types"].as_str().unwrap().contains("class"));
        assert!(parsed["handlers"].as_str().unwrap().contains("def"));
        // Scaffold mode: handlers import types
        assert!(parsed["handlers"]
            .as_str()
            .unwrap()
            .contains("from .models import"));
    }

    #[tokio::test]
    async fn test_handler_generate_no_scaffold() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .handler_generate(Parameters(HandlerGenerateParams {
                template_yaml: yaml.to_string(),
                language: "python".into(),
                step_filter: Some("validate_order".into()),
                scaffold: Some(false),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["language"], "python");
        // Non-scaffold mode: no type imports in handlers
        assert!(!parsed["handlers"]
            .as_str()
            .unwrap()
            .contains("from .models import"));
    }

    #[tokio::test]
    async fn test_handler_generate_invalid_language() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .handler_generate(Parameters(HandlerGenerateParams {
                template_yaml: yaml.to_string(),
                language: "cobol".into(),
                step_filter: None,
                scaffold: None,
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "invalid_language");
    }

    #[tokio::test]
    async fn test_schema_inspect() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .schema_inspect(Parameters(SchemaInspectParams {
                template_yaml: yaml.to_string(),
                step_filter: Some("validate_order".into()),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["template_name"], "codegen_test");
        let steps = parsed["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 1);
        assert!(steps[0]["has_result_schema"].as_bool().unwrap());
        assert!(!steps[0]["fields"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_schema_compare() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .schema_compare(Parameters(SchemaCompareParams {
                template_yaml: yaml.to_string(),
                producer_step: "validate_order".into(),
                consumer_step: "enrich_order".into(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["compatibility"].is_string());
        assert!(parsed["findings"].is_array());
    }

    #[tokio::test]
    async fn test_schema_compare_step_not_found() {
        let server = TaskerMcpServer::new();
        let yaml = include_str!("../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = server
            .schema_compare(Parameters(SchemaCompareParams {
                template_yaml: yaml.to_string(),
                producer_step: "nonexistent".into(),
                consumer_step: "enrich_order".into(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "step_not_found");
    }

    #[tokio::test]
    async fn test_schema_diff() {
        let server = TaskerMcpServer::new();
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
        let result = server
            .schema_diff(Parameters(SchemaDiffParams {
                before_yaml: before_yaml.to_string(),
                after_yaml: after_yaml.to_string(),
                step_filter: None,
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["compatibility"], "incompatible");
        let diffs = parsed["step_diffs"].as_array().unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0]["status"], "modified");

        let findings = diffs[0]["findings"].as_array().unwrap();
        // Should detect: FIELD_ADDED (email), FIELD_REMOVED (name, required→breaking),
        // REQUIRED_TO_OPTIONAL (name was required, removed is separate)
        assert!(findings.iter().any(|f| f["code"] == "FIELD_ADDED"));
        assert!(findings
            .iter()
            .any(|f| f["code"] == "FIELD_REMOVED" && f["breaking"] == true));
    }

    #[tokio::test]
    async fn test_content_publishing_template_validates() {
        let server = TaskerMcpServer::new();
        let yaml =
            include_str!("../../tests/fixtures/task_templates/content_publishing_template.yaml");
        let result = server
            .template_validate(Parameters(TemplateValidateParams {
                template_yaml: yaml.to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["valid"], true);
        assert_eq!(parsed["step_count"], 7);
    }

    #[tokio::test]
    async fn test_content_publishing_template_inspect() {
        let server = TaskerMcpServer::new();
        let yaml =
            include_str!("../../tests/fixtures/task_templates/content_publishing_template.yaml");
        let result = server
            .template_inspect(Parameters(TemplateInspectParams {
                template_yaml: yaml.to_string(),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "publish_article");
        assert_eq!(parsed["step_count"], 7);

        let root_steps = parsed["root_steps"].as_array().unwrap();
        assert_eq!(root_steps.len(), 1);
        assert_eq!(root_steps[0], "validate_content");

        let leaf_steps = parsed["leaf_steps"].as_array().unwrap();
        assert_eq!(leaf_steps.len(), 1);
        assert_eq!(leaf_steps[0], "update_analytics");
    }

    #[tokio::test]
    async fn test_content_publishing_handler_generate() {
        let server = TaskerMcpServer::new();
        let yaml =
            include_str!("../../tests/fixtures/task_templates/content_publishing_template.yaml");
        let result = server
            .handler_generate(Parameters(HandlerGenerateParams {
                template_yaml: yaml.to_string(),
                language: "python".into(),
                step_filter: None,
                scaffold: Some(true),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["language"], "python");
        assert!(parsed["types"].as_str().unwrap().contains("class"));
        assert!(parsed["handlers"].as_str().unwrap().contains("def"));
        assert!(parsed["tests"].as_str().unwrap().contains("def test_"));
    }

    // ── Profile management tool tests ──

    #[tokio::test]
    async fn test_connection_status_offline() {
        let server = TaskerMcpServer::offline();
        let result = server
            .connection_status(Parameters(ConnectionStatusParams { refresh: None }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["mode"], "offline");
        assert!(parsed["profiles"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_connection_status_with_profiles() {
        let toml_content = r#"
[profile.default]
description = "Local development"
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"
"#;
        let file: tasker_client::config::ProfileConfigFile = toml::from_str(toml_content).unwrap();
        let pm = ProfileManager::from_profile_file_for_test(file);
        let server = TaskerMcpServer::with_profile_manager(pm, false);

        let result = server
            .connection_status(Parameters(ConnectionStatusParams { refresh: None }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["mode"], "connected");
        assert_eq!(parsed["active_profile"], "default");

        let profiles = parsed["profiles"].as_array().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0]["name"], "default");
        assert_eq!(profiles[0]["description"], "Local development");
        assert_eq!(profiles[0]["is_active"], true);
    }

    #[tokio::test]
    async fn test_use_environment_offline() {
        let server = TaskerMcpServer::offline();
        let result = server
            .use_environment(Parameters(UseEnvironmentParams {
                profile: "staging".to_string(),
                probe_health: Some(false),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "offline_mode");
    }

    #[tokio::test]
    async fn test_use_environment_switch() {
        let toml_content = r#"
[profile.default]
transport = "rest"

[profile.grpc]
transport = "grpc"
"#;
        let file: tasker_client::config::ProfileConfigFile = toml::from_str(toml_content).unwrap();
        let pm = ProfileManager::from_profile_file_for_test(file);
        let server = TaskerMcpServer::with_profile_manager(pm, false);

        let result = server
            .use_environment(Parameters(UseEnvironmentParams {
                profile: "grpc".to_string(),
                probe_health: Some(false), // Skip health probe in test
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["switched_to"], "grpc");
        assert_eq!(parsed["profile"]["name"], "grpc");
        assert_eq!(parsed["profile"]["is_active"], true);
    }

    #[tokio::test]
    async fn test_use_environment_not_found() {
        let toml_content = r#"
[profile.default]
transport = "rest"
"#;
        let file: tasker_client::config::ProfileConfigFile = toml::from_str(toml_content).unwrap();
        let pm = ProfileManager::from_profile_file_for_test(file);
        let server = TaskerMcpServer::with_profile_manager(pm, false);

        let result = server
            .use_environment(Parameters(UseEnvironmentParams {
                profile: "nonexistent".to_string(),
                probe_health: Some(false),
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "profile_not_found");
    }
}
