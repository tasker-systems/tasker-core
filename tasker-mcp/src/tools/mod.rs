//! MCP tool parameter and response types, plus tier-organized tool implementations.
//!
//! All parameter structs derive `Deserialize + JsonSchema` for MCP tool registration.
//! All response structs derive `Serialize` for JSON output.

pub mod connected;
pub mod developer;
pub mod helpers;
pub mod params;
pub mod write;

pub use helpers::error_json;
pub use params::*;
