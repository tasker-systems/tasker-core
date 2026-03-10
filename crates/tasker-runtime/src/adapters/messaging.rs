//! Messaging adapter for emit operations.
//!
//! Wraps [`tasker_shared::messaging::service::MessagingProvider`] (PGMQ or RabbitMQ)
//! and implements [`EmittableResource`] for domain event emission through the
//! existing messaging infrastructure.

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::{
    EmitMetadata, EmitResult, EmittableResource, ResourceOperationError,
};
use tasker_shared::messaging::service::MessagingProvider;

/// Adapts the existing messaging infrastructure for grammar emit operations.
///
/// Wraps an `Arc<MessagingProvider>` so that composition steps using `emit`
/// can publish domain events through PGMQ or RabbitMQ without knowing the
/// underlying transport.
///
/// The topic is used as the queue name. The adapter ensures the queue exists
/// (idempotent) before sending each message. Metadata fields (correlation ID,
/// idempotency key, attributes) are embedded in the message envelope.
#[derive(Debug)]
pub struct MessagingEmitAdapter {
    provider: Arc<MessagingProvider>,
}

impl MessagingEmitAdapter {
    /// Create a new messaging emit adapter wrapping the given provider.
    pub fn new(provider: Arc<MessagingProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl EmittableResource for MessagingEmitAdapter {
    async fn emit(
        &self,
        topic: &str,
        payload: serde_json::Value,
        metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError> {
        // Ensure the queue exists (idempotent).
        self.provider.ensure_queue(topic).await.map_err(|e| {
            ResourceOperationError::Unavailable {
                message: format!("Failed to ensure queue '{topic}': {e}"),
            }
        })?;

        // Build an envelope that includes metadata alongside the payload.
        let envelope = serde_json::json!({
            "payload": payload,
            "correlation_id": metadata.correlation_id,
            "idempotency_key": metadata.idempotency_key,
            "attributes": metadata.attributes,
        });

        // Send the message.
        let msg_id = self
            .provider
            .send_message(topic, &envelope)
            .await
            .map_err(|e| ResourceOperationError::Unavailable {
                message: format!("Failed to send message to '{topic}': {e}"),
            })?;

        Ok(EmitResult {
            data: serde_json::json!({
                "message_id": msg_id.as_str(),
                "queue": topic,
            }),
            confirmed: true,
        })
    }
}
