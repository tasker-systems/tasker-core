//! Core resource types: `ResourceType`, `ResourceDefinition`, `ResourceSummary`.

use std::fmt;

use serde::Deserialize;

use super::config_value::ResourceConfig;

/// The kind of infrastructure resource.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    /// PostgreSQL database connection.
    Postgres,
    /// HTTP/HTTPS endpoint.
    Http,
    /// PGMQ message queue.
    Pgmq,
    /// User-defined resource type.
    Custom {
        /// The name of the custom resource type.
        type_name: String,
    },
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres => write!(f, "postgres"),
            Self::Http => write!(f, "http"),
            Self::Pgmq => write!(f, "pgmq"),
            Self::Custom { type_name } => write!(f, "{type_name}"),
        }
    }
}

/// A complete resource definition, typically deserialized from TOML config.
#[derive(Debug, Clone, Deserialize)]
pub struct ResourceDefinition {
    /// Unique name for this resource (e.g., `"primary_db"`).
    pub name: String,
    /// The kind of resource.
    pub resource_type: ResourceType,
    /// Key-value configuration for the resource.
    #[serde(default)]
    pub config: ResourceConfig,
    /// Optional name of the secrets provider to use for this resource.
    pub secrets_provider: Option<String>,
}

/// A lightweight summary of a resource's current state.
#[derive(Debug, Clone)]
pub struct ResourceSummary {
    /// Resource name.
    pub name: String,
    /// Resource type.
    pub resource_type: ResourceType,
    /// Whether the most recent health check passed.
    pub healthy: bool,
}
