//! HTTP adapters for persist, acquire, and emit operations.
//!
//! Wraps `tasker_secure::resource::http::HttpHandle` and implements
//! `PersistableResource` (POST/PUT/PATCH/DELETE), `AcquirableResource` (GET),
//! and `EmittableResource` (POST webhook).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use tasker_grammar::operations::{
    AcquirableResource, AcquireConstraints, AcquireResult, EmitMetadata, EmitResult,
    EmittableResource, PersistConstraints, PersistMode, PersistResult, PersistableResource,
    ResourceOperationError,
};
use tasker_secure::resource::{http::HttpHandle, ResourceHandleExt};

/// Map [`PersistMode`] to the corresponding HTTP method name.
///
/// - `Insert` -> `POST`
/// - `Update` -> `PATCH`
/// - `Upsert` -> `PUT`
/// - `Delete` -> `DELETE`
pub fn http_persist_method(mode: &PersistMode) -> &'static str {
    match mode {
        PersistMode::Insert => "POST",
        PersistMode::Update => "PATCH",
        PersistMode::Upsert => "PUT",
        PersistMode::Delete => "DELETE",
    }
}

/// Build the URL path for a persist operation.
///
/// Insert creates at `/{entity}`, while update/upsert/delete append identity
/// key values extracted from the data: `/{entity}/{pk1}/{pk2}`.
fn persist_url(
    entity: &str,
    mode: &PersistMode,
    data: &serde_json::Value,
    constraints: &PersistConstraints,
) -> String {
    match mode {
        PersistMode::Insert => format!("/{entity}"),
        PersistMode::Update | PersistMode::Upsert | PersistMode::Delete => {
            let pk_value = extract_pk_path(data, constraints);
            if pk_value.is_empty() {
                format!("/{entity}")
            } else {
                format!("/{entity}/{pk_value}")
            }
        }
    }
}

/// Extract primary-key path segments from data using constraint keys.
fn extract_pk_path(data: &serde_json::Value, constraints: &PersistConstraints) -> String {
    let keys = constraints
        .identity_keys
        .as_deref()
        .or(constraints.upsert_key.as_deref())
        .unwrap_or_default();
    keys.iter()
        .filter_map(|k| data.get(k))
        .map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Map an HTTP status code to a [`ResourceOperationError`].
fn map_http_error(
    status: reqwest::StatusCode,
    entity: &str,
    operation: &str,
) -> ResourceOperationError {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        ResourceOperationError::AuthorizationFailed {
            operation: operation.to_string(),
            entity: entity.to_string(),
        }
    } else if status == reqwest::StatusCode::NOT_FOUND {
        ResourceOperationError::EntityNotFound {
            entity: entity.to_string(),
        }
    } else if status == reqwest::StatusCode::CONFLICT {
        ResourceOperationError::Conflict {
            entity: entity.to_string(),
            reason: format!("HTTP {status}"),
        }
    } else {
        ResourceOperationError::Other {
            message: format!("HTTP {status}"),
            source: None,
        }
    }
}

/// Parse an HTTP response body as JSON, falling back to a status wrapper.
async fn parse_response_json(
    response: reqwest::Response,
) -> Result<serde_json::Value, ResourceOperationError> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| ResourceOperationError::Other {
            message: format!("Failed to read response body: {e}"),
            source: Some(Box::new(e)),
        })?;
    if body.is_empty() {
        return Ok(serde_json::json!({"status": status.as_u16()}));
    }
    Ok(serde_json::from_str(&body)
        .unwrap_or_else(|_| serde_json::json!({"status": status.as_u16(), "body": body})))
}

// ---------------------------------------------------------------------------
// HttpPersistAdapter
// ---------------------------------------------------------------------------

/// Adapts an [`HttpHandle`] for structured write operations.
///
/// Maps [`PersistMode`] to HTTP methods:
/// - `Insert` -> `POST /{entity}`
/// - `Update` -> `PATCH /{entity}/{id}`
/// - `Upsert` -> `PUT /{entity}/{id}`
/// - `Delete` -> `DELETE /{entity}/{id}`
#[derive(Debug)]
pub struct HttpPersistAdapter {
    handle: Arc<dyn tasker_secure::ResourceHandle>,
}

impl HttpPersistAdapter {
    /// Create a new persist adapter wrapping the given handle.
    pub fn new(handle: Arc<dyn tasker_secure::ResourceHandle>) -> Self {
        Self { handle }
    }

    fn http_handle(&self) -> Result<&HttpHandle, ResourceOperationError> {
        self.handle
            .as_http()
            .ok_or_else(|| ResourceOperationError::ValidationFailed {
                message: format!(
                    "Expected HTTP handle, got {:?}",
                    self.handle.resource_type()
                ),
            })
    }
}

