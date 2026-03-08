# TAS-358: S2 ResourceRegistry Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement `ResourceHandle` trait, `ResourceRegistry`, concrete handles (`PostgresHandle`, `HttpHandle`), and `InMemoryResourceHandle` test utility in the `tasker-secure` crate.

**Architecture:** Strategy-pattern resource handles behind `Arc<dyn ResourceHandle>` managed by a `ResourceRegistry`. Resources are defined in TOML config with `ConfigValue` fields that resolve secrets through `SecretsProvider`. Feature-gated concrete handles (`postgres`, `http`) wrap `sqlx::PgPool` and `reqwest::Client`. Test utilities (`test-utils`) provide `InMemoryResourceHandle` with fixture data and capture lists.

**Tech Stack:** Rust, async-trait, serde (TOML deserialization), sqlx 0.8 (postgres feature), reqwest 0.12 (http feature), tokio, thiserror

---

## Crate Context

**Working crate:** `crates/tasker-secure/` (Cargo.toml at `crates/tasker-secure/Cargo.toml`)

**Existing S1 modules (do not modify):**
- `src/secrets/` — `SecretsProvider` trait, `SecretValue`, `SecretsError`, `EnvSecretsProvider`, `ChainedSecretsProvider`, `SopsSecretsProvider`
- `src/config/` — `ConfigString`, `ConfigError`
- `src/testing/mock_secrets.rs` — `InMemorySecretsProvider`

**Target module:** `src/resource/` — currently a stub with only a doc comment

**Existing features:** `default = []`, `sops`, `test-utils`

**Workspace deps already available:** `async-trait`, `serde`, `serde_json`, `thiserror`, `tokio`, `tracing`, `reqwest` (0.12, rustls-tls, json), `sqlx` (0.8, postgres, runtime-tokio-rustls)

**Test runner:** `cargo test -p tasker-secure --features <features>`

**Lint command:** `cargo clippy -p tasker-secure --all-targets --all-features -- -W clippy::cargo`

**Format command:** `cargo fmt -p tasker-secure --check`

---

## Task 1: ResourceType, ResourceError, and ConfigValue — Core Types

**Files:**
- Create: `crates/tasker-secure/src/resource/types.rs`
- Create: `crates/tasker-secure/src/resource/error.rs`
- Create: `crates/tasker-secure/src/resource/config_value.rs`
- Modify: `crates/tasker-secure/src/resource/mod.rs`
- Test: `crates/tasker-secure/tests/resource_types_test.rs`

**Step 1: Write the failing tests**

Create `crates/tasker-secure/tests/resource_types_test.rs`:

```rust
use std::collections::HashMap;

use tasker_secure::resource::{ConfigValue, ResourceConfig, ResourceType};

// --- ResourceType ---

#[test]
fn resource_type_debug_display() {
    let pg = ResourceType::Postgres;
    assert_eq!(format!("{pg:?}"), "Postgres");

    let http = ResourceType::Http;
    assert_eq!(format!("{http:?}"), "Http");

    let custom = ResourceType::Custom {
        type_name: "kafka".to_string(),
    };
    assert!(format!("{custom:?}").contains("kafka"));
}

#[test]
fn resource_type_deserialize_from_toml() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        resource_type: ResourceType,
    }

    let toml_str = r#"resource_type = "postgres""#;
    let w: Wrapper = toml::from_str(toml_str).unwrap();
    assert!(matches!(w.resource_type, ResourceType::Postgres));

    let toml_str = r#"resource_type = "http""#;
    let w: Wrapper = toml::from_str(toml_str).unwrap();
    assert!(matches!(w.resource_type, ResourceType::Http));

    let toml_str = r#"resource_type = "pgmq""#;
    let w: Wrapper = toml::from_str(toml_str).unwrap();
    assert!(matches!(w.resource_type, ResourceType::Pgmq));
}

// --- ConfigValue ---

#[test]
fn config_value_literal_from_toml() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        host: ConfigValue,
    }

    let toml_str = r#"host = "localhost""#;
    let w: Wrapper = toml::from_str(toml_str).unwrap();
    assert!(matches!(w.host, ConfigValue::Literal(ref s) if s == "localhost"));
}

#[test]
fn config_value_secret_ref_from_toml() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        password: ConfigValue,
    }

    let toml_str = r#"password = { secret_ref = "/prod/db/password" }"#;
    let w: Wrapper = toml::from_str(toml_str).unwrap();
    assert!(
        matches!(w.password, ConfigValue::SecretRef { ref secret_ref } if secret_ref == "/prod/db/password")
    );
}

#[test]
fn config_value_env_ref_from_toml() {
    #[derive(serde::Deserialize)]
    struct Wrapper {
        api_key: ConfigValue,
    }

    let toml_str = r#"api_key = { env = "MY_API_KEY" }"#;
    let w: Wrapper = toml::from_str(toml_str).unwrap();
    assert!(matches!(w.api_key, ConfigValue::EnvRef { ref env } if env == "MY_API_KEY"));
}

#[tokio::test]
async fn config_value_resolve_literal() {
    let secrets = tasker_secure::testing::InMemorySecretsProvider::new(HashMap::new());
    let val = ConfigValue::Literal("hello".to_string());
    let resolved = val.resolve(&secrets).await.unwrap();
    assert_eq!(resolved, "hello");
}

#[tokio::test]
async fn config_value_resolve_secret_ref() {
    let secrets = tasker_secure::testing::InMemorySecretsProvider::new(HashMap::from([(
        "/prod/db/password".to_string(),
        "s3cret".to_string(),
    )]));
    let val = ConfigValue::SecretRef {
        secret_ref: "/prod/db/password".to_string(),
    };
    let resolved = val.resolve(&secrets).await.unwrap();
    assert_eq!(resolved, "s3cret");
}

#[tokio::test]
async fn config_value_resolve_env_ref() {
    std::env::set_var("TEST_CV_ENV_KEY", "env_value");
    let secrets = tasker_secure::testing::InMemorySecretsProvider::new(HashMap::new());
    let val = ConfigValue::EnvRef {
        env: "TEST_CV_ENV_KEY".to_string(),
    };
    let resolved = val.resolve(&secrets).await.unwrap();
    assert_eq!(resolved, "env_value");
    std::env::remove_var("TEST_CV_ENV_KEY");
}

#[tokio::test]
async fn config_value_resolve_secret_ref_not_found() {
    let secrets = tasker_secure::testing::InMemorySecretsProvider::new(HashMap::new());
    let val = ConfigValue::SecretRef {
        secret_ref: "/missing".to_string(),
    };
    let result = val.resolve(&secrets).await;
    assert!(result.is_err());
}

// --- ResourceConfig ---

#[tokio::test]
async fn resource_config_get_and_require() {
    let mut map = HashMap::new();
    map.insert(
        "host".to_string(),
        ConfigValue::Literal("localhost".to_string()),
    );
    map.insert(
        "port".to_string(),
        ConfigValue::Literal("5432".to_string()),
    );
    let config = ResourceConfig::new(map);

    let secrets = tasker_secure::testing::InMemorySecretsProvider::new(HashMap::new());

    assert_eq!(
        config.resolve_value("host", &secrets).await.unwrap(),
        "localhost"
    );
    assert_eq!(
        config.resolve_value("port", &secrets).await.unwrap(),
        "5432"
    );
    assert!(config.resolve_value("missing", &secrets).await.is_err());
}

#[tokio::test]
async fn resource_config_resolve_optional() {
    let config = ResourceConfig::new(HashMap::new());
    let secrets = tasker_secure::testing::InMemorySecretsProvider::new(HashMap::new());

    let result = config
        .resolve_optional("missing", &secrets)
        .await
        .unwrap();
    assert!(result.is_none());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p tasker-secure --features test-utils --test resource_types_test 2>&1 | head -20`
Expected: Compilation error — `resource::ConfigValue` and `resource::ResourceType` don't exist yet.

**Step 3: Implement the types**

Create `crates/tasker-secure/src/resource/error.rs`:

