//! # PGMQ Messaging Service
//!
//! PostgreSQL Message Queue implementation via tasker-pgmq crate.
//!
//! ## Features
//!
//! - **LISTEN/NOTIFY Support**: Event-driven message processing
//! - **Visibility Timeout**: Built-in PGMQ visibility semantics
//! - **Atomic Operations**: Database transaction guarantees
//! - **Full MessagingService Implementation**: Complete API compatibility
//! - **Push Notifications (TAS-133)**: Signal-only push via pg_notify
//! - **Shared Listener (TAS-149)**: Single PostgreSQL connection for all subscriptions

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures::Stream;
use sqlx::PgPool;
use tasker_pgmq::{PgmqClient, PgmqNotifyConfig, PgmqNotifyEvent};
use tracing::{debug, error, info, warn};

use crate::messaging::service::traits::{
    MessagingService, NotificationStream, QueueMessage, SupportsPushNotifications,
};
use crate::messaging::service::types::{
    MessageHandle, MessageId, MessageMetadata, MessageNotification, QueueHealthReport, QueueStats,
    QueuedMessage, ReceiptHandle,
};
use crate::messaging::MessagingError;

// =============================================================================
// Constants
// =============================================================================

/// Default buffer size for per-subscriber notification channels
const DEFAULT_NOTIFICATION_BUFFER_SIZE: usize = 100;

/// Buffer size for the internal listener command channel
const LISTENER_COMMAND_BUFFER_SIZE: usize = 128;

// =============================================================================
// Shared Listener Manager (TAS-149)
// =============================================================================

/// Commands sent to the shared listener background task
#[derive(Debug)]
enum ListenerCommand {
    /// Start listening on a PostgreSQL NOTIFY channel
    AddChannel(String),
    /// Register a subscriber for notifications on a specific queue
    AddSubscriber {
        queue_name: String,
        tx: tokio::sync::mpsc::Sender<MessageNotification>,
    },
}

/// Internal state shared across all clones of a [`SharedListenerManager`]
struct SharedListenerState {
    pool: PgPool,
    command_tx: tokio::sync::mpsc::Sender<ListenerCommand>,
    /// Receiver taken once when the background task starts
    command_rx: Mutex<Option<tokio::sync::mpsc::Receiver<ListenerCommand>>>,
    started: AtomicBool,
}

impl std::fmt::Debug for SharedListenerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedListenerState")
            .field("started", &self.started.load(Ordering::Relaxed))
            .finish()
    }
}

/// Manages a single shared `PgListener` for all PGMQ subscriptions (TAS-149)
///
/// Instead of creating a new `PgmqNotifyListener` per `subscribe()` or `subscribe_many()`
/// call, this manager maintains one PostgreSQL LISTEN connection shared across all
/// subscriptions within a [`PgmqMessagingService`] instance.
///
/// The background listener task is started lazily on the first subscription request.
/// Subsequent subscribe calls send commands to add channels and subscribers to the
/// already-running listener.
#[derive(Debug, Clone)]
struct SharedListenerManager {
    inner: Arc<SharedListenerState>,
}

impl SharedListenerManager {
    /// Create a new shared listener manager
    fn new(pool: PgPool) -> Self {
        let (command_tx, command_rx) = tokio::sync::mpsc::channel(LISTENER_COMMAND_BUFFER_SIZE);

        Self {
            inner: Arc::new(SharedListenerState {
                pool,
                command_tx,
                command_rx: Mutex::new(Some(command_rx)),
                started: AtomicBool::new(false),
            }),
        }
    }

    /// Ensure the background listener task is running
    ///
    /// This is idempotent - only the first call actually spawns the task.
    fn ensure_started(&self) {
        if self
            .inner
            .started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // We won the race - spawn the background task
            let command_rx = self
                .inner
                .command_rx
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .take()
                .expect("command_rx should only be taken once");

            let pool = self.inner.pool.clone();

            tokio::spawn(async move {
                shared_listener_task(pool, command_rx).await;
            });
        }
    }

    /// Send a command to the background listener task
    fn send_command(&self, command: ListenerCommand) -> Result<(), MessagingError> {
        self.inner.command_tx.try_send(command).map_err(|e| {
            MessagingError::internal(format!("Failed to send command to shared listener: {}", e))
        })
    }

    /// Add a PostgreSQL LISTEN channel to the shared listener
    fn add_channel(&self, channel: String) -> Result<(), MessagingError> {
        self.send_command(ListenerCommand::AddChannel(channel))
    }

    /// Register a subscriber for notifications on a specific queue
    fn add_subscriber(
        &self,
        queue_name: String,
        tx: tokio::sync::mpsc::Sender<MessageNotification>,
    ) -> Result<(), MessagingError> {
        self.send_command(ListenerCommand::AddSubscriber { queue_name, tx })
    }
}

