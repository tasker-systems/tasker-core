//! PostgreSQL resource handle backed by `sqlx::PgPool`.
//!
//! Requires the `postgres` feature flag.

use std::any::Any;
use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use super::config_value::ResourceConfig;
use super::error::ResourceError;
use super::handle::ResourceHandle;
use super::types::ResourceType;
use crate::secrets::SecretsProvider;

/// A live handle to a PostgreSQL database, wrapping a [`PgPool`].
///
/// Created from a [`ResourceConfig`] via [`PostgresHandle::from_config`], which
/// resolves host, port, database, user, and password (including secret
/// references) and builds a connection pool.
#[derive(Debug)]
pub struct PostgresHandle {
    name: String,
    resource_type: ResourceType,
    pool: Arc<PgPool>,
    #[expect(dead_code, reason = "stored for future credential refresh support")]
    config: ResourceConfig,
}

impl PostgresHandle {
    /// Build a `PostgresHandle` from a [`ResourceConfig`].
    ///
    /// # Required config keys
    ///
    /// - `host` — the database hostname
    /// - `database` — the database name
    ///
    /// # Optional config keys
    ///
    /// - `port` — defaults to `5432`
    /// - `user` — omitted from URL if absent
    /// - `password` — omitted from URL if absent
    /// - `max_connections` — pool maximum, defaults to `10`
    /// - `min_connections` — pool minimum, defaults to `1`
    pub async fn from_config(
        name: &str,
        config: &ResourceConfig,
        secrets: &dyn SecretsProvider,
    ) -> Result<Self, ResourceError> {
        let host = config.resolve_value("host", secrets).await.map_err(|e| {
            ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("missing or unresolvable 'host': {e}"),
            }
        })?;

        let database = config
            .resolve_value("database", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("missing or unresolvable 'database': {e}"),
            })?;

        let port = config
            .resolve_optional("port", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("unresolvable 'port': {e}"),
            })?
            .unwrap_or_else(|| "5432".to_string());

        let user = config
            .resolve_optional("user", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("unresolvable 'user': {e}"),
            })?;

        let password = config
            .resolve_optional("password", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("unresolvable 'password': {e}"),
            })?;

        let max_connections: u32 = config
            .resolve_optional("max_connections", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("unresolvable 'max_connections': {e}"),
            })?
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

        let min_connections: u32 = config
            .resolve_optional("min_connections", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("unresolvable 'min_connections': {e}"),
            })?
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);

        // Build connection URL: postgresql://[user[:password]@]host:port/database
        let auth_part = match (&user, &password) {
            (Some(u), Some(p)) => format!("{u}:{p}@"),
            (Some(u), None) => format!("{u}@"),
            _ => String::new(),
        };
        let url = format!("postgresql://{auth_part}{host}:{port}/{database}");

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .connect(&url)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("pool connection failed: {e}"),
            })?;

        Ok(Self {
            name: name.to_string(),
            resource_type: ResourceType::Postgres,
            pool: Arc::new(pool),
            config: config.clone(),
        })
    }

    /// Returns a reference to the underlying connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl ResourceHandle for PostgresHandle {
    fn resource_name(&self) -> &str {
        &self.name
    }

    fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }

    async fn refresh_credentials(
        &self,
        _secrets: &dyn SecretsProvider,
    ) -> Result<(), ResourceError> {
        tracing::warn!(
            resource = %self.name,
            "credential refresh requested but not yet implemented for PostgresHandle"
        );
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ResourceError> {
        sqlx::query("SELECT 1")
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| ResourceError::HealthCheckFailed {
                name: self.name.clone(),
                message: e.to_string(),
            })?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