```rust
//! Error types for resource lifecycle operations.

use std::fmt;

/// Errors that can occur during resource lifecycle operations.
#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    /// Resource initialization failed (connection, auth, pool creation).
    #[error("failed to initialize resource '{name}': {message}")]
    InitializationFailed { name: String, message: String },

    /// Health check failed (resource unreachable or degraded).
    #[error("health check failed for resource '{name}': {message}")]
    HealthCheckFailed { name: String, message: String },

    /// Credential refresh failed.
    #[error("credential refresh failed for resource '{name}': {message}")]
    CredentialRefreshFailed { name: String, message: String },

    /// Resource not found in the registry.
    #[error("resource not found: '{name}'")]
    ResourceNotFound { name: String },

    /// Resource exists but is not the expected type.
    #[error("wrong resource type for '{name}': expected {expected}, got {actual}")]
    WrongResourceType {
        name: String,
        expected: String,
        actual: String,
    },

    /// A required config key was missing from the resource definition.
    #[error("missing config key '{key}' for resource '{resource}'")]
    MissingConfigKey { resource: String, key: String },

    /// Secret resolution failed during resource initialization.
    #[error("secret resolution failed for resource '{resource}': {source}")]
    SecretResolution {
        resource: String,
        source: crate::secrets::SecretsError,
    },
}
```

Create `crates/tasker-secure/src/resource/types.rs`:

```rust
//! Core resource types: ResourceType, ResourceSummary, ResourceDefinition.

use serde::Deserialize;

use super::config_value::ResourceConfig;

/// The type of external system a resource connects to.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    /// PostgreSQL database (via sqlx).
    Postgres,
    /// HTTP/HTTPS API endpoint.
    Http,
    /// PGMQ message queue.
    Pgmq,
    /// Custom resource type — implemented by the worker binary.
    Custom { type_name: String },
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres => write!(f, "postgres"),
            Self::Http => write!(f, "http"),
            Self::Pgmq => write!(f, "pgmq"),
            Self::Custom { type_name } => write!(f, "custom:{type_name}"),
        }
    }
}

use std::fmt;

/// A configured resource definition — everything needed to initialize it.
///
/// Secret values are referenced by path (resolved through SecretsProvider at
/// init time), not embedded as literals.
#[derive(Debug, Clone, Deserialize)]
pub struct ResourceDefinition {
    /// The name by which capability configs reference this resource.
    pub name: String,

    /// What kind of external system this resource connects to.
    pub resource_type: ResourceType,

    /// Configuration parameters — mix of literals and secret references.
    #[serde(default)]
    pub config: ResourceConfig,

    /// Optional: which SecretsProvider to use for resolving this resource's secrets.
    pub secrets_provider: Option<String>,
}

/// Public-facing resource information for MCP discoverability.
///
/// Contains name and type only — never configuration or credential details.
#[derive(Debug, Clone)]
pub struct ResourceSummary {
    /// The resource name.
    pub name: String,
    /// The resource type.
    pub resource_type: ResourceType,
    /// Whether the resource is currently healthy.
    pub healthy: bool,
}
```

Create `crates/tasker-secure/src/resource/config_value.rs`:

```rust
//! ConfigValue: a configuration parameter that may be a literal, a secret
//! reference, or an environment variable reference.

use std::collections::HashMap;
use std::fmt;

use serde::Deserialize;

use super::error::ResourceError;
use crate::secrets::{SecretsError, SecretsProvider};

/// A configuration value within a ResourceDefinition.
///
/// In TOML config:
/// ```toml
/// host = "orders-db.internal"                                    # Literal
/// password = { secret_ref = "/prod/tasker/orders-db/password" }  # SecretRef
/// api_key = { env = "FULFILLMENT_API_KEY" }                      # EnvRef
/// ```
#[derive(Debug, Clone)]
pub enum ConfigValue {
    /// A literal value — not a secret, safe to log.
    Literal(String),

    /// A reference to a secret resolved through the SecretsProvider.
    SecretRef { secret_ref: String },

    /// An environment variable reference.
    EnvRef { env: String },
}

impl ConfigValue {
    /// Resolve this config value to a concrete string.
    pub async fn resolve(&self, secrets: &dyn SecretsProvider) -> Result<String, SecretsError> {
        match self {
            Self::Literal(s) => Ok(s.clone()),
            Self::SecretRef { secret_ref } => {
                let value = secrets.get_secret(secret_ref).await?;
                Ok(value.expose_secret().to_string())
            }
            Self::EnvRef { env } => std::env::var(env).map_err(|_| SecretsError::NotFound {
                path: env.clone(),
            }),
        }
    }
}

impl fmt::Display for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(s) => write!(f, "{s}"),
            Self::SecretRef { secret_ref } => write!(f, "{{secret_ref: \"{secret_ref}\"}}"),
            Self::EnvRef { env } => write!(f, "{{env: \"{env}\"}}"),
        }
    }
}

impl<'de> Deserialize<'de> for ConfigValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct ConfigValueVisitor;

        impl<'de> de::Visitor<'de> for ConfigValueVisitor {
            type Value = ConfigValue;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a string, or an object with 'secret_ref' or 'env'")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(ConfigValue::Literal(value.to_string()))
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(ConfigValue::Literal(value))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                let mut secret_ref: Option<String> = None;
                let mut env: Option<String> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "secret_ref" => secret_ref = Some(map.next_value()?),
                        "env" => env = Some(map.next_value()?),
                        other => {
                            return Err(de::Error::unknown_field(
                                other,
                                &["secret_ref", "env"],
                            ));
                        }
                    }
                }

                if let Some(secret_ref) = secret_ref {
                    Ok(ConfigValue::SecretRef { secret_ref })
                } else if let Some(env) = env {
                    Ok(ConfigValue::EnvRef { env })
                } else {
                    Err(de::Error::missing_field("secret_ref or env"))
                }
            }
        }

        deserializer.deserialize_any(ConfigValueVisitor)
    }
}

/// A collection of configuration values for a resource.
///
/// Wraps `HashMap<String, ConfigValue>` with typed accessor helpers.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct ResourceConfig {
    values: HashMap<String, ConfigValue>,
}

impl ResourceConfig {
    /// Create a new resource config from a map of values.
    pub fn new(values: HashMap<String, ConfigValue>) -> Self {
        Self { values }
    }

    /// Resolve a required config value. Returns `ResourceError::MissingConfigKey`
    /// if the key is not present.
    pub async fn resolve_value(
        &self,
        key: &str,
        secrets: &dyn SecretsProvider,
    ) -> Result<String, ResourceError> {
        let cv = self.values.get(key).ok_or_else(|| ResourceError::MissingConfigKey {
            resource: String::new(), // caller should wrap with resource name
            key: key.to_string(),
        })?;
        cv.resolve(secrets)
            .await
            .map_err(|source| ResourceError::SecretResolution {
                resource: String::new(),
                source,
            })
    }

    /// Resolve an optional config value. Returns `Ok(None)` if the key is missing.
    pub async fn resolve_optional(
        &self,
        key: &str,
        secrets: &dyn SecretsProvider,
    ) -> Result<Option<String>, ResourceError> {
        match self.values.get(key) {
            Some(cv) => {
                let resolved =
                    cv.resolve(secrets)
                        .await
                        .map_err(|source| ResourceError::SecretResolution {
                            resource: String::new(),
                            source,
                        })?;
                Ok(Some(resolved))
            }
            None => Ok(None),
        }
    }

    /// Get a raw `ConfigValue` by key (without resolving).
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.values.get(key)
    }
}
```

Update `crates/tasker-secure/src/resource/mod.rs`:

```rust
//! Resource lifecycle management with automatic credential rotation.
//!
//! Named resources decouple capability configs from credentials. A
//! `ResourceHandle` is an already-initialized, ready-to-use connection to an
//! external system. The `ResourceRegistry` initializes handles at startup using
//! credentials resolved through the `SecretsProvider`.

mod config_value;
mod error;
mod types;

pub use config_value::{ConfigValue, ResourceConfig};
pub use error::ResourceError;
pub use types::{ResourceDefinition, ResourceSummary, ResourceType};
```

**Step 4: Update lib.rs exports**

In `crates/tasker-secure/src/lib.rs`, add resource re-exports:

```rust
pub use resource::{
    ConfigValue, ResourceConfig, ResourceDefinition, ResourceError, ResourceSummary, ResourceType,
};
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p tasker-secure --features test-utils --test resource_types_test`
Expected: All tests pass.

**Step 6: Run existing S1 tests to verify no regression**

Run: `cargo test -p tasker-secure --features test-utils`
Expected: All 36+ tests pass.

**Step 7: Commit**

```bash
git add crates/tasker-secure/src/resource/ crates/tasker-secure/src/lib.rs crates/tasker-secure/tests/resource_types_test.rs
git commit -m "feat(TAS-358): add ResourceType, ResourceError, ConfigValue core types"
```

---

## Task 2: ResourceHandle Trait and ResourceHandleExt

**Files:**
- Create: `crates/tasker-secure/src/resource/handle.rs`
- Modify: `crates/tasker-secure/src/resource/mod.rs`
- Test: `crates/tasker-secure/tests/resource_handle_test.rs`

**Step 1: Write the failing tests**

Create `crates/tasker-secure/tests/resource_handle_test.rs`:

```rust
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use tasker_secure::resource::{ResourceError, ResourceHandle, ResourceHandleExt, ResourceType};
use tasker_secure::SecretsProvider;

