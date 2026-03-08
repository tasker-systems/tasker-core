//! Secrets provider trait and implementations.
//!
//! This module defines the [`SecretsProvider`] trait for pluggable secret
//! resolution and provides concrete implementations:
//!
//! - [`EnvSecretsProvider`] — resolves secrets from environment variables
//! - [`ChainedSecretsProvider`] — chains multiple providers with fallback
//! - [`SopsSecretsProvider`] — resolves secrets from SOPS-encrypted files
//!   (requires `sops` feature)

mod chained;
mod env;
mod error;
mod value;

#[cfg(feature = "sops")]
mod sops;
