use std::collections::HashMap;
use std::sync::Arc;

use tasker_secure::testing::InMemorySecretsProvider;
use tasker_secure::{ChainedSecretsProvider, SecretsError, SecretsProvider};

fn provider_a() -> Arc<dyn SecretsProvider> {
    let mut secrets = HashMap::new();
    secrets.insert("db/password".to_string(), "from-provider-a".to_string());
    Arc::new(InMemorySecretsProvider::with_name("provider-a", secrets))
}

fn provider_b() -> Arc<dyn SecretsProvider> {
    let mut secrets = HashMap::new();
    secrets.insert("db/password".to_string(), "from-provider-b".to_string());
    secrets.insert("api/key".to_string(), "api-key-from-b".to_string());
    Arc::new(InMemorySecretsProvider::with_name("provider-b", secrets))
}

#[tokio::test]
async fn first_provider_wins() {
    let chain = ChainedSecretsProvider::new(vec![provider_a(), provider_b()]);
    let result = chain.get_secret("db/password").await.unwrap();
    assert_eq!(result.expose_secret(), "from-provider-a");
}

#[tokio::test]
async fn falls_back_to_second_provider() {
    let chain = ChainedSecretsProvider::new(vec![provider_a(), provider_b()]);
    let result = chain.get_secret("api/key").await.unwrap();
    assert_eq!(result.expose_secret(), "api-key-from-b");
}

#[tokio::test]
async fn all_fail_returns_last_error() {
    let chain = ChainedSecretsProvider::new(vec![provider_a(), provider_b()]);
    let result = chain.get_secret("nonexistent").await;
    assert!(matches!(result, Err(SecretsError::NotFound { .. })));
}

#[tokio::test]
async fn empty_chain_returns_error() {
    let chain = ChainedSecretsProvider::new(vec![]);
    let result = chain.get_secret("anything").await;
    assert!(matches!(
        result,
        Err(SecretsError::ProviderUnavailable { .. })
    ));
}

#[tokio::test]
async fn health_check_passes_when_any_provider_healthy() {
    let chain = ChainedSecretsProvider::new(vec![provider_a(), provider_b()]);
    assert!(chain.health_check().await.is_ok());
}

#[tokio::test]
async fn provider_name_lists_chain() {
    let chain = ChainedSecretsProvider::new(vec![provider_a(), provider_b()]);
    let name = chain.provider_name();
    assert!(name.contains("provider-a"));
    assert!(name.contains("provider-b"));
}
