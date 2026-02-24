//! Handler scaffold intermediate representation and extraction from task templates.
//!
//! Converts `TaskTemplate` step definitions into `HandlerDef` structures that
//! Askama templates consume to generate runnable handler scaffolds per language.

use std::collections::HashMap;

use heck::{ToLowerCamelCase, ToSnakeCase, ToUpperCamelCase};
use serde_json::Value;

use super::schema::{self, FieldType};
use tasker_shared::models::core::task_template::TaskTemplate;

/// A handler scaffold for a single step.
#[derive(Debug, Clone)]
pub struct HandlerDef {
    /// Step name from YAML (e.g., "validate_order")
    pub step_name: String,
    /// Handler callable from YAML (e.g., "codegen_tests.validate_order")
    pub callable: String,
    /// Description from step definition
    pub description: Option<String>,
    /// Upstream dependencies with their result types
    pub dependencies: Vec<DependencyRef>,
    /// Whether this step has a result_schema (typed result available)
    pub has_result_schema: bool,
    /// Stub return fields derived from result_schema (field_name, stub_value pairs)
    pub stub_fields: Vec<StubField>,
}

/// A reference to an upstream step's result.
#[derive(Debug, Clone)]
pub struct DependencyRef {
    /// Upstream step name (e.g., "validate_order")
    pub step_name: String,
    /// Result type if upstream has result_schema (e.g., "ValidateOrderResult")
    pub result_type: Option<String>,
}

/// A stub field for generating placeholder return values.
#[derive(Debug, Clone)]
pub struct StubField {
    pub name: String,
    pub field_type: FieldType,
}

// =========================================================================
// Rendering helpers (called from Askama templates)
// =========================================================================

impl HandlerDef {
    /// PascalCase handler name (e.g., "ValidateOrderHandler").
    pub fn pascal_name(&self) -> String {
        format!("{}Handler", self.step_name.to_upper_camel_case())
    }

    /// snake_case step name (e.g., "validate_order").
    pub fn snake_name(&self) -> String {
        self.step_name.to_snake_case()
    }

    /// The PascalCase result type name if result_schema exists (e.g., "ValidateOrderResult").
    #[allow(
        dead_code,
        reason = "public API for handler IR consumers; exercised in tests"
    )]
    pub fn result_type_name(&self) -> Option<String> {
        if self.has_result_schema {
            Some(schema::to_pascal_result_name(&self.step_name))
        } else {
            None
        }
    }

    /// Check if this handler has any dependencies.
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }
}

impl DependencyRef {
    /// camelCase parameter name for TypeScript (e.g., "validateOrderResult").
    pub fn camel_param(&self) -> String {
        format!("{}_result", self.step_name).to_lower_camel_case()
    }

    /// snake_case parameter name with `_result` suffix (e.g., "validate_order_result").
    pub fn snake_param(&self) -> String {
        format!("{}_result", self.step_name.to_snake_case())
    }

    /// PascalCase parameter name for test variable names (e.g., "ValidateOrderResult").
    pub fn pascal_param(&self) -> String {
        format!("{}_result", self.step_name).to_upper_camel_case()
    }

    /// Type comment string showing the result type or "untyped".
    pub fn type_comment(&self) -> String {
        match &self.result_type {
            Some(t) => format!("{t} (typed)"),
            None => "untyped".to_string(),
        }
    }
}

impl StubField {
    /// Python stub value for this field type.
    pub fn python_value(&self) -> String {
        stub_value_python(&self.field_type)
    }

    /// Ruby stub value for this field type.
    pub fn ruby_value(&self) -> String {
        stub_value_ruby(&self.field_type)
    }

    /// TypeScript stub value for this field type.
    pub fn typescript_value(&self) -> String {
        stub_value_typescript(&self.field_type)
    }

    /// JSON stub value (for serde_json::json! macro and generic mocks).
    pub fn json_value(&self) -> String {
        stub_value_json(&self.field_type)
    }
}

// =========================================================================
// Stub value generation
// =========================================================================

fn stub_value_json(ft: &FieldType) -> String {
    match ft {
        FieldType::String | FieldType::StringEnum(_) => "\"\"".to_string(),
        FieldType::Integer => "0".to_string(),
        FieldType::Number => "0.0".to_string(),
        FieldType::Boolean => "false".to_string(),
        FieldType::Array(_) => "[]".to_string(),
        FieldType::Nested(_) | FieldType::Any => "{}".to_string(),
    }
}

