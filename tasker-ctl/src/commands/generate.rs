//! Code generation command handlers (TAS-280)
//!
//! CLI commands for generating typed models and handler scaffolds from task template definitions.

use std::path::PathBuf;
use tasker_client::ClientResult;

use crate::codegen::{self, TargetLanguage};
use crate::GenerateCommands;

pub(crate) async fn handle_generate_command(cmd: GenerateCommands) -> ClientResult<()> {
    match cmd {
        GenerateCommands::Types {
            template,
            language,
            output,
            step,
        } => handle_generate_types(&template, &language, output.as_deref(), step.as_deref()).await,
        GenerateCommands::Handler {
            template,
            language,
            output,
            step,
            with_tests,
        } => {
            handle_generate_handler(
                &template,
                &language,
                output.as_deref(),
                step.as_deref(),
                with_tests,
            )
            .await
        }
    }
}

/// Parse a template YAML file into a TaskTemplate.
fn load_template(
    template_path: &PathBuf,
) -> ClientResult<tasker_shared::models::core::task_template::TaskTemplate> {
    let yaml_content = std::fs::read_to_string(template_path).map_err(|e| {
        tasker_client::ClientError::config_error(format!(
            "Failed to read template file '{}': {}",
            template_path.display(),
            e
        ))
    })?;

    serde_yaml::from_str(&yaml_content).map_err(|e| {
        tasker_client::ClientError::config_error(format!(
            "Failed to parse template YAML '{}': {}",
            template_path.display(),
            e
        ))
    })
}

/// Write generated output to a file or stdout.
fn write_output(output: Option<&str>, generated: &str, label: &str) -> ClientResult<()> {
    if let Some(output_path) = output {
        let output_path = PathBuf::from(output_path);
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                tasker_client::ClientError::config_error(format!(
                    "Failed to create output directory '{}': {}",
                    parent.display(),
                    e
                ))
            })?;
        }
        std::fs::write(&output_path, generated).map_err(|e| {
            tasker_client::ClientError::config_error(format!(
                "Failed to write output file '{}': {}",
                output_path.display(),
                e
            ))
        })?;
        eprintln!("{label} written to {}", output_path.display());
    } else {
        print!("{generated}");
    }
    Ok(())
}

async fn handle_generate_types(
    template_path: &PathBuf,
    language: &str,
    output: Option<&str>,
    step: Option<&str>,
) -> ClientResult<()> {
    let target: TargetLanguage = language.parse().map_err(|e: codegen::CodegenError| {
        tasker_client::ClientError::config_error(e.to_string())
    })?;

    let template = load_template(template_path)?;

    let has_schemas =
        template.input_schema.is_some() || template.steps.iter().any(|s| s.result_schema.is_some());
    if !has_schemas {
        eprintln!(
            "Warning: '{}' has no input_schema and no steps with result_schema definitions.",
            template_path.display()
        );
        return Ok(());
    }

    let generated = codegen::generate_types(&template, target, step).map_err(|e| {
        tasker_client::ClientError::config_error(format!("Code generation failed: {e}"))
    })?;

    write_output(output, &generated, &format!("Generated {target} types"))
}

async fn handle_generate_handler(
    template_path: &PathBuf,
    language: &str,
    output: Option<&str>,
    step: Option<&str>,
    with_tests: bool,
) -> ClientResult<()> {
    let target: TargetLanguage = language.parse().map_err(|e: codegen::CodegenError| {
        tasker_client::ClientError::config_error(e.to_string())
    })?;

    let template = load_template(template_path)?;

    let mut generated = codegen::generate_handlers(&template, target, step).map_err(|e| {
        tasker_client::ClientError::config_error(format!("Handler generation failed: {e}"))
    })?;

    if with_tests {
        let tests = codegen::generate_tests(&template, target, step).map_err(|e| {
            tasker_client::ClientError::config_error(format!("Test generation failed: {e}"))
        })?;
        generated.push_str("\n\n");
        generated.push_str(&tests);
    }

    write_output(
        output,
        &generated,
        &format!("Generated {target} handler scaffolds"),
    )
}
