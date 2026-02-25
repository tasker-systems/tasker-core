//! Schema-driven code generation from task template `result_schema` definitions.
//!
//! Converts JSON Schema on `StepDefinition.result_schema` into typed models
//! for Python (Pydantic), Ruby (Dry::Struct), TypeScript (interfaces), and Rust (structs).
//! Also generates handler scaffolds with typed dependency wiring and test files.

pub mod handler;
pub mod handler_templates;
pub mod python;
pub mod ruby;
pub mod rust_gen;
pub mod schema;
pub mod typescript;
pub mod typescript_zod;

use schema::{SchemaError, TypeDef};
use std::fmt;
use tasker_shared::models::core::task_template::TaskTemplate;

/// Target language for code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetLanguage {
    Python,
    Ruby,
    TypeScript,
    Rust,
}

impl fmt::Display for TargetLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetLanguage::Python => write!(f, "python"),
            TargetLanguage::Ruby => write!(f, "ruby"),
            TargetLanguage::TypeScript => write!(f, "typescript"),
            TargetLanguage::Rust => write!(f, "rust"),
        }
    }
}

impl std::str::FromStr for TargetLanguage {
    type Err = CodegenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "python" | "py" => Ok(TargetLanguage::Python),
            "ruby" | "rb" => Ok(TargetLanguage::Ruby),
            "typescript" | "ts" => Ok(TargetLanguage::TypeScript),
            "rust" | "rs" => Ok(TargetLanguage::Rust),
            _ => Err(CodegenError::UnsupportedLanguage(s.to_string())),
        }
    }
}

/// Error during code generation.
#[derive(Debug, thiserror::Error)]
pub enum CodegenError {
    #[error("unsupported language: {0} (expected: python, ruby, typescript, rust)")]
    UnsupportedLanguage(String),

    #[error("schema extraction error for step '{step}': {source}")]
    SchemaExtraction {
        step: String,
        #[source]
        source: SchemaError,
    },

    #[error("template rendering error: {0}")]
    Rendering(String),
}

/// Generate typed models from a task template's `input_schema` and `result_schema` definitions.
///
/// When no step filter is active, extracts the task-level input type first,
/// then iterates all steps with `result_schema`. The input type appears first
/// in the output for natural reading order.
///
/// If `step_filter` is provided, only generates types for that step (no input type).
pub fn generate_types(
    template: &TaskTemplate,
    language: TargetLanguage,
    step_filter: Option<&str>,
) -> Result<String, CodegenError> {
    let mut all_types: Vec<TypeDef> = Vec::new();

    // Extract input types from template-level input_schema (skip when step-filtering)
    if step_filter.is_none() {
        if let Some(input_schema) = &template.input_schema {
            let input_types =
                schema::extract_input_types(&template.name, input_schema).map_err(|e| {
                    CodegenError::SchemaExtraction {
                        step: format!("{} (input_schema)", template.name),
                        source: e,
                    }
                })?;
            all_types.extend(input_types);
        }
    }

    for step in &template.steps {
        // Apply step filter if provided
        if let Some(filter) = step_filter {
            if step.name != filter {
                continue;
            }
        }

        if let Some(schema) = &step.result_schema {
            let types = schema::extract_types(&step.name, schema).map_err(|e| {
                CodegenError::SchemaExtraction {
                    step: step.name.clone(),
                    source: e,
                }
            })?;
            all_types.extend(types);
        }
    }

    match language {
        TargetLanguage::Python => python::render(&all_types),
        TargetLanguage::Ruby => ruby::render(&all_types),
        TargetLanguage::TypeScript => typescript_zod::render(&all_types),
        TargetLanguage::Rust => rust_gen::render(&all_types),
    }
}

/// Generate handler scaffolds from a task template.
///
/// Produces runnable handler code with typed `depends_on` wiring for each step.
/// If `step_filter` is provided, only generates handlers for that step.
pub fn generate_handlers(
    template: &TaskTemplate,
    language: TargetLanguage,
    step_filter: Option<&str>,
) -> Result<String, CodegenError> {
    let handlers = handler::extract_handlers(template, step_filter);
    let template_name = &template.name;

    handler_templates::render_handlers(&handlers, template_name, language)
}

/// Generate test scaffolds for handler functions.
///
/// Produces test files with mock dependency data derived from `result_schema`.
/// If `step_filter` is provided, only generates tests for that step.
pub fn generate_tests(
    template: &TaskTemplate,
    language: TargetLanguage,
    step_filter: Option<&str>,
) -> Result<String, CodegenError> {
    let handlers = handler::extract_handlers(template, step_filter);
    let template_name = &template.name;

    handler_templates::render_tests(&handlers, template_name, language)
}
