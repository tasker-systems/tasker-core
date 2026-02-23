//! Ruby Dry::Struct code generation.

use super::schema::TypeDef;
use super::CodegenError;

/// Render Ruby Dry::Struct classes from type definitions.
pub fn render(_types: &[TypeDef]) -> Result<String, CodegenError> {
    todo!("Ruby code generation will be implemented in the next commit")
}
