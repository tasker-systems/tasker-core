mod common;

use async_trait::async_trait;
use common::TestDb;
use sqlx::PgPool;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tasker_pgmq::listener::PgmqEventHandler;
use tasker_pgmq::{
    MessageReadyEvent, PgmqNotifyConfig, PgmqNotifyError, PgmqNotifyEvent, PgmqNotifyListener,
};

/// Helper: get the database URL the same way TestDb does
fn database_url() -> String {
    std::env::var("PGMQ_DATABASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("DATABASE_URL").ok())
        .unwrap_or_else(|| "postgresql://tasker:tasker@localhost:5432/tasker_rust_test".to_string())
}

/// Helper: create a second pool for sending NOTIFY from outside the listener's connection
async fn notify_pool() -> PgPool {
    PgPool::connect(&database_url())
        .await
        .expect("notify_pool connect")
}

/// Mock event handler that collects events
struct CollectingHandler {
    events: Arc<RwLock<Vec<PgmqNotifyEvent>>>,
    parse_errors: Arc<RwLock<Vec<String>>>,
}

impl CollectingHandler {
    fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            parse_errors: Arc::new(RwLock::new(Vec::new())),
        }
    }

    fn events_clone(&self) -> Arc<RwLock<Vec<PgmqNotifyEvent>>> {
        Arc::clone(&self.events)
    }
}

#[async_trait]
impl PgmqEventHandler for CollectingHandler {
    async fn handle_event(&self, event: PgmqNotifyEvent) -> tasker_pgmq::Result<()> {
        self.events.write().unwrap().push(event);
        Ok(())
    }

    async fn handle_parse_error(&self, channel: &str, payload: &str, _error: PgmqNotifyError) {
        self.parse_errors
            .write()
            .unwrap()
            .push(format!("{channel}:{payload}"));
    }

    async fn handle_connection_error(&self, _error: PgmqNotifyError) {}
}

// ---------------------------------------------------------------------------
// Test 13: Construction – verify config and default stats
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_listener_construction() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let config = PgmqNotifyConfig::new();
    let listener = PgmqNotifyListener::new(test_db.pool.clone(), config.clone(), 100)
        .await
        .expect("new listener");

    assert_eq!(
        listener.config().queue_naming_pattern,
        config.queue_naming_pattern
    );
    let stats = listener.stats();
    assert!(!stats.connected);
    assert_eq!(stats.channels_listening, 0);
    assert_eq!(stats.events_received, 0);
    assert_eq!(stats.parse_errors, 0);
}

// ---------------------------------------------------------------------------
// Test 14: Connect and disconnect lifecycle
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_listener_connect_disconnect() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let config = PgmqNotifyConfig::new();
    let mut listener = PgmqNotifyListener::new(test_db.pool.clone(), config, 100)
        .await
        .expect("new listener");

    // Connect
    listener.connect().await.expect("connect");
    assert!(listener.is_healthy().await);
    assert!(listener.stats().connected);

    // Connect again (idempotent)
    listener.connect().await.expect("connect again");
    assert!(listener.is_healthy().await);

    // Disconnect
    listener.disconnect().await.expect("disconnect");
    assert!(!listener.is_healthy().await);
    assert!(!listener.stats().connected);
    assert!(listener.listening_channels().is_empty());
}

// ---------------------------------------------------------------------------
// Test 15: Channel management (listen, duplicate, unlisten)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_listener_channel_management() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let config = PgmqNotifyConfig::new();
    let mut listener = PgmqNotifyListener::new(test_db.pool.clone(), config, 100)
        .await
        .expect("new listener");

    listener.connect().await.expect("connect");

    // Listen to two channels
    listener
        .listen_channel("chan_a")
        .await
        .expect("listen chan_a");
    listener
        .listen_channel("chan_b")
        .await
        .expect("listen chan_b");

    let channels = listener.listening_channels();
    assert_eq!(channels.len(), 2);
    assert!(channels.contains(&"chan_a".to_string()));
    assert!(channels.contains(&"chan_b".to_string()));

    // Duplicate listen (should be a no-op warning, not an error)
    listener
        .listen_channel("chan_a")
        .await
        .expect("listen chan_a duplicate");
    assert_eq!(listener.listening_channels().len(), 2);

    // Unlisten one
    listener
        .unlisten_channel("chan_a")
        .await
        .expect("unlisten chan_a");
    let channels = listener.listening_channels();
    assert_eq!(channels.len(), 1);
    assert!(channels.contains(&"chan_b".to_string()));

    assert_eq!(listener.stats().channels_listening, 1);

    listener.disconnect().await.expect("disconnect");
}

