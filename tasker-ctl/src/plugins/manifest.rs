//! Plugin manifest parsing (`tasker-plugin.toml`).

use std::path::{Path, PathBuf};

use serde::Deserialize;

const MANIFEST_FILENAME: &str = "tasker-plugin.toml";

/// Top-level plugin manifest parsed from `tasker-plugin.toml`.
#[derive(Debug, Deserialize)]
pub(crate) struct PluginManifest {
    /// Plugin metadata.
    pub plugin: PluginMetadata,

    /// Template references declared by this plugin.
    #[serde(default)]
    pub templates: Vec<TemplateReference>,
}

/// Core plugin metadata.
#[derive(Debug, Deserialize)]
pub(crate) struct PluginMetadata {
    /// Unique plugin name (e.g., "rails", "django", "axum").
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Plugin version.
    pub version: String,

    /// Target language (e.g., "ruby", "python", "rust", "typescript").
    pub language: String,

    /// Target framework (e.g., "rails", "django", "axum").
    pub framework: Option<String>,
}

/// Reference to a template directory within the plugin.
#[derive(Debug, Deserialize)]
pub(crate) struct TemplateReference {
    /// Template name (e.g., "step-handler", "initializer").
    pub name: String,

    /// Relative path to the template directory from the manifest location.
    pub path: String,

    /// Short description of what this template generates.
    pub description: String,
}

impl PluginManifest {
    /// Load a manifest from a directory containing `tasker-plugin.toml`.
    pub fn load(dir: &Path) -> Result<Self, ManifestError> {
        let manifest_path = dir.join(MANIFEST_FILENAME);
        let contents = std::fs::read_to_string(&manifest_path).map_err(|e| ManifestError::Io {
            path: manifest_path.clone(),
            source: e,
        })?;
        let manifest: Self = toml::from_str(&contents).map_err(|e| ManifestError::Parse {
            path: manifest_path,
            source: e,
        })?;
        Ok(manifest)
    }

    /// Validate that all referenced template directories exist.
    pub fn validate(&self, base_dir: &Path) -> Vec<String> {
        let mut errors = Vec::new();

        if self.plugin.name.is_empty() {
            errors.push("plugin.name is empty".to_string());
        }
        if self.plugin.language.is_empty() {
            errors.push("plugin.language is empty".to_string());
        }

        for tmpl in &self.templates {
            let tmpl_dir = base_dir.join(&tmpl.path);
            if !tmpl_dir.is_dir() {
                errors.push(format!(
                    "template '{}' path '{}' does not exist",
                    tmpl.name, tmpl.path
                ));
            }
        }

        errors
    }
}

/// Errors that can occur during manifest loading.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ManifestError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_manifest() {
        let toml_str = r#"
[plugin]
name = "rails"
description = "Ruby on Rails integration templates"
version = "0.1.0"
language = "ruby"
framework = "rails"

[[templates]]
name = "step-handler"
path = "templates/step_handler"
description = "Generate a Tasker step handler class"

[[templates]]
name = "initializer"
path = "templates/initializer"
description = "Generate a Rails initializer for Tasker"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.plugin.name, "rails");
        assert_eq!(manifest.plugin.language, "ruby");
        assert_eq!(manifest.plugin.framework.as_deref(), Some("rails"));
        assert_eq!(manifest.templates.len(), 2);
        assert_eq!(manifest.templates[0].name, "step-handler");
    }

    #[test]
    fn test_parse_manifest_no_framework() {
        let toml_str = r#"
[plugin]
name = "ruby-basic"
description = "Basic Ruby templates"
version = "0.1.0"
language = "ruby"

[[templates]]
name = "handler"
path = "templates/handler"
description = "Generate a basic handler"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert!(manifest.plugin.framework.is_none());
    }

    #[test]
    fn test_load_manifest_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_content = r#"
[plugin]
name = "test-plugin"
description = "Test plugin"
version = "0.1.0"
language = "rust"

[[templates]]
name = "handler"
path = "templates/handler"
description = "A handler template"
"#;
        fs::write(dir.path().join("tasker-plugin.toml"), manifest_content).unwrap();

        let manifest = PluginManifest::load(dir.path()).unwrap();
        assert_eq!(manifest.plugin.name, "test-plugin");
    }

    #[test]
    fn test_validate_missing_template_dir() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_content = r#"
[plugin]
name = "test"
description = "Test"
version = "0.1.0"
language = "rust"

[[templates]]
name = "handler"
path = "templates/nonexistent"
description = "Missing dir"
"#;
        let manifest: PluginManifest = toml::from_str(manifest_content).unwrap();
        let errors = manifest.validate(dir.path());
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("nonexistent"));
    }

    #[test]
    fn test_validate_valid_plugin() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("templates/handler")).unwrap();

        let manifest_content = r#"
[plugin]
name = "test"
description = "Test"
version = "0.1.0"
language = "rust"

[[templates]]
name = "handler"
path = "templates/handler"
description = "A handler"
"#;
        let manifest: PluginManifest = toml::from_str(manifest_content).unwrap();
        let errors = manifest.validate(dir.path());
        assert!(errors.is_empty());
    }
}
