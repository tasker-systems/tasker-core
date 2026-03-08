//! SOPS-encrypted file secrets provider.
//!
//! Loads secrets from YAML/JSON files with dot-separated path navigation.
//! Values are cached in memory as `SecretValue` after initial load.
//!
//! Currently supports plaintext YAML loading via `from_plaintext_yaml`.
//! Encrypted file support via the `rops` crate will be added as a follow-up.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;

use super::{SecretValue, SecretsError, SecretsProvider};

/// Secrets provider backed by a SOPS-encrypted (or plaintext) YAML/JSON file.
///
/// Values are loaded and cached at construction time. Dot-separated paths
/// navigate the decrypted structure: `"database.orders.password"`.
#[derive(Debug)]
pub struct SopsSecretsProvider {
    cache: Arc<RwLock<HashMap<String, SecretValue>>>,
    source_path: PathBuf,
}

impl SopsSecretsProvider {
    /// Load secrets from a plaintext YAML file (for development/testing).
    ///
    /// In production, use a constructor that decrypts SOPS-encrypted files.
    pub async fn from_plaintext_yaml(path: impl AsRef<Path>) -> Result<Self, SecretsError> {
        let path = path.as_ref();
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            SecretsError::ProviderUnavailable {
                message: format!("failed to read SOPS file '{}': {e}", path.display()),
            }
        })?;

        let value: serde_json::Value =
            serde_yaml::from_str(&content).map_err(|e| SecretsError::ProviderUnavailable {
                message: format!("failed to parse SOPS file '{}': {e}", path.display()),
            })?;

        let mut cache = HashMap::new();
        flatten_value(&value, String::new(), &mut cache);

        Ok(Self {
            cache: Arc::new(RwLock::new(cache)),
            source_path: path.to_path_buf(),
        })
    }
}

/// Flatten a nested JSON value into dot-separated paths.
/// Only leaf values (strings, numbers, bools) are included.
fn flatten_value(
    value: &serde_json::Value,
    prefix: String,
    out: &mut HashMap<String, SecretValue>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten_value(val, path, out);
            }
        }
        serde_json::Value::String(s) => {
            out.insert(prefix, SecretValue::from(s.clone()));
        }
        serde_json::Value::Number(n) => {
            out.insert(prefix, SecretValue::from(n.to_string()));
        }
        serde_json::Value::Bool(b) => {
            out.insert(prefix, SecretValue::from(b.to_string()));
        }
        serde_json::Value::Null | serde_json::Value::Array(_) => {}
    }
}

#[async_trait::async_trait]
impl SecretsProvider for SopsSecretsProvider {
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError> {
        let cache = self.cache.read().await;

        // Check if path points to an object (has children but no direct value)
        let has_children = cache.keys().any(|k| k.starts_with(&format!("{path}.")));
        if has_children && !cache.contains_key(path) {
            return Err(SecretsError::InvalidPath {
                path: path.to_string(),
                reason: "path refers to an object, not a leaf value".to_string(),
            });
        }

        cache
            .get(path)
            .map(|v| SecretValue::new(v.expose_secret()))
            .ok_or_else(|| SecretsError::NotFound {
                path: path.to_string(),
            })
    }

    fn provider_name(&self) -> &str {
        "sops"
    }

    async fn health_check(&self) -> Result<(), SecretsError> {
        let cache = self.cache.read().await;
        if cache.is_empty() {
            return Err(SecretsError::ProviderUnavailable {
                message: format!(
                    "SOPS file '{}' loaded but contains no secrets",
                    self.source_path.display()
                ),
            });
        }
        Ok(())
    }
}
