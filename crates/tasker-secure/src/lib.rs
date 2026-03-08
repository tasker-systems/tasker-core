//! Strategy-pattern secrets resolution, resource lifecycle, and data protection
//! for Tasker workflows.
//!
//! # Module structure
//!
//! - [`secrets`] — `SecretsProvider` trait and implementations: environment variables,
//!   chained resolution, SOPS-encrypted files (TAS-357)
//! - [`config`] — `ConfigString` type for transparent credential resolution in
//!   configuration values (TAS-357)
//! - [`resource`] — Resource lifecycle management with automatic credential
//!   rotation (TAS-358, future)
//! - [`classification`] — Data sensitivity classification and policy
//!   enforcement (TAS-360, future)
//! - [`encryption`] — Field-level encryption for sensitive workflow data
//!   (TAS-359, future)
//!
//! # Design principles
//!
//! - **Strategy pattern**: pluggable secret backends via `SecretsProvider` trait.
//! - **Zero-copy secrets**: `SecretValue` wraps `secrecy::SecretString` to prevent
//!   accidental logging.
//! - **No infrastructure dependencies**: no database, messaging, or orchestration coupling.
//! - **Independently testable**: `cargo test -p tasker-secure` with no services running.

pub mod classification;
pub mod config;
pub mod encryption;
pub mod resource;
pub mod secrets;

#[cfg(any(test, feature = "test-utils"))]
pub mod testing;

pub use config::ConfigString;
pub use resource::{
    ConfigValue, ResourceConfig, ResourceDefinition, ResourceError, ResourceSummary, ResourceType,
};
#[cfg(feature = "sops")]
pub use secrets::SopsSecretsProvider;
pub use secrets::{
    ChainedSecretsProvider, EnvSecretsProvider, SecretValue, SecretsError, SecretsProvider,
};
