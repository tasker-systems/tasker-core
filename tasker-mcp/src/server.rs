//! MCP ServerHandler implementation for Tasker.
//!
//! Provides the MCP server with 6 developer tooling tools:
//! - `template_validate` — Validate a task template for structural correctness
//! - `template_inspect` — Inspect template DAG structure and step details
//! - `template_generate` — Generate task template YAML from a structured spec
//! - `handler_generate` — Generate typed handler code for a template
//! - `schema_inspect` — Inspect result_schema field details per step
//! - `schema_compare` — Compare producer/consumer schema compatibility

use std::collections::{HashMap, HashSet};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};

use tasker_tooling::codegen::{self, TargetLanguage};
use tasker_tooling::schema_comparator;
use tasker_tooling::schema_inspector;
use tasker_tooling::template_generator;
use tasker_tooling::template_parser::parse_template_str;
use tasker_tooling::template_validator;

use crate::tools::*;

/// Tasker MCP server handler with 6 developer tooling tools.
#[derive(Debug, Clone)]
pub struct TaskerMcpServer {
    tool_router: ToolRouter<Self>,
}

impl TaskerMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TaskerMcpServer {
    fn get_info(&self) -> ServerInfo {
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
            instructions: Some(
                "Tasker is a workflow orchestration system. You help developers create and validate \
                 task templates (workflow definitions) and generate typed handler code.\n\
                 Workflow: template_generate → template_validate → handler_generate (per step)\n\
                 When debugging: template_inspect → schema_inspect → schema_compare"
                    .to_string(),
            ),
        }
    }
}

#[tool_router(router = tool_router)]
impl TaskerMcpServer {
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
        description = "Generate typed handler code for a task template. Returns types, handler scaffolds, and test files for the specified language (python, ruby, typescript, rust)."
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
        };

        serde_json::to_string_pretty(&response)
            .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
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
                                        field_type: format!("{:?}", f.field_type),
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
    fn test_server_info() {
        let server = TaskerMcpServer::new();
        let info = server.get_info();

        assert_eq!(info.server_info.name, "tasker-mcp");
        assert_eq!(info.server_info.version, env!("CARGO_PKG_VERSION"));
        assert!(info.instructions.is_some());
        assert!(info.instructions.unwrap().contains("template_generate"));
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
            }))
            .await;
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["language"], "python");
        assert!(parsed["types"].as_str().unwrap().contains("class"));
        assert!(parsed["handlers"].as_str().unwrap().contains("def"));
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
}
