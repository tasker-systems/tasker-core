//! CLI configuration for plugin paths and developer preferences.
//!
//! Separate from `tasker-client` config — this controls the CLI tool behavior,
//! not the API client connection settings.

pub(crate) mod loader;

pub(crate) use loader::load_cli_config;

use serde::Deserialize;

fn default_git_ref() -> String {
    "main".to_string()
}

fn default_config_path() -> String {
    "config/tasker/".to_string()
}

fn default_cache_max_age_hours() -> u64 {
    24
}

/// Configuration for a remote git repository providing plugins and/or config.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct RemoteConfig {
    /// Unique name for this remote (used in --remote flag and cache directory).
    pub name: String,

    /// Git URL (https:// or file://).
    pub url: String,

    /// Git ref to checkout (branch, tag, or commit). Default: "main".
    #[serde(default = "default_git_ref")]
    pub git_ref: String,

    /// Path within the repo to the config directory. Default: "config/tasker/".
    #[serde(default = "default_config_path")]
    pub config_path: String,

    /// Path within the repo to scan for plugins. Default: repo root.
    pub plugin_path: Option<String>,
}

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

    /// Remote git repositories for plugins and config (TAS-270).
    #[serde(default)]
    pub remotes: Vec<RemoteConfig>,

    /// Maximum age in hours before a cached remote is considered stale. Default: 24.
    #[serde(default = "default_cache_max_age_hours")]
    pub cache_max_age_hours: u64,
}
