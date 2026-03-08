use std::collections::HashMap;
use std::sync::Arc;

use tasker_secure::config::ConfigString;
use tasker_secure::testing::InMemorySecretsProvider;
use tasker_secure::SecretsProvider;

fn test_provider() -> Arc<dyn SecretsProvider> {
    let mut secrets = HashMap::new();
    secrets.insert(
        "/production/tasker/database/url".to_string(),
        "postgresql://prod:secret@db.example.com/tasker".to_string(),
    );
    secrets.insert(
        "redis/url".to_string(),
        "redis://cache.example.com:6379".to_string(),
    );
    Arc::new(InMemorySecretsProvider::new(secrets))
}

#[tokio::test]
async fn literal_resolves_to_itself() {
    let config = ConfigString::Literal("hello".to_string());
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await.unwrap();
    assert_eq!(result, "hello");
}

#[tokio::test]
async fn secret_ref_resolves_through_provider() {
    let config = ConfigString::SecretRef {
        path: "/production/tasker/database/url".to_string(),
        provider: None,
    };
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await.unwrap();
    assert_eq!(result, "postgresql://prod:secret@db.example.com/tasker");
}

#[tokio::test]
async fn env_ref_resolves_from_env() {
    std::env::set_var("TEST_CONFIG_STRING_VAR", "env-value");
    let config = ConfigString::EnvRef {
        var: "TEST_CONFIG_STRING_VAR".to_string(),
        default: None,
    };
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await.unwrap();
    assert_eq!(result, "env-value");
    std::env::remove_var("TEST_CONFIG_STRING_VAR");
}

#[tokio::test]
async fn env_ref_uses_default_when_unset() {
    let config = ConfigString::EnvRef {
        var: "DEFINITELY_NOT_SET_CONFIG_STRING".to_string(),
        default: Some("fallback-value".to_string()),
    };
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await.unwrap();
    assert_eq!(result, "fallback-value");
}

#[tokio::test]
async fn env_ref_fails_without_default_when_unset() {
    let config = ConfigString::EnvRef {
        var: "DEFINITELY_NOT_SET_CONFIG_STRING_2".to_string(),
        default: None,
    };
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn secret_ref_fails_when_not_found() {
    let config = ConfigString::SecretRef {
        path: "nonexistent/path".to_string(),
        provider: None,
    };
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await;
    assert!(result.is_err());
}

#[test]
fn deserialize_literal_string() {
    let toml_str = r#"value = "postgresql://localhost/tasker""#;
    let parsed: TestConfig = toml::from_str(toml_str).unwrap();
    assert!(matches!(parsed.value, ConfigString::Literal(_)));
}

#[test]
fn deserialize_secret_ref() {
    let toml_str = r#"
[value]
secret_ref = "/production/tasker/database/url"
"#;
    let parsed: TestConfig = toml::from_str(toml_str).unwrap();
    assert!(matches!(parsed.value, ConfigString::SecretRef { .. }));
}

#[test]
fn deserialize_secret_ref_with_provider() {
    let toml_str = r#"
[value]
secret_ref = "prod/tasker/db-url"
provider = "vault"
"#;
    let parsed: TestConfig = toml::from_str(toml_str).unwrap();
    match &parsed.value {
        ConfigString::SecretRef { path, provider } => {
            assert_eq!(path, "prod/tasker/db-url");
            assert_eq!(provider.as_deref(), Some("vault"));
        }
        _ => panic!("expected SecretRef"),
    }
}

#[derive(serde::Deserialize)]
struct TestConfig {
    value: ConfigString,
}
