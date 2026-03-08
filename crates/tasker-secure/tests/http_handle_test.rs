#[cfg(feature = "http")]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use tasker_secure::resource::http::{
        ApiKeyAuthStrategy, BearerTokenAuthStrategy, HttpAuthStrategy, HttpHandle,
    };
    use tasker_secure::resource::{
        ConfigValue, ResourceConfig, ResourceHandle, ResourceHandleExt, ResourceType,
    };
    use tasker_secure::testing::{InMemoryResourceHandle, InMemorySecretsProvider};

    fn make_secrets(entries: Vec<(&str, &str)>) -> Arc<dyn tasker_secure::SecretsProvider> {
        let map: HashMap<String, String> = entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Arc::new(InMemorySecretsProvider::new(map))
    }

    fn make_config(entries: Vec<(&str, &str)>) -> ResourceConfig {
        let values: HashMap<String, ConfigValue> = entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), ConfigValue::Literal(v.to_string())))
            .collect();
        ResourceConfig::new(values)
    }

    #[test]
    fn http_handle_resource_type_downcast() {
        let handle = InMemoryResourceHandle::new("test-http", ResourceType::Http);
        // InMemoryResourceHandle is not an HttpHandle, so as_http should return None
        assert!(handle.as_http().is_none());
    }

    #[tokio::test]
    async fn http_handle_from_config_with_api_key() {
        let secrets = make_secrets(vec![]);
        let config = make_config(vec![
            ("base_url", "https://api.example.com"),
            ("auth_type", "api_key"),
            ("auth_header", "X-Custom-Key"),
            ("auth_value", "my-secret-key"),
        ]);

        let handle = HttpHandle::from_config("test-api", &config, secrets.as_ref())
            .await
            .expect("should create handle with api_key auth");

        assert_eq!(handle.resource_name(), "test-api");
        assert_eq!(handle.base_url(), "https://api.example.com");
        assert_eq!(handle.resource_type(), &ResourceType::Http);
    }

    #[tokio::test]
    async fn http_handle_from_config_with_bearer_token() {
        let secrets = make_secrets(vec![]);
        let config = make_config(vec![
            ("base_url", "https://api.example.com"),
            ("auth_type", "bearer"),
            ("auth_value", "my-bearer-token"),
        ]);

        let handle = HttpHandle::from_config("test-bearer", &config, secrets.as_ref())
            .await
            .expect("should create handle with bearer auth");

        assert_eq!(handle.resource_name(), "test-bearer");
        assert_eq!(handle.base_url(), "https://api.example.com");
    }

    #[tokio::test]
    async fn http_handle_from_config_no_auth() {
        let secrets = make_secrets(vec![]);
        let config = make_config(vec![("base_url", "https://api.example.com")]);

        let handle = HttpHandle::from_config("test-noauth", &config, secrets.as_ref())
            .await
            .expect("should create handle without auth");

        assert_eq!(handle.resource_name(), "test-noauth");
        assert_eq!(handle.base_url(), "https://api.example.com");
    }

    #[tokio::test]
    async fn http_handle_missing_base_url() {
        let secrets = make_secrets(vec![]);
        let config = make_config(vec![]);

        let result = HttpHandle::from_config("test-missing", &config, secrets.as_ref()).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("base_url"),
            "error should mention base_url, got: {msg}"
        );
    }

    #[test]
    fn api_key_auth_strategy_apply() {
        let strategy = ApiKeyAuthStrategy::new("X-API-Key", "test-value");
        let client = reqwest::Client::new();
        let builder = client.get("https://example.com");

        // Verify apply does not panic
        let _builder = strategy.apply(builder);
    }

    #[test]
    fn bearer_token_auth_strategy_apply() {
        let strategy = BearerTokenAuthStrategy::new("my-token");
        let client = reqwest::Client::new();
        let builder = client.get("https://example.com");

        // Verify apply does not panic
        let _builder = strategy.apply(builder);
    }

    #[tokio::test]
    async fn http_handle_health_check_without_network() {
        let secrets = make_secrets(vec![]);
        let config = make_config(vec![
            ("base_url", "http://127.0.0.1:1"),
            ("timeout_ms", "100"),
        ]);

        let handle = HttpHandle::from_config("test-health", &config, secrets.as_ref())
            .await
            .expect("should create handle");

        let result = handle.health_check().await;
        assert!(result.is_err(), "health check should fail with no server");

        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("health check failed"),
            "error should mention health check, got: {msg}"
        );
    }
}
