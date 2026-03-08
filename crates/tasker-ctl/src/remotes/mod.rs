//! Remote repository management for fetching plugins and config from git repos.
//!
//! Remotes become cached local paths. The existing plugin discovery, registry,
//! template engine, and ConfigMerger work unchanged — this module handles:
//! fetch → cache → resolve path → hand off.

pub(crate) mod cache;
pub(crate) mod error;
pub(crate) mod git;

pub(crate) use cache::RemoteCache;
pub(crate) use error::RemoteError;

use std::path::PathBuf;

use crate::cli_config::CliConfig;

/// Resolve remote(s) to local cached plugin paths.
///
/// - If `remote` is specified: resolve that single remote's plugin path.
/// - If `url` is specified: resolve the ad-hoc URL's plugin path.
/// - If neither: return empty (caller can fall back to auto-discovery in Phase 5).
pub(crate) fn resolve_remote_plugin_paths(
    cli_config: &CliConfig,
    remote: Option<&str>,
    url: Option<&str>,
) -> tasker_client::ClientResult<Vec<PathBuf>> {
    if let Some(remote_name) = remote {
        let remote_config = cli_config
            .remotes
            .iter()
            .find(|r| r.name == remote_name)
            .ok_or_else(|| {
                tasker_client::ClientError::ConfigError(format!(
                    "Remote '{}' not found in configuration. Run `tasker-ctl remote list` to see configured remotes.",
                    remote_name
                ))
            })?;

        let cache_dir = RemoteCache::resolve(remote_config, cli_config.cache_max_age_hours)
            .map_err(|e| {
                tasker_client::ClientError::ConfigError(format!(
                    "Failed to resolve remote '{}': {}",
                    remote_name, e
                ))
            })?;

        let plugin_dir = if let Some(ref pp) = remote_config.plugin_path {
            cache_dir.join(pp)
        } else {
            cache_dir
        };

        Ok(vec![plugin_dir])
    } else if let Some(url) = url {
        let cache_dir = RemoteCache::resolve_url(url, None, cli_config.cache_max_age_hours)
            .map_err(|e| {
                tasker_client::ClientError::ConfigError(format!(
                    "Failed to resolve URL '{}': {}",
                    url, e
                ))
            })?;

        Ok(vec![cache_dir])
    } else {
        Ok(Vec::new())
    }
}

/// Resolve a remote or URL to a config source directory path.
///
/// Returns the path to the config directory within the cached remote.
pub(crate) fn resolve_remote_config_path(
    cli_config: &CliConfig,
    remote: Option<&str>,
    url: Option<&str>,
) -> tasker_client::ClientResult<Option<String>> {
    if let Some(remote_name) = remote {
        let remote_config = cli_config
            .remotes
            .iter()
            .find(|r| r.name == remote_name)
            .ok_or_else(|| {
                tasker_client::ClientError::ConfigError(format!(
                    "Remote '{}' not found in configuration",
                    remote_name
                ))
            })?;

        let cache_dir = RemoteCache::resolve(remote_config, cli_config.cache_max_age_hours)
            .map_err(|e| {
                tasker_client::ClientError::ConfigError(format!(
                    "Failed to resolve remote '{}': {}",
                    remote_name, e
                ))
            })?;

        let config_dir = cache_dir.join(&remote_config.config_path);
        validate_config_structure(&config_dir, remote_name)?;

        Ok(Some(config_dir.to_string_lossy().to_string()))
    } else if let Some(url) = url {
        let cache_dir = RemoteCache::resolve_url(url, None, cli_config.cache_max_age_hours)
            .map_err(|e| {
                tasker_client::ClientError::ConfigError(format!(
                    "Failed to resolve URL '{}': {}",
                    url, e
                ))
            })?;

        let config_dir = cache_dir.join("config/tasker/");
        validate_config_structure(&config_dir, url)?;

        Ok(Some(config_dir.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

/// Validate that a config directory has the expected structure.
fn validate_config_structure(
    config_dir: &std::path::Path,
    source_name: &str,
) -> tasker_client::ClientResult<()> {
    let base_dir = config_dir.join("base");
    if !base_dir.is_dir() {
        return Err(tasker_client::ClientError::ConfigError(format!(
            "Remote '{}' config path missing 'base/' directory at {}",
            source_name,
            config_dir.display()
        )));
    }

    let required_files = ["common.toml", "orchestration.toml", "worker.toml"];
    let mut missing = Vec::new();
    for file in &required_files {
        if !base_dir.join(file).is_file() {
            missing.push(*file);
        }
    }

    if !missing.is_empty() {
        tracing::warn!(
            remote = %source_name,
            missing = ?missing,
            "Remote config directory is missing expected base files"
        );
    }

    Ok(())
}

/// Resolve all configured remotes to plugin paths (for auto-discovery).
pub(crate) fn resolve_all_remote_plugin_paths(cli_config: &CliConfig) -> CliConfig {
    let mut extra_paths = Vec::new();

    for remote in &cli_config.remotes {
        match RemoteCache::resolve(remote, cli_config.cache_max_age_hours) {
            Ok(cache_dir) => {
                let plugin_dir = if let Some(ref pp) = remote.plugin_path {
                    cache_dir.join(pp)
                } else {
                    cache_dir
                };
                extra_paths.push(plugin_dir.to_string_lossy().to_string());
            }
            Err(e) => {
                tracing::warn!(
                    remote = %remote.name,
                    error = %e,
                    "Failed to resolve remote plugin path, skipping"
                );
            }
        }
    }

    let mut paths = cli_config.plugin_paths.clone();
    paths.extend(extra_paths);

    CliConfig {
        plugin_paths: paths,
        default_language: cli_config.default_language.clone(),
        default_output_dir: cli_config.default_output_dir.clone(),
        remotes: cli_config.remotes.clone(),
        cache_max_age_hours: cli_config.cache_max_age_hours,
    }
}
