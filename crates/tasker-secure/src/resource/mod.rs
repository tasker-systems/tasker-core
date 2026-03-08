//! Resource lifecycle management with automatic credential rotation.
//!
//! This module provides the foundation for managing infrastructure resources
//! (databases, HTTP endpoints, message queues) with configuration values that
//! can transparently resolve secrets and environment variables.
//!
//! # Types
//!
//! - [`ResourceType`] — the kind of infrastructure resource (Postgres, HTTP, PGMQ, custom).
//! - [`ResourceDefinition`] — a complete resource definition from configuration.
//! - [`ResourceConfig`] — key-value configuration with secret/env resolution.
//! - [`ConfigValue`] — a single config value: literal, secret ref, or env ref.
//! - [`ResourceSummary`] — lightweight health summary of a resource.
//! - [`ResourceError`] — errors during resource lifecycle operations.

mod config_value;
mod error;
mod types;

pub use config_value::{ConfigValue, ResourceConfig};
pub use error::ResourceError;
pub use types::{ResourceDefinition, ResourceSummary, ResourceType};
