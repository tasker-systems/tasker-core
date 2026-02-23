//! Python Pydantic BaseModel code generation.

use super::schema::TypeDef;
use super::CodegenError;

/// Render Python Pydantic models from type definitions.
pub fn render(_types: &[TypeDef]) -> Result<String, CodegenError> {
    todo!("Python code generation will be implemented in the next commit")
}