#[async_trait]
impl PersistableResource for HttpPersistAdapter {
    async fn persist(
        &self,
        entity: &str,
        data: serde_json::Value,
        constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        let http = self.http_handle()?;
        let path = persist_url(entity, &constraints.mode, &data, constraints);

        let request = match constraints.mode {
            PersistMode::Insert => http.post(&path).json(&data),
            PersistMode::Update => http.patch(&path).json(&data),
            PersistMode::Upsert => http.put(&path).json(&data),
            PersistMode::Delete => {
                let req = http.delete(&path);
                if data.as_object().map_or(false, |o| !o.is_empty()) {
                    req.json(&data)
                } else {
                    req
                }
            }
        };

        let response = request
            .send()
            .await
            .map_err(|e| ResourceOperationError::Unavailable {
                message: format!("HTTP request failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(map_http_error(response.status(), entity, "persist"));
        }

        let response_data = parse_response_json(response).await?;

        Ok(PersistResult {
            data: response_data,
            affected_count: Some(1),
        })
    }
}

// ---------------------------------------------------------------------------
// HttpAcquireAdapter
// ---------------------------------------------------------------------------

/// Adapts an [`HttpHandle`] for structured read operations (GET).
///
/// Converts JSON object params to query parameters, and applies pagination
/// from [`AcquireConstraints`] (limit/offset).
#[derive(Debug)]
pub struct HttpAcquireAdapter {
    handle: Arc<dyn tasker_secure::ResourceHandle>,
}

impl HttpAcquireAdapter {
    /// Create a new acquire adapter wrapping the given handle.
    pub fn new(handle: Arc<dyn tasker_secure::ResourceHandle>) -> Self {
        Self { handle }
    }

    fn http_handle(&self) -> Result<&HttpHandle, ResourceOperationError> {
        self.handle
            .as_http()
            .ok_or_else(|| ResourceOperationError::ValidationFailed {
                message: format!(
                    "Expected HTTP handle, got {:?}",
                    self.handle.resource_type()
                ),
            })
    }
}

#[async_trait]
impl AcquirableResource for HttpAcquireAdapter {
    async fn acquire(
        &self,
        entity: &str,
        params: serde_json::Value,
        constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        let http = self.http_handle()?;
        let path = format!("/{entity}");
        let mut request = http.get(&path);

        // Convert JSON object fields to query parameters, skipping internal keys.
        if let Some(obj) = params.as_object() {
            for (key, value) in obj {
                if key.starts_with('_') {
                    continue;
                }
                let str_val = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                request = request.query(&[(key.as_str(), str_val.as_str())]);
            }
        }

        // Apply pagination constraints.
        if let Some(limit) = constraints.limit {
            request = request.query(&[("limit", &limit.to_string())]);
        }
        if let Some(offset) = constraints.offset {
            request = request.query(&[("offset", &offset.to_string())]);
        }
        if let Some(timeout_ms) = constraints.timeout_ms {
            request = request.timeout(Duration::from_millis(timeout_ms));
        }

        let response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                ResourceOperationError::Timeout {
                    timeout_ms: constraints.timeout_ms.unwrap_or(0),
                }
            } else {
                ResourceOperationError::Unavailable {
                    message: format!("HTTP request failed: {e}"),
                }
            }
        })?;

        if !response.status().is_success() {
            return Err(map_http_error(response.status(), entity, "acquire"));
        }

        let data = parse_response_json(response).await?;

        Ok(AcquireResult {
            data,
            total_count: None,
        })
    }
}

// ---------------------------------------------------------------------------
// HttpEmitAdapter
// ---------------------------------------------------------------------------

/// Adapts an [`HttpHandle`] for event emission via POST webhook.
///
/// Posts the payload to `/{topic}` with correlation and idempotency headers
/// derived from [`EmitMetadata`].
#[derive(Debug)]
pub struct HttpEmitAdapter {
    handle: Arc<dyn tasker_secure::ResourceHandle>,
}

impl HttpEmitAdapter {
    /// Create a new emit adapter wrapping the given handle.
    pub fn new(handle: Arc<dyn tasker_secure::ResourceHandle>) -> Self {
        Self { handle }
    }

    fn http_handle(&self) -> Result<&HttpHandle, ResourceOperationError> {
        self.handle
            .as_http()
            .ok_or_else(|| ResourceOperationError::ValidationFailed {
                message: format!(
                    "Expected HTTP handle, got {:?}",
                    self.handle.resource_type()
                ),
            })
    }
}

#[async_trait]
impl EmittableResource for HttpEmitAdapter {
    async fn emit(
        &self,
        topic: &str,
        payload: serde_json::Value,
        metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError> {
        let http = self.http_handle()?;
        let path = format!("/{topic}");
        let mut request = http.post(&path).json(&payload);

        if let Some(ref correlation_id) = metadata.correlation_id {
            request = request.header("X-Correlation-ID", correlation_id);
        }
        if let Some(ref idempotency_key) = metadata.idempotency_key {
            request = request.header("Idempotency-Key", idempotency_key);
        }
        if let Some(ref attrs) = metadata.attributes {
            for (key, value) in attrs {
                request = request.header(key, value);
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| ResourceOperationError::Unavailable {
                message: format!("HTTP emit failed: {e}"),
            })?;

        let confirmed = response.status().is_success();

        if !confirmed {
            return Err(map_http_error(response.status(), topic, "emit"));
        }

        let data = parse_response_json(response).await?;

        Ok(EmitResult { data, confirmed })
    }
}
