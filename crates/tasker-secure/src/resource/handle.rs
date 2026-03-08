//! `ResourceHandle` trait for managing live infrastructure resources.
//!
//! Each resource handle provides a uniform interface for credential refresh,
//! health checking, and type-safe downcasting to concrete implementations.

use std::any::Any;
use std::fmt;

use super::error::ResourceError;
use super::types::ResourceType;
use crate::secrets::SecretsProvider;

/// A live handle to an infrastructure resource.
///
/// Implementors wrap connection pools, HTTP clients, or other resource-specific
/// state and expose a uniform interface for credential rotation and health
/// monitoring.
#[async_trait::async_trait]
pub trait ResourceHandle: Send + Sync + fmt::Debug {
    /// The unique name of this resource (matches `ResourceDefinition::name`).
    fn resource_name(&self) -> &str;

    /// The resource type (Postgres, HTTP, PGMQ, or custom).
    fn resource_type(&self) -> &ResourceType;

    /// Refresh the resource's credentials from a secrets provider.
    ///
    /// Implementations should re-read secrets and update their internal
    /// connection parameters without dropping existing connections.
    async fn refresh_credentials(&self, secrets: &dyn SecretsProvider)
        -> Result<(), ResourceError>;

    /// Check whether the resource is healthy (e.g., connection is alive).
    async fn health_check(&self) -> Result<(), ResourceError>;

    /// Downcast support — returns `self` as `&dyn Any` for type-safe downcasting.
    fn as_any(&self) -> &dyn Any;
}

/// Extension trait providing typed downcast helpers for [`ResourceHandle`].
///
/// Blanket-implemented for all `ResourceHandle` types. The `as_postgres()` and
/// `as_http()` methods are feature-gated — they return `None` when the
/// corresponding concrete handle type is not compiled in.
pub trait ResourceHandleExt {
    /// Attempt to downcast to a `PostgresHandle`.
    ///
    /// Returns `Some` only when the `postgres` feature is enabled **and** the
    /// underlying handle is actually a `PostgresHandle`.
    #[cfg(feature = "postgres")]
    fn as_postgres(&self) -> Option<&super::postgres::PostgresHandle>;

    /// Fallback: always returns `None` when the `postgres` feature is disabled.
    #[cfg(not(feature = "postgres"))]
    fn as_postgres(&self) -> Option<&dyn Any>;

    /// Attempt to downcast to an `HttpHandle`.
    ///
    /// Returns `Some` only when the `http` feature is enabled **and** the
    /// underlying handle is actually an `HttpHandle`.
    #[cfg(feature = "http")]
    fn as_http(&self) -> Option<&super::http::HttpHandle>;

    /// Fallback: always returns `None` when the `http` feature is disabled.
    #[cfg(not(feature = "http"))]
    fn as_http(&self) -> Option<&dyn Any>;
}

impl<T: ResourceHandle + ?Sized> ResourceHandleExt for T {
    #[cfg(feature = "postgres")]
    fn as_postgres(&self) -> Option<&super::postgres::PostgresHandle> {
        self.as_any()
            .downcast_ref::<super::postgres::PostgresHandle>()
    }

    #[cfg(not(feature = "postgres"))]
    fn as_postgres(&self) -> Option<&dyn Any> {
        None
    }

    #[cfg(feature = "http")]
    fn as_http(&self) -> Option<&super::http::HttpHandle> {
        self.as_any().downcast_ref::<super::http::HttpHandle>()
    }

    #[cfg(not(feature = "http"))]
    fn as_http(&self) -> Option<&dyn Any> {
        None
    }
}
