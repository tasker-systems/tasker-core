//! Result schema contract inspection utilities.
//!
//! Provides functions for inspecting and comparing `result_schema` definitions
//! across task template steps. Used for compatibility checking and schema
//! evolution analysis.

use serde_json::Value;
use tasker_shared::models::core::task_template::TaskTemplate;

/// Summary of schema presence across a task template's steps.
#[derive(Debug)]
pub struct SchemaReport {
    /// Template name.
    pub template_name: String,
    /// Whether the template has an input_schema.
    pub has_input_schema: bool,
    /// Per-step schema presence.
    pub steps: Vec<StepSchemaInfo>,
}

/// Schema presence information for a single step.
#[derive(Debug)]
pub struct StepSchemaInfo {
    /// Step name.
    pub name: String,
    /// Whether this step has a result_schema defined.
    pub has_result_schema: bool,
    /// Number of top-level properties in the result_schema, if present.
    pub property_count: Option<usize>,
}

/// Inspect a task template and return a summary of its schema definitions.
pub fn inspect(template: &TaskTemplate) -> SchemaReport {
    let steps = template
        .steps
        .iter()
        .map(|step| {
            let property_count = step.result_schema.as_ref().and_then(|schema| {
                schema
                    .get("properties")
                    .and_then(Value::as_object)
                    .map(|props| props.len())
            });

            StepSchemaInfo {
                name: step.name.clone(),
                has_result_schema: step.result_schema.is_some(),
                property_count,
            }
        })
        .collect();

    SchemaReport {
        template_name: template.name.clone(),
        has_input_schema: template.input_schema.is_some(),
        steps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspect_codegen_template() {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template =
            tasker_shared::models::core::task_template::TaskTemplate::from_yaml(yaml).unwrap();

        let report = inspect(&template);
        assert_eq!(report.template_name, "codegen_test");
        assert!(report.has_input_schema);
        assert!(!report.steps.is_empty());

        // validate_order has result_schema
        let validate = report
            .steps
            .iter()
            .find(|s| s.name == "validate_order")
            .unwrap();
        assert!(validate.has_result_schema);
        assert!(validate.property_count.unwrap() > 0);

        // process_payment has no result_schema
        let payment = report
            .steps
            .iter()
            .find(|s| s.name == "process_payment")
            .unwrap();
        assert!(!payment.has_result_schema);
        assert!(payment.property_count.is_none());
    }
}
