//! Plugin registry — discovers, loads, and indexes available plugins.

use std::path::PathBuf;

use super::discovery::discover_plugin_dirs;
use super::manifest::{ManifestError, PluginManifest, TemplateReference};
use crate::cli_config::loader::expand_path;
use crate::cli_config::CliConfig;

/// A discovered and loaded plugin with its filesystem location.
#[derive(Debug)]
pub(crate) struct LoadedPlugin {
    /// Directory containing the plugin manifest.
    pub dir: PathBuf,
    /// Parsed manifest.
    pub manifest: PluginManifest,
}

/// Registry of discovered plugins, providing lookup and filtering.
#[derive(Debug)]
pub(crate) struct PluginRegistry {
    plugins: Vec<LoadedPlugin>,
}

/// A resolved template reference with its absolute directory path.
#[derive(Debug)]
pub(crate) struct ResolvedTemplate<'a> {
    /// The plugin that owns this template.
    pub plugin: &'a LoadedPlugin,
    /// The template reference from the manifest.
    pub template: &'a TemplateReference,
    /// Absolute path to the template directory.
    pub template_dir: PathBuf,
}

impl PluginRegistry {
    /// Discover and load plugins from configured paths.
    pub fn discover(config: &CliConfig) -> Self {
        let search_paths: Vec<PathBuf> =
            config.plugin_paths.iter().map(|p| expand_path(p)).collect();

        let plugin_dirs = discover_plugin_dirs(&search_paths);
        let mut plugins = Vec::new();

        for dir in plugin_dirs {
            match PluginManifest::load(&dir) {
                Ok(manifest) => {
                    tracing::debug!(
                        name = %manifest.plugin.name,
                        ?dir,
                        "Discovered plugin"
                    );
                    plugins.push(LoadedPlugin { dir, manifest });
                }
                Err(ManifestError::Io { path, source }) => {
                    tracing::warn!(?path, error = %source, "Skipping plugin: cannot read manifest");
                }
                Err(ManifestError::Parse { path, source }) => {
                    tracing::warn!(?path, error = %source, "Skipping plugin: invalid manifest");
                }
            }
        }

        Self { plugins }
    }

    /// List all discovered plugins.
    pub fn plugins(&self) -> &[LoadedPlugin] {
        &self.plugins
    }

    /// Find all templates, optionally filtered by language and/or framework.
    pub fn find_templates(
        &self,
        language: Option<&str>,
        framework: Option<&str>,
    ) -> Vec<ResolvedTemplate<'_>> {
        let mut results = Vec::new();

        for plugin in &self.plugins {
            let lang_match = language
                .map(|l| plugin.manifest.plugin.language.eq_ignore_ascii_case(l))
                .unwrap_or(true);
            let fw_match = framework
                .map(|f| {
                    plugin
                        .manifest
                        .plugin
                        .framework
                        .as_ref()
                        .is_some_and(|pf| pf.eq_ignore_ascii_case(f))
                })
                .unwrap_or(true);

            if lang_match && fw_match {
                for tmpl in &plugin.manifest.templates {
                    results.push(ResolvedTemplate {
                        plugin,
                        template: tmpl,
                        template_dir: plugin.dir.join(&tmpl.path),
                    });
                }
            }
        }

