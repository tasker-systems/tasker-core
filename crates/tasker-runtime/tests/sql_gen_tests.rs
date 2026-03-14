//! Tests for SQL generation and identifier sanitization.

#![cfg(feature = "postgres")]

use tasker_grammar::operations::{
    AcquireConstraints, ConflictStrategy, PersistConstraints, PersistMode,
};
use tasker_runtime::adapters::sql_gen::{
    build_delete, build_insert, build_select, build_update, build_upsert, quote_identifier,
    validate_identifier,
};

#[test]
fn valid_simple_identifier() {
    assert!(validate_identifier("orders").is_ok());
}

#[test]
fn valid_identifier_with_underscore() {
    assert!(validate_identifier("order_line_items").is_ok());
}

#[test]
fn valid_identifier_starting_with_underscore() {
    assert!(validate_identifier("_internal").is_ok());
}

#[test]
fn invalid_identifier_starts_with_number() {
    assert!(validate_identifier("1orders").is_err());
}

#[test]
fn invalid_identifier_contains_semicolon() {
    assert!(validate_identifier("orders; DROP TABLE").is_err());
}

#[test]
fn invalid_identifier_contains_quotes() {
    assert!(validate_identifier("orders\"--").is_err());
}

#[test]
fn invalid_identifier_empty() {
    assert!(validate_identifier("").is_err());
}

#[test]
fn invalid_identifier_too_long() {
    let long_name = "a".repeat(64);
    assert!(validate_identifier(&long_name).is_err());
}

#[test]
fn valid_identifier_max_length() {
    let max_name = "a".repeat(63);
    assert!(validate_identifier(&max_name).is_ok());
}

#[test]
fn invalid_identifier_unicode() {
    assert!(validate_identifier("über_table").is_err());
}

#[test]
fn quote_identifier_wraps_in_double_quotes() {
    assert_eq!(quote_identifier("orders"), "\"orders\"");
}

#[test]
fn quote_identifier_handles_underscores() {
    assert_eq!(quote_identifier("order_line_items"), "\"order_line_items\"");
}

#[test]
fn build_insert_simple() {
    let columns = vec!["id".to_string(), "name".to_string(), "total".to_string()];
    let result = build_insert("orders", &columns, &PersistConstraints::default()).unwrap();
    assert_eq!(
        result.sql,
        "INSERT INTO \"orders\" (\"id\", \"name\", \"total\") VALUES ($1, $2, $3) RETURNING *"
    );
    assert_eq!(result.bind_columns, vec!["id", "name", "total"]);
}

#[test]
fn build_insert_single_column() {
    let columns = vec!["id".to_string()];
    let result = build_insert("users", &columns, &PersistConstraints::default()).unwrap();
    assert_eq!(
        result.sql,
        "INSERT INTO \"users\" (\"id\") VALUES ($1) RETURNING *"
    );
}

#[test]
fn build_insert_rejects_invalid_entity() {
    let columns = vec!["id".to_string()];
    assert!(build_insert(
        "orders; DROP TABLE users",
        &columns,
        &PersistConstraints::default()
    )
    .is_err());
}

#[test]
fn build_insert_rejects_invalid_column() {
    let columns = vec!["id".to_string(), "name; --".to_string()];
    assert!(build_insert("orders", &columns, &PersistConstraints::default()).is_err());
}

#[test]
fn build_insert_rejects_empty_columns() {
    assert!(build_insert("orders", &[], &PersistConstraints::default()).is_err());
}

#[test]
fn build_upsert_with_update_strategy() {
    let columns = vec!["id".to_string(), "name".to_string(), "total".to_string()];
    let constraints = PersistConstraints {
        mode: PersistMode::Upsert,
        upsert_key: Some(vec!["id".to_string()]),
        on_conflict: Some(ConflictStrategy::Update),
        ..Default::default()
    };
    let result = build_upsert("orders", &columns, &constraints).unwrap();
    assert!(result.sql.contains("ON CONFLICT (\"id\") DO UPDATE SET"));
    assert!(result.sql.contains("\"name\" = EXCLUDED.\"name\""));
    assert!(result.sql.contains("\"total\" = EXCLUDED.\"total\""));
    // Conflict key should NOT be in the UPDATE SET clause
    assert!(!result.sql.contains("\"id\" = EXCLUDED.\"id\""));
}

#[test]
fn build_upsert_with_skip_strategy() {
    let columns = vec!["id".to_string(), "name".to_string()];
    let constraints = PersistConstraints {
        mode: PersistMode::Upsert,
        upsert_key: Some(vec!["id".to_string()]),
        on_conflict: Some(ConflictStrategy::Skip),
        ..Default::default()
    };
    let result = build_upsert("orders", &columns, &constraints).unwrap();
    assert!(result.sql.contains("ON CONFLICT (\"id\") DO NOTHING"));
}

