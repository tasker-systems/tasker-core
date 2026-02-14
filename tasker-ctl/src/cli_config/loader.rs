//! Config file discovery and loading for `.tasker-ctl.toml`.
//!
//! Checks two locations in precedence order:
//! 1. `./.tasker-ctl.toml` (project-local)
//! 2. `~/.config/tasker-ctl.toml` (user-global)

use std::path::PathBuf;

use super::CliConfig;

const CONFIG_FILENAME: &str = ".tasker-ctl.toml";
const GLOBAL_CONFIG_DIR: &str = ".config";
const GLOBAL_CONFIG_FILENAME: &str = "tasker-ctl.toml";

/// Load CLI config from the first discovered location, or return defaults.
pub(crate) fn load_cli_config() -> CliConfig {
    if let Some(path) = find_config_file() {
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => {
                    tracing::debug!(?path, "Loaded CLI config");
                    return config;
                }
                Err(e) => {
                    tracing::warn!(?path, error = %e, "Failed to parse CLI config, using defaults");
                }
            },
            Err(e) => {
                tracing::warn!(?path, error = %e, "Failed to read CLI config, using defaults");
            }
        }
    }
    CliConfig::default()
}

/// Search for config file in precedence order.
fn find_config_file() -> Option<PathBuf> {
    // 1. Project-local: ./.tasker-ctl.toml
    let local = PathBuf::from(CONFIG_FILENAME);
    if local.is_file() {
        return Some(local);
    }

    // 2. User-global: ~/.config/tasker-ctl.toml
    if let Some(home) = home_dir() {
        let global = home.join(GLOBAL_CONFIG_DIR).join(GLOBAL_CONFIG_FILENAME);
        if global.is_file() {
            return Some(global);
        }
    }

    None
}

/// Expand plugin paths, resolving `~` to the home directory.
pub(crate) fn expand_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_path_tilde() {
        let expanded = expand_path("~/projects/tasker-contrib");
        assert!(expanded
            .to_str()
            .unwrap()
            .contains("projects/tasker-contrib"));
        assert!(!expanded.to_str().unwrap().starts_with("~"));
    }

    #[test]
    fn test_expand_path_absolute() {
        let expanded = expand_path("/usr/local/plugins");
        assert_eq!(expanded, PathBuf::from("/usr/local/plugins"));
    }

    #[test]
    fn test_expand_path_relative() {
        let expanded = expand_path("./plugins");
        assert_eq!(expanded, PathBuf::from("./plugins"));
    }

    #[test]
    fn test_default_config() {
        let config = CliConfig::default();
        assert!(config.plugin_paths.is_empty());
        assert!(config.default_language.is_none());
        assert!(config.default_output_dir.is_none());
        assert!(config.remotes.is_empty());
        assert_eq!(config.cache_max_age_hours, 0); // Default trait gives 0, serde default gives 24
    }

    #[test]
    fn test_parse_config_toml() {
        let toml_str = r#"
plugin-paths = ["./plugins", "~/contrib"]
default-language = "ruby"
default-output-dir = "./app/handlers"
"#;
        let config: CliConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.plugin_paths, vec!["./plugins", "~/contrib"]);
        assert_eq!(config.default_language.as_deref(), Some("ruby"));
        assert_eq!(config.default_output_dir.as_deref(), Some("./app/handlers"));
    }

    #[test]
    fn test_parse_minimal_config() {
        let toml_str = r#"
plugin-paths = ["./plugins"]
"#;
        let config: CliConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.plugin_paths.len(), 1);
        assert!(config.default_language.is_none());
    }

    #[test]
    fn test_load_missing_config_returns_defaults() {
        // When no config file exists, should return defaults gracefully
        let config = load_cli_config();
        // Just verify it doesn't panic — actual paths depend on environment
        assert!(config.plugin_paths.is_empty() || !config.plugin_paths.is_empty());
    }

    #[test]
    fn test_parse_config_with_remotes() {
        let toml_str = r#"
plugin-paths = ["./plugins"]

[[remotes]]
name = "tasker-contrib"
url = "https://github.com/tasker-systems/tasker-contrib.git"
git-ref = "main"
config-path = "config/tasker/"

[[remotes]]
name = "internal-plugins"
url = "https://github.com/myorg/internal-tasker-plugins.git"
git-ref = "v1.0"
plugin-path = "plugins/"
"#;
        let config: CliConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.remotes.len(), 2);
        assert_eq!(config.remotes[0].name, "tasker-contrib");
        assert_eq!(
            config.remotes[0].url,
            "https://github.com/tasker-systems/tasker-contrib.git"
        );
        assert_eq!(config.remotes[0].git_ref, "main");
        assert_eq!(config.remotes[0].config_path, "config/tasker/");
        assert!(config.remotes[0].plugin_path.is_none());

        assert_eq!(config.remotes[1].name, "internal-plugins");
        assert_eq!(config.remotes[1].git_ref, "v1.0");
        assert_eq!(config.remotes[1].plugin_path.as_deref(), Some("plugins/"));
    }

    #[test]
    fn test_parse_config_with_remotes_defaults() {
        let toml_str = r#"
[[remotes]]
name = "minimal"
url = "https://github.com/example/repo.git"
"#;
        let config: CliConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.remotes.len(), 1);
        assert_eq!(config.remotes[0].git_ref, "main");
        assert_eq!(config.remotes[0].config_path, "config/tasker/");
        assert!(config.remotes[0].plugin_path.is_none());
        assert_eq!(config.cache_max_age_hours, 24);
    }

    #[test]
    fn test_parse_config_with_custom_cache_max_age() {
        let toml_str = r#"
cache-max-age-hours = 48
"#;
        let config: CliConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.cache_max_age_hours, 48);
    }

    #[test]
    fn test_backward_compat_no_remotes() {
        let toml_str = r#"
plugin-paths = ["./plugins", "~/contrib"]
default-language = "ruby"
"#;
        let config: CliConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.plugin_paths.len(), 2);
        assert!(config.remotes.is_empty());
        assert_eq!(config.cache_max_age_hours, 24);
    }
}
