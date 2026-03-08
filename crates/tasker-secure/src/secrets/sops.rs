//! SOPS-encrypted file secrets provider using the `rops` crate.
//!
//! Decrypts SOPS-encrypted YAML files at startup using age keys, then caches
//! all values in memory. Dot-separated paths navigate the decrypted structure:
//! `"database.orders.password"`.
//!
//! Age private keys are resolved via (in order):
//! 1. `ROPS_AGE` environment variable (comma-separated)
//! 2. `ROPS_AGE_KEY_FILE` environment variable (file path override)
//! 3. `~/.config/rops/age_keys` default file
//!
//! Compatible with files encrypted by the Go `sops` CLI — teams already using
//! SOPS don't change their encrypted files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rops::cryptography::cipher::AES256GCM;
use rops::cryptography::hasher::SHA512;
use rops::file::format::YamlFileFormat;
use rops::file::state::{DecryptedFile, EncryptedFile};
use rops::file::RopsFile;
use tokio::sync::RwLock;

use super::{SecretValue, SecretsError, SecretsProvider};

/// Type aliases for the rops generic state machine.
type EncryptedRopsFile = RopsFile<EncryptedFile<AES256GCM, SHA512>, YamlFileFormat>;
type DecryptedRopsFile = RopsFile<DecryptedFile<SHA512>, YamlFileFormat>;

/// Secrets provider backed by a SOPS-encrypted YAML file.
///
/// Values are decrypted and cached at construction time. Dot-separated paths
/// navigate the decrypted structure: `"database.orders.password"`.
///
/// # Age key resolution
///
/// The `rops` crate reads age private keys from:
/// 1. `ROPS_AGE` environment variable (comma-separated key strings)
/// 2. `ROPS_AGE_KEY_FILE` environment variable (path to key file)
/// 3. `~/.config/rops/age_keys` (default location)
///
/// # Example
///
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use tasker_secure::SopsSecretsProvider;
/// use tasker_secure::SecretsProvider;
///
/// // Requires ROPS_AGE env var or ~/.config/rops/age_keys
/// let provider = SopsSecretsProvider::from_path("config/secrets.enc.yaml").await?;
/// let db_password = provider.get_secret("database.password").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct SopsSecretsProvider {
    cache: Arc<RwLock<HashMap<String, SecretValue>>>,
    source_path: PathBuf,
}

impl SopsSecretsProvider {
    /// Load and decrypt a SOPS-encrypted YAML file.
    ///
    /// The age private key must be available via `ROPS_AGE` env var,
    /// `ROPS_AGE_KEY_FILE`, or `~/.config/rops/age_keys`.
    pub async fn from_path(path: impl AsRef<Path>) -> Result<Self, SecretsError> {
        let path = path.as_ref();
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            SecretsError::ProviderUnavailable {
                message: format!("failed to read SOPS file '{}': {e}", path.display()),
            }
        })?;

        let encrypted: EncryptedRopsFile =
            content
                .parse()
                .map_err(|e| SecretsError::ProviderUnavailable {
                    message: format!("failed to parse SOPS file '{}': {e}", path.display()),
                })?;

        let decrypted: DecryptedRopsFile =
            encrypted
                .decrypt()
                .map_err(|e| SecretsError::ProviderUnavailable {
                    message: format!("failed to decrypt SOPS file '{}': {e}", path.display()),
                })?;

        let yaml_map: serde_yaml::Mapping = decrypted.into_inner_map();
        let json_value = yaml_mapping_to_json(&yaml_map);

        let mut cache = HashMap::new();
        flatten_value(&json_value, String::new(), &mut cache);

        Ok(Self {
            cache: Arc::new(RwLock::new(cache)),
            source_path: path.to_path_buf(),
        })
    }
}

/// Convert a serde_yaml::Mapping to serde_json::Value for uniform path navigation.
fn yaml_mapping_to_json(mapping: &serde_yaml::Mapping) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key, value) in mapping {
        if let serde_yaml::Value::String(k) = key {
            map.insert(k.clone(), yaml_value_to_json(value));
        }
    }
    serde_json::Value::Object(map)
}

fn yaml_value_to_json(value: &serde_yaml::Value) -> serde_json::Value {
    match value {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::json!(f)
            } else {
                serde_json::Value::String(n.to_string())
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s.clone()),
        serde_yaml::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.iter().map(yaml_value_to_json).collect())
        }
        serde_yaml::Value::Mapping(m) => yaml_mapping_to_json(m),
        serde_yaml::Value::Tagged(tagged) => yaml_value_to_json(&tagged.value),
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
