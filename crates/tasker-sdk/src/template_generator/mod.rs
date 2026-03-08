//! Structured spec → TaskTemplate YAML generation.
//!
//! Converts a high-level [`TemplateSpec`] into a valid task template YAML string,
//! building JSON Schema `result_schema` from field specifications.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

/// Specification for generating a task template.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TemplateSpec {
    /// Task name.
    pub name: String,
    /// Namespace for organization.
    pub namespace: String,
    /// Semantic version (defaults to "1.0.0").
    pub version: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Step definitions.
    pub steps: Vec<StepSpec>,
}

/// Specification for a single step.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StepSpec {
    /// Step name.
    pub name: String,
    /// Step description.
    pub description: Option<String>,
    /// Handler callable (auto-generated as `{namespace}.{name}` if omitted).
    pub handler: Option<String>,
    /// Dependencies on other steps.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Output fields that form the result_schema.
    #[serde(default)]
    pub outputs: Vec<FieldSpec>,
}

/// Specification for a single output field.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FieldSpec {
    /// Field name.
    pub name: String,
    /// Field type: string, integer, number, boolean, array:T, object.
    pub field_type: String,
    /// Whether this field is required.
    #[serde(default = "default_true")]
    pub required: bool,
    /// Field description.
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Error during template generation.
#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error("YAML serialization failed: {0}")]
    YamlSerialization(#[from] serde_yaml::Error),
    #[error("unsupported field type: '{0}'")]
    UnsupportedFieldType(String),
}

/// Generate a task template YAML string from a structured specification.
pub fn generate_yaml(spec: &TemplateSpec) -> Result<String, GenerateError> {
    let version = spec.version.clone().unwrap_or_else(|| "1.0.0".into());

    let steps: Vec<Value> = spec
        .steps
        .iter()
        .map(|step| build_step_value(spec, step))
        .collect::<Result<Vec<_>, _>>()?;

    let mut template = Map::new();
    template.insert("name".into(), json!(spec.name));
    template.insert("namespace_name".into(), json!(spec.namespace));
    template.insert("version".into(), json!(version));

    if let Some(desc) = &spec.description {
        template.insert("description".into(), json!(desc));
    }

    template.insert("steps".into(), json!(steps));

    let yaml = serde_yaml::to_string(&template)?;
    Ok(yaml)
}

fn build_step_value(spec: &TemplateSpec, step: &StepSpec) -> Result<Value, GenerateError> {
    let callable = step
        .handler
        .clone()
        .unwrap_or_else(|| format!("{}.{}", spec.namespace, step.name));

    let mut step_map = Map::new();
    step_map.insert("name".into(), json!(step.name));

    if let Some(desc) = &step.description {
        step_map.insert("description".into(), json!(desc));
    }

    step_map.insert("handler".into(), json!({ "callable": callable }));

    if !step.depends_on.is_empty() {
        step_map.insert("depends_on".into(), json!(step.depends_on));
    }

    if !step.outputs.is_empty() {
        let schema = build_result_schema(&step.outputs)?;
        step_map.insert("result_schema".into(), schema);
    }

    Ok(Value::Object(step_map))
}

fn build_result_schema(fields: &[FieldSpec]) -> Result<Value, GenerateError> {
    let mut properties = Map::new();
    let mut required = Vec::new();

    for field in fields {
        let type_schema = field_type_to_schema(&field.field_type)?;
        let mut field_schema = match type_schema {
            Value::Object(m) => m,
            _ => {
                let mut m = Map::new();
                m.insert("type".into(), type_schema);
                m
            }
        };

        if let Some(desc) = &field.description {
            field_schema.insert("description".into(), json!(desc));
        }

        properties.insert(field.name.clone(), Value::Object(field_schema));

        if field.required {
            required.push(json!(field.name));
        }
    }

    let mut schema = Map::new();
    schema.insert("type".into(), json!("object"));
    if !required.is_empty() {
        schema.insert("required".into(), Value::Array(required));
    }
    schema.insert("properties".into(), Value::Object(properties));

    Ok(Value::Object(schema))
}

