//! Rust struct code generation.

use super::schema::TypeDef;
use super::CodegenError;

/// Render Rust structs from type definitions.
pub fn render(_types: &[TypeDef]) -> Result<String, CodegenError> {
    todo!("Rust code generation will be implemented in the next commit")
}
