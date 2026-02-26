//! Shared developer tooling for Tasker: code generation, template parsing, and schema inspection.
//!
//! This crate provides the core tooling capabilities consumed by both `tasker-ctl` (CLI)
//! and `tasker-mcp` (MCP server). It depends on `tasker-shared` for model types but
//! contains no runtime orchestration logic.
//!
//! # Modules
//!
//! - [`codegen`] — Schema-driven code generation from task template `result_schema` definitions
//! - [`template_engine`] — Tera-based runtime template rendering for plugin templates
//! - [`template_parser`] — Task template YAML parsing with rich error reporting
//! - [`schema_inspector`] — Result schema contract inspection utilities

pub mod codegen;
pub mod schema_inspector;
pub mod template_engine;
pub mod template_parser;
