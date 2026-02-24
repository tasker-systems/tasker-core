//! JSON Schema to intermediate type representation.
//!
//! Converts JSON Schema `Value` objects from `result_schema` fields into
//! language-agnostic `TypeDef` structures that language generators consume.

use serde_json::Value;
use std::fmt;

/// A resolved type definition extracted from a JSON Schema.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDef {
    /// PascalCase type name (e.g., `ValidateOrderResult`)
    pub name: String,
    /// Fields of this type
    pub fields: Vec<FieldDef>,
    /// Description from JSON Schema, if any
    pub description: Option<String>,
}

/// A single field within a type definition.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDef {
    /// Original property name from JSON Schema
    pub name: String,
    /// Resolved type
    pub field_type: FieldType,
    /// Whether this field is required (present in `required` array)
    pub required: bool,
    /// Description from JSON Schema, if any
    pub description: Option<String>,
}

/// Language-agnostic type representation.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    String,
    Integer,
    Number,
    Boolean,
    Array(Box<FieldType>),
    /// References another TypeDef by name
    Nested(String),
    /// A string with a fixed set of allowed values
    StringEnum(Vec<String>),
    /// Fallback for unrecognized or unsupported schema constructs
    Any,
}

impl fmt::Display for FieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldType::String => write!(f, "String"),
            FieldType::Integer => write!(f, "Integer"),
            FieldType::Number => write!(f, "Number"),
            FieldType::Boolean => write!(f, "Boolean"),
            FieldType::Array(inner) => write!(f, "Array<{inner}>"),
            FieldType::Nested(name) => write!(f, "{name}"),
            FieldType::StringEnum(ref values) => {
                write!(f, "Enum({})", values.join(", "))
            }
            FieldType::Any => write!(f, "Any"),
        }
    }
}

// =========================================================================
// Language-specific rendering helpers (called from Askama templates)
// =========================================================================

impl FieldDef {
    /// Field name in snake_case (for Python, Rust, Ruby attribute names).
    pub fn snake_name(&self) -> String {
        use heck::ToSnakeCase;
        self.name.to_snake_case()
    }

    /// Field name escaped for Rust (prefixes reserved keywords with `r#`).
    pub fn rust_name(&self) -> String {
        let name = self.snake_name();
        match name.as_str() {
            "type" | "struct" | "enum" | "fn" | "let" | "mut" | "ref" | "self" | "super"
            | "crate" | "mod" | "pub" | "use" | "impl" | "trait" | "where" | "async" | "await"
            | "move" | "return" | "match" | "if" | "else" | "loop" | "for" | "while" | "break"
            | "continue" | "in" | "as" | "const" | "static" | "extern" | "unsafe" | "dyn"
            | "abstract" | "become" | "box" | "do" | "final" | "macro" | "override" | "priv"
            | "typeof" | "unsized" | "virtual" | "yield" | "try" => format!("r#{name}"),
            _ => name,
        }
    }

    /// Whether this field name is a Rust reserved keyword (needing `r#` prefix).
    pub fn is_rust_keyword(&self) -> bool {
        self.rust_name() != self.snake_name()
    }
}

impl FieldType {
    /// Python type annotation.
    pub fn python_type(&self) -> String {
        match self {
            FieldType::String => "str".to_string(),
            FieldType::Integer => "int".to_string(),
            FieldType::Number => "float".to_string(),
            FieldType::Boolean => "bool".to_string(),
            FieldType::Array(inner) => format!("list[{}]", inner.python_type()),
            FieldType::Nested(name) => name.clone(),
            FieldType::StringEnum(_) => "str".to_string(),
            FieldType::Any => "Any".to_string(),
        }
    }

    /// Ruby Dry::Types type.
    pub fn ruby_type(&self) -> String {
        match self {
            FieldType::String => "Types::Strict::String".to_string(),
            FieldType::Integer => "Types::Strict::Integer".to_string(),
            FieldType::Number => "Types::Strict::Float".to_string(),
            FieldType::Boolean => "Types::Strict::Bool".to_string(),
            FieldType::Array(inner) => format!("Types::Strict::Array.of({})", inner.ruby_type()),
            FieldType::Nested(name) => name.clone(),
            FieldType::StringEnum(_) => "Types::Strict::String".to_string(),
            FieldType::Any => "Types::Nominal::Any".to_string(),
        }
    }

