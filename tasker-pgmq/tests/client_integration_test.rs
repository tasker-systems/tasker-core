mod common;

use common::TestDb;
use serde_json::{json, Value};
use tasker_pgmq::{PgmqClient, PgmqNotifyClientFactory, PgmqNotifyConfig};

/// Helper: create a PgmqClient from a TestDb pool
async fn make_client(test_db: &TestDb) -> PgmqClient {
    PgmqClient::new_with_pool(test_db.pool.clone()).await
}

/// Helper: create a unique queue via PgmqClient and return its name
async fn create_client_queue(client: &PgmqClient, base: &str, test_id: &str) -> String {
    let name = format!("{}_{}", base, test_id);
    client.create_queue(&name).await.expect("create_queue");
    name
}

/// Helper: drop a queue, ignoring errors (for cleanup)
async fn cleanup_queue(test_db: &TestDb, queue_name: &str) {
    let _ = sqlx::query("SELECT pgmq.drop_queue($1)")
        .bind(queue_name)
        .execute(&test_db.pool)
        .await;
}

// ---------------------------------------------------------------------------
// Test 1: Full queue lifecycle – create, send, read, delete, drop
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_queue_lifecycle() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;
    let q = create_client_queue(&client, "lifecycle_queue", &test_db.test_id).await;

    // Send
    let msg = json!({"action": "test_lifecycle"});
    let msg_id = client.send_json_message(&q, &msg).await.expect("send");
    assert!(msg_id > 0);

    // Read
    let msgs = client
        .read_messages(&q, Some(30), Some(10))
        .await
        .expect("read");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].msg_id, msg_id);
    assert_eq!(msgs[0].message["action"], "test_lifecycle");

    // Delete
    client.delete_message(&q, msg_id).await.expect("delete");

    // Verify gone
    let msgs = client
        .read_messages(&q, Some(0), Some(10))
        .await
        .expect("read after delete");
    assert!(msgs.is_empty());

    // Drop queue
    client.drop_queue(&q).await.expect("drop_queue");
}

// ---------------------------------------------------------------------------
// Test 2: Send with delay (0s delay = immediately visible)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_send_with_delay() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;
    let q = create_client_queue(&client, "delay_queue", &test_db.test_id).await;

    let msg = json!({"action": "delayed"});
    let msg_id = client
        .send_message_with_delay(&q, &msg, 0)
        .await
        .expect("send_with_delay");
    assert!(msg_id > 0);

    let msgs = client
        .read_messages(&q, Some(0), Some(10))
        .await
        .expect("read");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].msg_id, msg_id);

    cleanup_queue(&test_db, &q).await;
}

// ---------------------------------------------------------------------------
// Test 3: Pop message (returns Some then None)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_pop_message() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;
    let q = create_client_queue(&client, "pop_queue", &test_db.test_id).await;

    client
        .send_json_message(&q, &json!({"pop": true}))
        .await
        .expect("send");

    let first = client.pop_message(&q).await.expect("pop first");
    assert!(first.is_some());
    assert_eq!(first.unwrap().message["pop"], true);

    let second = client.pop_message(&q).await.expect("pop second");
    assert!(second.is_none());

    cleanup_queue(&test_db, &q).await;
}

// ---------------------------------------------------------------------------
// Test 4: Archive message (removed from active queue)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_archive_message() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;
    let q = create_client_queue(&client, "archive_queue", &test_db.test_id).await;

    let msg_id = client
        .send_json_message(&q, &json!({"archive": "me"}))
        .await
        .expect("send");

    client.archive_message(&q, msg_id).await.expect("archive");

    let msgs = client
        .read_messages(&q, Some(0), Some(10))
        .await
        .expect("read after archive");
    assert!(msgs.is_empty());

    cleanup_queue(&test_db, &q).await;
}

// ---------------------------------------------------------------------------
// Test 5: Set visibility timeout (make invisible then re-visible)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_set_visibility_timeout() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;
    let q = create_client_queue(&client, "vt_queue", &test_db.test_id).await;

    let msg_id = client
        .send_json_message(&q, &json!({"vt": "test"}))
        .await
        .expect("send");

    // Read with long vt to make invisible
    let msgs = client
        .read_messages(&q, Some(600), Some(10))
        .await
        .expect("read with vt=600");
    assert_eq!(msgs.len(), 1);

    // Message should now be invisible
    let hidden = client
        .read_messages(&q, Some(0), Some(10))
        .await
        .expect("read hidden");
    assert!(hidden.is_empty());

    // Reset vt to 0 to make it visible immediately
    client
        .set_visibility_timeout(&q, msg_id, 0)
        .await
        .expect("set_vt");

    let visible = client
        .read_messages(&q, Some(0), Some(10))
        .await
        .expect("read after vt reset");
    assert_eq!(visible.len(), 1);

    cleanup_queue(&test_db, &q).await;
}

// ---------------------------------------------------------------------------
// Test 6: Queue metrics
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_queue_metrics() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;
    let q = create_client_queue(&client, "metrics_queue", &test_db.test_id).await;

    for i in 0..3 {
        client
            .send_json_message(&q, &json!({"idx": i}))
            .await
            .expect("send");
    }

    let metrics = client.queue_metrics(&q).await.expect("queue_metrics");
    assert_eq!(metrics.queue_name, q);
    assert_eq!(metrics.message_count, 3);

    cleanup_queue(&test_db, &q).await;
}

