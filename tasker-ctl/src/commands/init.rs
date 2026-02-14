//! `tasker-ctl init` command: bootstrap a `.tasker-ctl.toml` with sensible defaults.

use std::path::Path;

use askama::Template;

use crate::output;

/// Askama template for generating `.tasker-ctl.toml`.
#[derive(Template, Debug)]
#[template(path = "init-config.toml")]
struct InitConfigTemplate {
    include_contrib: bool,
}

pub(crate) async fn handle_init_command(no_contrib: bool) -> tasker_client::ClientResult<()> {
    let config_path = Path::new(".tasker-ctl.toml");

    if config_path.exists() {
        output::warning(".tasker-ctl.toml already exists in this directory.");
        output::hint("Remove it first if you want to reinitialize.");
        return Err(tasker_client::ClientError::ConfigError(
            ".tasker-ctl.toml already exists".to_string(),
        ));
    }

    let template = InitConfigTemplate {
        include_contrib: !no_contrib,
    };
    let content = template.render().map_err(|e| {
        tasker_client::ClientError::ConfigError(format!("Template rendering failed: {}", e))
    })?;

    std::fs::write(config_path, content).map_err(|e| {
        tasker_client::ClientError::ConfigError(format!("Failed to write .tasker-ctl.toml: {}", e))
    })?;

    output::success("Created .tasker-ctl.toml");
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
