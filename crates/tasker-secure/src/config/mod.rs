//! Configuration value types with secrets resolution support.
//!
//! `ConfigString` extends Tasker's configuration layer to support
//! `{secret_ref: "..."}` alongside existing `${VAR:-default}` interpolation.

use std::fmt;

use serde::Deserialize;

use crate::secrets::{SecretsError, SecretsProvider};

/// A configuration value that may be a literal, a secret reference, or an
/// environment variable reference.
///
/// # TOML syntax
///
/// ```toml
/// # Literal string:
/// url = "postgresql://localhost/tasker"
///
/// # Secrets manager reference:
/// url = {secret_ref = "/production/tasker/database/url"}
///
/// # With a named provider:
/// url = {secret_ref = "prod/tasker/db-url", provider = "vault"}
/// ```
#[derive(Debug, Clone)]
pub enum ConfigString {
    /// A plain string value.
    Literal(String),

    /// A reference to a secret managed by a `SecretsProvider`.
    SecretRef {
        /// The secret path (provider-specific format).
        path: String,
        /// Optional named provider (for `ChainedSecretsProvider` routing).
        provider: Option<String>,
    },

    /// A reference to an environment variable with optional default.
    EnvRef {
        /// The environment variable name.
        var: String,
        /// Default value if the variable is not set.
        default: Option<String>,
    },
}

/// Error type for `ConfigString` resolution.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The referenced secret could not be resolved.
    #[error("failed to resolve secret ref '{path}': {source}")]
    SecretResolution {
        /// The secret path that failed.
        path: String,
        /// The underlying secrets error.
        source: SecretsError,
    },

    /// The referenced environment variable is not set and no default was provided.
    #[error("environment variable '{var}' is not set and no default provided")]
    EnvVarNotSet {
        /// The environment variable name.
        var: String,
    },
}

impl ConfigString {
    /// Resolve this configuration value to a concrete string.
    pub async fn resolve(&self, secrets: &dyn SecretsProvider) -> Result<String, ConfigError> {
        match self {
            Self::Literal(s) => Ok(s.clone()),

            Self::SecretRef { path, .. } => {
                let value =
                    secrets
                        .get_secret(path)
                        .await
                        .map_err(|e| ConfigError::SecretResolution {
                            path: path.clone(),
                            source: e,
                        })?;
                Ok(value.expose_secret().to_string())
            }

            Self::EnvRef { var, default } => match std::env::var(var) {
                Ok(value) => Ok(value),
                Err(_) => default
                    .clone()
                    .ok_or_else(|| ConfigError::EnvVarNotSet { var: var.clone() }),
            },
        }
    }
}

impl fmt::Display for ConfigString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(s) => write!(f, "{s}"),
            Self::SecretRef { path, .. } => write!(f, "{{secret_ref: \"{path}\"}}"),
            Self::EnvRef { var, default } => match default {
                Some(d) => write!(f, "${{{var}:-{d}}}"),
                None => write!(f, "${{{var}}}"),
            },
        }
    }
}

impl<'de> Deserialize<'de> for ConfigString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct ConfigStringVisitor;

        impl<'de> de::Visitor<'de> for ConfigStringVisitor {
            type Value = ConfigString;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a string or an object with 'secret_ref'")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(ConfigString::Literal(value.to_string()))
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(ConfigString::Literal(value))
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: de::MapAccess<'de>,
            {
                let mut secret_ref: Option<String> = None;
                let mut provider: Option<String> = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "secret_ref" => {
                            secret_ref = Some(map.next_value()?);
                        }
                        "provider" => {
                            provider = Some(map.next_value()?);
                        }
                        other => {
                            return Err(de::Error::unknown_field(
                                other,
                                &["secret_ref", "provider"],
                            ));
                        }
                    }
                }

                let path = secret_ref.ok_or_else(|| de::Error::missing_field("secret_ref"))?;

                Ok(ConfigString::SecretRef { path, provider })
            }
        }

        deserializer.deserialize_any(ConfigStringVisitor)
    }
}
