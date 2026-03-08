#![cfg(feature = "sops")]

use std::path::PathBuf;
use tasker_secure::{SecretsError, SecretsProvider, SopsSecretsProvider};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[tokio::test]
async fn resolves_nested_path_from_yaml() {
    let provider =
        SopsSecretsProvider::from_plaintext_yaml(fixtures_dir().join("test-secrets.yaml"))
            .await
            .unwrap();

    let result = provider
        .get_secret("database.orders.password")
        .await
        .unwrap();
    assert_eq!(result.expose_secret(), "orders-db-secret");
}

#[tokio::test]
async fn resolves_deeply_nested_path() {
    let provider =
        SopsSecretsProvider::from_plaintext_yaml(fixtures_dir().join("test-secrets.yaml"))
            .await
            .unwrap();

    let result = provider.get_secret("api.stripe.secret_key").await.unwrap();
    assert_eq!(result.expose_secret(), "sk_test_12345");
}

#[tokio::test]
async fn missing_path_returns_not_found() {
    let provider =
        SopsSecretsProvider::from_plaintext_yaml(fixtures_dir().join("test-secrets.yaml"))
            .await
            .unwrap();

    let result = provider.get_secret("database.nonexistent.key").await;
    assert!(matches!(result, Err(SecretsError::NotFound { .. })));
}

#[tokio::test]
async fn path_to_object_returns_invalid_path() {
    let provider =
        SopsSecretsProvider::from_plaintext_yaml(fixtures_dir().join("test-secrets.yaml"))
            .await
            .unwrap();

    let result = provider.get_secret("database.orders").await;
    assert!(matches!(result, Err(SecretsError::InvalidPath { .. })));
}

#[tokio::test]
async fn provider_name_is_sops() {
    let provider =
        SopsSecretsProvider::from_plaintext_yaml(fixtures_dir().join("test-secrets.yaml"))
            .await
            .unwrap();

    assert_eq!(provider.provider_name(), "sops");
}

#[tokio::test]
async fn health_check_ok_when_loaded() {
    let provider =
        SopsSecretsProvider::from_plaintext_yaml(fixtures_dir().join("test-secrets.yaml"))
            .await
            .unwrap();

    assert!(provider.health_check().await.is_ok());
}

#[tokio::test]
async fn nonexistent_file_returns_error() {
    let result =
        SopsSecretsProvider::from_plaintext_yaml(fixtures_dir().join("nonexistent.yaml")).await;

    assert!(result.is_err());
}
