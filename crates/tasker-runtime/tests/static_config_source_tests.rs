//! Tests for StaticConfigSource resource definition lookup.

use tasker_runtime::sources::static_config::StaticConfigSource;
use tasker_runtime::ResourceDefinitionSource;
use tasker_secure::{ResourceConfig, ResourceDefinition, ResourceType};

fn make_definition(name: &str, rt: ResourceType) -> ResourceDefinition {
    ResourceDefinition {
        name: name.to_string(),
        resource_type: rt,
        config: ResourceConfig::default(),
        secrets_provider: None,
    }
}

#[tokio::test]
async fn resolve_existing_returns_definition() {
    let source = StaticConfigSource::new(vec![
        make_definition("db1", ResourceType::Postgres),
        make_definition("api1", ResourceType::Http),
    ]);

    let result = source.resolve("db1").await;
    assert!(result.is_some());
    let def = result.unwrap();
    assert_eq!(def.name, "db1");
    assert_eq!(def.resource_type, ResourceType::Postgres);
}

#[tokio::test]
async fn resolve_missing_returns_none() {
    let source = StaticConfigSource::new(vec![make_definition("db1", ResourceType::Postgres)]);

    let result = source.resolve("nonexistent").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn list_names_returns_all_registered() {
    let source = StaticConfigSource::new(vec![
        make_definition("db1", ResourceType::Postgres),
        make_definition("api1", ResourceType::Http),
        make_definition("queue1", ResourceType::Messaging),
    ]);

    let mut names = source.list_names().await;
    names.sort();
    assert_eq!(names, vec!["api1", "db1", "queue1"]);
}

#[tokio::test]
async fn empty_source_has_no_names() {
    let source = StaticConfigSource::new(vec![]);

    let names = source.list_names().await;
    assert!(names.is_empty());

    let result = source.resolve("anything").await;
    assert!(result.is_none());
}

#[tokio::test]
async fn watch_returns_none_for_static_source() {
    let source = StaticConfigSource::new(vec![make_definition("db1", ResourceType::Postgres)]);

    let watcher = source.watch().await;
    assert!(watcher.is_none());
}
