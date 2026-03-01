//! Shared SDK for Tasker: code generation, template parsing, schema inspection, and operational tooling.
//!
//! This crate provides the core tooling capabilities consumed by both `tasker-ctl` (CLI)
//! and `tasker-mcp` (MCP server). It depends on `tasker-shared` for model types.
//!
//! # Modules
//!
//! ## Developer Tooling
//! - [`codegen`] — Schema-driven code generation from task template `result_schema` definitions
//! - [`schema_comparator`] — Producer/consumer schema compatibility checking
//! - [`schema_diff`] — Temporal schema diff between template versions
//! - [`schema_inspector`] — Result schema contract inspection utilities
//! - [`template_engine`] — Tera-based runtime template rendering for plugin templates
//! - [`template_generator`] — Structured spec → task template YAML generation
//! - [`template_parser`] — Task template YAML parsing with rich error reporting
//! - [`template_validator`] — Structural validation, cycle detection, and DAG analysis
//!
//! ## Operational Tooling
//! - [`operational`] — Client factory, enum parsing, and shared response types for connected tools

pub mod codegen;
pub mod operational;
pub mod schema_comparator;
pub mod schema_diff;
pub mod schema_inspector;
pub mod template_engine;
pub mod template_generator;
pub mod template_parser;
pub mod template_validator;
