//! Operational tooling for connected Tasker environments.
//!
//! Provides shared client construction, enum parsing, and response types
//! consumed by both `tasker-mcp` (JSON serialization) and `tasker-ctl` (terminal formatting).

pub mod client_factory;
pub mod enums;
pub mod responses;