// ---------------------------------------------------------------------------
// Test 16: NotConnected errors before connect()
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_listener_not_connected_errors() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let config = PgmqNotifyConfig::new();
    let mut listener = PgmqNotifyListener::new(test_db.pool.clone(), config, 100)
        .await
        .expect("new listener");

    // listen_channel before connect
    let err = listener.listen_channel("test").await;
    assert!(
        matches!(err, Err(PgmqNotifyError::NotConnected)),
        "listen_channel should return NotConnected"
    );

    // unlisten_channel before connect
    let err = listener.unlisten_channel("test").await;
    assert!(
        matches!(err, Err(PgmqNotifyError::NotConnected)),
        "unlisten_channel should return NotConnected"
    );

    // start_listening before connect
    let err = listener.start_listening().await;
    assert!(
        matches!(err, Err(PgmqNotifyError::NotConnected)),
        "start_listening should return NotConnected"
    );
}

// ---------------------------------------------------------------------------
// Test 17: Convenience channel subscription methods
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_listener_convenience_channels() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let config = PgmqNotifyConfig::new()
        .with_default_namespace("rust")
        .with_default_namespace("python");
    let mut listener = PgmqNotifyListener::new(test_db.pool.clone(), config, 100)
        .await
        .expect("new listener");

    listener.connect().await.expect("connect");

    listener
        .listen_queue_created()
        .await
        .expect("listen_queue_created");
    listener
        .listen_message_ready_for_namespace("orders")
        .await
        .expect("listen_message_ready_for_namespace");
    listener
        .listen_message_ready_global()
        .await
        .expect("listen_message_ready_global");
    listener
        .listen_default_namespaces()
        .await
        .expect("listen_default_namespaces");

    let channels = listener.listening_channels();
    assert!(channels.contains(&"pgmq_queue_created".to_string()));
    assert!(channels.contains(&"pgmq_message_ready.orders".to_string()));
    assert!(channels.contains(&"pgmq_message_ready".to_string()));
    assert!(channels.contains(&"pgmq_message_ready.rust".to_string()));
    assert!(channels.contains(&"pgmq_message_ready.python".to_string()));
    assert_eq!(channels.len(), 5);

    listener.disconnect().await.expect("disconnect");
}

// ---------------------------------------------------------------------------
// Test 18: Listener receives events via pg_notify + start_listening + next_event
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_listener_receives_events() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let config = PgmqNotifyConfig::new();
    let channel_name = format!("test_events_{}", test_db.test_id);

    let mut listener = PgmqNotifyListener::new(test_db.pool.clone(), config, 100)
        .await
        .expect("new listener");

    listener.connect().await.expect("connect");
    listener
        .listen_channel(&channel_name)
        .await
        .expect("listen_channel");
    listener.start_listening().await.expect("start_listening");

    // Small delay to let the background listener task start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send a pg_notify from a separate connection
    let pool2 = notify_pool().await;
    let event = PgmqNotifyEvent::MessageReady(MessageReadyEvent::new(42, "test_queue", "test"));
    let payload = serde_json::to_string(&event).expect("serialize event");
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(&channel_name)
        .bind(&payload)
        .execute(&pool2)
        .await
        .expect("pg_notify");

    // Receive with timeout
    let result = tokio::time::timeout(Duration::from_secs(5), listener.next_event()).await;
    let received = result
        .expect("timeout waiting for event")
        .expect("next_event error");
    assert!(received.is_some());
    let received = received.unwrap();
    assert_eq!(received.namespace(), "test");
    assert_eq!(received.queue_name(), "test_queue");
    assert_eq!(received.msg_id(), Some(42));

    // Verify stats
    let stats = listener.stats();
    assert!(stats.events_received >= 1);
}

