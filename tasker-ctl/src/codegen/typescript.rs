//! TypeScript interface code generation.

use super::schema::TypeDef;
use super::CodegenError;

/// Render TypeScript interfaces from type definitions.
pub fn render(_types: &[TypeDef]) -> Result<String, CodegenError> {
    todo!("TypeScript code generation will be implemented in the next commit")
}
