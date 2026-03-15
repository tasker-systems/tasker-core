//! Tests for resource types, config values, and resource definitions.

use std::collections::HashMap;

use tasker_secure::testing::InMemorySecretsProvider;
use tasker_secure::{
    ConfigValue, ResourceConfig, ResourceDefinition, ResourceError, ResourceSummary, ResourceType,
};

// ── ResourceType ────────────────────────────────────────────────────────────

#[test]
fn resource_type_debug_display() {
    assert_eq!(format!("{}", ResourceType::Postgres), "postgres");
    assert_eq!(format!("{}", ResourceType::Http), "http");
    assert_eq!(format!("{}", ResourceType::Messaging), "messaging");

    let custom = ResourceType::Custom {
        type_name: "redis".to_string(),
    };
    assert_eq!(format!("{}", custom), "redis");

    // Debug should also work
    assert_eq!(format!("{:?}", ResourceType::Postgres), "Postgres");
}

#[test]
fn resource_type_deserialize_from_toml() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        rt: ResourceType,
    }

    let pg: Wrapper = toml::from_str(r#"rt = "postgres""#).unwrap();
    assert_eq!(pg.rt, ResourceType::Postgres);

    let http: Wrapper = toml::from_str(r#"rt = "http""#).unwrap();
    assert_eq!(http.rt, ResourceType::Http);

    let pgmq: Wrapper = toml::from_str(r#"rt = "pgmq""#).unwrap();
    assert_eq!(pgmq.rt, ResourceType::Messaging);
}

#[test]
fn resource_type_deserialize_messaging_canonical() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        rt: ResourceType,
    }

    let messaging: Wrapper = toml::from_str(r#"rt = "messaging""#).unwrap();
    assert_eq!(messaging.rt, ResourceType::Messaging);
}

// ── ConfigValue ─────────────────────────────────────────────────────────────

#[test]
fn config_value_literal_from_toml() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        host: ConfigValue,
    }

    let w: Wrapper = toml::from_str(r#"host = "localhost""#).unwrap();
    assert!(matches!(w.host, ConfigValue::Literal(ref s) if s == "localhost"));
}

#[test]
fn config_value_secret_ref_from_toml() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        password: ConfigValue,
    }

    let w: Wrapper = toml::from_str(r#"password = { secret_ref = "/prod/db/password" }"#).unwrap();
    match &w.password {
        ConfigValue::SecretRef { secret_ref } => {
            assert_eq!(secret_ref, "/prod/db/password");
        }
        other => panic!("expected SecretRef, got {other:?}"),
    }
}

#[test]
fn config_value_env_ref_from_toml() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        api_key: ConfigValue,
    }

    let w: Wrapper = toml::from_str(r#"api_key = { env = "MY_API_KEY" }"#).unwrap();
    match &w.api_key {
        ConfigValue::EnvRef { env } => {
            assert_eq!(env, "MY_API_KEY");
        }
        other => panic!("expected EnvRef, got {other:?}"),
    }
}

#[tokio::test]
async fn config_value_resolve_literal() {
    let secrets = InMemorySecretsProvider::new(HashMap::new());
    let val = ConfigValue::Literal("localhost".to_string());
    let resolved = val.resolve(&secrets).await.unwrap();
    assert_eq!(resolved, "localhost");
}

#[tokio::test]
async fn config_value_resolve_secret_ref() {
    let mut map = HashMap::new();
    map.insert("/prod/db/password".to_string(), "s3cret!".to_string());
    let secrets = InMemorySecretsProvider::new(map);

    let val = ConfigValue::SecretRef {
        secret_ref: "/prod/db/password".to_string(),
    };
    let resolved = val.resolve(&secrets).await.unwrap();
    assert_eq!(resolved, "s3cret!");
}

#[tokio::test]
async fn config_value_resolve_env_ref() {
    // Use a unique env var name to avoid collisions
    let var_name = "TASKER_SECURE_TEST_ENV_REF_42";
    // SAFETY: This test runs with a unique env var name to avoid collisions
    // with other tests. The var is removed at the end of the test.
    unsafe {
        std::env::set_var(var_name, "env-value-42");
    }

    let secrets = InMemorySecretsProvider::new(HashMap::new());
    let val = ConfigValue::EnvRef {
        env: var_name.to_string(),
    };
    let resolved = val.resolve(&secrets).await.unwrap();
    assert_eq!(resolved, "env-value-42");

    // SAFETY: Cleaning up the env var set earlier in this test.
    unsafe {
        std::env::remove_var(var_name);
    }
}

#[tokio::test]
async fn config_value_resolve_secret_ref_not_found() {
    let secrets = InMemorySecretsProvider::new(HashMap::new());
    let val = ConfigValue::SecretRef {
        secret_ref: "/missing/secret".to_string(),
    };
    let err = val.resolve(&secrets).await.unwrap_err();
    assert!(
        matches!(err, tasker_secure::SecretsError::NotFound { .. }),
        "expected NotFound, got {err:?}"
    );
}

// ── ResourceConfig ──────────────────────────────────────────────────────────

#[tokio::test]
async fn resource_config_get_and_require() {
    let mut values = HashMap::new();
    values.insert(
        "host".to_string(),
        ConfigValue::Literal("localhost".to_string()),
    );
    let config = ResourceConfig::new(values);

    let secrets = InMemorySecretsProvider::new(HashMap::new());

    // resolve_value for present key succeeds
    let host = config.resolve_value("host", &secrets).await.unwrap();
    assert_eq!(host, "localhost");

    // resolve_value for missing key returns MissingConfigKey error
    let err = config.resolve_value("port", &secrets).await.unwrap_err();
    assert!(
        matches!(err, ResourceError::MissingConfigKey { .. }),
        "expected MissingConfigKey, got {err:?}"
    );
}

#[tokio::test]
async fn resource_config_resolve_optional() {
    let config = ResourceConfig::default();
    let secrets = InMemorySecretsProvider::new(HashMap::new());

    // Missing key returns Ok(None)
    let result = config.resolve_optional("missing", &secrets).await.unwrap();
    assert!(result.is_none());
}

// ── ResourceDefinition & ResourceSummary ────────────────────────────────────

#[test]
fn resource_definition_deserialize_from_toml() {
    let toml_str = r#"
        name = "primary_db"
        resource_type = "postgres"

        [config]
        host = "localhost"
        port = "5432"
        password = { secret_ref = "/prod/db/password" }
    "#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.name, "primary_db");
    assert_eq!(def.resource_type, ResourceType::Postgres);
    assert!(def.config.get("host").is_some());
    assert!(def.config.get("password").is_some());
    assert!(def.secrets_provider.is_none());
}

#[test]
fn resource_summary_fields() {
    let summary = ResourceSummary {
        name: "db".to_string(),
        resource_type: ResourceType::Postgres,
        healthy: true,
    };
    assert_eq!(summary.name, "db");
    assert!(summary.healthy);
    // Debug must be implemented
    let _ = format!("{summary:?}");
}

// ── ResourceError ───────────────────────────────────────────────────────────

#[test]
fn resource_error_display() {
    let err = ResourceError::ResourceNotFound {
        name: "db".to_string(),
    };
    let msg = format!("{err}");
    assert!(msg.contains("db"), "error message should contain name");

    let err = ResourceError::WrongResourceType {
        name: "conn".to_string(),
        expected: "postgres".to_string(),
        actual: "http".to_string(),
    };
    let msg = format!("{err}");
    assert!(msg.contains("postgres"));
    assert!(msg.contains("http"));
}
