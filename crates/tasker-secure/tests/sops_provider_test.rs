#![cfg(feature = "sops")]

use std::path::PathBuf;
use tasker_secure::{SecretsError, SecretsProvider, SopsSecretsProvider};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Set the ROPS_AGE env var to the test age private key so rops can decrypt.
fn set_test_age_key() {
    // Read the private key from the test fixture
    let key_file = fixtures_dir().join("test-age-key.txt");
    let content = std::fs::read_to_string(&key_file)
        .unwrap_or_else(|_| panic!("missing test fixture: {}", key_file.display()));
    // Extract the AGE-SECRET-KEY line (skip comments)
    let secret_key = content
        .lines()
        .find(|l| l.starts_with("AGE-SECRET-KEY-"))
        .expect("test-age-key.txt must contain an AGE-SECRET-KEY line");
    std::env::set_var("ROPS_AGE", secret_key);
}

#[tokio::test]
async fn decrypts_and_resolves_nested_path() {
    set_test_age_key();
    let provider = SopsSecretsProvider::from_path(fixtures_dir().join("test-secrets.enc.yaml"))
        .await
        .unwrap();

    let result = provider
        .get_secret("database.orders.password")
        .await
        .unwrap();
    assert_eq!(result.expose_secret(), "orders-db-secret");
}

#[tokio::test]
async fn decrypts_and_resolves_deeply_nested_path() {
    set_test_age_key();
    let provider = SopsSecretsProvider::from_path(fixtures_dir().join("test-secrets.enc.yaml"))
        .await
        .unwrap();

    let result = provider.get_secret("api.stripe.secret_key").await.unwrap();
    assert_eq!(result.expose_secret(), "sk_test_12345");
}

#[tokio::test]
async fn missing_path_returns_not_found() {
    set_test_age_key();
    let provider = SopsSecretsProvider::from_path(fixtures_dir().join("test-secrets.enc.yaml"))
        .await
        .unwrap();

    let result = provider.get_secret("database.nonexistent.key").await;
    assert!(matches!(result, Err(SecretsError::NotFound { .. })));
}

#[tokio::test]
async fn path_to_object_returns_invalid_path() {
    set_test_age_key();
    let provider = SopsSecretsProvider::from_path(fixtures_dir().join("test-secrets.enc.yaml"))
        .await
        .unwrap();

    let result = provider.get_secret("database.orders").await;
    assert!(matches!(result, Err(SecretsError::InvalidPath { .. })));
}

#[tokio::test]
async fn provider_name_is_sops() {
    set_test_age_key();
    let provider = SopsSecretsProvider::from_path(fixtures_dir().join("test-secrets.enc.yaml"))
        .await
        .unwrap();

    assert_eq!(provider.provider_name(), "sops");
}

#[tokio::test]
async fn health_check_ok_when_loaded() {
    set_test_age_key();
    let provider = SopsSecretsProvider::from_path(fixtures_dir().join("test-secrets.enc.yaml"))
        .await
        .unwrap();

    assert!(provider.health_check().await.is_ok());
}

#[tokio::test]
async fn nonexistent_file_returns_error() {
    set_test_age_key();
    let result = SopsSecretsProvider::from_path(fixtures_dir().join("nonexistent.yaml")).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn resolves_all_leaf_values() {
    set_test_age_key();
    let provider = SopsSecretsProvider::from_path(fixtures_dir().join("test-secrets.enc.yaml"))
        .await
        .unwrap();

    // Verify every leaf value from the fixture
    assert_eq!(
        provider
            .get_secret("database.orders.username")
            .await
            .unwrap()
            .expose_secret(),
        "orders-user"
    );
    assert_eq!(
        provider
            .get_secret("database.analytics.password")
            .await
            .unwrap()
            .expose_secret(),
        "analytics-db-secret"
    );
    assert_eq!(
        provider
            .get_secret("api.sendgrid.api_key")
            .await
            .unwrap()
            .expose_secret(),
        "SG.test-key"
    );
}
