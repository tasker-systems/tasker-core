//! Configuration Documentation Generation (TAS-175)
//!
//! Askama-based documentation generation for Tasker's configuration system.
//! Produces markdown references, annotated TOML examples, and CLI explain output.
//!
//! ## Architecture
//!
//! ```text
//! config/tasker/base/*.toml   →  DocContextBuilder  →  Askama Templates  →  Output
//! (_docs metadata + defaults)    (unified context)     (compile-time)       (md/toml/txt)
//! ```
//!
//! ## Module Structure
//!
//! - `templates` — Askama `#[derive(Template)]` struct definitions

pub(crate) mod templates;

pub(crate) use templates::{
    AnnotatedConfigTemplate, ConfigReferenceTemplate, DocIndexTemplate, ParameterExplainTemplate,
    SectionDetailTemplate,
};
