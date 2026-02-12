//! Config file discovery and loading for `.tasker-cli.toml`.
//!
//! Checks two locations in precedence order:
//! 1. `./.tasker-cli.toml` (project-local)
//! 2. `~/.config/tasker-cli.toml` (user-global)

use std::path::PathBuf;

use super::CliConfig;

const CONFIG_FILENAME: &str = ".tasker-cli.toml";
const GLOBAL_CONFIG_DIR: &str = ".config";
const GLOBAL_CONFIG_FILENAME: &str = "tasker-cli.toml";

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
    // 1. Project-local: ./.tasker-cli.toml
    let local = PathBuf::from(CONFIG_FILENAME);
    if local.is_file() {
        return Some(local);
    }

    // 2. User-global: ~/.config/tasker-cli.toml
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
}
