#[cfg(feature = "postgres")]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use tasker_secure::resource::{ResourceHandleExt, ResourceType};
    use tasker_secure::testing::InMemoryResourceHandle;

    #[test]
    fn postgres_handle_resource_type_downcast() {
        let handle = InMemoryResourceHandle::new("test_db", ResourceType::Postgres);
        // InMemoryResourceHandle is NOT a PostgresHandle, so downcast returns None.
        assert!(
            handle.as_postgres().is_none(),
            "InMemoryResourceHandle should not downcast to PostgresHandle"
        );
    }

    #[test]
    fn postgres_config_parsing() {
        let toml_str = r#"
            host = "db.example.com"
            port = "5433"
            database = "myapp"
            user = "admin"
            max_connections = "20"
            min_connections = "2"

            [password]
            secret_ref = "db/password"
        "#;

        let config: tasker_secure::resource::ResourceConfig =
            toml::from_str(toml_str).expect("should parse resource config from TOML");

        assert!(config.get("host").is_some(), "host key should be present");
        assert!(config.get("port").is_some(), "port key should be present");
        assert!(
            config.get("database").is_some(),
            "database key should be present"
        );
        assert!(config.get("user").is_some(), "user key should be present");
        assert!(
            config.get("password").is_some(),
            "password key should be present"
        );
        assert!(
            config.get("max_connections").is_some(),
            "max_connections key should be present"
        );
        assert!(
            config.get("min_connections").is_some(),
            "min_connections key should be present"
        );
    }

    #[tokio::test]
    async fn postgres_handle_from_config() {
        use tasker_secure::resource::postgres::PostgresHandle;
        use tasker_secure::resource::{ConfigValue, ResourceConfig, ResourceHandle};
        use tasker_secure::testing::InMemorySecretsProvider;

        // If DATABASE_URL is set, try a real connection; otherwise just verify the
        // API compiles and config resolution works.
        let database_url = std::env::var("DATABASE_URL").ok();

        if let Some(url) = database_url {
            // Parse DATABASE_URL to extract components.
            // Format: postgresql://user:password@host:port/database
            let url = url::Url::parse(&url).expect("DATABASE_URL should be a valid URL");
            let host = url.host_str().unwrap_or("localhost").to_string();
            let port = url.port().unwrap_or(5432).to_string();
            let database = url.path().trim_start_matches('/').to_string();
            let user = url.username().to_string();
            let password = url.password().unwrap_or("").to_string();

            let mut values = HashMap::new();
            values.insert("host".to_string(), ConfigValue::Literal(host));
            values.insert("port".to_string(), ConfigValue::Literal(port));
            values.insert("database".to_string(), ConfigValue::Literal(database));
            if !user.is_empty() {
                values.insert("user".to_string(), ConfigValue::Literal(user));
            }
            if !password.is_empty() {
                values.insert("password".to_string(), ConfigValue::Literal(password));
            }

            let config = ResourceConfig::new(values);
            let secrets: Arc<dyn tasker_secure::secrets::SecretsProvider> =
                Arc::new(InMemorySecretsProvider::new(HashMap::new()));

            let handle = PostgresHandle::from_config("test_pg", &config, secrets.as_ref())
                .await
                .expect("should create PostgresHandle from DATABASE_URL components");

            assert_eq!(handle.resource_name(), "test_pg");
            assert_eq!(handle.resource_type(), &ResourceType::Postgres);

            // Verify the pool is functional.
            handle
                .health_check()
                .await
                .expect("health check should pass");
        } else {
            // No DATABASE_URL — verify the API exists and config validation works.
            let mut values = HashMap::new();
            values.insert(
                "host".to_string(),
                ConfigValue::Literal("localhost".to_string()),
            );
            values.insert(
                "database".to_string(),
                ConfigValue::Literal("nonexistent_db".to_string()),
            );

            let config = ResourceConfig::new(values);
            let secrets: Arc<dyn tasker_secure::secrets::SecretsProvider> =
                Arc::new(InMemorySecretsProvider::new(HashMap::new()));

            // Connection will likely fail, but this verifies the API compiles
            // and config resolution runs.
            let result = PostgresHandle::from_config("test_pg", &config, secrets.as_ref()).await;
            // We expect this to either succeed (if there's a local postgres) or
            // fail with InitializationFailed — either way the API works.
            if let Err(e) = &result {
                let err_msg = format!("{e}");
                assert!(
                    err_msg.contains("initialization failed"),
                    "error should be InitializationFailed, got: {err_msg}"
                );
            }
        }
    }
}