    /// TypeScript type annotation.
    pub fn typescript_type(&self) -> String {
        match self {
            FieldType::String => "string".to_string(),
            FieldType::Integer | FieldType::Number => "number".to_string(),
            FieldType::Boolean => "boolean".to_string(),
            FieldType::Array(inner) => format!("{}[]", inner.typescript_type()),
            FieldType::Nested(name) => name.clone(),
            FieldType::StringEnum(ref values) => {
                let quoted: Vec<String> = values.iter().map(|v| format!("\"{v}\"")).collect();
                quoted.join(" | ")
            }
            FieldType::Any => "unknown".to_string(),
        }
    }

    /// Rust type.
    pub fn rust_type(&self) -> String {
        match self {
            FieldType::String => "String".to_string(),
            FieldType::Integer => "i64".to_string(),
            FieldType::Number => "f64".to_string(),
            FieldType::Boolean => "bool".to_string(),
            FieldType::Array(inner) => format!("Vec<{}>", inner.rust_type()),
            FieldType::Nested(name) => name.clone(),
            FieldType::StringEnum(_) => "String".to_string(),
            FieldType::Any => "serde_json::Value".to_string(),
        }
    }

    /// Whether this type is a string enum (for template rendering).
    pub fn is_string_enum(&self) -> bool {
        matches!(self, FieldType::StringEnum(_))
    }

    /// Get enum values if this is a StringEnum.
    pub fn enum_values(&self) -> Vec<&str> {
        match self {
            FieldType::StringEnum(values) => values.iter().map(|s| s.as_str()).collect(),
            _ => vec![],
        }
    }
}

/// Error during schema extraction.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("result_schema must be a JSON object, got: {0}")]
    NotAnObject(String),
}

/// Extract type definitions from a step's `result_schema`.
///
/// Returns types in dependency order (nested types before their parents).
/// The root type is named `{step_name}Result` in PascalCase.
pub fn extract_types(step_name: &str, schema: &Value) -> Result<Vec<TypeDef>, SchemaError> {
    let obj = schema
        .as_object()
        .ok_or_else(|| SchemaError::NotAnObject(format!("{schema}")))?;

    let root_name = to_pascal_result_name(step_name);
    let mut types = Vec::new();

    extract_object_type(&root_name, obj, &mut types);

    Ok(types)
}

/// Recursively extract a single object type and any nested object types.
fn extract_object_type(
    type_name: &str,
    obj: &serde_json::Map<String, Value>,
    types: &mut Vec<TypeDef>,
) {
    let description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);

    let required_fields: Vec<&str> = obj
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let properties = match obj.get("properties").and_then(|v| v.as_object()) {
        Some(props) => props,
        None => {
            // Object without properties — emit an empty type
            types.push(TypeDef {
                name: type_name.to_string(),
                fields: vec![],
                description,
            });
            return;
        }
    };

    let mut fields = Vec::new();

    for (prop_name, prop_schema) in properties {
        let field_type = resolve_field_type(type_name, prop_name, prop_schema, types);
        let field_desc = prop_schema
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from);

        fields.push(FieldDef {
            name: prop_name.clone(),
            field_type,
            required: required_fields.contains(&prop_name.as_str()),
            description: field_desc,
        });
    }

    types.push(TypeDef {
        name: type_name.to_string(),
        fields,
        description,
    });
}

