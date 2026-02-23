//! Askama template structs for rendering handler scaffolds and test files.
//!
//! Each language has a handler template and a test template, rendered from
//! `HandlerDef` IR extracted by `handler::extract_handlers()`.

use super::handler::HandlerDef;
use super::{CodegenError, TargetLanguage};

/// Render handler scaffolds for the given language.
pub fn render_handlers(
    _handlers: &[HandlerDef],
    _template_name: &str,
    _language: TargetLanguage,
) -> Result<String, CodegenError> {
    // Placeholder — Askama templates wired in commit 3
    Ok(String::new())
}

/// Render test scaffolds for the given language.
pub fn render_tests(
    _handlers: &[HandlerDef],
    _template_name: &str,
    _language: TargetLanguage,
) -> Result<String, CodegenError> {
    // Placeholder — Askama templates wired in commit 4
    Ok(String::new())
}