fn stub_value_python(ft: &FieldType) -> String {
    match ft {
        FieldType::String | FieldType::StringEnum(_) => "\"\"".to_string(),
        FieldType::Integer => "0".to_string(),
        FieldType::Number => "0.0".to_string(),
        FieldType::Boolean => "False".to_string(),
        FieldType::Array(_) => "[]".to_string(),
        FieldType::Nested(_) | FieldType::Any => "{}".to_string(),
    }
}

fn stub_value_ruby(ft: &FieldType) -> String {
    match ft {
        FieldType::String | FieldType::StringEnum(_) => "\"\"".to_string(),
        FieldType::Integer => "0".to_string(),
        FieldType::Number => "0.0".to_string(),
        FieldType::Boolean => "false".to_string(),
        FieldType::Array(_) => "[]".to_string(),
        FieldType::Nested(_) | FieldType::Any => "{}".to_string(),
    }
}

fn stub_value_typescript(ft: &FieldType) -> String {
    match ft {
        FieldType::String | FieldType::StringEnum(_) => "\"\"".to_string(),
        FieldType::Integer | FieldType::Number => "0".to_string(),
        FieldType::Boolean => "false".to_string(),
        FieldType::Array(_) => "[]".to_string(),
        FieldType::Nested(_) | FieldType::Any => "{}".to_string(),
    }
}

// =========================================================================
// Extraction
// =========================================================================

/// Extract handler definitions from a task template.
///
/// Builds `HandlerDef` for each step (optionally filtered by name).
/// For each dependency, looks up whether the upstream step has `result_schema`
/// and derives the typed result name.
pub fn extract_handlers(template: &TaskTemplate, step_filter: Option<&str>) -> Vec<HandlerDef> {
    // Build a map of step_name → has result_schema
    let schema_map: HashMap<&str, bool> = template
        .steps
        .iter()
        .map(|s| (s.name.as_str(), s.result_schema.is_some()))
        .collect();

    template
        .steps
        .iter()
        .filter(|step| step_filter.map(|f| step.name == f).unwrap_or(true))
        .map(|step| {
            let dependencies: Vec<DependencyRef> = step
                .dependencies
                .iter()
                .map(|dep_name| {
                    let has_schema = schema_map.get(dep_name.as_str()).copied().unwrap_or(false);
                    DependencyRef {
                        step_name: dep_name.clone(),
                        result_type: if has_schema {
                            Some(schema::to_pascal_result_name(dep_name))
                        } else {
                            None
                        },
                    }
                })
                .collect();

            let has_result_schema = step.result_schema.is_some();

            let stub_fields = step
                .result_schema
                .as_ref()
                .map(extract_stub_fields)
                .unwrap_or_default();

            HandlerDef {
                step_name: step.name.clone(),
                callable: step.handler.callable.clone(),
                description: step.description.clone(),
                dependencies,
                has_result_schema,
                stub_fields,
            }
        })
        .collect()
}

/// Extract stub fields from a result_schema for placeholder return values.
fn extract_stub_fields(schema: &Value) -> Vec<StubField> {
    let properties = schema.get("properties").and_then(|v| v.as_object());

    match properties {
        Some(props) => props
            .iter()
            .map(|(name, prop_schema)| {
                let field_type = resolve_simple_type(prop_schema);
                StubField {
                    name: name.clone(),
                    field_type,
                }
            })
            .collect(),
        None => vec![],
    }
}