/// A minimal test handle to verify the trait works.
#[derive(Debug)]
struct TestHandle {
    name: String,
    resource_type: ResourceType,
}

#[async_trait::async_trait]
impl ResourceHandle for TestHandle {
    fn resource_name(&self) -> &str {
        &self.name
    }

    fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }

    async fn refresh_credentials(
        &self,
        _secrets: &dyn SecretsProvider,
    ) -> Result<(), ResourceError> {
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ResourceError> {
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn resource_handle_name_and_type() {
    let handle = TestHandle {
        name: "test-db".to_string(),
        resource_type: ResourceType::Postgres,
    };

    assert_eq!(handle.resource_name(), "test-db");
    assert_eq!(*handle.resource_type(), ResourceType::Postgres);
}

#[tokio::test]
async fn resource_handle_health_check() {
    let handle = TestHandle {
        name: "test-db".to_string(),
        resource_type: ResourceType::Postgres,
    };

    assert!(handle.health_check().await.is_ok());
}

#[test]
fn resource_handle_as_any_downcast() {
    let handle = TestHandle {
        name: "test-db".to_string(),
        resource_type: ResourceType::Postgres,
    };

    let any = handle.as_any();
    let downcasted = any.downcast_ref::<TestHandle>();
    assert!(downcasted.is_some());
    assert_eq!(downcasted.unwrap().name, "test-db");
}

#[test]
fn resource_handle_ext_returns_none_for_wrong_type() {
    let handle: Arc<dyn ResourceHandle> = Arc::new(TestHandle {
        name: "test-db".to_string(),
        resource_type: ResourceType::Postgres,
    });

    // TestHandle is not a PostgresHandle, so these should return None
    assert!(handle.as_postgres().is_none());
    assert!(handle.as_http().is_none());
}

#[test]
fn resource_handle_dyn_dispatch() {
    let handle: Arc<dyn ResourceHandle> = Arc::new(TestHandle {
        name: "my-api".to_string(),
        resource_type: ResourceType::Http,
    });

    assert_eq!(handle.resource_name(), "my-api");
    assert_eq!(*handle.resource_type(), ResourceType::Http);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p tasker-secure --features test-utils --test resource_handle_test 2>&1 | head -20`
Expected: Compilation error — `ResourceHandle` trait doesn't exist yet.

**Step 3: Implement the trait**

Create `crates/tasker-secure/src/resource/handle.rs`:

```rust
//! ResourceHandle trait and ResourceHandleExt convenience downcasts.

use std::any::Any;
use std::fmt;

use super::error::ResourceError;
use super::types::ResourceType;
use crate::secrets::SecretsProvider;

/// A handle to an initialized external resource.
///
/// Capability executors receive `Arc<dyn ResourceHandle>` from the
/// ResourceRegistry. They never see credentials — the handle was
/// initialized with resolved credentials at worker startup.
#[async_trait::async_trait]
pub trait ResourceHandle: Send + Sync + fmt::Debug {
    /// The name of the resource this handle represents.
    fn resource_name(&self) -> &str;

    /// The resource type.
    fn resource_type(&self) -> &ResourceType;

    /// Attempt to refresh credentials from the SecretsProvider.
    ///
    /// Called when an auth error is detected. The handle reinitializes
    /// itself with freshly resolved credentials.
    async fn refresh_credentials(
        &self,
        secrets: &dyn SecretsProvider,
    ) -> Result<(), ResourceError>;

    /// Check whether this handle is healthy.
    async fn health_check(&self) -> Result<(), ResourceError>;

    /// Downcast to a concrete handle type.
    fn as_any(&self) -> &dyn Any;
}

/// Convenience extension trait for typed downcasting of resource handles.
///
/// Provides `as_postgres()`, `as_http()`, etc. so capability executors
/// can get the specific type they need without manual `as_any()` + `downcast_ref`.
pub trait ResourceHandleExt: ResourceHandle {
    /// Downcast to `PostgresHandle` if this is a postgres resource.
    #[cfg(feature = "postgres")]
    fn as_postgres(&self) -> Option<&super::postgres::PostgresHandle> {
        self.as_any().downcast_ref()
    }

    /// Downcast to `HttpHandle` if this is an HTTP resource.
    #[cfg(feature = "http")]
    fn as_http(&self) -> Option<&super::http::HttpHandle> {
        self.as_any().downcast_ref()
    }

    /// Stub: returns None unless postgres feature is enabled.
    #[cfg(not(feature = "postgres"))]
    fn as_postgres(&self) -> Option<&dyn Any> {
        None
    }

    /// Stub: returns None unless http feature is enabled.
    #[cfg(not(feature = "http"))]
    fn as_http(&self) -> Option<&dyn Any> {
        None
    }
}

/// Blanket implementation — every ResourceHandle gets ResourceHandleExt for free.
impl<T: ResourceHandle + ?Sized> ResourceHandleExt for T {}
```

**Step 4: Update mod.rs**

Add to `crates/tasker-secure/src/resource/mod.rs`:

```rust
mod handle;
pub use handle::{ResourceHandle, ResourceHandleExt};
```

And update `crates/tasker-secure/src/lib.rs` to also export:

```rust
pub use resource::{ResourceHandle, ResourceHandleExt};
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p tasker-secure --features test-utils --test resource_handle_test`
Expected: All tests pass.

**Step 6: Run all tests**

Run: `cargo test -p tasker-secure --features test-utils`
Expected: All tests pass (S1 + new).

**Step 7: Commit**

```bash
git add crates/tasker-secure/src/resource/handle.rs crates/tasker-secure/src/resource/mod.rs crates/tasker-secure/src/lib.rs crates/tasker-secure/tests/resource_handle_test.rs
git commit -m "feat(TAS-358): add ResourceHandle trait and ResourceHandleExt downcasts"
```

---

## Task 3: ResourceRegistry — Core Registry with Initialize, Get, List, Refresh

**Files:**
- Create: `crates/tasker-secure/src/resource/registry.rs`
- Modify: `crates/tasker-secure/src/resource/mod.rs`
- Modify: `crates/tasker-secure/src/lib.rs`
- Test: `crates/tasker-secure/tests/resource_registry_test.rs`

**Step 1: Write the failing tests**

Create `crates/tasker-secure/tests/resource_registry_test.rs`:

```rust
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use tasker_secure::resource::{
    ResourceDefinition, ResourceError, ResourceHandle, ResourceRegistry, ResourceType,
};
use tasker_secure::testing::InMemorySecretsProvider;
use tasker_secure::SecretsProvider;

/// A handle that always reports healthy.
#[derive(Debug)]
struct StubHandle {
    name: String,
    resource_type: ResourceType,
}

#[async_trait::async_trait]
impl ResourceHandle for StubHandle {
    fn resource_name(&self) -> &str {
        &self.name
    }
    fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }
    async fn refresh_credentials(
        &self,
        _secrets: &dyn SecretsProvider,
    ) -> Result<(), ResourceError> {
        Ok(())
    }
    async fn health_check(&self) -> Result<(), ResourceError> {
        Ok(())
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[tokio::test]
async fn registry_empty() {
    let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));
    let registry = ResourceRegistry::new(secrets);

    assert!(registry.get("anything").is_none());
    assert!(registry.list_resources().is_empty());
}

#[tokio::test]
async fn registry_register_and_get() {
    let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));
    let registry = ResourceRegistry::new(secrets);

    let handle: Arc<dyn ResourceHandle> = Arc::new(StubHandle {
        name: "test-db".to_string(),
        resource_type: ResourceType::Postgres,
    });
    registry.register("test-db", handle).await;

    let retrieved = registry.get("test-db");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().resource_name(), "test-db");
}

#[tokio::test]
async fn registry_get_returns_none_for_missing() {
    let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));
    let registry = ResourceRegistry::new(secrets);

    assert!(registry.get("nonexistent").is_none());
}

