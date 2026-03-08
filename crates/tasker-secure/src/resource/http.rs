//! HTTP resource handle with pluggable authentication strategies.
//!
//! Provides [`HttpHandle`] for managing HTTP client connections with automatic
//! authentication via [`HttpAuthStrategy`] implementations.

use std::any::Any;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use super::config_value::ResourceConfig;
use super::error::ResourceError;
use super::handle::ResourceHandle;
use super::types::ResourceType;
use crate::secrets::SecretsProvider;

/// Strategy for applying authentication to outgoing HTTP requests.
///
/// Implementations add auth headers, bearer tokens, or other credentials
/// to a [`reqwest::RequestBuilder`] before the request is sent.
pub trait HttpAuthStrategy: Send + Sync + fmt::Debug {
    /// Apply authentication to the given request builder.
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder;
}

/// API key authentication strategy.
///
/// Adds a custom header (e.g., `X-API-Key`) with the given value to every
/// outgoing request.
#[derive(Debug, Clone)]
pub struct ApiKeyAuthStrategy {
    header: String,
    value: String,
}

impl ApiKeyAuthStrategy {
    /// Create a new API key strategy with the given header name and value.
    pub fn new(header: &str, value: &str) -> Self {
        Self {
            header: header.to_string(),
            value: value.to_string(),
        }
    }
}

impl HttpAuthStrategy for ApiKeyAuthStrategy {
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.header(&self.header, &self.value)
    }
}

/// Bearer token authentication strategy.
///
/// Adds an `Authorization: Bearer <token>` header to every outgoing request.
#[derive(Debug, Clone)]
pub struct BearerTokenAuthStrategy {
    token: String,
}

impl BearerTokenAuthStrategy {
    /// Create a new bearer token strategy.
    pub fn new(token: &str) -> Self {
        Self {
            token: token.to_string(),
        }
    }
}

impl HttpAuthStrategy for BearerTokenAuthStrategy {
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.bearer_auth(&self.token)
    }
}

/// No-op authentication strategy (pass-through).
#[derive(Debug, Clone)]
struct NoAuthStrategy;

impl HttpAuthStrategy for NoAuthStrategy {
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
    }
}

/// A live handle to an HTTP endpoint with automatic authentication.
///
/// Created from a [`ResourceConfig`] via [`HttpHandle::from_config`]. The handle
/// wraps a [`reqwest::Client`] and an [`HttpAuthStrategy`], applying authentication
/// to every outgoing request.
///
/// # Config keys
///
/// | Key | Required | Description |
/// |-----|----------|-------------|
/// | `base_url` | Yes | Base URL for all requests |
/// | `auth_type` | No | `"api_key"` or `"bearer"` |
/// | `auth_header` | No | Header name for API key auth (default: `X-API-Key`) |
/// | `auth_value` | No | API key or bearer token value |
/// | `timeout_ms` | No | Request timeout in milliseconds (default: 30000) |
pub struct HttpHandle {
    name: String,
    client: Arc<reqwest::Client>,
    base_url: String,
    auth: Arc<dyn HttpAuthStrategy>,
    resource_type: ResourceType,
    #[expect(dead_code, reason = "stored for future credential refresh support")]
    config: ResourceConfig,
}

impl fmt::Debug for HttpHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpHandle")
            .field("name", &self.name)
            .field("base_url", &self.base_url)
            .field("auth", &self.auth)
            .field("resource_type", &self.resource_type)
            .finish_non_exhaustive()
    }
}

impl HttpHandle {
    /// Create an `HttpHandle` from configuration, resolving secrets as needed.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceError::MissingConfigKey`] if `base_url` is absent, or
    /// [`ResourceError::InitializationFailed`] if the client cannot be built.
    pub async fn from_config(
        name: &str,
        config: &ResourceConfig,
        secrets: &dyn SecretsProvider,
    ) -> Result<Self, ResourceError> {
        let base_url = config
            .resolve_value("base_url", secrets)
            .await
            .map_err(|e| match e {
                ResourceError::MissingConfigKey { key, .. } => ResourceError::MissingConfigKey {
                    resource: name.to_string(),
                    key,
                },
                other => other,
            })?;

        let auth_type = config.resolve_optional("auth_type", secrets).await?;
        let timeout_ms: u64 = config
            .resolve_optional("timeout_ms", secrets)
            .await?
            .and_then(|v| v.parse().ok())
            .unwrap_or(30_000);

        let auth: Arc<dyn HttpAuthStrategy> = match auth_type.as_deref() {
            Some("api_key") => {
                let header = config
                    .resolve_optional("auth_header", secrets)
                    .await?
                    .unwrap_or_else(|| "X-API-Key".to_string());
                let value = config
                    .resolve_optional("auth_value", secrets)
                    .await?
                    .unwrap_or_default();
                Arc::new(ApiKeyAuthStrategy::new(&header, &value))
            }
            Some("bearer") => {
                let token = config
                    .resolve_optional("auth_value", secrets)
                    .await?
                    .unwrap_or_default();
                Arc::new(BearerTokenAuthStrategy::new(&token))
            }
            _ => Arc::new(NoAuthStrategy),
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(timeout_ms))
            .build()
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("failed to build HTTP client: {e}"),
            })?;

        Ok(Self {
            name: name.to_string(),
            client: Arc::new(client),
            base_url,
            auth,
            resource_type: ResourceType::Http,
            config: config.clone(),
        })
    }

    /// The base URL for this HTTP endpoint.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Create a GET request to the given path (appended to `base_url`).
    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.auth.apply(self.client.get(&url))
    }

    /// Create a POST request to the given path (appended to `base_url`).
    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.auth.apply(self.client.post(&url))
    }

    /// Create a PUT request to the given path (appended to `base_url`).
    pub fn put(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.auth.apply(self.client.put(&url))
    }

    /// Create a DELETE request to the given path (appended to `base_url`).
    pub fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.auth.apply(self.client.delete(&url))
    }
}

#[async_trait::async_trait]
impl ResourceHandle for HttpHandle {
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
            "HttpHandle credential refresh is not yet implemented"
        );
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ResourceError> {
        self.client.head(&self.base_url).send().await.map_err(|e| {
            ResourceError::HealthCheckFailed {
                name: self.name.clone(),
                message: format!("HEAD {}: {e}", self.base_url),
            }
        })?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