#[test]
fn build_upsert_with_composite_key() {
    let columns = vec![
        "order_id".to_string(),
        "line_num".to_string(),
        "qty".to_string(),
    ];
    let constraints = PersistConstraints {
        mode: PersistMode::Upsert,
        upsert_key: Some(vec!["order_id".to_string(), "line_num".to_string()]),
        on_conflict: Some(ConflictStrategy::Update),
        ..Default::default()
    };
    let result = build_upsert("line_items", &columns, &constraints).unwrap();
    assert!(result
        .sql
        .contains("ON CONFLICT (\"order_id\", \"line_num\") DO UPDATE SET"));
}

#[test]
fn build_upsert_requires_upsert_key() {
    let columns = vec!["id".to_string()];
    let constraints = PersistConstraints {
        mode: PersistMode::Upsert,
        on_conflict: Some(ConflictStrategy::Update),
        ..Default::default()
    };
    assert!(build_upsert("orders", &columns, &constraints).is_err());
}

#[test]
fn build_update_simple() {
    let columns = vec!["name".to_string(), "total".to_string()];
    let identity_keys = vec!["id".to_string()];
    let result = build_update("orders", &columns, &identity_keys).unwrap();
    assert_eq!(
        result.sql,
        "UPDATE \"orders\" SET \"name\" = $1, \"total\" = $2 WHERE \"id\" = $3 RETURNING *"
    );
    assert_eq!(result.bind_columns, vec!["name", "total", "id"]);
}

#[test]
fn build_update_composite_key() {
    let columns = vec!["qty".to_string()];
    let identity_keys = vec!["order_id".to_string(), "line_num".to_string()];
    let result = build_update("line_items", &columns, &identity_keys).unwrap();
    assert!(result
        .sql
        .contains("WHERE \"order_id\" = $2 AND \"line_num\" = $3"));
}

#[test]
fn build_update_requires_identity_keys() {
    let columns = vec!["name".to_string()];
    assert!(build_update("orders", &columns, &[]).is_err());
}

#[test]
fn build_delete_simple() {
    let identity_keys = vec!["id".to_string()];
    let result = build_delete("orders", &identity_keys).unwrap();
    assert_eq!(
        result.sql,
        "DELETE FROM \"orders\" WHERE \"id\" = $1 RETURNING *"
    );
}

#[test]
fn build_delete_composite_key() {
    let identity_keys = vec!["order_id".to_string(), "line_num".to_string()];
    let result = build_delete("line_items", &identity_keys).unwrap();
    assert!(result
        .sql
        .contains("WHERE \"order_id\" = $1 AND \"line_num\" = $2"));
}

#[test]
fn build_delete_requires_identity_keys() {
    assert!(build_delete("orders", &[]).is_err());
}

#[test]
fn build_select_all_columns() {
    let result = build_select(
        "orders",
        &[],
        &serde_json::json!({}),
        &AcquireConstraints::default(),
    )
    .unwrap();
    assert_eq!(result.sql, "SELECT * FROM \"orders\"");
}

#[test]
fn build_select_specific_columns() {
    let columns = vec!["id".to_string(), "name".to_string()];
    let result = build_select(
        "orders",
        &columns,
        &serde_json::json!({}),
        &AcquireConstraints::default(),
    )
    .unwrap();
    assert_eq!(result.sql, "SELECT \"id\", \"name\" FROM \"orders\"");
}

#[test]
fn build_select_with_params() {
    let params = serde_json::json!({"status": "pending", "customer_id": 42});
    let result = build_select("orders", &[], &params, &AcquireConstraints::default()).unwrap();
    assert!(result.sql.contains("WHERE"));
    assert!(result.sql.contains("\"customer_id\" = $"));
    assert!(result.sql.contains("\"status\" = $"));
}

#[test]
fn build_select_with_limit_and_offset() {
    let constraints = AcquireConstraints {
        limit: Some(100),
        offset: Some(50),
        ..Default::default()
    };
    let result = build_select("orders", &[], &serde_json::json!({}), &constraints).unwrap();
    assert!(result.sql.contains("LIMIT 100"));
    assert!(result.sql.contains("OFFSET 50"));
}

#[test]
fn build_select_rejects_invalid_entity() {
    assert!(build_select(
        "orders; --",
        &[],
        &serde_json::json!({}),
        &AcquireConstraints::default()
    )
    .is_err());
}
