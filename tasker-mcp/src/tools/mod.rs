//! MCP tool parameter and response types.
//!
//! All parameter structs derive `Deserialize + JsonSchema` for MCP tool registration.
//! All response structs derive `Serialize` for JSON output.

pub mod params;

pub use params::*;
