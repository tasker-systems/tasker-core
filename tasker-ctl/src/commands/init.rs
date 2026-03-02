//! `tasker-ctl init` command: bootstrap a `.config/tasker.toml` with sensible defaults.

use std::path::{Path, PathBuf};

use askama::Template;

use crate::output;

/// Askama template for generating `.config/tasker.toml` (unified format, TAS-311).
#[derive(Template, Debug)]
#[template(path = "init-unified.toml")]
struct InitUnifiedTemplate {
    include_contrib: bool,
}

pub(crate) async fn handle_init_command(no_contrib: bool) -> tasker_client::ClientResult<()> {
    let config_dir = PathBuf::from(".config");
    let config_path = config_dir.join("tasker.toml");
    let legacy_path = Path::new(".tasker-ctl.toml");

    if config_path.exists() {
        output::warning(format!(
            "{} already exists in this directory.",
            config_path.display()
        ));
        output::hint("Remove it first if you want to reinitialize.");
        return Err(tasker_client::ClientError::ConfigError(format!(
            "{} already exists",
            config_path.display()
        )));
    }

    if legacy_path.exists() {
        output::warning(
            "Found legacy .tasker-ctl.toml — consider migrating to .config/tasker.toml.",
        );
        output::hint(
            "The new unified config combines CLI settings and connection profiles in one file.",
        );
    }

    // Ensure .config/ directory exists
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).map_err(|e| {
            tasker_client::ClientError::ConfigError(format!(
                "Failed to create .config/ directory: {e}"
            ))
        })?;
    }

    let template = InitUnifiedTemplate {
        include_contrib: !no_contrib,
    };
    let content = template.render().map_err(|e| {
        tasker_client::ClientError::ConfigError(format!("Template rendering failed: {}", e))
    })?;

    std::fs::write(&config_path, content).map_err(|e| {
        tasker_client::ClientError::ConfigError(format!(
            "Failed to write {}: {}",
            config_path.display(),
            e
        ))
    })?;

    output::success(format!("Created {}", config_path.display()));
    output::blank();

    if no_contrib {
        output::hint("Add a remote to get started:");
        output::plain(
            "  tasker-ctl remote add tasker-contrib https://github.com/tasker-systems/tasker-contrib.git",
        );
    } else {
        output::hint("Next steps:");
        output::plain(
            "  tasker-ctl remote update              # Fetch remote templates and config",
        );
        output::plain("  tasker-ctl template list               # Browse available templates");
        output::plain(
            "  tasker-ctl template generate step_handler --language ruby --param name=MyHandler",
        );
    }

    Ok(())
}