        results
    }

    /// Find a specific template by name, optionally scoped to a plugin.
    pub fn find_template_by_name<'a>(
        &'a self,
        template_name: &str,
        plugin_name: Option<&str>,
    ) -> Option<ResolvedTemplate<'a>> {
        for plugin in &self.plugins {
            if let Some(pn) = plugin_name {
                if !plugin.manifest.plugin.name.eq_ignore_ascii_case(pn) {
                    continue;
                }
            }

            for tmpl in &plugin.manifest.templates {
                if tmpl.name.eq_ignore_ascii_case(template_name) {
                    return Some(ResolvedTemplate {
                        plugin,
                        template: tmpl,
                        template_dir: plugin.dir.join(&tmpl.path),
                    });
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_plugin(base: &std::path::Path, name: &str, language: &str, framework: Option<&str>) {
        let plugin_dir = base.join(name);
        let tmpl_dir = plugin_dir.join("templates/handler");
        fs::create_dir_all(&tmpl_dir).unwrap();

        let fw_line = framework
            .map(|f| format!("framework = \"{f}\""))
            .unwrap_or_default();

        let manifest = format!(
            r#"
[plugin]
name = "{name}"
description = "Test {name} plugin"
version = "0.1.0"
language = "{language}"
{fw_line}

[[templates]]
name = "step-handler"
path = "templates/handler"
description = "Generate a step handler"
"#
        );
        fs::write(plugin_dir.join("tasker-plugin.toml"), manifest).unwrap();
    }

    #[test]
    fn test_registry_discover() {
        let dir = tempfile::tempdir().unwrap();
        create_plugin(dir.path(), "rails", "ruby", Some("rails"));
        create_plugin(dir.path(), "django", "python", Some("django"));

        let config = CliConfig {
            plugin_paths: vec![dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };

        let registry = PluginRegistry::discover(&config);
        assert_eq!(registry.plugins().len(), 2);
    }

    #[test]
    fn test_find_templates_all() {
        let dir = tempfile::tempdir().unwrap();
        create_plugin(dir.path(), "rails", "ruby", Some("rails"));
        create_plugin(dir.path(), "django", "python", Some("django"));

        let config = CliConfig {
            plugin_paths: vec![dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };
        let registry = PluginRegistry::discover(&config);

        let all = registry.find_templates(None, None);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_find_templates_by_language() {
        let dir = tempfile::tempdir().unwrap();
        create_plugin(dir.path(), "rails", "ruby", Some("rails"));
        create_plugin(dir.path(), "django", "python", Some("django"));

        let config = CliConfig {
            plugin_paths: vec![dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };
        let registry = PluginRegistry::discover(&config);

        let ruby = registry.find_templates(Some("ruby"), None);
        assert_eq!(ruby.len(), 1);
        assert_eq!(ruby[0].plugin.manifest.plugin.name, "rails");
    }

    #[test]
    fn test_find_templates_by_framework() {
        let dir = tempfile::tempdir().unwrap();
        create_plugin(dir.path(), "rails", "ruby", Some("rails"));
        create_plugin(dir.path(), "sinatra", "ruby", Some("sinatra"));

        let config = CliConfig {
            plugin_paths: vec![dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };
        let registry = PluginRegistry::discover(&config);

        let rails = registry.find_templates(Some("ruby"), Some("rails"));
        assert_eq!(rails.len(), 1);
        assert_eq!(rails[0].plugin.manifest.plugin.name, "rails");
    }

    #[test]
    fn test_find_template_by_name() {
        let dir = tempfile::tempdir().unwrap();
        create_plugin(dir.path(), "rails", "ruby", Some("rails"));

        let config = CliConfig {
            plugin_paths: vec![dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };
        let registry = PluginRegistry::discover(&config);

        let tmpl = registry.find_template_by_name("step-handler", None);
        assert!(tmpl.is_some());
        assert_eq!(tmpl.unwrap().template.name, "step-handler");
    }

    #[test]
    fn test_find_template_by_name_scoped() {
        let dir = tempfile::tempdir().unwrap();
        create_plugin(dir.path(), "rails", "ruby", Some("rails"));
        create_plugin(dir.path(), "django", "python", Some("django"));

        let config = CliConfig {
            plugin_paths: vec![dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };
        let registry = PluginRegistry::discover(&config);

        let tmpl = registry.find_template_by_name("step-handler", Some("django"));
        assert!(tmpl.is_some());
        assert_eq!(tmpl.unwrap().plugin.manifest.plugin.name, "django");
    }

    #[test]
    fn test_find_template_not_found() {
        let dir = tempfile::tempdir().unwrap();
        create_plugin(dir.path(), "rails", "ruby", Some("rails"));

        let config = CliConfig {
            plugin_paths: vec![dir.path().to_string_lossy().to_string()],
            ..Default::default()
        };
        let registry = PluginRegistry::discover(&config);

        let tmpl = registry.find_template_by_name("nonexistent", None);
        assert!(tmpl.is_none());
    }

    #[test]
    fn test_empty_registry() {
        let config = CliConfig::default();
        let registry = PluginRegistry::discover(&config);
        assert!(registry.plugins().is_empty());
        assert!(registry.find_templates(None, None).is_empty());
    }
}