fn field_type_to_schema(field_type: &str) -> Result<Value, GenerateError> {
    match field_type {
        "string" => Ok(json!({"type": "string"})),
        "integer" => Ok(json!({"type": "integer"})),
        "number" => Ok(json!({"type": "number"})),
        "boolean" => Ok(json!({"type": "boolean"})),
        "object" => Ok(json!({"type": "object"})),
        _ if field_type.starts_with("array:") => {
            let item_type = &field_type[6..];
            let items_schema = field_type_to_schema(item_type)?;
            Ok(json!({
                "type": "array",
                "items": items_schema
            }))
        }
        _ => Err(GenerateError::UnsupportedFieldType(field_type.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_parser::parse_template_str;
    use crate::template_validator::validate;

    #[test]
    fn test_minimal_spec_generates_valid_yaml() {
        let spec = TemplateSpec {
            name: "test_task".into(),
            namespace: "test_ns".into(),
            version: None,
            description: Some("A test task".into()),
            steps: vec![StepSpec {
                name: "step_one".into(),
                description: Some("First step".into()),
                handler: None,
                depends_on: vec![],
                outputs: vec![FieldSpec {
                    name: "result".into(),
                    field_type: "string".into(),
                    required: true,
                    description: Some("The result".into()),
                }],
            }],
        };

        let yaml = generate_yaml(&spec).unwrap();
        assert!(yaml.contains("test_task"));
        assert!(yaml.contains("test_ns"));

        // Verify it parses back
        let template = parse_template_str(&yaml).unwrap();
        assert_eq!(template.name, "test_task");
        assert_eq!(template.steps.len(), 1);
    }

    #[test]
    fn test_auto_generated_callables() {
        let spec = TemplateSpec {
            name: "my_task".into(),
            namespace: "my_ns".into(),
            version: Some("2.0.0".into()),
            description: None,
            steps: vec![
                StepSpec {
                    name: "fetch_data".into(),
                    description: None,
                    handler: None, // Should auto-generate as "my_ns.fetch_data"
                    depends_on: vec![],
                    outputs: vec![],
                },
                StepSpec {
                    name: "process".into(),
                    description: None,
                    handler: Some("custom.handler".into()),
                    depends_on: vec!["fetch_data".into()],
                    outputs: vec![],
                },
            ],
        };

        let yaml = generate_yaml(&spec).unwrap();
        assert!(yaml.contains("my_ns.fetch_data"));
        assert!(yaml.contains("custom.handler"));

        let template = parse_template_str(&yaml).unwrap();
        assert_eq!(template.steps[0].handler.callable, "my_ns.fetch_data");
        assert_eq!(template.steps[1].handler.callable, "custom.handler");
    }

    #[test]
    fn test_round_trip_generate_parse_validate() {
        let spec = TemplateSpec {
            name: "round_trip".into(),
            namespace: "rt".into(),
            version: Some("1.0.0".into()),
            description: Some("Round trip test".into()),
            steps: vec![
                StepSpec {
                    name: "validate".into(),
                    description: Some("Validate input".into()),
                    handler: None,
                    depends_on: vec![],
                    outputs: vec![
                        FieldSpec {
                            name: "valid".into(),
                            field_type: "boolean".into(),
                            required: true,
                            description: None,
                        },
                        FieldSpec {
                            name: "score".into(),
                            field_type: "number".into(),
                            required: false,
                            description: Some("Validation score".into()),
                        },
                    ],
                },
                StepSpec {
                    name: "transform".into(),
                    description: None,
                    handler: None,
                    depends_on: vec!["validate".into()],
                    outputs: vec![FieldSpec {
                        name: "items".into(),
                        field_type: "array:string".into(),
                        required: true,
                        description: None,
                    }],
                },
            ],
        };

        let yaml = generate_yaml(&spec).unwrap();
        let template = parse_template_str(&yaml).unwrap();
        let report = validate(&template);

        assert!(
            report.valid,
            "Generated template should be valid: {:?}",
            report.findings
        );
        assert!(!report.has_cycles);
        assert_eq!(report.step_count, 2);
    }

    #[test]
    fn test_unsupported_field_type() {
        let spec = TemplateSpec {
            name: "bad".into(),
            namespace: "ns".into(),
            version: None,
            description: None,
            steps: vec![StepSpec {
                name: "s".into(),
                description: None,
                handler: None,
                depends_on: vec![],
                outputs: vec![FieldSpec {
                    name: "f".into(),
                    field_type: "unsupported_type".into(),
                    required: true,
                    description: None,
                }],
            }],
        };

        let result = generate_yaml(&spec);
        assert!(result.is_err());
    }

    #[test]
    fn test_array_type_generation() {
        let spec = TemplateSpec {
            name: "arr_test".into(),
            namespace: "ns".into(),
            version: None,
            description: None,
            steps: vec![StepSpec {
                name: "step_a".into(),
                description: None,
                handler: None,
                depends_on: vec![],
                outputs: vec![FieldSpec {
                    name: "tags".into(),
                    field_type: "array:string".into(),
                    required: true,
                    description: None,
                }],
            }],
        };

        let yaml = generate_yaml(&spec).unwrap();
        let template = parse_template_str(&yaml).unwrap();
        let schema = template.steps[0].result_schema.as_ref().unwrap();
        let items = schema
            .get("properties")
            .unwrap()
            .get("tags")
            .unwrap()
            .get("items")
            .unwrap();
        assert_eq!(items.get("type").unwrap(), "string");
    }
}
