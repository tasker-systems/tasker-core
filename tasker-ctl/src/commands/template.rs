//! Template listing, info, and generation commands.

use std::collections::HashMap;
use std::path::Path;

use crate::cli_config::CliConfig;
use crate::output;
use crate::plugins::PluginRegistry;
use crate::template_engine::TemplateEngine;
use crate::TemplateCommands;

pub(crate) async fn handle_template_command(
    cmd: TemplateCommands,
    cli_config: &CliConfig,
) -> tasker_client::ClientResult<()> {
    match cmd {
        TemplateCommands::List {
            language,
            framework,
        } => list_templates(cli_config, language.as_deref(), framework.as_deref()),

        TemplateCommands::Info { name, plugin } => {
            show_template_info(cli_config, &name, plugin.as_deref())
        }

        TemplateCommands::Generate {
            template,
            param,
            language,
            plugin,
            output,
        } => generate_template(
            cli_config,
            &template,
            &param,
            language.as_deref(),
            plugin.as_deref(),
            output.as_deref(),
        ),
    }
}

fn list_templates(
    cli_config: &CliConfig,
    language: Option<&str>,
    framework: Option<&str>,
) -> tasker_client::ClientResult<()> {
    let registry = PluginRegistry::discover(cli_config);
    let templates = registry.find_templates(language, framework);

    if templates.is_empty() {
        output::warning("No templates found.");
        if language.is_some() || framework.is_some() {
            output::hint("Try without filters to see all available templates.");
        }
        return Ok(());
    }

    output::header("Available templates:");
    output::blank();
    for resolved in &templates {
        let plugin = &resolved.plugin.manifest.plugin;
        let fw = plugin.framework.as_deref().unwrap_or("-");
        output::item(format!(
            "{} [{}/{}] (plugin: {})",
            resolved.template.name, plugin.language, fw, plugin.name
        ));
        output::dim(format!("    {}", resolved.template.description));
    }
    output::blank();

    Ok(())
}

fn show_template_info(
    cli_config: &CliConfig,
    name: &str,
    plugin: Option<&str>,
) -> tasker_client::ClientResult<()> {
    let registry = PluginRegistry::discover(cli_config);

    let resolved = match registry.find_template_by_name(name, plugin) {
        Some(r) => r,
        None => {
            output::error(format!("Template '{name}' not found."));
            if let Some(pn) = plugin {
                output::dim(format!("  Searched in plugin: {pn}"));
            }
            std::process::exit(1);
        }
    };

    let p = &resolved.plugin.manifest.plugin;
    output::header(format!("Template: {}", resolved.template.name));
    output::label("Plugin", format!("{} (v{})", p.name, p.version));
    output::label("Language", &p.language);
    if let Some(fw) = &p.framework {
        output::label("Framework", fw);
    }
    output::label("Description", &resolved.template.description);

    // Try to load metadata for parameter info
    match TemplateEngine::load(&resolved.template_dir) {
        Ok(engine) => {
            let meta = engine.metadata();
            output::dim(format!(
                "  Template metadata: {} - {}",
                meta.name, meta.description
            ));
            if !meta.parameters.is_empty() {
                output::blank();
                output::header("Parameters:");
                for param in &meta.parameters {
                    let required = if param.required { " (required)" } else { "" };
                    let default = param
                        .default
                        .as_ref()
                        .map(|d| format!(" [default: {d}]"))
                        .unwrap_or_default();
                    output::plain(format!("  --param {}={}", param.name, param.description));
                    output::dim(format!("         {required}{default}"));
                }
            }
            if !meta.outputs.is_empty() {
                output::blank();
                output::header("Outputs:");
                for out in &meta.outputs {
                    let subdir = out
                        .subdir
                        .as_ref()
                        .map(|s| format!("{s}/"))
                        .unwrap_or_default();
                    output::plain(format!("  {subdir}{}", out.filename));
                }
            }
        }
        Err(e) => {
            tracing::debug!(error = %e, "Could not load template metadata");
            output::dim("  (No template.toml metadata available)");
        }
    }

    Ok(())
}

fn generate_template(
    cli_config: &CliConfig,
    template_name: &str,
    params: &[String],
    language: Option<&str>,
    plugin: Option<&str>,
    output_dir: Option<&str>,
) -> tasker_client::ClientResult<()> {
    let registry = PluginRegistry::discover(cli_config);

    // Resolve language: explicit --language > config default
    let effective_language = language.or(cli_config.default_language.as_deref());

    // If language specified but no plugin, try filtering by language first
    let resolved = if let Some(lang) = effective_language {
        let templates = registry.find_templates(Some(lang), None);
        templates
            .into_iter()
            .find(|r| r.template.name.eq_ignore_ascii_case(template_name))
    } else {
        registry.find_template_by_name(template_name, plugin)
    };

    let resolved = match resolved {
        Some(r) => r,
        None => {
            output::error(format!("Template '{template_name}' not found."));
            std::process::exit(1);
        }
    };

    // Parse --param key=value pairs
    let param_map = parse_params(params)?;

    // Load engine
    let engine = match TemplateEngine::load(&resolved.template_dir) {
        Ok(e) => e,
        Err(e) => {
            output::error(format!("Failed to load template: {e}"));
            std::process::exit(1);
        }
    };

    // Validate required params
    let errors = engine.metadata().validate_params(&param_map);
    if !errors.is_empty() {
        output::error("Parameter validation failed:");
        for err in &errors {
            output::error(format!("  - {err}"));
        }
        output::hint(format!(
            "Use 'tasker-ctl template info {template_name}' to see required parameters."
        ));
        std::process::exit(1);
    }

    // Render
    let rendered = match engine.render(&param_map) {
        Ok(r) => r,
        Err(e) => {
            output::error(format!("Template rendering failed: {e}"));
            std::process::exit(1);
        }
    };

    // Determine output directory
    let out_dir = output_dir
        .or(cli_config.default_output_dir.as_deref())
        .unwrap_or(".");

    // Write files
    for file in &rendered {
        let full_path = Path::new(out_dir).join(&file.path);
        if let Some(parent) = full_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    tasker_client::ClientError::ConfigError(format!(
                        "failed to create directory {}: {e}",
                        parent.display()
                    ))
                })?;
            }
        }
        std::fs::write(&full_path, &file.content).map_err(|e| {
            tasker_client::ClientError::ConfigError(format!(
                "failed to write {}: {e}",
                full_path.display()
            ))
        })?;
        output::success(format!("Created: {}", full_path.display()));
    }

    output::blank();
    output::success(format!(
        "Generated {} file(s) from template '{}'.",
        rendered.len(),
        template_name
    ));

    Ok(())
}

fn parse_params(params: &[String]) -> tasker_client::ClientResult<HashMap<String, String>> {
    let mut map = HashMap::new();
    for param in params {
        let (key, value) = param.split_once('=').ok_or_else(|| {
            tasker_client::ClientError::ConfigError(format!(
                "invalid parameter format: '{param}'. Expected key=value"
            ))
        })?;
        map.insert(key.to_string(), value.to_string());
    }
    Ok(map)
}