/// Background task that manages a single PgListener connection (TAS-149)
///
/// Uses `tokio::select!` to multiplex between:
/// - PostgreSQL notifications from `PgListener::recv()`
/// - Control commands from `command_rx`
async fn shared_listener_task(
    pool: PgPool,
    mut command_rx: tokio::sync::mpsc::Receiver<ListenerCommand>,
) {
    use sqlx::postgres::PgListener;

    let mut listener = match PgListener::connect_with(&pool).await {
        Ok(l) => l,
        Err(e) => {
            error!("TAS-149: Failed to create shared PgListener: {}", e);
            return;
        }
    };

    info!("TAS-149: Shared PgListener started");

    // Per-queue subscribers: queue_name -> Vec<Sender>
    let mut subscribers: HashMap<String, Vec<tokio::sync::mpsc::Sender<MessageNotification>>> =
        HashMap::new();

    // Track channels already listened to (avoid duplicate LISTEN calls)
    let mut listening_channels: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    // Process any commands that arrived before the listener was ready
    while let Ok(command) = command_rx.try_recv() {
        process_command(
            command,
            &mut listener,
            &mut subscribers,
            &mut listening_channels,
        )
        .await;
    }

    loop {
        tokio::select! {
            notification = listener.recv() => {
                match notification {
                    Ok(notification) => {
                        debug!(
                            channel = %notification.channel(),
                            "TAS-149: Shared listener received notification"
                        );

                        match serde_json::from_str::<PgmqNotifyEvent>(notification.payload()) {
                            Ok(event) => {
                                dispatch_notification(&event, &mut subscribers).await;
                            }
                            Err(e) => {
                                warn!(
                                    channel = %notification.channel(),
                                    error = %e,
                                    "TAS-149: Failed to parse notification payload"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        error!("TAS-149: Shared PgListener connection error: {}", e);
                        break;
                    }
                }
            }

            command = command_rx.recv() => {
                match command {
                    Some(cmd) => {
                        process_command(
                            cmd,
                            &mut listener,
                            &mut subscribers,
                            &mut listening_channels,
                        )
                        .await;
                    }
                    None => {
                        // All command senders dropped - service is shutting down
                        info!("TAS-149: Command channel closed, shutting down shared listener");
                        break;
                    }
                }
            }
        }
    }

    info!(
        channels = listening_channels.len(),
        subscribers = subscribers.len(),
        "TAS-149: Shared PgListener stopped"
    );
}

/// Process a single listener command
async fn process_command(
    command: ListenerCommand,
    listener: &mut sqlx::postgres::PgListener,
    subscribers: &mut HashMap<String, Vec<tokio::sync::mpsc::Sender<MessageNotification>>>,
    listening_channels: &mut std::collections::HashSet<String>,
) {
    match command {
        ListenerCommand::AddChannel(channel) => {
            if listening_channels.contains(&channel) {
                debug!(
                    channel = %channel,
                    "TAS-149: Already listening on channel, skipping"
                );
                return;
            }

            match listener.listen(&channel).await {
                Ok(()) => {
                    listening_channels.insert(channel.clone());
                    info!(
                        channel = %channel,
                        total_channels = listening_channels.len(),
                        "TAS-149: Added LISTEN channel"
                    );
                }
                Err(e) => {
                    error!(
                        channel = %channel,
                        error = %e,
                        "TAS-149: Failed to listen on channel"
                    );
                }
            }
        }
        ListenerCommand::AddSubscriber { queue_name, tx } => {
            subscribers.entry(queue_name.clone()).or_default().push(tx);

            debug!(
                queue = %queue_name,
                total_queues = subscribers.len(),
                "TAS-149: Added subscriber"
            );
        }
    }
}

/// Dispatch a parsed notification event to matching subscribers
async fn dispatch_notification(
    event: &PgmqNotifyEvent,
    subscribers: &mut HashMap<String, Vec<tokio::sync::mpsc::Sender<MessageNotification>>>,
) {
    let event_queue_name = event.queue_name();

    if let Some(senders) = subscribers.get_mut(event_queue_name) {
        if let Some(notification) = convert_event_to_notification(event) {
            // Remove closed senders while dispatching
            senders.retain(|tx| !tx.is_closed());

            for tx in senders.iter() {
                if tx.send(notification.clone()).await.is_err() {
                    warn!(
                        queue = %event_queue_name,
                        "TAS-149: Subscriber receiver dropped"
                    );
                }
            }
        }
    }
}

/// Convert a [`PgmqNotifyEvent`] to a [`MessageNotification`]
///
/// Returns `None` for events that don't map to message notifications (e.g. QueueCreated).
fn convert_event_to_notification(event: &PgmqNotifyEvent) -> Option<MessageNotification> {
    match event {
        PgmqNotifyEvent::MessageWithPayload(e) => {
            // Small message (< 7KB): Full payload included
            let handle = MessageHandle::Pgmq {
                msg_id: e.msg_id,
                queue_name: e.queue_name.clone(),
            };
            let metadata = MessageMetadata {
                receive_count: 0,
                enqueued_at: e.ready_at,
            };
            let payload_bytes = serde_json::to_vec(&e.message).unwrap_or_else(|_| Vec::new());
            let queued_msg = QueuedMessage::with_handle(payload_bytes, handle, metadata);
            Some(MessageNotification::message(queued_msg))
        }
        PgmqNotifyEvent::MessageReady(e) => {
            // Large message (>= 7KB): Signal only with msg_id
            Some(MessageNotification::available_with_msg_id(
                e.queue_name.clone(),
                e.msg_id,
            ))
        }
        PgmqNotifyEvent::QueueCreated(_) => None,
        PgmqNotifyEvent::BatchReady(e) => {
            let msg_id = e.msg_ids.first().copied();
            Some(match msg_id {
                Some(id) => MessageNotification::available_with_msg_id(e.queue_name.clone(), id),
                None => MessageNotification::available(e.queue_name.clone()),
            })
        }
    }
}

/// Extract namespace from a queue name for LISTEN channel resolution
///
/// Must match the SQL function `tasker.extract_queue_namespace()` in
/// `migrations/20250826180921_add_pgmq_notifications.sql` — both produce
/// the channel suffix used with `pgmq_message_ready.{namespace}`.
///
/// Rules (matching SQL):
/// 1. Orchestration queues (`orchestration_*`) → `"orchestration"`
/// 2. Worker queues (`worker_{ns}_queue`) → `"{ns}"`
/// 3. Other `*_queue` → strip `_queue` suffix
/// 4. Fallback → queue name as-is
fn extract_namespace_from_queue(queue_name: &str) -> String {
    // 1. Orchestration queues: all map to "orchestration" namespace
    //    (matches SQL: IF queue_name ~ '^orchestration' THEN RETURN 'orchestration')
    if queue_name.starts_with("orchestration") {
        return "orchestration".to_string();
    }

    // 2. Worker queues: worker_{namespace}_queue → namespace
    if let Some(ns) = queue_name
        .strip_prefix("worker_")
        .and_then(|s| s.strip_suffix("_queue"))
    {
        return ns.to_string();
    }

    // 3. Other queues ending in _queue: strip suffix
    if let Some(ns) = queue_name.strip_suffix("_queue") {
        return ns.to_string();
    }

    // 4. Fallback: return as-is
    queue_name.to_string()
}

// =============================================================================
// PGMQ Messaging Service
// =============================================================================

/// PGMQ-based messaging service implementation
///
/// Wraps the `tasker_pgmq::PgmqClient` to provide the `MessagingService` trait interface.
/// Supports all standard PGMQ operations plus LISTEN/NOTIFY for event-driven processing.
///
/// ## Shared Listener (TAS-149)
///
/// Push notification subscriptions share a single PostgreSQL LISTEN connection
/// via [`SharedListenerManager`]. This prevents connection pool exhaustion when
/// subscribing to many queues across namespaces.
///
/// # Example
///
/// ```ignore
/// use tasker_shared::messaging::service::providers::PgmqMessagingService;
/// use tasker_shared::messaging::service::MessagingService;
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let service = PgmqMessagingService::new("postgresql://localhost/tasker").await?;
///
/// // Create a queue
/// service.ensure_queue("my_queue").await?;
///
/// // Send a message
/// let msg_id = service.send_message("my_queue", &serde_json::json!({"key": "value"})).await?;
///
/// // Receive messages
/// let messages = service.receive_messages::<serde_json::Value>(
///     "my_queue",
///     10,
///     Duration::from_secs(30),
/// ).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct PgmqMessagingService {
    /// Underlying PGMQ client
    client: PgmqClient,
    /// TAS-149: Shared listener for all push notification subscriptions
    shared_listener: SharedListenerManager,
}

impl PgmqMessagingService {
    /// Create a new PGMQ messaging service from database URL
    ///
    /// Uses default `PgmqNotifyConfig`. For custom configuration, use `new_with_config`.
    pub async fn new(database_url: &str) -> Result<Self, MessagingError> {
        let client = PgmqClient::new(database_url)
            .await
            .map_err(|e| MessagingError::connection(e.to_string()))?;
        let shared_listener = SharedListenerManager::new(client.pool().clone());
        Ok(Self {
            client,
            shared_listener,
        })
    }

    /// Create a new PGMQ messaging service with database URL and custom configuration
    ///
    /// Allows configuring notify behavior, queue naming patterns, and other options.
    pub async fn new_with_config(
        database_url: &str,
        config: PgmqNotifyConfig,
    ) -> Result<Self, MessagingError> {
        let client = PgmqClient::new_with_config(database_url, config)
            .await
            .map_err(|e| MessagingError::connection(e.to_string()))?;
        let shared_listener = SharedListenerManager::new(client.pool().clone());
        Ok(Self {
            client,
            shared_listener,
        })
    }

    /// Create a new PGMQ messaging service with an existing connection pool
    ///
    /// Uses default `PgmqNotifyConfig`. For custom configuration, use `new_with_pool_and_config`.
    /// This is the preferred constructor when pool configuration is managed externally
    /// (e.g., from TOML config via SystemContext).
    pub async fn new_with_pool(pool: PgPool) -> Self {
        let client = PgmqClient::new_with_pool(pool).await;
        let shared_listener = SharedListenerManager::new(client.pool().clone());
        Self {
            client,
            shared_listener,
        }
    }

    /// Create a new PGMQ messaging service with existing pool and custom configuration
    ///
    /// Combines externally-managed pool configuration with custom notify behavior.
    /// Use this when you need both pool tuning (from TOML) and notify configuration.
    pub async fn new_with_pool_and_config(pool: PgPool, config: PgmqNotifyConfig) -> Self {
        let client = PgmqClient::new_with_pool_and_config(pool, config).await;
        let shared_listener = SharedListenerManager::new(client.pool().clone());
        Self {
            client,
            shared_listener,
        }
    }

    /// Create from an existing PgmqClient
    ///
    /// Escape hatch for when you need full control over client construction.
    pub fn from_client(client: PgmqClient) -> Self {
        let shared_listener = SharedListenerManager::new(client.pool().clone());
        Self {
            client,
            shared_listener,
        }
    }

    /// Get a reference to the underlying PgmqClient
    pub fn client(&self) -> &PgmqClient {
        &self.client
    }

    /// Get a reference to the underlying connection pool
    pub fn pool(&self) -> &PgPool {
        self.client.pool()
    }

    /// Check if this service has LISTEN/NOTIFY capabilities
    pub fn has_notify_capabilities(&self) -> bool {
        self.client.has_notify_capabilities()
    }
}

#[async_trait]
impl MessagingService for PgmqMessagingService {
    async fn ensure_queue(&self, queue_name: &str) -> Result<(), MessagingError> {
        self.client
            .create_queue(queue_name)
            .await
            .map_err(|e| MessagingError::queue_creation(queue_name, e.to_string()))
    }

    async fn verify_queues(
        &self,
        queue_names: &[String],
    ) -> Result<QueueHealthReport, MessagingError> {
        let mut report = QueueHealthReport::new();

        for queue_name in queue_names {
            // Try to get metrics for the queue to verify it exists
            match self.client.queue_metrics(queue_name).await {
                Ok(_) => report.add_healthy(queue_name),
                Err(e) => {
                    // Check if it's a "queue not found" error or something else
                    let error_str = e.to_string();
                    if error_str.contains("does not exist") || error_str.contains("not found") {
                        report.add_missing(queue_name);
                    } else {
                        report.add_error(queue_name, error_str);
                    }
                }
            }
        }

        Ok(report)
    }

    async fn send_message<T: QueueMessage>(
        &self,
        queue_name: &str,
        message: &T,
    ) -> Result<MessageId, MessagingError> {
        // Serialize to JSON Value for PGMQ
        let json_value: serde_json::Value = serde_json::from_slice(&message.to_bytes()?)
            .map_err(|e| MessagingError::serialization(e.to_string()))?;

        let msg_id = self
            .client
            .send_json_message(queue_name, &json_value)
            .await
            .map_err(|e| MessagingError::send(queue_name, e.to_string()))?;

        Ok(MessageId::from(msg_id))
    }

    async fn send_batch<T: QueueMessage>(
        &self,
        queue_name: &str,
        messages: &[T],
    ) -> Result<Vec<MessageId>, MessagingError> {
        // PGMQ doesn't have a native batch send, so we send one by one
        // This could be optimized with a transaction in the future
        let mut ids = Vec::with_capacity(messages.len());

        for message in messages {
            let id = self.send_message(queue_name, message).await?;
            ids.push(id);
        }

        Ok(ids)
    }

    async fn receive_messages<T: QueueMessage>(
        &self,
        queue_name: &str,
        max_messages: usize,
        visibility_timeout: Duration,
    ) -> Result<Vec<QueuedMessage<T>>, MessagingError> {
        let vt_seconds = visibility_timeout.as_secs() as i32;

        let messages = self
            .client
            .read_messages(queue_name, Some(vt_seconds), Some(max_messages as i32))
            .await
            .map_err(|e| MessagingError::receive(queue_name, e.to_string()))?;

        let mut result = Vec::with_capacity(messages.len());

        for msg in messages {
            // Serialize the message to bytes then deserialize to T
            let bytes = serde_json::to_vec(&msg.message)
                .map_err(|e| MessagingError::serialization(e.to_string()))?;

            let deserialized = T::from_bytes(&bytes)?;

            // TAS-133: Use explicit MessageHandle::Pgmq for provider-agnostic message wrapper
            result.push(QueuedMessage::with_handle(
                deserialized,
                MessageHandle::Pgmq {
                    msg_id: msg.msg_id,
                    queue_name: queue_name.to_string(),
                },
                MessageMetadata::new(msg.read_ct as u32, msg.enqueued_at),
            ));
        }

        Ok(result)
    }

    async fn ack_message(
        &self,
        queue_name: &str,
        receipt_handle: &ReceiptHandle,
    ) -> Result<(), MessagingError> {
        let message_id = receipt_handle
            .as_i64()
            .ok_or_else(|| MessagingError::invalid_receipt_handle(receipt_handle.as_str()))?;

        // Archive the message (PGMQ's equivalent of ack)
        self.client
            .archive_message(queue_name, message_id)
            .await
            .map_err(|e| MessagingError::ack(queue_name, message_id, e.to_string()))
    }

    async fn nack_message(
        &self,
        queue_name: &str,
        receipt_handle: &ReceiptHandle,
        requeue: bool,
    ) -> Result<(), MessagingError> {
        let message_id = receipt_handle
            .as_i64()
            .ok_or_else(|| MessagingError::invalid_receipt_handle(receipt_handle.as_str()))?;

        if requeue {
            // Make the message visible again by setting visibility timeout to 0
            // PGMQ doesn't have a direct "nack" operation, so we use set_visibility_timeout
            self.client
                .set_visibility_timeout(queue_name, message_id, 0)
                .await
                .map_err(|e| MessagingError::nack(queue_name, message_id, e.to_string()))?;
        } else {
            // Delete the message (dead-letter behavior)
            self.client
                .delete_message(queue_name, message_id)
                .await
                .map_err(|e| MessagingError::nack(queue_name, message_id, e.to_string()))?;
        }

        Ok(())
    }

    async fn extend_visibility(
        &self,
        queue_name: &str,
        receipt_handle: &ReceiptHandle,
        extension: Duration,
    ) -> Result<(), MessagingError> {
        let message_id = receipt_handle
            .as_i64()
            .ok_or_else(|| MessagingError::invalid_receipt_handle(receipt_handle.as_str()))?;

        let vt_seconds = extension.as_secs() as i32;

        self.client
            .set_visibility_timeout(queue_name, message_id, vt_seconds)
            .await
            .map_err(|e| {
                MessagingError::extend_visibility(queue_name, message_id, e.to_string())
            })?;

        Ok(())
    }

    async fn queue_stats(&self, queue_name: &str) -> Result<QueueStats, MessagingError> {
        let metrics = self
            .client
            .queue_metrics(queue_name)
            .await
            .map_err(|e| MessagingError::queue_stats(queue_name, e.to_string()))?;

        let mut stats = QueueStats::new(queue_name, metrics.message_count as u64);

        if let Some(age_seconds) = metrics.oldest_message_age_seconds {
            stats = stats.with_oldest_message_age_ms((age_seconds * 1000) as u64);
        }

        // PGMQ metrics don't include in-flight count directly, but we could query it
        // For now, we leave it as None

        Ok(stats)
    }

    async fn health_check(&self) -> Result<bool, MessagingError> {
        self.client
            .health_check()
            .await
            .map_err(|e| MessagingError::health_check(e.to_string()))
    }

    fn provider_name(&self) -> &'static str {
        "pgmq"
    }
}

// =============================================================================
// SupportsPushNotifications Implementation (TAS-133 + TAS-149)
// =============================================================================

impl SupportsPushNotifications for PgmqMessagingService {
    /// Subscribe to push notifications for a PGMQ queue
    ///
    /// TAS-149: Uses the shared listener instead of creating a new `PgmqNotifyListener`
    /// per call. The shared listener is started lazily on first subscribe and reused
    /// for all subsequent subscriptions.
    ///
    /// PGMQ uses PostgreSQL LISTEN/NOTIFY for push notifications with two modes (TAS-133):
    ///
    /// - **Small messages (< 7KB)**: Full payload included in notification via `MessageWithPayload`
    ///   event, returned as `MessageNotification::Message` - process directly without fetch
    /// - **Large messages (>= 7KB)**: Signal-only via `MessageReady` event, returned as
    ///   `MessageNotification::Available` with `msg_id` - fetch via `read_specific_message()`
    ///
    /// # Fallback Polling
    ///
    /// **Important**: pg_notify is not guaranteed delivery. Notifications can be
    /// lost under load or if the listener disconnects. Consumers should always
    /// implement fallback polling (see `requires_fallback_polling()`).
    fn subscribe(
        &self,
        queue_name: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = MessageNotification> + Send>>, MessagingError> {
        let config = self.client.config();

        // Resolve the LISTEN channel from queue name
        let namespace = extract_namespace_from_queue(queue_name);
        let channel = config
            .message_ready_channel(&namespace)
            .map_err(|e| MessagingError::configuration("pgmq", e.to_string()))?;

        // Start the shared listener if not already running
        self.shared_listener.ensure_started();

        // Create a channel for this subscriber
        let (tx, rx) =
            tokio::sync::mpsc::channel::<MessageNotification>(DEFAULT_NOTIFICATION_BUFFER_SIZE);

        // Register the channel and subscriber with the shared listener
        self.shared_listener.add_channel(channel)?;
        self.shared_listener
            .add_subscriber(queue_name.to_string(), tx)?;

        debug!(
            queue = %queue_name,
            "TAS-149: Subscribed via shared listener"
        );

        // Convert the mpsc receiver to a Stream
        let stream = futures::stream::unfold(rx, |mut rx| async move {
            rx.recv().await.map(|item| (item, rx))
        });

        Ok(Box::pin(stream))
    }

    /// PGMQ requires fallback polling
    ///
    /// PostgreSQL LISTEN/NOTIFY is **not guaranteed delivery**:
    /// - Notifications can be lost under heavy load
    /// - Notifications are missed if the listener disconnects
    /// - Messages enqueued before subscription started won't trigger notifications
    ///
    /// Always combine PGMQ push notifications with periodic polling.
    fn requires_fallback_polling(&self) -> bool {
        true
    }

    /// Recommended fallback polling interval for PGMQ
    ///
    /// Returns a 5-second interval by default. This provides a good balance between:
    /// - Catching missed notifications quickly
    /// - Not overloading the database with frequent polls
    fn fallback_polling_interval(&self) -> Option<Duration> {
        Some(Duration::from_secs(5))
    }

    /// PGMQ supports fetching messages by ID after signal-only notifications
    ///
    /// Returns `true` because PGMQ's large message flow (>7KB) sends signal-only
    /// notifications containing only the message ID. Consumers must use
    /// `read_specific_message(msg_id)` to fetch the full message content.
    ///
    /// This is the flow handled by `ExecuteStepFromEventMessage` in the worker.
    fn supports_fetch_by_message_id(&self) -> bool {
        true
    }

    /// Subscribe to multiple queues using a SINGLE shared PostgreSQL connection (TAS-149)
    ///
    /// TAS-149: All queues share the same background `PgListener` task, regardless of
    /// whether they were subscribed via `subscribe()` or `subscribe_many()`. Multiple
    /// calls to this method add to the existing shared listener rather than creating
    /// new connections.
    ///
    /// # Resource Efficiency
    ///
    /// - **Before TAS-133**: N queues = N connections held permanently
    /// - **After TAS-133**: N queues = 1 connection per `subscribe_many()` call
    /// - **After TAS-149**: N queues = 1 connection total across ALL subscribe calls
    fn subscribe_many(
        &self,
        queue_names: &[&str],
    ) -> Result<Vec<(String, NotificationStream)>, MessagingError> {
        if queue_names.is_empty() {
            return Ok(Vec::new());
        }

        let config = self.client.config();

        // Start the shared listener if not already running
        self.shared_listener.ensure_started();

        // Collect unique channels to listen to and create per-queue subscriber channels
        let mut channel_set = std::collections::HashSet::new();
        let mut result_streams: Vec<(String, NotificationStream)> = Vec::new();

        for queue_name in queue_names {
            // Resolve LISTEN channel
            let namespace = extract_namespace_from_queue(queue_name);
            let channel = config
                .message_ready_channel(&namespace)
                .map_err(|e| MessagingError::configuration("pgmq", e.to_string()))?;
            channel_set.insert(channel);

            // Create per-queue subscriber channel
            let (tx, rx) =
                tokio::sync::mpsc::channel::<MessageNotification>(DEFAULT_NOTIFICATION_BUFFER_SIZE);

            // Register subscriber
            self.shared_listener
                .add_subscriber(queue_name.to_string(), tx)?;

            // Convert receiver to stream
            let stream = futures::stream::unfold(rx, |mut rx| async move {
                rx.recv().await.map(|item| (item, rx))
            });
            result_streams.push((
                queue_name.to_string(),
                Box::pin(stream) as NotificationStream,
            ));
        }

        // Add all unique channels
        for channel in &channel_set {
            self.shared_listener.add_channel(channel.clone())?;
        }

        info!(
            queue_count = queue_names.len(),
            channel_count = channel_set.len(),
            "TAS-149: subscribe_many completed via shared listener"
        );

        Ok(result_streams)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// Get database URL for tests, preferring PGMQ_DATABASE_URL for split-db mode
    fn get_test_database_url() -> String {
        std::env::var("PGMQ_DATABASE_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var("DATABASE_URL").ok())
            .unwrap_or_else(|| {
                "postgresql://tasker:tasker@localhost:5432/tasker_rust_test".to_string()
            })
    }

    /// Generate unique queue name to avoid test conflicts
    fn unique_queue_name(prefix: &str) -> String {
        let test_id = &Uuid::new_v4().to_string()[..8];
        format!("{}_{}", prefix, test_id)
    }

    #[tokio::test]
    async fn test_pgmq_service_creation() {
        let database_url = get_test_database_url();
        let service = PgmqMessagingService::new(&database_url).await;
        assert!(service.is_ok(), "Should connect to database");

        let service = service.unwrap();
        assert_eq!(service.provider_name(), "pgmq");
        // Note: has_notify_capabilities() depends on connection mode configuration
        // It's informational, not a requirement for basic operations
        let _ = service.has_notify_capabilities();
    }

    #[tokio::test]
    async fn test_pgmq_health_check() {
        let database_url = get_test_database_url();
        let service = PgmqMessagingService::new(&database_url).await.unwrap();

        let health = service.health_check().await;
        assert!(health.is_ok(), "Health check should succeed");
        assert!(health.unwrap(), "Health check should return true");
    }

    #[tokio::test]
    async fn test_pgmq_ensure_queue() {
        let database_url = get_test_database_url();
        let service = PgmqMessagingService::new(&database_url).await.unwrap();
        let queue_name = unique_queue_name("test_ensure");

        // Create queue
        let result = service.ensure_queue(&queue_name).await;
        assert!(result.is_ok(), "Should create queue: {:?}", result.err());

        // Idempotent - creating again should succeed
        let result = service.ensure_queue(&queue_name).await;
        assert!(result.is_ok(), "Should be idempotent");
    }

    #[tokio::test]
    async fn test_pgmq_send_receive_roundtrip() {
        let database_url = get_test_database_url();
        let service = PgmqMessagingService::new(&database_url).await.unwrap();
        let queue_name = unique_queue_name("test_roundtrip");

        service.ensure_queue(&queue_name).await.unwrap();

        // Send message
        let msg = serde_json::json!({"test": "hello", "value": 42});
        let msg_id = service.send_message(&queue_name, &msg).await.unwrap();
        assert!(!msg_id.as_str().is_empty(), "Should return message ID");

        // Receive message
        let messages: Vec<QueuedMessage<serde_json::Value>> = service
            .receive_messages(&queue_name, 10, Duration::from_secs(30))
            .await
            .unwrap();

        assert_eq!(messages.len(), 1, "Should receive one message");
        assert_eq!(messages[0].message["test"], "hello");
        assert_eq!(messages[0].message["value"], 42);
        assert_eq!(messages[0].receive_count(), 1);

        // Ack message
        let ack_result = service
            .ack_message(&queue_name, &messages[0].receipt_handle)
            .await;
        assert!(ack_result.is_ok(), "Should ack message");

        // Should be empty now
        let messages2: Vec<QueuedMessage<serde_json::Value>> = service
            .receive_messages(&queue_name, 10, Duration::from_secs(30))
            .await
            .unwrap();
        assert_eq!(messages2.len(), 0, "Queue should be empty after ack");
    }

    #[tokio::test]
    async fn test_pgmq_nack_requeue() {
        let database_url = get_test_database_url();
        let service = PgmqMessagingService::new(&database_url).await.unwrap();
        let queue_name = unique_queue_name("test_nack");

        service.ensure_queue(&queue_name).await.unwrap();

        // Send message
        let msg = serde_json::json!({"action": "retry_me"});
        service.send_message(&queue_name, &msg).await.unwrap();

        // Receive with visibility timeout
        let messages: Vec<QueuedMessage<serde_json::Value>> = service
            .receive_messages(&queue_name, 1, Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);

        // Nack with requeue (sets visibility to 0)
        service
            .nack_message(&queue_name, &messages[0].receipt_handle, true)
            .await
            .unwrap();

        // Message should be immediately visible again
        let messages2: Vec<QueuedMessage<serde_json::Value>> = service
            .receive_messages(&queue_name, 1, Duration::from_secs(30))
            .await
            .unwrap();
        assert_eq!(messages2.len(), 1, "Message should be requeued");
        assert_eq!(
            messages2[0].receive_count(),
            2,
            "Receive count should increment"
        );
    }

    #[tokio::test]
    async fn test_pgmq_queue_stats() {
        let database_url = get_test_database_url();
        let service = PgmqMessagingService::new(&database_url).await.unwrap();
        let queue_name = unique_queue_name("test_stats");

        service.ensure_queue(&queue_name).await.unwrap();

        // Send a few messages
        for i in 0..3 {
            let msg = serde_json::json!({"index": i});
            service.send_message(&queue_name, &msg).await.unwrap();
        }

        // Check stats
        let stats = service.queue_stats(&queue_name).await.unwrap();
        assert_eq!(stats.queue_name, queue_name);
        assert_eq!(stats.message_count, 3);
    }

    #[tokio::test]
    async fn test_pgmq_verify_queues() {
        let database_url = get_test_database_url();
        let service = PgmqMessagingService::new(&database_url).await.unwrap();

        let existing_queue = unique_queue_name("test_verify_exists");
        let missing_queue = unique_queue_name("test_verify_missing");

        // Create only the first queue
        service.ensure_queue(&existing_queue).await.unwrap();

        // Verify both
        let report = service
            .verify_queues(&[existing_queue.clone(), missing_queue.clone()])
            .await
            .unwrap();

        assert!(
            report.healthy.contains(&existing_queue),
            "Should find existing queue"
        );
        assert!(
            report.missing.contains(&missing_queue),
            "Should identify missing queue"
        );
    }

    #[tokio::test]
    async fn test_pgmq_send_batch() {
        let database_url = get_test_database_url();
        let service = PgmqMessagingService::new(&database_url).await.unwrap();
        let queue_name = unique_queue_name("test_batch");

        service.ensure_queue(&queue_name).await.unwrap();

        // Send batch
        let messages = vec![
            serde_json::json!({"batch": 1}),
            serde_json::json!({"batch": 2}),
            serde_json::json!({"batch": 3}),
        ];
        let ids = service.send_batch(&queue_name, &messages).await.unwrap();
        assert_eq!(ids.len(), 3, "Should return 3 message IDs");

        // Verify all messages are in queue
        let stats = service.queue_stats(&queue_name).await.unwrap();
        assert_eq!(stats.message_count, 3);
    }

    // =========================================================================
    // TAS-149: Shared Listener Manager Tests
    // =========================================================================

    #[test]
    fn test_extract_namespace_from_queue() {
        // Standard worker queues: worker_{namespace}_queue
        assert_eq!(
            extract_namespace_from_queue("worker_default_queue"),
            "default"
        );
        assert_eq!(
            extract_namespace_from_queue("worker_conditional_approval_py_queue"),
            "conditional_approval_py"
        );
        assert_eq!(
            extract_namespace_from_queue("worker_diamond_workflow_dsl_py_queue"),
            "diamond_workflow_dsl_py"
        );
        // Orchestration queues: all map to "orchestration" (matches SQL extract_queue_namespace)
        assert_eq!(
            extract_namespace_from_queue("orchestration_step_results"),
            "orchestration"
        );
        assert_eq!(
            extract_namespace_from_queue("orchestration_task_requests"),
            "orchestration"
        );
        assert_eq!(
            extract_namespace_from_queue("orchestration_task_finalizations"),
            "orchestration"
        );
        assert_eq!(
            extract_namespace_from_queue("orchestration_queue"),
            "orchestration"
        );
        // Non-worker, non-orchestration queues: strip _queue suffix
        assert_eq!(extract_namespace_from_queue("orders_queue"), "orders");
        // No pattern match: returned as-is
        assert_eq!(extract_namespace_from_queue("simple"), "simple");
    }

    #[test]
    fn test_convert_event_queue_created_returns_none() {
        use tasker_pgmq::QueueCreatedEvent;
        let event = PgmqNotifyEvent::QueueCreated(QueueCreatedEvent::new("test_queue", "test"));
        assert!(convert_event_to_notification(&event).is_none());
    }

    #[test]
    fn test_convert_event_message_ready() {
        use tasker_pgmq::MessageReadyEvent;
        let event = PgmqNotifyEvent::MessageReady(MessageReadyEvent::new(42, "test_queue", "test"));
        let notification = convert_event_to_notification(&event).unwrap();
        assert!(notification.is_available());
        assert_eq!(notification.queue_name(), "test_queue");
        assert_eq!(notification.msg_id(), Some(42));
    }

    #[test]
    fn test_convert_event_batch_ready_with_ids() {
        use tasker_pgmq::BatchReadyEvent;
        let event =
            PgmqNotifyEvent::BatchReady(BatchReadyEvent::new(vec![1, 2, 3], "test_queue", "test"));
        let notification = convert_event_to_notification(&event).unwrap();
        assert!(notification.is_available());
        assert_eq!(notification.msg_id(), Some(1));
    }

    #[test]
    fn test_convert_event_batch_ready_empty() {
        use tasker_pgmq::BatchReadyEvent;
        let event = PgmqNotifyEvent::BatchReady(BatchReadyEvent::new(vec![], "test_queue", "test"));
        let notification = convert_event_to_notification(&event).unwrap();
        assert!(notification.is_available());
        assert_eq!(notification.msg_id(), None);
    }
}