// ---------------------------------------------------------------------------
// Test 7: Health check, client status, and notify capabilities
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_health_and_status() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;

    let healthy = client.health_check().await.expect("health_check");
    assert!(healthy);

    let status = client.get_client_status().await.expect("get_client_status");
    assert!(status.connected);
    assert_eq!(status.client_type, "pgmq-unified");

    // Default config has triggers disabled
    assert!(!client.has_notify_capabilities());
}

// ---------------------------------------------------------------------------
// Test 8: Send within a transaction
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_send_with_transaction() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;
    let q = create_client_queue(&client, "tx_queue", &test_db.test_id).await;

    let mut tx = test_db.pool.begin().await.expect("begin tx");
    let msg_id = client
        .send_with_transaction(&q, &json!({"tx": true}), &mut tx)
        .await
        .expect("send_with_transaction");
    assert!(msg_id > 0);

    // Before commit, message shouldn't be visible from another connection
    tx.commit().await.expect("commit");

    // After commit, message is readable
    let msgs = client
        .read_messages(&q, Some(0), Some(10))
        .await
        .expect("read after commit");
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].message["tx"], true);

    cleanup_queue(&test_db, &q).await;
}

// ---------------------------------------------------------------------------
// Test 9: Read specific message (found + not-found)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_read_specific_message() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;
    let q = create_client_queue(&client, "specific_queue", &test_db.test_id).await;

    let msg_id = client
        .send_json_message(&q, &json!({"specific": "value"}))
        .await
        .expect("send");

    // Found path
    let found = client
        .read_specific_message::<Value>(&q, msg_id, 30)
        .await
        .expect("read_specific found");
    assert!(found.is_some());
    let m = found.unwrap();
    assert_eq!(m.msg_id, msg_id);
    assert_eq!(m.message["specific"], "value");

    // Not-found path (non-existent ID)
    let not_found = client
        .read_specific_message::<Value>(&q, 999_999_999, 30)
        .await
        .expect("read_specific not found");
    assert!(not_found.is_none());

    cleanup_queue(&test_db, &q).await;
}

// ---------------------------------------------------------------------------
// Test 10: Namespace operations (initialize, process, complete)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_namespace_operations() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;

    let ns = format!("nstest{}", &test_db.test_id);

    // Initialize creates worker_{ns}_queue
    client
        .initialize_namespace_queues(&[ns.as_str()])
        .await
        .expect("initialize_namespace_queues");

    // Process empty queue
    let empty = client
        .process_namespace_queue(&ns, Some(0), 10)
        .await
        .expect("process empty");
    assert!(empty.is_empty());

    // Send a message to the namespace queue
    let queue_name = format!("worker_{ns}_queue");
    client
        .send_json_message(&queue_name, &json!({"ns": "work"}))
        .await
        .expect("send to ns queue");

    // Process should find the message
    let msgs = client
        .process_namespace_queue(&ns, Some(30), 10)
        .await
        .expect("process with msg");
    assert_eq!(msgs.len(), 1);

    // Complete (delete) the message
    client
        .complete_message(&ns, msgs[0].msg_id)
        .await
        .expect("complete_message");

    // After completion, process returns empty
    let after = client
        .process_namespace_queue(&ns, Some(0), 10)
        .await
        .expect("process after complete");
    assert!(after.is_empty());

    cleanup_queue(&test_db, &queue_name).await;
}

// ---------------------------------------------------------------------------
// Test 11: extract_namespace (regex match, fallback, no-match)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_extract_namespace() {
    let test_db = TestDb::new().await.expect("TestDb::new");
    let client = make_client(&test_db).await;

    // Regex match: "orders_queue" -> "orders"
    assert_eq!(
        client.extract_namespace("orders_queue"),
        Some("orders".to_string())
    );

    // Regex match: "worker_rust_queue" -> "worker_rust"
    // The default pattern (?P<namespace>\w+)_queue captures the longest \w+ before _queue
    assert_eq!(
        client.extract_namespace("worker_rust_queue"),
        Some("worker_rust".to_string())
    );

    // No _queue suffix, no regex match -> None
    assert_eq!(client.extract_namespace("no_match"), None);

    // Fallback: "_queue" suffix but greedy regex still matches
    assert_eq!(
        client.extract_namespace("my_service_queue"),
        Some("my_service".to_string())
    );
}

// ---------------------------------------------------------------------------
// Test 12: PgmqClientFactory methods
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_client_factory_methods() {
    let test_db = TestDb::new().await.expect("TestDb::new");

    // create_with_pool
    let client = PgmqNotifyClientFactory::create_with_pool(test_db.pool.clone()).await;
    let healthy = client.health_check().await.expect("health_check");
    assert!(healthy);

    // create_with_pool_and_config
    let config = PgmqNotifyConfig::new().with_triggers_enabled(true);
    let client =
        PgmqNotifyClientFactory::create_with_pool_and_config(test_db.pool.clone(), config).await;
    assert!(client.has_notify_capabilities());
    assert!(client.config().enable_triggers);
}
