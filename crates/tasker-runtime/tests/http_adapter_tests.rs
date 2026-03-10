//! Tests for HTTP adapter URL and method construction logic.

use tasker_grammar::operations::PersistMode;
use tasker_runtime::adapters::http::http_persist_method;

#[test]
fn persist_mode_insert_maps_to_post() {
    assert_eq!(http_persist_method(&PersistMode::Insert), "POST");
}

#[test]
fn persist_mode_update_maps_to_patch() {
    assert_eq!(http_persist_method(&PersistMode::Update), "PATCH");
}

#[test]
fn persist_mode_upsert_maps_to_put() {
    assert_eq!(http_persist_method(&PersistMode::Upsert), "PUT");
}

#[test]
fn persist_mode_delete_maps_to_delete() {
    assert_eq!(http_persist_method(&PersistMode::Delete), "DELETE");
}
