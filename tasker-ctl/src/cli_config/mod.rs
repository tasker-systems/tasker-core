//! CLI configuration for plugin paths and developer preferences.
//!
//! Separate from `tasker-client` config — this controls the CLI tool behavior,
//! not the API client connection settings.

pub(crate) mod loader;

pub(crate) use loader::load_cli_config;

use serde::Deserialize;

/// CLI-specific configuration for plugin discovery and template generation.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct CliConfig {
    /// Additional paths to scan for CLI plugins (beyond built-in locations).
    #[serde(default)]
    pub plugin_paths: Vec<String>,

    /// Default language for template generation (e.g., "ruby", "python", "rust").
    pub default_language: Option<String>,

    /// Default output directory for generated files.
    pub default_output_dir: Option<String>,
}