/// Resolve the FieldType for a single property schema.
fn resolve_field_type(
    parent_type_name: &str,
    prop_name: &str,
    schema: &Value,
    types: &mut Vec<TypeDef>,
) -> FieldType {
    let type_str = schema.get("type").and_then(|v| v.as_str());

    // Check for enum values (applies to string type with constrained values)
    if let Some(enum_values) = schema.get("enum").and_then(|v| v.as_array()) {
        let variants: Vec<String> = enum_values
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        if !variants.is_empty() {
            return FieldType::StringEnum(variants);
        }
    }

    match type_str {
        Some("string") => FieldType::String,
        Some("integer") => FieldType::Integer,
        Some("number") => FieldType::Number,
        Some("boolean") => FieldType::Boolean,
        Some("array") => {
            let items_type = schema
                .get("items")
                .map(|items| resolve_field_type(parent_type_name, prop_name, items, types))
                .unwrap_or(FieldType::Any);
            FieldType::Array(Box::new(items_type))
        }
        Some("object") => {
            if let Some(obj) = schema.as_object() {
                // Only create a named nested type if the object has properties.
                // Bare `{type: object}` with no properties is an open-content
                // container (dict/map/HashMap) — use Any instead of an empty struct.
                if obj.contains_key("properties") {
                    let nested_name = format!("{parent_type_name}{}", to_pascal_case(prop_name));
                    extract_object_type(&nested_name, obj, types);
                    FieldType::Nested(nested_name)
                } else {
                    FieldType::Any
                }
            } else {
                FieldType::Any
            }
        }
        Some("null") => FieldType::Any,
        // Unsupported or missing type — check for $ref, allOf, oneOf, etc.
        _ => FieldType::Any,
    }
}

/// Convert a step name like `validate_order` to `ValidateOrderResult`.
pub(crate) fn to_pascal_result_name(step_name: &str) -> String {
    use heck::ToUpperCamelCase;
    format!("{}Result", step_name.to_upper_camel_case())
}