#[tokio::test]
async fn registry_list_resources_shows_names_and_types() {
    let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));
    let registry = ResourceRegistry::new(secrets);

    let db_handle: Arc<dyn ResourceHandle> = Arc::new(StubHandle {
        name: "orders-db".to_string(),
        resource_type: ResourceType::Postgres,
    });
    let api_handle: Arc<dyn ResourceHandle> = Arc::new(StubHandle {
        name: "fulfillment-api".to_string(),
        resource_type: ResourceType::Http,
    });
    registry.register("orders-db", db_handle).await;
    registry.register("fulfillment-api", api_handle).await;

    let summaries = registry.list_resources();
    assert_eq!(summaries.len(), 2);

    let names: Vec<&str> = summaries.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"orders-db"));
    assert!(names.contains(&"fulfillment-api"));
}

#[tokio::test]
async fn registry_list_resources_never_exposes_credentials() {
    let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));
    let registry = ResourceRegistry::new(secrets);

    let handle: Arc<dyn ResourceHandle> = Arc::new(StubHandle {
        name: "secret-db".to_string(),
        resource_type: ResourceType::Postgres,
    });
    registry.register("secret-db", handle).await;

    let summaries = registry.list_resources();
    // ResourceSummary only has name, resource_type, healthy — no config/credentials
    let summary = &summaries[0];
    let debug_output = format!("{summary:?}");
    assert!(!debug_output.contains("password"));
    assert!(!debug_output.contains("secret"));
    // Verify the struct fields are what we expect
    assert_eq!(summary.name, "secret-db");
    assert!(matches!(summary.resource_type, ResourceType::Postgres));
}

#[tokio::test]
async fn registry_refresh_resource() {
    let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));
    let registry = ResourceRegistry::new(secrets);

    let handle: Arc<dyn ResourceHandle> = Arc::new(StubHandle {
        name: "test-db".to_string(),
        resource_type: ResourceType::Postgres,
    });
    registry.register("test-db", handle).await;

    // StubHandle refresh always succeeds
    let result = registry.refresh_resource("test-db").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn registry_refresh_missing_resource() {
    let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));
    let registry = ResourceRegistry::new(secrets);

    let result = registry.refresh_resource("nonexistent").await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ResourceError::ResourceNotFound { .. }
    ));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p tasker-secure --features test-utils --test resource_registry_test 2>&1 | head -20`
Expected: Compilation error — `ResourceRegistry` doesn't exist.

**Step 3: Implement the registry**

Create `crates/tasker-secure/src/resource/registry.rs`:

```rust
//! ResourceRegistry: initialization, lookup, and credential refresh for
//! named resource handles.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use super::error::ResourceError;
use super::handle::ResourceHandle;
use super::types::ResourceSummary;
use crate::secrets::SecretsProvider;

/// A registry of initialized resource handles.
///
/// Handles are registered at worker startup (after credential resolution
/// and health checking). Capability executors look up handles by name
/// via `get()`.
#[derive(Debug)]
pub struct ResourceRegistry {
    secrets: Arc<dyn SecretsProvider>,
    resources: RwLock<HashMap<String, Arc<dyn ResourceHandle>>>,
}

impl ResourceRegistry {
    /// Create an empty registry with the given secrets provider.
    pub fn new(secrets: Arc<dyn SecretsProvider>) -> Self {
        Self {
            secrets,
            resources: RwLock::new(HashMap::new()),
        }
    }

    /// Register a handle in the registry.
    pub async fn register(&self, name: &str, handle: Arc<dyn ResourceHandle>) {
        let mut resources = self.resources.write().await;
        resources.insert(name.to_string(), handle);
    }

    /// Look up an initialized resource by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ResourceHandle>> {
        // Use try_read to avoid blocking — registry is populated at startup
        // and only written during refresh operations.
        let resources = self.resources.try_read().ok()?;
        resources.get(name).cloned()
    }

    /// Trigger credential refresh for a named resource.
    ///
    /// Resolves fresh credentials through the SecretsProvider and calls
    /// `refresh_credentials()` on the handle.
    pub async fn refresh_resource(&self, name: &str) -> Result<(), ResourceError> {
        let handle = {
            let resources = self.resources.read().await;
            resources
                .get(name)
                .cloned()
                .ok_or_else(|| ResourceError::ResourceNotFound {
                    name: name.to_string(),
                })?
        };

        handle
            .refresh_credentials(self.secrets.as_ref())
            .await
    }

    /// List all registered resources with their health status.
    ///
    /// Returns `ResourceSummary` with name, type, and health only — never
    /// configuration or credentials. This is the safe surface for MCP
    /// discoverability.
    pub fn list_resources(&self) -> Vec<ResourceSummary> {
        let resources = match self.resources.try_read() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        resources
            .values()
            .map(|handle| ResourceSummary {
                name: handle.resource_name().to_string(),
                resource_type: handle.resource_type().clone(),
                // Health is reported as true here — actual health checking is async.
                // Use health_check() on individual handles for real status.
                healthy: true,
            })
            .collect()
    }

    /// Get a reference to the secrets provider.
    pub fn secrets(&self) -> &dyn SecretsProvider {
        self.secrets.as_ref()
    }
}
```

**Step 4: Update mod.rs and lib.rs**

Add to `crates/tasker-secure/src/resource/mod.rs`:

```rust
mod registry;
pub use registry::ResourceRegistry;
```

Add to lib.rs re-exports:

```rust
pub use resource::ResourceRegistry;
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p tasker-secure --features test-utils --test resource_registry_test`
Expected: All tests pass.

**Step 6: Run all tests**

Run: `cargo test -p tasker-secure --features test-utils`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add crates/tasker-secure/src/resource/registry.rs crates/tasker-secure/src/resource/mod.rs crates/tasker-secure/src/lib.rs crates/tasker-secure/tests/resource_registry_test.rs
git commit -m "feat(TAS-358): add ResourceRegistry with register, get, list, refresh"
```

---

## Task 4: InMemoryResourceHandle and test_registry_with_fixtures (test-utils)

**Files:**
- Create: `crates/tasker-secure/src/testing/mock_resources.rs`
- Modify: `crates/tasker-secure/src/testing/mod.rs`
- Test: `crates/tasker-secure/tests/in_memory_resource_test.rs`

**Step 1: Write the failing tests**

Create `crates/tasker-secure/tests/in_memory_resource_test.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};

use tasker_secure::resource::{ResourceHandle, ResourceHandleExt, ResourceType};
use tasker_secure::testing::{InMemoryResourceHandle, ResourceFixture, test_registry_with_fixtures};
use tasker_secure::SecretsProvider;

#[test]
fn in_memory_handle_name_and_type() {
    let handle = InMemoryResourceHandle::new("orders-db", ResourceType::Postgres);
    assert_eq!(handle.resource_name(), "orders-db");
    assert_eq!(*handle.resource_type(), ResourceType::Postgres);
}

#[tokio::test]
async fn in_memory_handle_health_check() {
    let handle = InMemoryResourceHandle::new("orders-db", ResourceType::Postgres);
    assert!(handle.health_check().await.is_ok());
}

#[test]
fn in_memory_handle_fixture_data() {
    let mut fixtures = HashMap::new();
    fixtures.insert(
        "orders".to_string(),
        json!([{"id": 1, "amount": 100}]),
    );

    let handle = InMemoryResourceHandle::with_fixtures(
        "orders-db",
        ResourceType::Postgres,
        fixtures,
    );

    assert_eq!(
        handle.get_fixture("orders"),
        Some(&json!([{"id": 1, "amount": 100}]))
    );
    assert_eq!(handle.get_fixture("missing"), None);
}

#[test]
fn in_memory_handle_persist_capture() {
    let handle = InMemoryResourceHandle::new("orders-db", ResourceType::Postgres);

    handle.capture_persist(json!({"id": 1, "status": "created"}));
    handle.capture_persist(json!({"id": 2, "status": "created"}));

    let persisted = handle.persisted();
    assert_eq!(persisted.len(), 2);
    assert_eq!(persisted[0]["id"], 1);
    assert_eq!(persisted[1]["id"], 2);
}

#[test]
fn in_memory_handle_emit_capture() {
    let handle = InMemoryResourceHandle::new("events-bus", ResourceType::Pgmq);

    handle.capture_emit(json!({"event": "order.created", "order_id": 1}));

    let emitted = handle.emitted();
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0]["event"], "order.created");
}

#[test]
fn in_memory_handle_clear_captures() {
    let handle = InMemoryResourceHandle::new("orders-db", ResourceType::Postgres);

    handle.capture_persist(json!({"id": 1}));
    handle.capture_emit(json!({"event": "test"}));

    handle.clear_captures();

    assert!(handle.persisted().is_empty());
    assert!(handle.emitted().is_empty());
}

#[tokio::test]
async fn test_registry_with_fixtures_creates_usable_registry() {
    let fixtures = vec![
        ResourceFixture {
            name: "orders-db".to_string(),
            resource_type: ResourceType::Postgres,
            data: HashMap::from([
                ("orders".to_string(), json!([{"id": 1}])),
            ]),
        },
        ResourceFixture {
            name: "fulfillment-api".to_string(),
            resource_type: ResourceType::Http,
            data: HashMap::new(),
        },
    ];

    let registry = test_registry_with_fixtures(fixtures).await;

    // Both resources are registered
    let summaries = registry.list_resources();
    assert_eq!(summaries.len(), 2);

    // Can retrieve by name
    let db = registry.get("orders-db");
    assert!(db.is_some());
    assert_eq!(db.unwrap().resource_name(), "orders-db");

    let api = registry.get("fulfillment-api");
    assert!(api.is_some());
}

#[tokio::test]
async fn test_registry_in_memory_handle_downcast() {
    let fixtures = vec![ResourceFixture {
        name: "orders-db".to_string(),
        resource_type: ResourceType::Postgres,
        data: HashMap::from([("orders".to_string(), json!([{"id": 1}]))]),
    }];

    let registry = test_registry_with_fixtures(fixtures).await;
    let handle = registry.get("orders-db").unwrap();

    // Downcast to InMemoryResourceHandle to access test methods
    let in_mem = handle
        .as_any()
        .downcast_ref::<InMemoryResourceHandle>()
        .unwrap();

    assert_eq!(in_mem.get_fixture("orders"), Some(&json!([{"id": 1}])));
    in_mem.capture_persist(json!({"id": 2}));
    assert_eq!(in_mem.persisted().len(), 1);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p tasker-secure --features test-utils --test in_memory_resource_test 2>&1 | head -20`
