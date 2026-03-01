//! Tier 1 — Developer Tooling (offline) tool implementations.
//!
//! Pure functions that take param structs and return JSON strings.
//! No async, no client, no `&self` — these work entirely offline.

use std::collections::{HashMap, HashSet};

use tasker_sdk::codegen::{self, TargetLanguage};
use tasker_sdk::schema_comparator;
use tasker_sdk::schema_diff;
use tasker_sdk::schema_inspector;
use tasker_sdk::template_generator;
use tasker_sdk::template_parser::parse_template_str;

use super::helpers::{error_json, topological_sort};
use super::params::{
    FieldDetail, HandlerGenerateParams, HandlerGenerateResponse, SchemaCompareParams,
    SchemaDiffParams, SchemaInspectParams, SchemaInspectResponse, StepInspection, StepSchemaDetail,
    TemplateGenerateParams, TemplateInspectParams, TemplateInspectResponse, TemplateValidateParams,
};

pub fn template_validate(params: TemplateValidateParams) -> String {
    match parse_template_str(&params.template_yaml) {
        Ok(template) => {
            let report = tasker_sdk::template_validator::validate(&template);
            serde_json::to_string_pretty(&report)
                .unwrap_or_else(|e| error_json("serialization_error", &e.to_string()))
        }
        Err(e) => error_json("yaml_parse_error", &e.to_string()),
    }
}

pub fn template_inspect(params: TemplateInspectParams) -> String {
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

pub fn template_generate(params: TemplateGenerateParams) -> String {
    let spec: tasker_sdk::template_generator::TemplateSpec = params.into();
    match template_generator::generate_yaml(&spec) {
        Ok(yaml) => yaml,
        Err(e) => error_json("generation_error", &e.to_string()),
    }
}

pub fn handler_generate(params: HandlerGenerateParams) -> String {
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

pub fn schema_inspect(params: SchemaInspectParams) -> String {
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

pub fn schema_compare(params: SchemaCompareParams) -> String {
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

pub fn schema_diff(params: SchemaDiffParams) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::params::{FieldSpecParam, StepSpecParam};

    #[test]
    fn test_template_validate_valid() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = template_validate(TemplateValidateParams {
            template_yaml: yaml.to_string(),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["valid"], true);
        assert_eq!(parsed["step_count"], 5);
    }

    #[test]
    fn test_template_validate_cycle() {
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
        let result = template_validate(TemplateValidateParams {
            template_yaml: yaml.to_string(),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["valid"], false);
        assert_eq!(parsed["has_cycles"], true);
    }

    #[test]
    fn test_template_validate_invalid_yaml() {
        let result = template_validate(TemplateValidateParams {
            template_yaml: "not: [valid: yaml".to_string(),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "yaml_parse_error");
    }

    #[test]
    fn test_template_inspect() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = template_inspect(TemplateInspectParams {
            template_yaml: yaml.to_string(),
        });
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

    #[test]
    fn test_template_generate() {
        let result = template_generate(TemplateGenerateParams {
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
        });
        assert!(result.contains("test_task"));
        assert!(result.contains("ns.step_one"));
    }

    #[test]
    fn test_handler_generate_scaffold() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = handler_generate(HandlerGenerateParams {
            template_yaml: yaml.to_string(),
            language: "python".into(),
            step_filter: Some("validate_order".into()),
            scaffold: Some(true),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["language"], "python");
        assert!(parsed["types"].as_str().unwrap().contains("class"));
        assert!(parsed["handlers"].as_str().unwrap().contains("def"));
        assert!(parsed["handlers"]
            .as_str()
            .unwrap()
            .contains("from .models import"));
    }

    #[test]
    fn test_handler_generate_no_scaffold() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = handler_generate(HandlerGenerateParams {
            template_yaml: yaml.to_string(),
            language: "python".into(),
            step_filter: Some("validate_order".into()),
            scaffold: Some(false),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["language"], "python");
        assert!(!parsed["handlers"]
            .as_str()
            .unwrap()
            .contains("from .models import"));
    }

    #[test]
    fn test_handler_generate_invalid_language() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = handler_generate(HandlerGenerateParams {
            template_yaml: yaml.to_string(),
            language: "cobol".into(),
            step_filter: None,
            scaffold: None,
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "invalid_language");
    }

    #[test]
    fn test_schema_inspect() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = schema_inspect(SchemaInspectParams {
            template_yaml: yaml.to_string(),
            step_filter: Some("validate_order".into()),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["template_name"], "codegen_test");
        let steps = parsed["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 1);
        assert!(steps[0]["has_result_schema"].as_bool().unwrap());
        assert!(!steps[0]["fields"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_schema_compare() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = schema_compare(SchemaCompareParams {
            template_yaml: yaml.to_string(),
            producer_step: "validate_order".into(),
            consumer_step: "enrich_order".into(),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["compatibility"].is_string());
        assert!(parsed["findings"].is_array());
    }

    #[test]
    fn test_schema_compare_step_not_found() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let result = schema_compare(SchemaCompareParams {
            template_yaml: yaml.to_string(),
            producer_step: "nonexistent".into(),
            consumer_step: "enrich_order".into(),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["error"], "step_not_found");
    }

    #[test]
    fn test_schema_diff() {
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
        let result = schema_diff(SchemaDiffParams {
            before_yaml: before_yaml.to_string(),
            after_yaml: after_yaml.to_string(),
            step_filter: None,
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["compatibility"], "incompatible");
        let diffs = parsed["step_diffs"].as_array().unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0]["status"], "modified");

        let findings = diffs[0]["findings"].as_array().unwrap();
        assert!(findings.iter().any(|f| f["code"] == "FIELD_ADDED"));
        assert!(findings
            .iter()
            .any(|f| f["code"] == "FIELD_REMOVED" && f["breaking"] == true));
    }

    #[test]
    fn test_content_publishing_template_validates() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/content_publishing_template.yaml");
        let result = template_validate(TemplateValidateParams {
            template_yaml: yaml.to_string(),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["valid"], true);
        assert_eq!(parsed["step_count"], 7);
    }

    #[test]
    fn test_content_publishing_template_inspect() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/content_publishing_template.yaml");
        let result = template_inspect(TemplateInspectParams {
            template_yaml: yaml.to_string(),
        });
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

    #[test]
    fn test_content_publishing_handler_generate() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/content_publishing_template.yaml");
        let result = handler_generate(HandlerGenerateParams {
            template_yaml: yaml.to_string(),
            language: "python".into(),
            step_filter: None,
            scaffold: Some(true),
        });
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["language"], "python");
        assert!(parsed["types"].as_str().unwrap().contains("class"));
        assert!(parsed["handlers"].as_str().unwrap().contains("def"));
        assert!(parsed["tests"].as_str().unwrap().contains("def test_"));
    }
}