/// Convert a property name to PascalCase.
pub(crate) fn to_pascal_case(name: &str) -> String {
    use heck::ToUpperCamelCase;
    name.to_upper_camel_case()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_flat_schema_with_primitive_types() {
        let schema = json!({
            "type": "object",
            "required": ["name", "count"],
            "properties": {
                "name": { "type": "string" },
                "count": { "type": "integer" },
                "ratio": { "type": "number" },
                "active": { "type": "boolean" }
            }
        });

        let types = extract_types("process_data", &schema).unwrap();
        assert_eq!(types.len(), 1);

        let root = &types[0];
        assert_eq!(root.name, "ProcessDataResult");
        assert_eq!(root.fields.len(), 4);

        let name_field = root.fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(name_field.field_type, FieldType::String);
        assert!(name_field.required);

        let count_field = root.fields.iter().find(|f| f.name == "count").unwrap();
        assert_eq!(count_field.field_type, FieldType::Integer);
        assert!(count_field.required);

        let ratio_field = root.fields.iter().find(|f| f.name == "ratio").unwrap();
        assert_eq!(ratio_field.field_type, FieldType::Number);
        assert!(!ratio_field.required);

        let active_field = root.fields.iter().find(|f| f.name == "active").unwrap();
        assert_eq!(active_field.field_type, FieldType::Boolean);
        assert!(!active_field.required);
    }

    #[test]
    fn test_nested_object_generates_multiple_types() {
        let schema = json!({
            "type": "object",
            "required": ["address"],
            "properties": {
                "address": {
                    "type": "object",
                    "required": ["street"],
                    "properties": {
                        "street": { "type": "string" },
                        "city": { "type": "string" }
                    }
                }
            }
        });

        let types = extract_types("validate_order", &schema).unwrap();
        assert_eq!(types.len(), 2);

        // Nested type comes first (dependency order)
        let nested = &types[0];
        assert_eq!(nested.name, "ValidateOrderResultAddress");
        assert_eq!(nested.fields.len(), 2);

        let street = nested.fields.iter().find(|f| f.name == "street").unwrap();
        assert_eq!(street.field_type, FieldType::String);
        assert!(street.required);

        // Root type comes last
        let root = &types[1];
        assert_eq!(root.name, "ValidateOrderResult");
        let addr_field = root.fields.iter().find(|f| f.name == "address").unwrap();
        assert_eq!(
            addr_field.field_type,
            FieldType::Nested("ValidateOrderResultAddress".to_string())
        );
    }

    #[test]
    fn test_array_with_typed_items() {
        let schema = json!({
            "type": "object",
            "properties": {
                "tags": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "scores": {
                    "type": "array",
                    "items": { "type": "number" }
                }
            }
        });

        let types = extract_types("get_results", &schema).unwrap();
        assert_eq!(types.len(), 1);

        let root = &types[0];
        let tags = root.fields.iter().find(|f| f.name == "tags").unwrap();
        assert_eq!(
            tags.field_type,
            FieldType::Array(Box::new(FieldType::String))
        );

        let scores = root.fields.iter().find(|f| f.name == "scores").unwrap();
        assert_eq!(
            scores.field_type,
            FieldType::Array(Box::new(FieldType::Number))
        );
    }

    #[test]
    fn test_empty_properties_produces_empty_type() {
        let schema = json!({
            "type": "object"
        });

        let types = extract_types("empty_step", &schema).unwrap();
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].name, "EmptyStepResult");
        assert!(types[0].fields.is_empty());
    }

    #[test]
    fn test_description_propagation() {
        let schema = json!({
            "type": "object",
            "description": "The validation result",
            "properties": {
                "valid": {
                    "type": "boolean",
                    "description": "Whether validation passed"
                }
            }
        });

        let types = extract_types("validate", &schema).unwrap();
        assert_eq!(
            types[0].description.as_deref(),
            Some("The validation result")
        );
        assert_eq!(
            types[0].fields[0].description.as_deref(),
            Some("Whether validation passed")
        );
    }

    #[test]
    fn test_unsupported_schema_falls_back_to_any() {
        let schema = json!({
            "type": "object",
            "properties": {
                "unknown": { "type": "null" },
                "no_type": { "description": "missing type field" },
                "untyped_array": { "type": "array" }
            }
        });

        let types = extract_types("fallback", &schema).unwrap();
        let root = &types[0];

        let unknown = root.fields.iter().find(|f| f.name == "unknown").unwrap();
        assert_eq!(unknown.field_type, FieldType::Any);

        let no_type = root.fields.iter().find(|f| f.name == "no_type").unwrap();
        assert_eq!(no_type.field_type, FieldType::Any);

        let untyped_array = root
            .fields
            .iter()
            .find(|f| f.name == "untyped_array")
            .unwrap();
        assert_eq!(
            untyped_array.field_type,
            FieldType::Array(Box::new(FieldType::Any))
        );
    }

    #[test]
    fn test_string_enum_values() {
        let schema = json!({
            "type": "object",
            "required": ["status"],
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["completed", "processing", "failed"]
                },
                "optional_tier": {
                    "type": "string",
                    "enum": ["free", "pro", "enterprise"]
                }
            }
        });

        let types = extract_types("check_status", &schema).unwrap();
        assert_eq!(types.len(), 1);

        let root = &types[0];
        let status = root.fields.iter().find(|f| f.name == "status").unwrap();
        assert_eq!(
            status.field_type,
            FieldType::StringEnum(vec![
                "completed".to_string(),
                "processing".to_string(),
                "failed".to_string()
            ])
        );
        assert!(status.required);
        assert_eq!(status.field_type.python_type(), "str");
        assert_eq!(
            status.field_type.typescript_type(),
            "\"completed\" | \"processing\" | \"failed\""
        );
        assert_eq!(status.field_type.ruby_type(), "Types::Strict::String");
        assert_eq!(status.field_type.rust_type(), "String");

        let tier = root
            .fields
            .iter()
            .find(|f| f.name == "optional_tier")
            .unwrap();
        assert!(!tier.required);
        assert!(tier.field_type.is_string_enum());
        assert_eq!(
            tier.field_type.enum_values(),
            vec!["free", "pro", "enterprise"]
        );
    }

    #[test]
    fn test_non_object_schema_returns_error() {
        let schema = json!("not an object");
        let result = extract_types("bad", &schema);
        assert!(result.is_err());
    }
}