/// Resolve a simple FieldType from a property schema (no nested extraction needed).
fn resolve_simple_type(schema: &Value) -> FieldType {
    match schema.get("type").and_then(|v| v.as_str()) {
        Some("string") => FieldType::String,
        Some("integer") => FieldType::Integer,
        Some("number") => FieldType::Number,
        Some("boolean") => FieldType::Boolean,
        Some("array") => FieldType::Array(Box::new(FieldType::Any)),
        Some("object") => FieldType::Nested("Object".to_string()),
        _ => FieldType::Any,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codegen_test_template() -> TaskTemplate {
        let yaml =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        TaskTemplate::from_yaml(yaml).expect("fixture should parse")
    }

    #[test]
    fn test_extract_handlers_all_steps() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, None);

        assert_eq!(handlers.len(), 4);
        assert_eq!(handlers[0].step_name, "validate_order");
        assert_eq!(handlers[1].step_name, "enrich_order");
        assert_eq!(handlers[2].step_name, "process_payment");
        assert_eq!(handlers[3].step_name, "generate_report");
    }

    #[test]
    fn test_step_with_no_dependencies() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, None);

        let validate = &handlers[0];
        assert!(validate.dependencies.is_empty());
        assert_eq!(validate.callable, "codegen_tests.validate_order");
        assert_eq!(
            validate.description.as_deref(),
            Some("Validates an incoming order")
        );
    }

    #[test]
    fn test_step_with_typed_dependency() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, None);

        // enrich_order depends on validate_order (which has result_schema)
        let enrich = &handlers[1];
        assert_eq!(enrich.dependencies.len(), 1);
        assert_eq!(enrich.dependencies[0].step_name, "validate_order");
        assert_eq!(
            enrich.dependencies[0].result_type.as_deref(),
            Some("ValidateOrderResult")
        );
    }

    #[test]
    fn test_step_with_untyped_dependency() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, None);

        // generate_report depends on enrich_order (typed) and process_payment (no result_schema)
        let report = &handlers[3];
        assert_eq!(report.dependencies.len(), 2);

        let enrich_dep = report
            .dependencies
            .iter()
            .find(|d| d.step_name == "enrich_order")
            .unwrap();
        assert!(enrich_dep.result_type.is_some());

        let payment_dep = report
            .dependencies
            .iter()
            .find(|d| d.step_name == "process_payment")
            .unwrap();
        assert!(payment_dep.result_type.is_none());
    }

    #[test]
    fn test_step_filter() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, Some("enrich_order"));

        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].step_name, "enrich_order");
    }

    #[test]
    fn test_result_schema_detection() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, None);

        assert!(handlers[0].has_result_schema); // validate_order
        assert!(handlers[1].has_result_schema); // enrich_order
        assert!(!handlers[2].has_result_schema); // process_payment
        assert!(handlers[3].has_result_schema); // generate_report
    }

    #[test]
    fn test_result_type_names() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, None);

        assert_eq!(
            handlers[0].result_type_name().as_deref(),
            Some("ValidateOrderResult")
        );
        assert_eq!(
            handlers[1].result_type_name().as_deref(),
            Some("EnrichOrderResult")
        );
        assert!(handlers[2].result_type_name().is_none());
        assert_eq!(
            handlers[3].result_type_name().as_deref(),
            Some("GenerateReportResult")
        );
    }

    #[test]
    fn test_stub_fields_populated_for_typed_step() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, None);

        let validate = &handlers[0];
        assert!(!validate.stub_fields.is_empty());
        assert!(validate.stub_fields.iter().any(|f| f.name == "validated"));
        assert!(validate.stub_fields.iter().any(|f| f.name == "order_total"));
    }

    #[test]
    fn test_stub_fields_empty_for_untyped_step() {
        let template = codegen_test_template();
        let handlers = extract_handlers(&template, None);

        let payment = &handlers[2];
        assert!(payment.stub_fields.is_empty());
    }

    #[test]
    fn test_pascal_name() {
        let handler = HandlerDef {
            step_name: "validate_order".to_string(),
            callable: String::new(),
            description: None,
            dependencies: vec![],
            has_result_schema: false,
            stub_fields: vec![],
        };
        assert_eq!(handler.pascal_name(), "ValidateOrderHandler");
    }

    #[test]
    fn test_dependency_ref_helpers() {
        let dep = DependencyRef {
            step_name: "validate_order".to_string(),
            result_type: Some("ValidateOrderResult".to_string()),
        };
        assert_eq!(dep.snake_param(), "validate_order_result");
        assert_eq!(dep.camel_param(), "validateOrderResult");
        assert_eq!(dep.type_comment(), "ValidateOrderResult (typed)");

        let untyped = DependencyRef {
            step_name: "process_payment".to_string(),
            result_type: None,
        };
        assert_eq!(untyped.type_comment(), "untyped");
    }
}