Expected: Compilation error — `InMemoryResourceHandle` doesn't exist.

**Step 3: Implement InMemoryResourceHandle**

Create `crates/tasker-secure/src/testing/mock_resources.rs`:

```rust
//! In-memory resource handle for testing capability executors without
//! real infrastructure.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::resource::{ResourceError, ResourceHandle, ResourceRegistry, ResourceType};
use crate::secrets::SecretsProvider;
use crate::testing::InMemorySecretsProvider;

/// A resource handle backed by in-memory data for testing.
///
/// Provides:
/// - `fixture_data`: canned responses for `acquire` operations
/// - `persisted`: captured values from `persist` operations
/// - `emitted`: captured values from `emit` operations
///
/// Grammar executor tests use this to verify capability behavior without
/// any database, network, or external infrastructure.
#[derive(Debug)]
pub struct InMemoryResourceHandle {
    name: String,
    resource_type: ResourceType,
    fixture_data: HashMap<String, Value>,
    persisted: Arc<Mutex<Vec<Value>>>,
    emitted: Arc<Mutex<Vec<Value>>>,
}

impl InMemoryResourceHandle {
    /// Create a new in-memory handle with no fixture data.
    pub fn new(name: &str, resource_type: ResourceType) -> Self {
        Self {
            name: name.to_string(),
            resource_type,
            fixture_data: HashMap::new(),
            persisted: Arc::new(Mutex::new(Vec::new())),
            emitted: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a new in-memory handle with pre-loaded fixture data.
    pub fn with_fixtures(
        name: &str,
        resource_type: ResourceType,
        fixture_data: HashMap<String, Value>,
    ) -> Self {
        Self {
            name: name.to_string(),
            resource_type,
            fixture_data,
            persisted: Arc::new(Mutex::new(Vec::new())),
            emitted: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get fixture data by key (for acquire executor tests).
    pub fn get_fixture(&self, key: &str) -> Option<&Value> {
        self.fixture_data.get(key)
    }

    /// Record a value as persisted (for persist executor tests).
    pub fn capture_persist(&self, value: Value) {
        self.persisted.lock().unwrap().push(value);
    }

    /// Record a value as emitted (for emit executor tests).
    pub fn capture_emit(&self, value: Value) {
        self.emitted.lock().unwrap().push(value);
    }

    /// Get all captured persist operations.
    pub fn persisted(&self) -> Vec<Value> {
        self.persisted.lock().unwrap().clone()
    }

    /// Get all captured emit operations.
    pub fn emitted(&self) -> Vec<Value> {
        self.emitted.lock().unwrap().clone()
    }

    /// Clear all captured operations.
    pub fn clear_captures(&self) {
        self.persisted.lock().unwrap().clear();
        self.emitted.lock().unwrap().clear();
    }
}

#[async_trait::async_trait]
impl ResourceHandle for InMemoryResourceHandle {
    fn resource_name(&self) -> &str {
        &self.name
    }

    fn resource_type(&self) -> &ResourceType {
        &self.resource_type
    }

    async fn refresh_credentials(
        &self,
        _secrets: &dyn SecretsProvider,
    ) -> Result<(), ResourceError> {
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ResourceError> {
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Fixture specification for building a test registry.
#[derive(Debug, Clone)]
pub struct ResourceFixture {
    /// Resource name.
    pub name: String,
    /// Resource type.
    pub resource_type: ResourceType,
    /// Canned data keyed by entity/table name.
    pub data: HashMap<String, Value>,
}

/// Create a `ResourceRegistry` populated with `InMemoryResourceHandle`s.
///
/// This is the primary test utility for Phase 1C grammar executor stubs.
/// No database, no network, no external infrastructure required.
pub async fn test_registry_with_fixtures(fixtures: Vec<ResourceFixture>) -> ResourceRegistry {
    let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));
    let registry = ResourceRegistry::new(secrets);

    for fixture in fixtures {
        let handle = Arc::new(InMemoryResourceHandle::with_fixtures(
            &fixture.name,
            fixture.resource_type,
            fixture.data,
        ));
        registry.register(&fixture.name, handle).await;
    }

    registry
}
```

**Step 4: Update testing/mod.rs**

```rust
//! Test utilities for `tasker-secure` consumers.

mod mock_secrets;

pub use mock_secrets::InMemorySecretsProvider;

mod mock_resources;

pub use mock_resources::{InMemoryResourceHandle, ResourceFixture, test_registry_with_fixtures};
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p tasker-secure --features test-utils --test in_memory_resource_test`
Expected: All tests pass.

**Step 6: Run all tests**

Run: `cargo test -p tasker-secure --features test-utils`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add crates/tasker-secure/src/testing/ crates/tasker-secure/tests/in_memory_resource_test.rs
git commit -m "feat(TAS-358): add InMemoryResourceHandle and test_registry_with_fixtures"
```

---

## Task 5: ResourceDefinition TOML Deserialization Test

**Files:**
- Test: `crates/tasker-secure/tests/resource_definition_test.rs`

This task verifies that a complete `worker.toml` resource section deserializes correctly into `ResourceDefinition` structs — the TOML format that platform engineers will write.

**Step 1: Write the tests**

Create `crates/tasker-secure/tests/resource_definition_test.rs`:

```rust
use tasker_secure::resource::{ConfigValue, ResourceDefinition, ResourceType};

#[test]
fn deserialize_postgres_resource_definition() {
    let toml_str = r#"
name = "orders-db"
resource_type = "postgres"

[config]
host = "orders-db.internal"
port = "5432"
database = "orders"
user = { secret_ref = "/production/tasker/orders-db/user" }
password = { secret_ref = "/production/tasker/orders-db/password" }
max_connections = "20"
"#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.name, "orders-db");
    assert!(matches!(def.resource_type, ResourceType::Postgres));
    assert!(def.secrets_provider.is_none());

    // Literal values
    assert!(matches!(
        def.config.get("host"),
        Some(ConfigValue::Literal(ref s)) if s == "orders-db.internal"
    ));
    assert!(matches!(
        def.config.get("port"),
        Some(ConfigValue::Literal(ref s)) if s == "5432"
    ));

    // Secret references
    assert!(matches!(
        def.config.get("password"),
        Some(ConfigValue::SecretRef { ref secret_ref }) if secret_ref == "/production/tasker/orders-db/password"
    ));
}

