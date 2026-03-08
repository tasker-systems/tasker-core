//! Configuration values with secret and environment variable resolution.

use std::collections::HashMap;
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::Deserialize;

use crate::secrets::{SecretsError, SecretsProvider};

use super::error::ResourceError;

/// A configuration value that may be a literal, a secret reference, or an
/// environment variable reference.
#[derive(Clone)]
pub enum ConfigValue {
    /// A plain-text literal value.
    Literal(String),
    /// A reference to a secret managed by a [`SecretsProvider`].
    SecretRef {
        /// The path to resolve through a secrets provider.
        secret_ref: String,
    },
    /// A reference to an environment variable.
    EnvRef {
        /// The environment variable name.
        env: String,
    },
}

impl ConfigValue {
    /// Resolve this config value to a plain string.
    ///
    /// - `Literal` values are returned as-is.
    /// - `SecretRef` values are resolved through the given secrets provider.
    /// - `EnvRef` values are read from the process environment.
    pub async fn resolve(&self, secrets: &dyn SecretsProvider) -> Result<String, SecretsError> {
        match self {
            Self::Literal(s) => Ok(s.clone()),
            Self::SecretRef { secret_ref } => {
                let secret = secrets.get_secret(secret_ref).await?;
                Ok(secret.expose_secret().to_string())
            }
            Self::EnvRef { env } => std::env::var(env).map_err(|_| SecretsError::NotFound {
                path: format!("env:{env}"),
            }),
        }
    }
}

impl fmt::Debug for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(s) => write!(f, "Literal({s:?})"),
            Self::SecretRef { secret_ref } => write!(f, "SecretRef({secret_ref:?})"),
            Self::EnvRef { env } => write!(f, "EnvRef({env:?})"),
        }
    }
}

impl fmt::Display for ConfigValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(s) => write!(f, "{s}"),
            Self::SecretRef { secret_ref } => write!(f, "secret_ref:{secret_ref}"),
            Self::EnvRef { env } => write!(f, "env:{env}"),
        }
    }
}

impl<'de> Deserialize<'de> for ConfigValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(ConfigValueVisitor)
    }
}

struct ConfigValueVisitor;

impl<'de> Visitor<'de> for ConfigValueVisitor {
    type Value = ConfigValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a string literal, or an object with 'secret_ref' or 'env' key")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(ConfigValue::Literal(v.to_string()))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let Some((key, value)) = map.next_entry::<String, String>()? else {
            return Err(de::Error::custom(
                "expected object with 'secret_ref' or 'env' key",
            ));
        };

        match key.as_str() {
            "secret_ref" => Ok(ConfigValue::SecretRef { secret_ref: value }),
            "env" => Ok(ConfigValue::EnvRef { env: value }),
            other => Err(de::Error::unknown_field(other, &["secret_ref", "env"])),
        }
    }
}

/// A map of configuration key-value pairs, where values may reference secrets
/// or environment variables.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct ResourceConfig {
    values: HashMap<String, ConfigValue>,
}

impl ResourceConfig {
    /// Create a new `ResourceConfig` from a map of values.
    pub fn new(values: HashMap<String, ConfigValue>) -> Self {
        Self { values }
    }

    /// Get a config value by key without resolving it.
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.values.get(key)
    }

    /// Resolve a required config value to a string.
    ///
    /// Returns [`ResourceError::MissingConfigKey`] if the key is absent, or
    /// [`ResourceError::SecretResolution`] if secret resolution fails.
    pub async fn resolve_value(
        &self,
        key: &str,
        secrets: &dyn SecretsProvider,
    ) -> Result<String, ResourceError> {
        let value = self
            .values
            .get(key)
            .ok_or_else(|| ResourceError::MissingConfigKey {
                resource: String::new(),
                key: key.to_string(),
            })?;

        value
            .resolve(secrets)
            .await
            .map_err(|source| ResourceError::SecretResolution {
                resource: String::new(),
                source,
            })
    }

    /// Resolve an optional config value. Returns `Ok(None)` if the key is absent.
    pub async fn resolve_optional(
        &self,
        key: &str,
        secrets: &dyn SecretsProvider,
    ) -> Result<Option<String>, ResourceError> {
        match self.values.get(key) {
            None => Ok(None),
            Some(value) => value.resolve(secrets).await.map(Some).map_err(|source| {
                ResourceError::SecretResolution {
                    resource: String::new(),
                    source,
                }
            }),
        }
    }
}
