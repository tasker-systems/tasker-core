//! Task template YAML parsing with rich error reporting.
//!
//! Provides functions for loading `TaskTemplate` definitions from YAML files,
//! with structured errors suitable for both CLI and MCP consumption.

use std::path::Path;

use tasker_shared::models::core::task_template::TaskTemplate;

/// Error during template parsing.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("failed to read template file '{}': {source}", path.display())]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse template YAML '{}': {source}", path.display())]
    Yaml {
        path: std::path::PathBuf,
        source: serde_yaml::Error,
    },
}

/// Parse a task template YAML file into a [`TaskTemplate`].
pub fn parse_template(path: &Path) -> Result<TaskTemplate, ParseError> {
    let yaml_content = std::fs::read_to_string(path).map_err(|e| ParseError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    serde_yaml::from_str(&yaml_content).map_err(|e| ParseError::Yaml {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Parse a task template from a YAML string.
pub fn parse_template_str(yaml: &str) -> Result<TaskTemplate, serde_yaml::Error> {
    serde_yaml::from_str(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_template_from_fixture() {
        let fixture =
            include_str!("../../../tests/fixtures/task_templates/codegen_test_template.yaml");
        let template = parse_template_str(fixture).unwrap();
        assert_eq!(template.name, "codegen_test");
        assert!(!template.steps.is_empty());
    }

    #[test]
    fn test_parse_template_file_not_found() {
        let result = parse_template(Path::new("/nonexistent/template.yaml"));
        assert!(matches!(result, Err(ParseError::Io { .. })));
    }

    #[test]
    fn test_parse_template_invalid_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.yaml");
        std::fs::write(&path, "not: [valid: yaml: for: template").unwrap();
        let result = parse_template(&path);
        assert!(matches!(result, Err(ParseError::Yaml { .. })));
    }
}