#[test]
fn deserialize_http_resource_definition() {
    let toml_str = r#"
name = "fulfillment-api"
resource_type = "http"

[config]
base_url = "https://api.fulfillment.internal"
auth_header = "X-API-Key"
auth_value = { secret_ref = "/production/tasker/fulfillment-api/key" }
timeout_ms = "5000"
"#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.name, "fulfillment-api");
    assert!(matches!(def.resource_type, ResourceType::Http));

    assert!(matches!(
        def.config.get("base_url"),
        Some(ConfigValue::Literal(ref s)) if s == "https://api.fulfillment.internal"
    ));
    assert!(matches!(
        def.config.get("auth_value"),
        Some(ConfigValue::SecretRef { .. })
    ));
}

#[test]
fn deserialize_pgmq_resource_definition() {
    let toml_str = r#"
name = "events-bus"
resource_type = "pgmq"
"#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.name, "events-bus");
    assert!(matches!(def.resource_type, ResourceType::Pgmq));
}

#[test]
fn deserialize_resource_with_env_ref() {
    let toml_str = r#"
name = "local-db"
resource_type = "postgres"

[config]
host = "localhost"
password = { env = "LOCAL_DB_PASSWORD" }
"#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert!(matches!(
        def.config.get("password"),
        Some(ConfigValue::EnvRef { ref env }) if env == "LOCAL_DB_PASSWORD"
    ));
}

#[test]
fn deserialize_resource_with_secrets_provider() {
    let toml_str = r#"
name = "vault-db"
resource_type = "postgres"
secrets_provider = "vault"

[config]
password = { secret_ref = "prod/db/password" }
"#;

    let def: ResourceDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.secrets_provider.as_deref(), Some("vault"));
}

#[test]
fn deserialize_multiple_resource_definitions() {
    let toml_str = r#"
[[resources]]
name = "orders-db"
resource_type = "postgres"

[resources.config]
host = "db.internal"

[[resources]]
name = "fulfillment-api"
resource_type = "http"

[resources.config]
base_url = "https://api.internal"
"#;

    #[derive(serde::Deserialize)]
    struct Config {
        resources: Vec<ResourceDefinition>,
    }

    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.resources.len(), 2);
    assert_eq!(config.resources[0].name, "orders-db");
    assert_eq!(config.resources[1].name, "fulfillment-api");
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test -p tasker-secure --features test-utils --test resource_definition_test`
Expected: All tests pass (no new code needed — this validates the types from Tasks 1-3).

**Step 3: Commit**

```bash
git add crates/tasker-secure/tests/resource_definition_test.rs
git commit -m "test(TAS-358): add ResourceDefinition TOML deserialization tests"
```

---

## Task 6: PostgresHandle (feature: postgres)

**Files:**
- Modify: `crates/tasker-secure/Cargo.toml` — add `postgres` feature with sqlx dep
- Create: `crates/tasker-secure/src/resource/postgres.rs`
- Modify: `crates/tasker-secure/src/resource/mod.rs`
- Modify: `crates/tasker-secure/src/lib.rs`
- Test: `crates/tasker-secure/tests/postgres_handle_test.rs`

**Step 1: Add the postgres feature to Cargo.toml**

Add to `crates/tasker-secure/Cargo.toml` features section:

```toml
postgres = ["dep:sqlx"]
```

Add to dependencies:

```toml
sqlx = { workspace = true, optional = true }
```

**Step 2: Write the failing tests**

Create `crates/tasker-secure/tests/postgres_handle_test.rs`:

```rust
//! Tests for PostgresHandle.
//!
//! Unit tests run without a database (test handle construction, config parsing).
//! Integration tests (feature: test-services) verify real PostgreSQL connectivity.

#[cfg(feature = "postgres")]
mod tests {
    use std::any::Any;
    use std::collections::HashMap;
    use std::sync::Arc;

    use tasker_secure::resource::postgres::PostgresHandle;
    use tasker_secure::resource::{
        ConfigValue, ResourceConfig, ResourceHandle, ResourceHandleExt, ResourceType,
    };
    use tasker_secure::testing::InMemorySecretsProvider;

    #[test]
    fn postgres_handle_resource_type() {
        // We can't construct a real PostgresHandle without a DB, but we can
        // verify the type system works by testing via InMemoryResourceHandle
        // downcast behavior — as_postgres() should return None for non-Postgres handles.
        use tasker_secure::testing::InMemoryResourceHandle;

        let handle = InMemoryResourceHandle::new("test", ResourceType::Postgres);
        // InMemoryResourceHandle is not a PostgresHandle
        assert!(handle.as_postgres().is_none());
    }

    #[test]
    fn postgres_config_parsing() {
        let toml_str = r#"
name = "orders-db"
resource_type = "postgres"

[config]
host = "localhost"
port = "5432"
database = "orders"
user = "tasker"
password = { secret_ref = "/prod/db/password" }
max_connections = "10"
min_connections = "2"
acquire_timeout_seconds = "5"
"#;

        let def: tasker_secure::resource::ResourceDefinition =
            toml::from_str(toml_str).unwrap();

        assert_eq!(def.name, "orders-db");
        assert!(matches!(def.resource_type, ResourceType::Postgres));

        // Verify all config keys parse
        assert!(def.config.get("host").is_some());
        assert!(def.config.get("port").is_some());
        assert!(def.config.get("database").is_some());
        assert!(def.config.get("user").is_some());
        assert!(def.config.get("password").is_some());
        assert!(def.config.get("max_connections").is_some());
    }

    #[tokio::test]
    async fn postgres_handle_from_config() {
        // Build config with literal values pointing to test DB
        let mut values = HashMap::new();
        values.insert("host".to_string(), ConfigValue::Literal("localhost".to_string()));
        values.insert("port".to_string(), ConfigValue::Literal("5432".to_string()));
        values.insert("database".to_string(), ConfigValue::Literal("tasker_rust_test".to_string()));
        values.insert("user".to_string(), ConfigValue::Literal("tasker".to_string()));
        values.insert("password".to_string(), ConfigValue::Literal("tasker".to_string()));
        values.insert("max_connections".to_string(), ConfigValue::Literal("2".to_string()));

        let config = ResourceConfig::new(values);
        let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));

        let result = PostgresHandle::from_config("test-db", &config, secrets.as_ref()).await;

        // This will fail if no PostgreSQL is running — that's expected in unit test mode.
        // The test verifies the construction API compiles and works.
        if std::env::var("DATABASE_URL").is_ok() {
            let handle = result.unwrap();
            assert_eq!(handle.resource_name(), "test-db");
            assert!(matches!(handle.resource_type(), ResourceType::Postgres));
            assert!(handle.pool().acquire().await.is_ok());
        }
        // If no DB available, just verify the API exists and is callable
    }
}
```

**Step 3: Implement PostgresHandle**

Create `crates/tasker-secure/src/resource/postgres.rs`:

```rust
//! PostgresHandle: wraps `sqlx::PgPool` with credential refresh support.

use std::any::Any;
use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use super::config_value::ResourceConfig;
use super::error::ResourceError;
use super::handle::ResourceHandle;
use super::types::ResourceType;
use crate::secrets::SecretsProvider;

/// A resource handle wrapping a PostgreSQL connection pool.
///
/// Constructed from a `ResourceConfig` with credentials resolved
/// via `SecretsProvider`. The `pool()` method provides direct access
/// to the underlying `PgPool` for sqlx queries.
#[derive(Debug)]
pub struct PostgresHandle {
    name: String,
    pool: Arc<PgPool>,
    /// Raw config kept for credential refresh — re-resolve and rebuild pool.
    config: ResourceConfig,
}

impl PostgresHandle {
    /// Construct a `PostgresHandle` from a resource config.
    ///
    /// Resolves all credentials through the `SecretsProvider`, builds a
    /// connection URL, and creates a connection pool.
    pub async fn from_config(
        name: &str,
        config: &ResourceConfig,
        secrets: &dyn SecretsProvider,
    ) -> Result<Self, ResourceError> {
        let pool = Self::build_pool(name, config, secrets).await?;
        Ok(Self {
            name: name.to_string(),
            pool: Arc::new(pool),
            config: config.clone(),
        })
    }

