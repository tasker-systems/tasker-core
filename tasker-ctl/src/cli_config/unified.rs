//! Unified configuration file support (TAS-311).
//!
//! Parses `.config/tasker.toml` which combines both `[profile.*]` sections
//! (consumed by tasker-client) and a `[cli]` section (consumed by tasker-ctl).

use std::collections::HashMap;

use serde::Deserialize;
use tasker_client::config::ProfileConfig;

use super::{CliConfig, RemoteConfig};

fn default_cache_max_age_hours() -> u64 {
    24
}

fn default_git_ref() -> String {
    "main".to_string()
}

fn default_config_path() -> String {
    "config/tasker/".to_string()
}

/// The unified `.config/tasker.toml` file format.
///
/// Contains both profile definitions (reused from tasker-client) and
/// CLI-specific settings. When `[cli]` is absent, CLI defaults apply.
#[derive(Debug, Deserialize)]
pub(crate) struct UnifiedConfigFile {
    /// Named profiles — same structure as `ProfileConfigFile`.
    /// Parsed to allow serde to accept the unified file; profile loading is
    /// handled by `tasker_client::config::ClientConfig::find_profile_config_file()`.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "parsed for serde compatibility, consumed by tasker-client"
        )
    )]
    #[serde(default)]
    pub profile: HashMap<String, ProfileConfig>,

    /// CLI-specific settings (plugin paths, template defaults, remotes).
    #[serde(default)]
    pub cli: Option<CliSection>,
}

/// The `[cli]` section within a unified config file.
///
/// Uses kebab-case to match the standalone `.tasker-ctl.toml` format.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct CliSection {
    /// Additional paths to scan for CLI plugins.
    #[serde(default)]
    pub plugin_paths: Vec<String>,

    /// Default language for template generation.
    pub default_language: Option<String>,

    /// Default output directory for generated files.
    pub default_output_dir: Option<String>,

    /// Remote git repositories for plugins and config.
    #[serde(default)]
    pub remotes: Vec<UnifiedRemoteConfig>,

    /// Maximum age in hours before a cached remote is considered stale.
    #[serde(default = "default_cache_max_age_hours")]
    pub cache_max_age_hours: u64,
}

/// Remote config within unified file — mirrors `RemoteConfig` with same field names.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct UnifiedRemoteConfig {
    pub name: String,
    pub url: String,
    #[serde(default = "default_git_ref")]
    pub git_ref: String,
    #[serde(default = "default_config_path")]
    pub config_path: String,
    pub plugin_path: Option<String>,
}

impl From<UnifiedRemoteConfig> for RemoteConfig {
    fn from(r: UnifiedRemoteConfig) -> Self {
        Self {
            name: r.name,
            url: r.url,
            git_ref: r.git_ref,
            config_path: r.config_path,
            plugin_path: r.plugin_path,
        }
    }
}

impl From<CliSection> for CliConfig {
    fn from(section: CliSection) -> Self {
        Self {
            plugin_paths: section.plugin_paths,
            default_language: section.default_language,
            default_output_dir: section.default_output_dir,
            remotes: section.remotes.into_iter().map(Into::into).collect(),
            cache_max_age_hours: section.cache_max_age_hours,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unified_file_both_sections() {
        let toml_str = r#"
[profile.default]
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"

[profile.default.worker]
base_url = "http://localhost:8081"

[cli]
plugin-paths = ["./plugins"]
default-language = "ruby"
default-output-dir = "./app/handlers"
cache-max-age-hours = 48

[[cli.remotes]]
name = "tasker-contrib"
url = "https://github.com/tasker-systems/tasker-contrib.git"
"#;
        let unified: UnifiedConfigFile = toml::from_str(toml_str).unwrap();
        assert!(unified.profile.contains_key("default"));
        let cli = unified.cli.unwrap();
        assert_eq!(cli.plugin_paths, vec!["./plugins"]);
        assert_eq!(cli.default_language.as_deref(), Some("ruby"));
        assert_eq!(cli.cache_max_age_hours, 48);
        assert_eq!(cli.remotes.len(), 1);
        assert_eq!(cli.remotes[0].name, "tasker-contrib");
    }

    #[test]
    fn test_parse_unified_file_profiles_only() {
        let toml_str = r#"
[profile.default]
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"

[profile.default.worker]
base_url = "http://localhost:8081"
"#;
        let unified: UnifiedConfigFile = toml::from_str(toml_str).unwrap();
        assert!(unified.profile.contains_key("default"));
        assert!(unified.cli.is_none());
    }

    #[test]
    fn test_parse_unified_file_cli_only() {
        let toml_str = r#"
[cli]
plugin-paths = ["./plugins"]
default-language = "python"
"#;
        let unified: UnifiedConfigFile = toml::from_str(toml_str).unwrap();
        assert!(unified.profile.is_empty());
        let cli = unified.cli.unwrap();
        assert_eq!(cli.default_language.as_deref(), Some("python"));
        assert_eq!(cli.cache_max_age_hours, 24); // serde default
    }

    #[test]
    fn test_cli_section_converts_to_cli_config() {
        let toml_str = r#"
[cli]
plugin-paths = ["./plugins", "~/contrib"]
default-language = "ruby"
default-output-dir = "./app/handlers"
cache-max-age-hours = 12

[[cli.remotes]]
name = "tasker-contrib"
url = "https://github.com/tasker-systems/tasker-contrib.git"
"#;
        let unified: UnifiedConfigFile = toml::from_str(toml_str).unwrap();
        let cli_config: CliConfig = unified.cli.unwrap().into();
        assert_eq!(cli_config.plugin_paths, vec!["./plugins", "~/contrib"]);
        assert_eq!(cli_config.default_language.as_deref(), Some("ruby"));
        assert_eq!(cli_config.cache_max_age_hours, 12);
        assert_eq!(cli_config.remotes.len(), 1);
        assert_eq!(cli_config.remotes[0].name, "tasker-contrib");
    }

    #[test]
    fn test_profile_config_file_ignores_cli_section() {
        // Verify that ProfileConfigFile (from tasker-client) can parse the unified
        // file without errors — the [cli] section is silently ignored.
        use tasker_client::config::ProfileConfigFile;

        let toml_str = r#"
[profile.default]
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"

[profile.default.worker]
base_url = "http://localhost:8081"

[cli]
plugin-paths = ["./plugins"]
default-language = "ruby"

[[cli.remotes]]
name = "tasker-contrib"
url = "https://github.com/tasker-systems/tasker-contrib.git"
"#;
        let profile_file: ProfileConfigFile = toml::from_str(toml_str).unwrap();
        assert!(profile_file.profile.contains_key("default"));
    }
}
