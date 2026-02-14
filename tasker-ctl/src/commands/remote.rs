//! Remote management commands: list, add, remove, update.

use std::path::PathBuf;

use crate::cli_config::{CliConfig, RemoteConfig};
use crate::output;
use crate::remotes::RemoteCache;
use crate::RemoteCommands;

pub(crate) async fn handle_remote_command(
    cmd: RemoteCommands,
    cli_config: &CliConfig,
) -> tasker_client::ClientResult<()> {
    match cmd {
        RemoteCommands::List => list_remotes(cli_config),
        RemoteCommands::Add {
            name,
            url,
            git_ref,
            config_path,
            plugin_path,
        } => add_remote(&name, &url, &git_ref, &config_path, plugin_path.as_deref()),
        RemoteCommands::Remove { name } => remove_remote(&name),
        RemoteCommands::Update { name } => update_remotes(cli_config, name.as_deref()),
    }
}

fn list_remotes(cli_config: &CliConfig) -> tasker_client::ClientResult<()> {
    if cli_config.remotes.is_empty() {
        output::warning("No remotes configured.");
        output::blank();
        output::hint("Add a remote with:");
        output::plain(
            "  tasker-ctl remote add tasker-contrib https://github.com/tasker-systems/tasker-contrib.git",
        );
        return Ok(());
    }

    output::header("Configured remotes:");
    output::blank();

    for remote in &cli_config.remotes {
        let cache_dir = RemoteCache::cache_dir(&remote.name).ok();
        let cache_status = match &cache_dir {
            Some(dir) if dir.exists() => {
                let age = RemoteCache::last_fetch_time(dir)
                    .and_then(|t| t.elapsed().ok())
                    .map(format_duration);
                match age {
                    Some(age_str) => format!("cached (last fetched {})", age_str),
                    None => "cached (unknown age)".to_string(),
                }
            }
            _ => "not cached".to_string(),
        };

        output::success(format!("{} [{}]", remote.name, cache_status));
        output::label("    URL", &remote.url);
        output::label("    Ref", &remote.git_ref);
        output::label("    Config path", &remote.config_path);
        if let Some(plugin_path) = &remote.plugin_path {
            output::label("    Plugin path", plugin_path);
        }
        output::blank();
    }

    Ok(())
}

fn add_remote(
    name: &str,
    url: &str,
    git_ref: &str,
    config_path: &str,
    plugin_path: Option<&str>,
) -> tasker_client::ClientResult<()> {
    // Find and read existing config file, or create new one
    let config_file = find_or_create_config_file()?;
    let contents = std::fs::read_to_string(&config_file).unwrap_or_default();

    // Parse with toml_edit to preserve formatting
    let mut doc: toml_edit::DocumentMut = contents.parse().map_err(|e| {
        tasker_client::ClientError::ConfigError(format!("Failed to parse config file: {}", e))
    })?;

    // Check for duplicate name
    if let Some(remotes) = doc.get("remotes").and_then(|v| v.as_array_of_tables()) {
        for remote in remotes.iter() {
            if remote.get("name").and_then(|v| v.as_str()) == Some(name) {
                return Err(tasker_client::ClientError::ConfigError(format!(
                    "Remote '{}' already exists in configuration",
                    name
                )));
            }
        }
    }

    // Build new remote table
    let mut table = toml_edit::Table::new();
    table.insert("name", toml_edit::value(name));
    table.insert("url", toml_edit::value(url));
    if git_ref != "main" {
        table.insert("git-ref", toml_edit::value(git_ref));
    }
    if config_path != "config/tasker/" {
        table.insert("config-path", toml_edit::value(config_path));
    }
    if let Some(pp) = plugin_path {
        table.insert("plugin-path", toml_edit::value(pp));
    }

    // Append to [[remotes]] array
    if doc.get("remotes").is_none() {
        let arr = toml_edit::ArrayOfTables::new();
        doc.insert("remotes", toml_edit::Item::ArrayOfTables(arr));
    }
    if let Some(remotes) = doc
        .get_mut("remotes")
        .and_then(|v| v.as_array_of_tables_mut())
    {
        remotes.push(table);
    }

    // Write back
    std::fs::write(&config_file, doc.to_string()).map_err(|e| {
        tasker_client::ClientError::ConfigError(format!("Failed to write config file: {}", e))
    })?;

    output::success(format!("Added remote '{}'", name));
    output::label("  Config file", config_file.display());
    output::blank();
    output::hint(format!(
        "Run `tasker-ctl remote update {}` to fetch the remote.",
        name
    ));

    Ok(())
}