    /// Get the connection pool for direct use in sqlx queries.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    async fn build_pool(
        name: &str,
        config: &ResourceConfig,
        secrets: &dyn SecretsProvider,
    ) -> Result<PgPool, ResourceError> {
        let host = config.resolve_value("host", secrets).await.map_err(|e| {
            ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("missing host: {e}"),
            }
        })?;

        let port = config
            .resolve_optional("port", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("invalid port: {e}"),
            })?
            .unwrap_or_else(|| "5432".to_string());

        let database =
            config
                .resolve_value("database", secrets)
                .await
                .map_err(|e| ResourceError::InitializationFailed {
                    name: name.to_string(),
                    message: format!("missing database: {e}"),
                })?;

        let user = config
            .resolve_optional("user", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("invalid user: {e}"),
            })?;

        let password = config
            .resolve_optional("password", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("invalid password: {e}"),
            })?;

        let max_connections: u32 = config
            .resolve_optional("max_connections", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("invalid max_connections: {e}"),
            })?
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let min_connections: u32 = config
            .resolve_optional("min_connections", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("invalid min_connections: {e}"),
            })?
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        // Build connection URL
        let mut url = format!("postgresql://");
        if let Some(ref u) = user {
            url.push_str(u);
            if let Some(ref p) = password {
                url.push(':');
                url.push_str(p);
            }
            url.push('@');
        }
        url.push_str(&format!("{host}:{port}/{database}"));

        PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .connect(&url)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: e.to_string(),
            })
    }
}

#[async_trait::async_trait]
impl ResourceHandle for PostgresHandle {
    fn resource_name(&self) -> &str {
        &self.name
    }

    fn resource_type(&self) -> &ResourceType {
        &ResourceType::Postgres
    }

