//! Runtime template engine for plugin-provided templates.
//!
//! Uses Tera for runtime template rendering (complementing Askama's compile-time
//! templates used for built-in docs generation). Plugins provide `.tera` template
//! files and `template.toml` metadata describing parameters and output files.

mod engine;
mod filters;
mod loader;
mod metadata;

pub use engine::{EngineError, RenderedFile, TemplateEngine};
pub use metadata::{MetadataError, OutputFile, ParameterDef, TemplateMetadata};
