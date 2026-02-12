//! Template metadata parsing (`template.toml`).

use std::path::Path;

use serde::Deserialize;

const METADATA_FILENAME: &str = "template.toml";

/// Metadata for a template, parsed from `template.toml`.
#[derive(Debug, Deserialize)]
pub(crate) struct TemplateMetadata {
    /// Template display name.
    pub name: String,

    /// Description of what this template generates.
    pub description: String,

    /// Parameters that the template accepts.
    #[serde(default)]
    pub parameters: Vec<ParameterDef>,

    /// Output files to generate.
    pub outputs: Vec<OutputFile>,
}

/// A parameter the template accepts.
#[derive(Debug, Deserialize)]
pub(crate) struct ParameterDef {
    /// Parameter name (used as `--param name=value`).
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Whether this parameter is required.
    #[serde(default)]
    pub required: bool,

    /// Default value if not provided.
    pub default: Option<String>,
}

/// An output file to generate from a template.
#[derive(Debug, Deserialize)]
pub(crate) struct OutputFile {
    /// Tera template filename (e.g., `handler.rb.tera`).
    pub template: String,

    /// Output filename pattern (can use Tera variables, e.g., `{{ name | snake_case }}.rb`).
    pub filename: String,

    /// Optional subdirectory for output.
    pub subdir: Option<String>,
}

impl TemplateMetadata {
    /// Load metadata from a template directory containing `template.toml`.
    pub fn load(template_dir: &Path) -> Result<Self, MetadataError> {
        let path = template_dir.join(METADATA_FILENAME);
        let contents = std::fs::read_to_string(&path).map_err(|e| MetadataError::Io {
            path: path.clone(),
            source: e,
        })?;
        let metadata: Self =
            toml::from_str(&contents).map_err(|e| MetadataError::Parse { path, source: e })?;
        Ok(metadata)
    }

    /// Validate that required parameters have values in the provided params map.
    pub fn validate_params(
        &self,
        params: &std::collections::HashMap<String, String>,
    ) -> Vec<String> {
        let mut errors = Vec::new();
        for p in &self.parameters {
            if p.required && !params.contains_key(&p.name) && p.default.is_none() {
                errors.push(format!("missing required parameter: {}", p.name));
            }
        }
        errors
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum MetadataError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: std::path::PathBuf,
        source: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parse_metadata() {
        let toml_str = r#"
name = "step-handler"
description = "Generate a Tasker step handler"

[[parameters]]
name = "name"
description = "Handler class name"
required = true

[[parameters]]
name = "handler_type"
description = "Type of handler"
required = false
default = "generic"

[[outputs]]
template = "handler.rb.tera"
filename = "{{ name | snake_case }}_handler.rb"

[[outputs]]
template = "handler_spec.rb.tera"
filename = "{{ name | snake_case }}_handler_spec.rb"
subdir = "spec"
"#;
        let meta: TemplateMetadata = toml::from_str(toml_str).unwrap();
        assert_eq!(meta.name, "step-handler");
        assert_eq!(meta.parameters.len(), 2);
        assert!(meta.parameters[0].required);
        assert!(!meta.parameters[1].required);
        assert_eq!(meta.parameters[1].default.as_deref(), Some("generic"));
        assert_eq!(meta.outputs.len(), 2);
        assert_eq!(meta.outputs[1].subdir.as_deref(), Some("spec"));
    }

    #[test]
    fn test_validate_params_ok() {
        let meta: TemplateMetadata = toml::from_str(
            r#"
name = "test"
description = "Test"
[[parameters]]
name = "name"
description = "Name"
required = true
[[outputs]]
template = "t.tera"
filename = "out.txt"
"#,
        )
        .unwrap();

        let mut params = HashMap::new();
        params.insert("name".to_string(), "Foo".to_string());
        assert!(meta.validate_params(&params).is_empty());
    }

    #[test]
    fn test_validate_params_missing_required() {
        let meta: TemplateMetadata = toml::from_str(
            r#"
name = "test"
description = "Test"
[[parameters]]
name = "name"
description = "Name"
required = true
[[outputs]]
template = "t.tera"
filename = "out.txt"
"#,
        )
        .unwrap();

        let params = HashMap::new();
        let errors = meta.validate_params(&params);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("name"));
    }

    #[test]
    fn test_validate_params_with_default() {
        let meta: TemplateMetadata = toml::from_str(
            r#"
name = "test"
description = "Test"
[[parameters]]
name = "handler_type"
description = "Type"
required = true
default = "generic"
[[outputs]]
template = "t.tera"
filename = "out.txt"
"#,
        )
        .unwrap();

        let params = HashMap::new();
        // Has default, so not an error even though required=true and not provided
        assert!(meta.validate_params(&params).is_empty());
    }

    #[test]
    fn test_load_metadata_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"
name = "handler"
description = "A handler"
[[outputs]]
template = "handler.tera"
filename = "handler.rs"
"#;
        std::fs::write(dir.path().join("template.toml"), content).unwrap();

        let meta = TemplateMetadata::load(dir.path()).unwrap();
        assert_eq!(meta.name, "handler");
    }
}