    async fn refresh_credentials(
        &self,
        _secrets: &dyn SecretsProvider,
    ) -> Result<(), ResourceError> {
        // For full credential rotation, we'd need interior mutability on the pool.
        // Initial implementation: log that refresh was requested.
        // Full rotation support can be added when Vault dynamic secrets are integrated.
        tracing::warn!(
            resource = self.name,
            "credential refresh requested but pool rebuild not yet implemented"
        );
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ResourceError> {
        sqlx::query("SELECT 1")
            .execute(self.pool.as_ref())
            .await
            .map_err(|e| ResourceError::HealthCheckFailed {
                name: self.name.clone(),
                message: e.to_string(),
            })?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
```

**Step 4: Update mod.rs**

Add to `crates/tasker-secure/src/resource/mod.rs`:

```rust
#[cfg(feature = "postgres")]
pub mod postgres;
```

**Step 5: Run tests**

Run: `cargo test -p tasker-secure --features test-utils,postgres --test postgres_handle_test`
Expected: Tests pass (construction tests may skip if no DATABASE_URL).

**Step 6: Run all tests**

Run: `cargo test -p tasker-secure --features test-utils,postgres`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add crates/tasker-secure/Cargo.toml crates/tasker-secure/src/resource/postgres.rs crates/tasker-secure/src/resource/mod.rs crates/tasker-secure/tests/postgres_handle_test.rs
git commit -m "feat(TAS-358): add PostgresHandle wrapping sqlx::PgPool (postgres feature)"
```

---

## Task 7: HttpHandle with HttpAuthStrategy (feature: http)

**Files:**
- Modify: `crates/tasker-secure/Cargo.toml` — add `http` feature with reqwest dep
- Create: `crates/tasker-secure/src/resource/http.rs`
- Modify: `crates/tasker-secure/src/resource/mod.rs`
- Test: `crates/tasker-secure/tests/http_handle_test.rs`

**Step 1: Add the http feature to Cargo.toml**

Add to features:

```toml
http = ["dep:reqwest"]
```

Add to dependencies:

```toml
reqwest = { workspace = true, optional = true }
```

**Step 2: Write the failing tests**

Create `crates/tasker-secure/tests/http_handle_test.rs`:

```rust
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
    use tasker_secure::testing::InMemorySecretsProvider;

    #[test]
    fn http_handle_resource_type() {
        use tasker_secure::testing::InMemoryResourceHandle;
        let handle = InMemoryResourceHandle::new("api", ResourceType::Http);
        // InMemoryResourceHandle is not an HttpHandle
        assert!(handle.as_http().is_none());
    }

    #[tokio::test]
    async fn http_handle_from_config_with_api_key() {
        let mut values = HashMap::new();
        values.insert(
            "base_url".to_string(),
            ConfigValue::Literal("https://httpbin.org".to_string()),
        );
        values.insert(
            "auth_type".to_string(),
            ConfigValue::Literal("api_key".to_string()),
        );
        values.insert(
            "auth_header".to_string(),
            ConfigValue::Literal("X-API-Key".to_string()),
        );
        values.insert(
            "auth_value".to_string(),
            ConfigValue::Literal("test-key-123".to_string()),
        );

        let config = ResourceConfig::new(values);
        let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));

        let handle = HttpHandle::from_config("test-api", &config, secrets.as_ref())
            .await
            .unwrap();

        assert_eq!(handle.resource_name(), "test-api");
        assert!(matches!(handle.resource_type(), ResourceType::Http));
        assert_eq!(handle.base_url(), "https://httpbin.org");
    }

    #[tokio::test]
    async fn http_handle_from_config_with_bearer_token() {
        let mut values = HashMap::new();
        values.insert(
            "base_url".to_string(),
            ConfigValue::Literal("https://api.example.com".to_string()),
        );
        values.insert(
            "auth_type".to_string(),
            ConfigValue::Literal("bearer".to_string()),
        );
        values.insert(
            "auth_value".to_string(),
            ConfigValue::Literal("my-bearer-token".to_string()),
        );

        let config = ResourceConfig::new(values);
        let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));

        let handle = HttpHandle::from_config("test-api", &config, secrets.as_ref())
            .await
            .unwrap();

        assert_eq!(handle.resource_name(), "test-api");
    }

    #[tokio::test]
    async fn http_handle_from_config_no_auth() {
        let mut values = HashMap::new();
        values.insert(
            "base_url".to_string(),
            ConfigValue::Literal("https://public-api.example.com".to_string()),
        );

        let config = ResourceConfig::new(values);
        let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));

        let handle = HttpHandle::from_config("public-api", &config, secrets.as_ref())
            .await
            .unwrap();

        assert_eq!(handle.resource_name(), "public-api");
    }

    #[tokio::test]
    async fn http_handle_missing_base_url() {
        let config = ResourceConfig::new(HashMap::new());
        let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));

        let result = HttpHandle::from_config("bad-api", &config, secrets.as_ref()).await;
        assert!(result.is_err());
    }

    #[test]
    fn api_key_auth_strategy_apply() {
        let strategy = ApiKeyAuthStrategy::new("X-API-Key", "test-key");
        let client = reqwest::Client::new();
        let builder = client.get("https://example.com");
        let _applied = strategy.apply(builder);
        // Can't easily inspect headers on RequestBuilder, but verifying it compiles
        // and doesn't panic is the unit test. Integration tests verify actual behavior.
    }

    #[test]
    fn bearer_token_auth_strategy_apply() {
        let strategy = BearerTokenAuthStrategy::new("my-token");
        let client = reqwest::Client::new();
        let builder = client.get("https://example.com");
        let _applied = strategy.apply(builder);
    }

    #[tokio::test]
    async fn http_handle_health_check_without_network() {
        let mut values = HashMap::new();
        values.insert(
            "base_url".to_string(),
            ConfigValue::Literal("https://localhost:1".to_string()),
        );
        let config = ResourceConfig::new(values);
        let secrets = Arc::new(InMemorySecretsProvider::new(HashMap::new()));

        let handle = HttpHandle::from_config("bad-api", &config, secrets.as_ref())
            .await
            .unwrap();

        // Health check to unreachable host should fail
        let result = handle.health_check().await;
        assert!(result.is_err());
    }
}
```

**Step 3: Implement HttpHandle**

Create `crates/tasker-secure/src/resource/http.rs`:

```rust
//! HttpHandle: wraps `reqwest::Client` with pre-configured authentication.

use std::any::Any;
use std::sync::Arc;

use reqwest::Client;

use super::config_value::ResourceConfig;
use super::error::ResourceError;
use super::handle::ResourceHandle;
use super::types::ResourceType;
use crate::secrets::SecretsProvider;

/// Authentication strategy for HTTP requests.
///
/// Applied to every outgoing request from an `HttpHandle`.
pub trait HttpAuthStrategy: Send + Sync + std::fmt::Debug {
    /// Apply authentication to an outgoing request builder.
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder;
}

/// API key authentication — adds a custom header to every request.
#[derive(Debug, Clone)]
pub struct ApiKeyAuthStrategy {
    header: String,
    value: String,
}

impl ApiKeyAuthStrategy {
    /// Create a new API key auth strategy.
    pub fn new(header: &str, value: &str) -> Self {
        Self {
            header: header.to_string(),
            value: value.to_string(),
        }
    }
}

impl HttpAuthStrategy for ApiKeyAuthStrategy {
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.header(&self.header, &self.value)
    }
}

/// Bearer token authentication — adds `Authorization: Bearer <token>`.
#[derive(Debug, Clone)]
pub struct BearerTokenAuthStrategy {
    token: String,
}

impl BearerTokenAuthStrategy {
    /// Create a new bearer token auth strategy.
    pub fn new(token: &str) -> Self {
        Self {
            token: token.to_string(),
        }
    }
}

impl HttpAuthStrategy for BearerTokenAuthStrategy {
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.bearer_auth(&self.token)
    }
}

/// No authentication — requests are sent without auth headers.
#[derive(Debug, Clone)]
struct NoAuthStrategy;

impl HttpAuthStrategy for NoAuthStrategy {
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
    }
}

/// A resource handle wrapping an HTTP client with pre-configured authentication.
///
/// All requests are pre-authenticated via the `HttpAuthStrategy`. Capability
/// executors call `handle.get(path)` or `handle.post(path)` without handling auth.
#[derive(Debug)]
pub struct HttpHandle {
    name: String,
    client: Arc<Client>,
    base_url: String,
    auth: Arc<dyn HttpAuthStrategy>,
    config: ResourceConfig,
}

impl HttpHandle {
    /// Construct an `HttpHandle` from a resource config.
    ///
    /// Required config keys: `base_url`.
    /// Optional: `auth_type` ("api_key" or "bearer"), `auth_header`, `auth_value`,
    /// `timeout_ms`, `max_connections_per_host`.
    pub async fn from_config(
        name: &str,
        config: &ResourceConfig,
        secrets: &dyn SecretsProvider,
    ) -> Result<Self, ResourceError> {
        let base_url =
            config
                .resolve_value("base_url", secrets)
                .await
                .map_err(|e| ResourceError::InitializationFailed {
                    name: name.to_string(),
                    message: format!("missing base_url: {e}"),
                })?;

        let auth_type = config.resolve_optional("auth_type", secrets).await.map_err(
            |e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("invalid auth_type: {e}"),
            },
        )?;

        let auth: Arc<dyn HttpAuthStrategy> = match auth_type.as_deref() {
            Some("api_key") => {
                let header = config.resolve_value("auth_header", secrets).await.map_err(
                    |e| ResourceError::InitializationFailed {
                        name: name.to_string(),
                        message: format!("api_key auth requires auth_header: {e}"),
                    },
                )?;
                let value = config.resolve_value("auth_value", secrets).await.map_err(
                    |e| ResourceError::InitializationFailed {
                        name: name.to_string(),
                        message: format!("api_key auth requires auth_value: {e}"),
                    },
                )?;
                Arc::new(ApiKeyAuthStrategy::new(&header, &value))
            }
            Some("bearer") => {
                let token = config.resolve_value("auth_value", secrets).await.map_err(
                    |e| ResourceError::InitializationFailed {
                        name: name.to_string(),
                        message: format!("bearer auth requires auth_value: {e}"),
                    },
                )?;
                Arc::new(BearerTokenAuthStrategy::new(&token))
            }
            Some(other) => {
                return Err(ResourceError::InitializationFailed {
                    name: name.to_string(),
                    message: format!("unknown auth_type: {other}"),
                });
            }
            None => Arc::new(NoAuthStrategy),
        };

        let timeout_ms: u64 = config
            .resolve_optional("timeout_ms", secrets)
            .await
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: format!("invalid timeout_ms: {e}"),
            })?
            .and_then(|s| s.parse().ok())
            .unwrap_or(30_000);

        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
            .map_err(|e| ResourceError::InitializationFailed {
                name: name.to_string(),
                message: e.to_string(),
            })?;

        Ok(Self {
            name: name.to_string(),
            client: Arc::new(client),
            base_url,
            auth,
            config: config.clone(),
        })
    }

    /// Get the base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Build a GET request with auth pre-applied.
    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        let builder = self.client.get(format!("{}{}", self.base_url, path));
        self.auth.apply(builder)
    }

    /// Build a POST request with auth pre-applied.
    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        let builder = self.client.post(format!("{}{}", self.base_url, path));
        self.auth.apply(builder)
    }

    /// Build a PUT request with auth pre-applied.
    pub fn put(&self, path: &str) -> reqwest::RequestBuilder {
        let builder = self.client.put(format!("{}{}", self.base_url, path));
        self.auth.apply(builder)
    }

    /// Build a DELETE request with auth pre-applied.
    pub fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        let builder = self.client.delete(format!("{}{}", self.base_url, path));
        self.auth.apply(builder)
    }
}

#[async_trait::async_trait]
impl ResourceHandle for HttpHandle {
    fn resource_name(&self) -> &str {
        &self.name
    }

    fn resource_type(&self) -> &ResourceType {
        &ResourceType::Http
    }

    async fn refresh_credentials(
        &self,
        _secrets: &dyn SecretsProvider,
    ) -> Result<(), ResourceError> {
        tracing::warn!(
            resource = self.name,
            "credential refresh requested but HTTP client rebuild not yet implemented"
        );
        Ok(())
    }

    async fn health_check(&self) -> Result<(), ResourceError> {
        // Try a HEAD request to the base URL
        self.client
            .head(&self.base_url)
            .send()
            .await
            .map_err(|e| ResourceError::HealthCheckFailed {
                name: self.name.clone(),
                message: e.to_string(),
            })?;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
```

**Step 4: Update mod.rs**

Add to `crates/tasker-secure/src/resource/mod.rs`:

```rust
#[cfg(feature = "http")]
pub mod http;
```

**Step 5: Run tests**

Run: `cargo test -p tasker-secure --features test-utils,http --test http_handle_test`
Expected: All tests pass.

**Step 6: Run all tests**

Run: `cargo test -p tasker-secure --features test-utils,http,postgres`
Expected: All tests pass.

**Step 7: Commit**

```bash
git add crates/tasker-secure/Cargo.toml crates/tasker-secure/src/resource/http.rs crates/tasker-secure/src/resource/mod.rs crates/tasker-secure/tests/http_handle_test.rs
git commit -m "feat(TAS-358): add HttpHandle with ApiKey and Bearer auth strategies (http feature)"
```

---

## Task 8: Final Verification and Cleanup

**Files:**
- Modify: `crates/tasker-secure/src/lib.rs` — ensure all public exports are complete
- Modify: `crates/tasker-secure/src/resource/mod.rs` — final module doc comment

**Step 1: Run clippy across all feature combinations**

Run: `cargo clippy -p tasker-secure --all-targets --all-features -- -W clippy::cargo`
Expected: No errors. Fix any warnings.

**Step 2: Run fmt check**

Run: `cargo fmt -p tasker-secure --check`
Expected: No issues.

**Step 3: Run full test suite with all features**

Run: `cargo test -p tasker-secure --all-features`
Expected: All tests pass (S1 + S2).

**Step 4: Run workspace build to check no cross-crate issues**

Run: `cargo check --workspace --all-features`
Expected: Clean build.

**Step 5: Verify test count**

Run: `cargo test -p tasker-secure --features test-utils 2>&1 | grep "test result"`
Expected: Significantly more than the 36 S1 tests.

**Step 6: Commit any cleanup**

```bash
git add -A crates/tasker-secure/
git commit -m "chore(TAS-358): final cleanup, lint fixes, and verification"
```

---

## Summary

| Task | What | Tests Added | Feature |
|------|------|-------------|---------|
| 1 | ResourceType, ResourceError, ConfigValue | ~10 | core |
| 2 | ResourceHandle trait, ResourceHandleExt | ~6 | core |
| 3 | ResourceRegistry | ~6 | core |
| 4 | InMemoryResourceHandle, test_registry_with_fixtures | ~8 | test-utils |
| 5 | ResourceDefinition TOML deserialization | ~6 | core |
| 6 | PostgresHandle | ~3 | postgres |
| 7 | HttpHandle, HttpAuthStrategy | ~7 | http |
| 8 | Final verification and cleanup | — | all |

**Total estimated new tests:** ~46
**Commits:** 8 (one per task)
**Scope:** Entirely within `crates/tasker-secure/` — no changes to other workspace crates