fn remove_remote(name: &str) -> tasker_client::ClientResult<()> {
    // Remove from config file
    let config_file = find_config_file()?;
    let contents = std::fs::read_to_string(&config_file).map_err(|e| {
        tasker_client::ClientError::ConfigError(format!("Failed to read config file: {}", e))
    })?;

    let mut doc: toml_edit::DocumentMut = contents.parse().map_err(|e| {
        tasker_client::ClientError::ConfigError(format!("Failed to parse config file: {}", e))
    })?;

    // Find and remove the remote from [[remotes]]
    let mut found = false;
    if let Some(remotes) = doc
        .get_mut("remotes")
        .and_then(|v| v.as_array_of_tables_mut())
    {
        let mut i = 0;
        while i < remotes.len() {
            if remotes
                .get(i)
                .and_then(|t| t.get("name"))
                .and_then(|v| v.as_str())
                == Some(name)
            {
                remotes.remove(i);
                found = true;
                break;
            }
            i += 1;
        }

        // Remove the key entirely if no remotes left
        let is_empty = remotes.is_empty();
        if is_empty {
            doc.remove("remotes");
        }
    }

    if !found {
        return Err(tasker_client::ClientError::ConfigError(format!(
            "Remote '{}' not found in configuration",
            name
        )));
    }

    std::fs::write(&config_file, doc.to_string()).map_err(|e| {
        tasker_client::ClientError::ConfigError(format!("Failed to write config file: {}", e))
    })?;

    // Remove cache
    match RemoteCache::remove(name) {
        Ok(()) => {
            output::success(format!("Removed remote '{}' and its cache.", name));
        }
        Err(e) => {
            output::success(format!("Removed remote '{}' from config.", name));
            output::warning(format!("Failed to remove cache: {}", e));
        }
    }

    Ok(())
}

fn update_remotes(cli_config: &CliConfig, name: Option<&str>) -> tasker_client::ClientResult<()> {
    let remotes: Vec<&RemoteConfig> = if let Some(name) = name {
        let remote = cli_config.remotes.iter().find(|r| r.name == name);
        match remote {
            Some(r) => vec![r],
            None => {
                return Err(tasker_client::ClientError::ConfigError(format!(
                    "Remote '{}' not found in configuration",
                    name
                )));
            }
        }
    } else {
        cli_config.remotes.iter().collect()
    };

    if remotes.is_empty() {
        output::warning("No remotes configured to update.");
        return Ok(());
    }

    for remote in &remotes {
        output::dim(format!("Updating remote '{}'...", remote.name));
        match RemoteCache::update(remote) {
            Ok(path) => {
                output::success(format!("Updated '{}' → {}", remote.name, path.display()));
            }
            Err(e) => {
                output::error(format!("Failed to update '{}': {}", remote.name, e));
            }
        }
    }

    Ok(())
}

/// Find the .tasker-ctl.toml config file, or error if it doesn't exist.
fn find_config_file() -> tasker_client::ClientResult<PathBuf> {
    let local = PathBuf::from(".tasker-ctl.toml");
    if local.is_file() {
        return Ok(local);
    }

    if let Ok(home) = std::env::var("HOME") {
        let global = PathBuf::from(home).join(".config").join("tasker-ctl.toml");
        if global.is_file() {
            return Ok(global);
        }
    }

    Err(tasker_client::ClientError::ConfigError(
        "No .tasker-ctl.toml found. Create one in the current directory or at ~/.config/tasker-ctl.toml".to_string(),
    ))
}

/// Find or create the .tasker-ctl.toml config file (creates local by default).
fn find_or_create_config_file() -> tasker_client::ClientResult<PathBuf> {
    match find_config_file() {
        Ok(path) => Ok(path),
        Err(_) => {
            let path = PathBuf::from(".tasker-ctl.toml");
            std::fs::write(&path, "").map_err(|e| {
                tasker_client::ClientError::ConfigError(format!(
                    "Failed to create .tasker-ctl.toml: {}",
                    e
                ))
            })?;
            Ok(path)
        }
    }
}

/// Format a duration in a human-readable way.
fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}