// ---------------------------------------------------------------------------
// Test 19: Handler receives events via start_listening_with_handler
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_listener_handler_receives_events() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let config = PgmqNotifyConfig::new();
    let channel_name = format!("test_handler_{}", test_db.test_id);

    let mut listener = PgmqNotifyListener::new(test_db.pool.clone(), config, 100)
        .await
        .expect("new listener");

    listener.connect().await.expect("connect");
    listener
        .listen_channel(&channel_name)
        .await
        .expect("listen_channel");

    let handler = CollectingHandler::new();
    let events_ref = handler.events_clone();

    let handle = listener
        .start_listening_with_handler(handler)
        .await
        .expect("start_listening_with_handler");

    // Small delay for task to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Send event
    let pool2 = notify_pool().await;
    let event =
        PgmqNotifyEvent::MessageReady(MessageReadyEvent::new(99, "handler_queue", "handler_ns"));
    let payload = serde_json::to_string(&event).expect("serialize");
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(&channel_name)
        .bind(&payload)
        .execute(&pool2)
        .await
        .expect("pg_notify");

    // Wait for handler to receive
    tokio::time::sleep(Duration::from_millis(500)).await;

    let received = events_ref.read().unwrap();
    assert!(!received.is_empty(), "handler should have received events");
    assert_eq!(received[0].namespace(), "handler_ns");
    assert_eq!(received[0].msg_id(), Some(99));

    // Abort the background task
    handle.abort();
}

// ---------------------------------------------------------------------------
// Test 20: Malformed notify payloads increment parse_errors
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_listener_malformed_notify() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let config = PgmqNotifyConfig::new();
    let channel_name = format!("test_malformed_{}", test_db.test_id);

    let mut listener = PgmqNotifyListener::new(test_db.pool.clone(), config, 100)
        .await
        .expect("new listener");

    listener.connect().await.expect("connect");
    listener
        .listen_channel(&channel_name)
        .await
        .expect("listen_channel");
    listener.start_listening().await.expect("start_listening");

    tokio::time::sleep(Duration::from_millis(100)).await;

    let pool2 = notify_pool().await;

    // Send invalid JSON
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(&channel_name)
        .bind("this is not valid json {{{")
        .execute(&pool2)
        .await
        .expect("pg_notify invalid");

    // Send valid event
    let event = PgmqNotifyEvent::MessageReady(MessageReadyEvent::new(77, "test_queue", "test"));
    let payload = serde_json::to_string(&event).expect("serialize");
    sqlx::query("SELECT pg_notify($1, $2)")
        .bind(&channel_name)
        .bind(&payload)
        .execute(&pool2)
        .await
        .expect("pg_notify valid");

    // Receive the valid event (invalid one is silently dropped by start_listening)
    let result = tokio::time::timeout(Duration::from_secs(5), listener.next_event()).await;
    let received = result
        .expect("timeout waiting for valid event")
        .expect("next_event error");
    assert!(received.is_some());
    assert_eq!(received.unwrap().msg_id(), Some(77));

    // Verify parse_errors incremented
    let stats = listener.stats();
    assert!(
        stats.parse_errors >= 1,
        "parse_errors should be >= 1, got {}",
        stats.parse_errors
    );
}

// ---------------------------------------------------------------------------
// Test 21: Debug implementation
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_listener_debug_impl() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let config = PgmqNotifyConfig::new();
    let listener = PgmqNotifyListener::new(test_db.pool.clone(), config, 100)
        .await
        .expect("new listener");

    let debug_str = format!("{:?}", listener);
    assert!(
        debug_str.contains("PgmqNotifyListener"),
        "Debug output should contain struct name"
    );
}
