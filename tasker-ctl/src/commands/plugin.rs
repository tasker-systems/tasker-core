//! Plugin management commands.

use crate::cli_config::CliConfig;
use crate::output;
use crate::plugins::{PluginManifest, PluginRegistry};
use crate::PluginCommands;

pub(crate) async fn handle_plugin_command(
    cmd: PluginCommands,
    cli_config: &CliConfig,
) -> tasker_client::ClientResult<()> {
    match cmd {
        PluginCommands::List => list_plugins(cli_config),
        PluginCommands::Validate { path } => validate_plugin(&path),
    }
}

fn list_plugins(cli_config: &CliConfig) -> tasker_client::ClientResult<()> {
    let registry = PluginRegistry::discover(cli_config);
    let plugins = registry.plugins();

    if plugins.is_empty() {
        output::warning("No plugins discovered.");
        if cli_config.plugin_paths.is_empty() {
            output::blank();
            output::hint("Configure plugin paths in .tasker-ctl.toml:");
            output::blank();
            output::plain(
                "  plugin-paths = [\"./tasker-cli-plugins\", \"~/projects/tasker-contrib\"]",
            );
        } else {
            output::blank();
            output::plain("Searched paths:");
            for path in &cli_config.plugin_paths {
                output::plain(format!("  - {path}"));
            }
        }
        return Ok(());
    }

    output::header("Discovered plugins:");
    output::blank();
    for plugin in plugins {
        let m = &plugin.manifest.plugin;
        let framework = m.framework.as_deref().unwrap_or("-");
        output::success(format!(
            "{} (v{}) [{}/{}]",
            m.name, m.version, m.language, framework
        ));
        output::dim(format!("    {}", m.description));
        output::label("    Path", plugin.dir.display());

        if !plugin.manifest.templates.is_empty() {
            output::plain("    Templates:");
            for tmpl in &plugin.manifest.templates {
                output::plain(format!("      - {}: {}", tmpl.name, tmpl.description));
            }
        }
        output::blank();
    }

    Ok(())
}

fn validate_plugin(path: &str) -> tasker_client::ClientResult<()> {
    let dir = std::path::Path::new(path);

    if !dir.is_dir() {
        output::error(format!("'{path}' is not a directory"));
        std::process::exit(1);
    }

    match PluginManifest::load(dir) {
        Ok(manifest) => {
            output::success(format!("Plugin manifest loaded: {}", manifest.plugin.name));
            output::label("  Description", &manifest.plugin.description);
            output::label("  Version", &manifest.plugin.version);
            output::label("  Language", &manifest.plugin.language);
            if let Some(fw) = &manifest.plugin.framework {
                output::label("  Framework", fw);
            }
            output::label("  Templates", manifest.templates.len());

            let errors = manifest.validate(dir);
            if errors.is_empty() {
                output::blank();
                output::success("Validation passed.");
            } else {
                output::blank();
                output::error("Validation errors:");
                for err in &errors {
                    output::error(format!("  - {err}"));
                }
                std::process::exit(1);
            }
        }
        Err(e) => {
            output::error(format!("Error loading plugin: {e}"));
            std::process::exit(1);
        }
    }

    Ok(())
}
