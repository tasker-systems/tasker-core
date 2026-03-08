use tasker_secure::{EnvSecretsProvider, SecretsError, SecretsProvider};

#[tokio::test]
async fn resolves_env_var_without_prefix() {
    std::env::set_var("TEST_SECRET_NO_PREFIX", "my-secret-value");
    let provider = EnvSecretsProvider::new(None);
    let result = provider.get_secret("TEST_SECRET_NO_PREFIX").await.unwrap();
    assert_eq!(result.expose_secret(), "my-secret-value");
    std::env::remove_var("TEST_SECRET_NO_PREFIX");
}

#[tokio::test]
async fn resolves_env_var_with_prefix() {
    std::env::set_var("TASKER_SECRET_DB_PASSWORD", "s3cret");
    let provider = EnvSecretsProvider::with_prefix("TASKER_SECRET_");
    let result = provider.get_secret("DB_PASSWORD").await.unwrap();
    assert_eq!(result.expose_secret(), "s3cret");
    std::env::remove_var("TASKER_SECRET_DB_PASSWORD");
}

#[tokio::test]
async fn normalizes_path_separators() {
    std::env::set_var("TASKER_SECRET_ORDERS_DB_PASSWORD", "normalized");
    let provider = EnvSecretsProvider::with_prefix("TASKER_SECRET_");
    let result = provider.get_secret("orders-db/password").await.unwrap();
    assert_eq!(result.expose_secret(), "normalized");
    std::env::remove_var("TASKER_SECRET_ORDERS_DB_PASSWORD");
}

#[tokio::test]
async fn missing_var_returns_not_found() {
    let provider = EnvSecretsProvider::new(None);
    let result = provider.get_secret("DEFINITELY_NOT_SET_12345").await;
    assert!(matches!(result, Err(SecretsError::NotFound { .. })));
}

#[tokio::test]
async fn health_check_always_ok() {
    let provider = EnvSecretsProvider::new(None);
    assert!(provider.health_check().await.is_ok());
}

#[tokio::test]
async fn provider_name_is_env() {
    let provider = EnvSecretsProvider::new(None);
    assert_eq!(provider.provider_name(), "env");
}

#[tokio::test]
async fn get_secrets_batch() {
    std::env::set_var("BATCH_A", "val_a");
    std::env::set_var("BATCH_B", "val_b");
    let provider = EnvSecretsProvider::new(None);
    let results = provider.get_secrets(&["BATCH_A", "BATCH_B"]).await.unwrap();
    assert_eq!(results["BATCH_A"].expose_secret(), "val_a");
    assert_eq!(results["BATCH_B"].expose_secret(), "val_b");
    std::env::remove_var("BATCH_A");
    std::env::remove_var("BATCH_B");
}

#[tokio::test]
async fn normalization_handles_mixed_case_and_hyphens() {
    std::env::set_var("TASKER_SECRET_MY_API_KEY", "key-value");
    let provider = EnvSecretsProvider::with_prefix("TASKER_SECRET_");
    let result = provider.get_secret("my-api-key").await.unwrap();
    assert_eq!(result.expose_secret(), "key-value");
    std::env::remove_var("TASKER_SECRET_MY_API_KEY");
}

#[tokio::test]
async fn resolves_database_url_from_env() {
    let url = "postgresql://tasker:tasker@localhost:5432/tasker_test";
    std::env::set_var("TEST_DATABASE_URL_FOR_SECURE", url);
    let provider = EnvSecretsProvider::new(None);
    let result = provider
        .get_secret("TEST_DATABASE_URL_FOR_SECURE")
        .await
        .unwrap();
    assert_eq!(result.expose_secret(), url);
    std::env::remove_var("TEST_DATABASE_URL_FOR_SECURE");
}
