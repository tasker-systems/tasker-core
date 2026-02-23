//! Code generation command handlers (TAS-280)
//!
//! CLI commands for generating typed models from task template `result_schema` definitions.

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
    }
}

async fn handle_generate_types(
    template_path: &PathBuf,
    language: &str,
    output: Option<&str>,
    step: Option<&str>,
) -> ClientResult<()> {
    // Parse target language
    let target: TargetLanguage = language.parse().map_err(|e: codegen::CodegenError| {
        tasker_client::ClientError::config_error(e.to_string())
    })?;

    // Read and parse the template YAML
    let yaml_content = std::fs::read_to_string(template_path).map_err(|e| {
        tasker_client::ClientError::config_error(format!(
            "Failed to read template file '{}': {}",
            template_path.display(),
            e
        ))
    })?;

    let template: tasker_shared::models::core::task_template::TaskTemplate =
        serde_yaml::from_str(&yaml_content).map_err(|e| {
            tasker_client::ClientError::config_error(format!(
                "Failed to parse template YAML '{}': {}",
                template_path.display(),
                e
            ))
        })?;

    // Check if any steps have result_schema
    let has_schemas = template.steps.iter().any(|s| s.result_schema.is_some());

    if !has_schemas {
        eprintln!(
            "Warning: No steps in '{}' have result_schema definitions.",
            template_path.display()
        );
        return Ok(());
    }

    // Generate types
    let generated = codegen::generate_types(&template, target, step).map_err(|e| {
        tasker_client::ClientError::config_error(format!("Code generation failed: {e}"))
    })?;

    // Write output
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
        std::fs::write(&output_path, &generated).map_err(|e| {
            tasker_client::ClientError::config_error(format!(
                "Failed to write output file '{}': {}",
                output_path.display(),
                e
            ))
        })?;
        eprintln!(
            "Generated {} types written to {}",
            target,
            output_path.display()
        );
    } else {
        print!("{generated}");
    }

    Ok(())
}
