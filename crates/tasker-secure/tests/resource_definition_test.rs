//! Tests for deserializing ResourceDefinition from TOML config fragments.

use tasker_secure::resource::{ConfigValue, ResourceDefinition, ResourceType};

#[test]
fn deserialize_postgres_resource_definition() {
    let toml_str = r#"
        name = "primary_db"
        resource_type = "postgres"

        [config]
        host = "localhost"
        port = "5432"
        database = "tasker"
        user = { secret_ref = "db/primary/user" }
        password = { secret_ref = "db/primary/password" }
    "#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.name, "primary_db");
    assert_eq!(def.resource_type, ResourceType::Postgres);
    assert!(def.secrets_provider.is_none());

    let config = &def.config;

    match config.get("host").unwrap() {
        ConfigValue::Literal(v) => assert_eq!(v, "localhost"),
        other => panic!("expected Literal for host, got {other:?}"),
    }
    match config.get("port").unwrap() {
        ConfigValue::Literal(v) => assert_eq!(v, "5432"),
        other => panic!("expected Literal for port, got {other:?}"),
    }
    match config.get("database").unwrap() {
        ConfigValue::Literal(v) => assert_eq!(v, "tasker"),
        other => panic!("expected Literal for database, got {other:?}"),
    }
    match config.get("user").unwrap() {
        ConfigValue::SecretRef { secret_ref } => assert_eq!(secret_ref, "db/primary/user"),
        other => panic!("expected SecretRef for user, got {other:?}"),
    }
    match config.get("password").unwrap() {
        ConfigValue::SecretRef { secret_ref } => assert_eq!(secret_ref, "db/primary/password"),
        other => panic!("expected SecretRef for password, got {other:?}"),
    }
}

#[test]
fn deserialize_http_resource_definition() {
    let toml_str = r#"
        name = "payment_api"
        resource_type = "http"

        [config]
        base_url = "https://api.payments.example.com"
        auth_value = { secret_ref = "payment/api_key" }
    "#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.name, "payment_api");
    assert_eq!(def.resource_type, ResourceType::Http);

    match def.config.get("base_url").unwrap() {
        ConfigValue::Literal(v) => assert_eq!(v, "https://api.payments.example.com"),
        other => panic!("expected Literal for base_url, got {other:?}"),
    }
    match def.config.get("auth_value").unwrap() {
        ConfigValue::SecretRef { secret_ref } => assert_eq!(secret_ref, "payment/api_key"),
        other => panic!("expected SecretRef for auth_value, got {other:?}"),
    }
}

#[test]
fn deserialize_messaging_resource_definition() {
    let toml_str = r#"
        name = "task_queue"
        resource_type = "pgmq"
    "#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.name, "task_queue");
    assert_eq!(def.resource_type, ResourceType::Messaging);
    assert!(def.config.get("anything").is_none());
}

#[test]
fn deserialize_resource_with_env_ref() {
    let toml_str = r#"
        name = "local_db"
        resource_type = "postgres"

        [config]
        host = "localhost"
        password = { env = "LOCAL_DB_PASSWORD" }
    "#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.name, "local_db");
    assert_eq!(def.resource_type, ResourceType::Postgres);

    match def.config.get("host").unwrap() {
        ConfigValue::Literal(v) => assert_eq!(v, "localhost"),
        other => panic!("expected Literal for host, got {other:?}"),
    }
    match def.config.get("password").unwrap() {
        ConfigValue::EnvRef { env } => assert_eq!(env, "LOCAL_DB_PASSWORD"),
        other => panic!("expected EnvRef for password, got {other:?}"),
    }
}

#[test]
fn deserialize_resource_with_secrets_provider() {
    let toml_str = r#"
        name = "vault_db"
        resource_type = "postgres"
        secrets_provider = "vault"

        [config]
        host = "db.internal"
        password = { secret_ref = "database/creds/primary" }
    "#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.name, "vault_db");
    assert_eq!(def.resource_type, ResourceType::Postgres);
    assert_eq!(def.secrets_provider.as_deref(), Some("vault"));

    match def.config.get("password").unwrap() {
        ConfigValue::SecretRef { secret_ref } => {
            assert_eq!(secret_ref, "database/creds/primary");
        }
        other => panic!("expected SecretRef for password, got {other:?}"),
    }
}

#[test]
fn deserialize_multiple_resource_definitions() {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct ResourceList {
        resources: Vec<ResourceDefinition>,
    }

    let toml_str = r#"
        [[resources]]
        name = "primary_db"
        resource_type = "postgres"

        [resources.config]
        host = "localhost"
        password = { secret_ref = "db/password" }

        [[resources]]
        name = "cache_queue"
        resource_type = "pgmq"

        [[resources]]
        name = "external_api"
        resource_type = "http"

        [resources.config]
        base_url = "https://api.example.com"
    "#;

    let list: ResourceList = toml::from_str(toml_str).unwrap();
    assert_eq!(list.resources.len(), 3);

    assert_eq!(list.resources[0].name, "primary_db");
    assert_eq!(list.resources[0].resource_type, ResourceType::Postgres);
    match list.resources[0].config.get("password").unwrap() {
        ConfigValue::SecretRef { secret_ref } => assert_eq!(secret_ref, "db/password"),
        other => panic!("expected SecretRef, got {other:?}"),
    }

    assert_eq!(list.resources[1].name, "cache_queue");
    assert_eq!(list.resources[1].resource_type, ResourceType::Messaging);

    assert_eq!(list.resources[2].name, "external_api");
    assert_eq!(list.resources[2].resource_type, ResourceType::Http);
    match list.resources[2].config.get("base_url").unwrap() {
        ConfigValue::Literal(v) => assert_eq!(v, "https://api.example.com"),
        other => panic!("expected Literal, got {other:?}"),
    }
}
