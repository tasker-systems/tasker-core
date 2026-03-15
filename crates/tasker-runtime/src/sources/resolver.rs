//! Definition-based resource handle resolver.
//!
//! Bridges [`ResourceDefinitionSource`] to [`ResourceHandleResolver`] by
//! looking up a resource definition by name and dispatching to the
//! appropriate handle constructor based on [`ResourceType`].

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::ResourceOperationError;
use tasker_secure::{ResourceHandle, SecretsProvider};

#[cfg(any(feature = "postgres", feature = "http"))]
use tasker_secure::ResourceType;

#[cfg(feature = "http")]
use tasker_secure::resource::http::HttpHandle;
#[cfg(feature = "postgres")]
use tasker_secure::resource::postgres::PostgresHandle;

#[cfg(any(feature = "postgres", feature = "http"))]
use crate::provider::map_resource_error;

use super::{ResourceDefinitionSource, ResourceHandleResolver};

/// Resolves resource handles by looking up definitions and initializing handles.
///
/// Given a resource name:
/// 1. Queries the [`ResourceDefinitionSource`] for a [`ResourceDefinition`]
/// 2. Dispatches to the appropriate `from_config` constructor based on [`ResourceType`]
/// 3. Returns the initialized handle as `Arc<dyn ResourceHandle>`
///
/// Feature-gated: `postgres` and `http` handle construction require their
/// respective features to be enabled. Unsupported types return
/// `ResourceOperationError::ValidationFailed`.
#[derive(Debug)]
pub struct DefinitionBasedResolver {
    source: Arc<dyn ResourceDefinitionSource>,
    // Used only when `postgres` or `http` features are enabled.
    #[cfg_attr(
        not(any(feature = "postgres", feature = "http")),
        expect(
            dead_code,
            reason = "secrets is consumed inside feature-gated cfg blocks; unused without postgres/http"
        )
    )]
    secrets: Arc<dyn SecretsProvider>,
}

impl DefinitionBasedResolver {
    /// Create a new resolver backed by the given definition source and secrets provider.
    pub fn new(
        source: Arc<dyn ResourceDefinitionSource>,
        secrets: Arc<dyn SecretsProvider>,
    ) -> Self {
        Self { source, secrets }
    }
}

#[async_trait]
impl ResourceHandleResolver for DefinitionBasedResolver {
    async fn resolve(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceOperationError> {
        let definition = self.source.resolve(resource_ref).await.ok_or_else(|| {
            ResourceOperationError::EntityNotFound {
                entity: resource_ref.to_string(),
            }
        })?;

        #[cfg(any(feature = "postgres", feature = "http"))]
        {
            let handle: Arc<dyn ResourceHandle> = match definition.resource_type {
                #[cfg(feature = "postgres")]
                ResourceType::Postgres => Arc::new(
                    PostgresHandle::from_config(
                        &definition.name,
                        &definition.config,
                        self.secrets.as_ref(),
                    )
                    .await
                    .map_err(map_resource_error)?,
                ),
                #[cfg(feature = "http")]
                ResourceType::Http => Arc::new(
                    HttpHandle::from_config(
                        &definition.name,
                        &definition.config,
                        self.secrets.as_ref(),
                    )
                    .await
                    .map_err(map_resource_error)?,
                ),
                other => {
                    return Err(ResourceOperationError::ValidationFailed {
                        message: format!("No handle factory for resource type: {other}"),
                    });
                }
            };
            return Ok(handle);
        }

        #[cfg(not(any(feature = "postgres", feature = "http")))]
        Err(ResourceOperationError::ValidationFailed {
            message: format!(
                "No handle factory for resource type: {}",
                definition.resource_type
            ),
        })
    }
}
