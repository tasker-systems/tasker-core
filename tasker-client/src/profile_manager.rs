//! Multi-profile session management and health probing.
//!
//! `ProfileManager` holds multiple resolved client configurations and supports
//! active profile switching, health probing, and profile metadata inspection.
//! This follows the kubectl / `~/.aws/config` model for multi-environment
//! management of CLI and MCP tooling consumers.

use std::path::Path;
use std::time::Instant;

use serde::Serialize;
use tracing::{debug, warn};

use crate::config::{ClientConfig, ProfileConfig, ProfileConfigFile, Transport};
use crate::error::{ClientError, ClientResult};
use crate::transport::{OrchestrationClient, UnifiedOrchestrationClient};
use crate::transport::{UnifiedWorkerClient, WorkerClient};

/// Health status of a profile's endpoints.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileHealthStatus {
    /// Health has not been checked yet
    #[default]
    Unknown,
    /// All probed endpoints are healthy
    Healthy,
    /// Some endpoints are healthy, some are not
    Degraded,
    /// No endpoints are reachable
    Unreachable,
}

impl std::fmt::Display for ProfileHealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Unreachable => write!(f, "unreachable"),
        }
    }
}

/// Snapshot of a profile's health probe result.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileHealthSnapshot {
    pub status: ProfileHealthStatus,
    pub orchestration_healthy: Option<bool>,
    pub worker_healthy: Option<bool>,
    #[serde(skip)]
    pub last_checked: Option<Instant>,
    pub error_message: Option<String>,
}

impl Default for ProfileHealthSnapshot {
    fn default() -> Self {
        Self {
            status: ProfileHealthStatus::Unknown,
            orchestration_healthy: None,
            worker_healthy: None,
            last_checked: None,
            error_message: None,
        }
    }
}

/// Summary of a loaded profile for display and serialization.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileSummary {
    pub name: String,
    pub description: Option<String>,
    pub transport: Transport,
    pub orchestration_url: String,
    pub worker_url: String,
    pub namespaces: Option<Vec<String>>,
    pub health_status: ProfileHealthStatus,
    pub is_active: bool,
}

/// A single loaded profile with its resolved config, raw metadata, and cached health.
///
/// Co-locates all facets of a profile rather than spreading them across parallel maps.
#[derive(Debug)]
struct ProfileEntry {
    name: String,
    config: ClientConfig,
    metadata: ProfileConfig,
    health: ProfileHealthSnapshot,
}

/// Manages multiple named profiles with active selection and health probing.
///
/// Profiles are loaded once from TOML and stored as a fixed-size `Vec<ProfileEntry>`.
/// The set is small (typically 2-6 profiles) so linear lookup by name is preferred
/// over hash maps — it keeps all profile data co-located and avoids synchronizing
/// parallel maps keyed by the same name.
#[derive(Debug)]
pub struct ProfileManager {
    /// All loaded profiles (fixed after construction)
    entries: Vec<ProfileEntry>,
    /// Currently active profile name
    active_profile: String,
    /// Health probe timeout in milliseconds
    health_probe_timeout_ms: u64,
}

impl ProfileManager {
    /// Default health probe timeout (5 seconds).
    pub const DEFAULT_HEALTH_PROBE_TIMEOUT_MS: u64 = 5000;

    /// Load all profiles from the auto-discovered profile config file.
    ///
    /// Uses the same search order as `ClientConfig::find_profile_config_file()`.
    /// If no profile file is found, returns a manager with just a "default" profile
    /// using hardcoded defaults.
    pub fn load() -> ClientResult<Self> {
        if let Some(path) = ClientConfig::find_profile_config_file() {
            debug!("Loading profiles from: {}", path.display());
            Self::load_from_path(&path)
        } else {
            debug!("No profile config file found, using default profile");
            let entries = vec![ProfileEntry {
                name: "default".to_string(),
                config: ClientConfig::default(),
                metadata: ProfileConfig::default(),
                health: ProfileHealthSnapshot::default(),
            }];
            Ok(Self {
                entries,
                active_profile: "default".to_string(),
                health_probe_timeout_ms: Self::DEFAULT_HEALTH_PROBE_TIMEOUT_MS,
            })
        }
    }

    /// Load all profiles from an explicit file path.
    pub fn load_from_path(path: &Path) -> ClientResult<Self> {
        let profile_file = ClientConfig::load_profile_file(path)?;
        Self::from_profile_file(profile_file)
    }

    /// Create from an already-parsed profile config file.
    fn from_profile_file(file: ProfileConfigFile) -> ClientResult<Self> {
        let mut entries = Vec::with_capacity(file.profile.len());

        for (name, raw) in &file.profile {
            // Resolve each profile: defaults → [profile.default] → [profile.{name}] → env
            match ClientConfig::resolve_from_file(name, &file) {
                Ok(config) => {
                    entries.push(ProfileEntry {
                        name: name.clone(),
                        config,
                        metadata: raw.clone(),
                        health: ProfileHealthSnapshot::default(),
                    });
                }
                Err(e) => {
                    warn!(profile = %name, error = %e, "Failed to resolve profile, skipping");
                }
            }
        }

        // Sort entries by name for deterministic ordering
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        // Determine initial active profile: "default" if it exists, otherwise first
        let active_profile = if entries.iter().any(|e| e.name == "default") {
            "default".to_string()
        } else {
            entries
                .first()
                .map(|e| e.name.clone())
                .unwrap_or_else(|| "default".to_string())
        };

        // If no profiles were resolved, add a default
        if entries.is_empty() {
            entries.push(ProfileEntry {
                name: "default".to_string(),
                config: ClientConfig::default(),
                metadata: ProfileConfig::default(),
                health: ProfileHealthSnapshot::default(),
            });
        }

        Ok(Self {
            entries,
            active_profile,
            health_probe_timeout_ms: Self::DEFAULT_HEALTH_PROBE_TIMEOUT_MS,
        })
    }

    /// Find a profile entry by name.
    fn find(&self, name: &str) -> Option<&ProfileEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Find a profile entry by name (mutable).
    fn find_mut(&mut self, name: &str) -> Option<&mut ProfileEntry> {
        self.entries.iter_mut().find(|e| e.name == name)
    }

    /// Create an offline-only manager with no profiles loaded.
    pub fn offline() -> Self {
        Self {
            entries: Vec::new(),
            active_profile: String::new(),
            health_probe_timeout_ms: Self::DEFAULT_HEALTH_PROBE_TIMEOUT_MS,
        }
    }

    /// Returns true if this manager has no profiles loaded (offline mode).
    pub fn is_offline(&self) -> bool {
        self.entries.is_empty()
    }

    /// List all loaded profile names.
    pub fn list_profile_names(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// List all profiles with summary information.
    pub fn list_profiles(&self) -> Vec<ProfileSummary> {
        self.entries
            .iter()
            .map(|entry| ProfileSummary {
                name: entry.name.clone(),
                description: entry.metadata.description.clone(),
                transport: entry.config.transport,
                orchestration_url: entry.config.orchestration.base_url.clone(),
                worker_url: entry.config.worker.base_url.clone(),
                namespaces: entry.metadata.namespaces.clone(),
                health_status: entry.health.status,
                is_active: entry.name == self.active_profile,
            })
            .collect()
    }

    /// Get the active profile name.
    pub fn active_profile_name(&self) -> &str {
        &self.active_profile
    }

    /// Get the active profile's resolved `ClientConfig`.
    pub fn active_config(&self) -> Option<&ClientConfig> {
        self.find(&self.active_profile).map(|e| &e.config)
    }

    /// Switch the active profile. Returns error if the profile doesn't exist.
    pub fn switch_profile(&mut self, name: &str) -> ClientResult<()> {
        if self.find(name).is_some() {
            self.active_profile = name.to_string();
            debug!(profile = %name, "Switched active profile");
            Ok(())
        } else {
            let available: Vec<&str> = self.list_profile_names();
            Err(ClientError::config_error(format!(
                "Profile '{}' not found. Available profiles: {}",
                name,
                available.join(", ")
            )))
        }
    }

    /// Get a profile's resolved `ClientConfig` by name.
    pub fn get_config(&self, name: &str) -> Option<&ClientConfig> {
        self.find(name).map(|e| &e.config)
    }

    /// Probe health for a specific profile.
    ///
    /// Constructs temporary clients with a short timeout, probes orchestration
    /// and worker endpoints, and caches the result on the entry.
    pub async fn probe_health(
        &mut self,
        profile_name: &str,
    ) -> ClientResult<ProfileHealthSnapshot> {
        let config = self
            .find(profile_name)
            .ok_or_else(|| {
                ClientError::config_error(format!("Profile '{}' not found", profile_name))
            })?
            .config
            .clone();

        let snapshot = Self::probe_config_health(&config, self.health_probe_timeout_ms).await;

        // Cache result on the entry
        if let Some(entry) = self.find_mut(profile_name) {
            entry.health = snapshot.clone();
        }
        Ok(snapshot)
    }

    /// Probe health for the active profile.
    pub async fn probe_active_health(&mut self) -> ClientResult<ProfileHealthSnapshot> {
        let name = self.active_profile.clone();
        if name.is_empty() {
            return Ok(ProfileHealthSnapshot::default());
        }
        self.probe_health(&name).await
    }

    /// Probe health for all loaded profiles concurrently.
    pub async fn probe_all_health(&mut self) -> Vec<(String, ProfileHealthSnapshot)> {
        let configs: Vec<(String, ClientConfig)> = self
            .entries
            .iter()
            .map(|e| (e.name.clone(), e.config.clone()))
            .collect();

        let timeout_ms = self.health_probe_timeout_ms;

        let futures: Vec<_> = configs
            .into_iter()
            .map(|(name, config)| {
                let timeout = timeout_ms;
                async move {
                    let snapshot = Self::probe_config_health(&config, timeout).await;
                    (name, snapshot)
                }
            })
            .collect();

        let probed = futures::future::join_all(futures).await;

        // Update cached health on each entry
        for (name, snapshot) in &probed {
            if let Some(entry) = self.find_mut(name) {
                entry.health = snapshot.clone();
            }
        }

        probed
    }

    /// Get cached health for a profile (without re-probing).
    pub fn cached_health(&self, profile_name: &str) -> Option<&ProfileHealthSnapshot> {
        self.find(profile_name).map(|e| &e.health)
    }

    /// Check if any loaded profile is connected (healthy or degraded).
    pub fn is_any_connected(&self) -> bool {
        self.entries.iter().any(|e| {
            matches!(
                e.health.status,
                ProfileHealthStatus::Healthy | ProfileHealthStatus::Degraded
            )
        })
    }

    /// Set the health probe timeout in milliseconds.
    pub fn set_health_probe_timeout_ms(&mut self, timeout_ms: u64) {
        self.health_probe_timeout_ms = timeout_ms;
    }

    /// Create a ProfileManager from a parsed ProfileConfigFile (for testing).
    pub fn from_profile_file_for_test(file: ProfileConfigFile) -> Self {
        Self::from_profile_file(file).unwrap_or_else(|_| Self::offline())
    }

    /// Probe a single config's endpoints and return a health snapshot.
    async fn probe_config_health(config: &ClientConfig, timeout_ms: u64) -> ProfileHealthSnapshot {
        let timeout_duration = std::time::Duration::from_millis(timeout_ms);

        // Create a config with shortened timeout for probing
        let mut probe_config = config.clone();
        probe_config.orchestration.timeout_ms = timeout_ms;
        probe_config.worker.timeout_ms = timeout_ms;

        // Probe orchestration endpoint
        let orch_healthy = match tokio::time::timeout(timeout_duration, async {
            let client = UnifiedOrchestrationClient::from_config(&probe_config).await?;
            client.health_check().await
        })
        .await
        {
            Ok(Ok(())) => {
                debug!(url = %probe_config.orchestration.base_url, "Orchestration endpoint healthy");
                true
            }
            Ok(Err(e)) => {
                debug!(url = %probe_config.orchestration.base_url, error = %e, "Orchestration endpoint unhealthy");
                false
            }
            Err(_) => {
                debug!(url = %probe_config.orchestration.base_url, "Orchestration endpoint timed out");
                false
            }
        };

        // Probe worker endpoint
        let worker_healthy = match tokio::time::timeout(timeout_duration, async {
            let client = UnifiedWorkerClient::from_config(&probe_config).await?;
            client.health_check().await
        })
        .await
        {
            Ok(Ok(_)) => {
                debug!(url = %probe_config.worker.base_url, "Worker endpoint healthy");
                true
            }
            Ok(Err(e)) => {
                debug!(url = %probe_config.worker.base_url, error = %e, "Worker endpoint unhealthy");
                false
            }
            Err(_) => {
                debug!(url = %probe_config.worker.base_url, "Worker endpoint timed out");
                false
            }
        };

        let status = match (orch_healthy, worker_healthy) {
            (true, true) => ProfileHealthStatus::Healthy,
            (false, false) => ProfileHealthStatus::Unreachable,
            _ => ProfileHealthStatus::Degraded,
        };

        ProfileHealthSnapshot {
            status,
            orchestration_healthy: Some(orch_healthy),
            worker_healthy: Some(worker_healthy),
            last_checked: Some(Instant::now()),
            error_message: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_profile_file(toml_str: &str) -> ProfileConfigFile {
        toml::from_str(toml_str).expect("Failed to parse TOML")
    }

    #[test]
    fn test_backward_compatible_parsing() {
        // Old-style TOML without description or namespaces should parse fine
        let toml = r#"
[profile.default]
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"

[profile.grpc]
transport = "grpc"

[profile.grpc.orchestration]
base_url = "http://localhost:9190"
"#;
        let file = parse_profile_file(toml);
        assert_eq!(file.profile.len(), 2);

        let default = file.profile.get("default").unwrap();
        assert!(default.description.is_none());
        assert!(default.namespaces.is_none());
        assert_eq!(default.transport, Some(Transport::Rest));
    }

    #[test]
    fn test_new_metadata_fields_parse() {
        let toml = r#"
[profile.staging]
description = "Staging - US East"
transport = "grpc"
namespaces = ["orders", "analytics"]

[profile.staging.orchestration]
base_url = "https://staging-orch:9090"
"#;
        let file = parse_profile_file(toml);
        let staging = file.profile.get("staging").unwrap();

        assert_eq!(staging.description.as_deref(), Some("Staging - US East"));
        assert_eq!(
            staging.namespaces.as_deref(),
            Some(&["orders".to_string(), "analytics".to_string()][..])
        );
        assert_eq!(staging.transport, Some(Transport::Grpc));
    }

    #[test]
    fn test_mixed_old_and_new_fields() {
        let toml = r#"
[profile.default]
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"

[profile.staging]
description = "Staging environment"
transport = "grpc"
namespaces = ["default"]

[profile.staging.orchestration]
base_url = "https://staging:9090"
"#;
        let file = parse_profile_file(toml);
        assert_eq!(file.profile.len(), 2);

        // default has no metadata
        let default = file.profile.get("default").unwrap();
        assert!(default.description.is_none());

        // staging has metadata
        let staging = file.profile.get("staging").unwrap();
        assert_eq!(staging.description.as_deref(), Some("Staging environment"));
    }

    #[test]
    fn test_profile_manager_from_profile_file() {
        let toml = r#"
[profile.default]
transport = "rest"

[profile.default.orchestration]
base_url = "http://localhost:8080"

[profile.grpc]
transport = "grpc"

[profile.grpc.orchestration]
base_url = "http://localhost:9190"
"#;
        let file = parse_profile_file(toml);
        let pm = ProfileManager::from_profile_file(file).unwrap();

        assert!(!pm.is_offline());
        assert_eq!(pm.active_profile_name(), "default");

        let names = pm.list_profile_names();
        assert!(names.contains(&"default"));
        assert!(names.contains(&"grpc"));
    }

    #[test]
    fn test_switch_profile() {
        let toml = r#"
[profile.default]
transport = "rest"

[profile.grpc]
transport = "grpc"
"#;
        let file = parse_profile_file(toml);
        let mut pm = ProfileManager::from_profile_file(file).unwrap();

        assert_eq!(pm.active_profile_name(), "default");

        pm.switch_profile("grpc").unwrap();
        assert_eq!(pm.active_profile_name(), "grpc");

        let config = pm.active_config().unwrap();
        assert_eq!(config.transport, Transport::Grpc);
    }

    #[test]
    fn test_switch_profile_nonexistent() {
        let toml = r#"
[profile.default]
transport = "rest"
"#;
        let file = parse_profile_file(toml);
        let mut pm = ProfileManager::from_profile_file(file).unwrap();

        let result = pm.switch_profile("nonexistent");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_list_profiles_with_metadata() {
        let toml = r#"
[profile.default]
description = "Local development"
transport = "rest"
namespaces = ["default"]

[profile.default.orchestration]
base_url = "http://localhost:8080"

[profile.staging]
description = "Staging - US East"
transport = "grpc"
namespaces = ["orders", "analytics"]

[profile.staging.orchestration]
base_url = "https://staging:9090"
"#;
        let file = parse_profile_file(toml);
        let pm = ProfileManager::from_profile_file(file).unwrap();

        let summaries = pm.list_profiles();
        assert_eq!(summaries.len(), 2);

        // Sorted alphabetically
        let default_summary = summaries.iter().find(|s| s.name == "default").unwrap();
        assert_eq!(
            default_summary.description.as_deref(),
            Some("Local development")
        );
        assert!(default_summary.is_active);
        assert_eq!(default_summary.health_status, ProfileHealthStatus::Unknown);
        assert_eq!(
            default_summary.namespaces.as_deref(),
            Some(&["default".to_string()][..])
        );

        let staging_summary = summaries.iter().find(|s| s.name == "staging").unwrap();
        assert_eq!(
            staging_summary.description.as_deref(),
            Some("Staging - US East")
        );
        assert!(!staging_summary.is_active);
        assert_eq!(staging_summary.transport, Transport::Grpc);
    }

    #[test]
    fn test_offline_manager() {
        let pm = ProfileManager::offline();

        assert!(pm.is_offline());
        assert!(pm.active_profile_name().is_empty());
        assert!(pm.active_config().is_none());
        assert!(pm.list_profile_names().is_empty());
        assert!(pm.list_profiles().is_empty());
    }

    #[test]
    fn test_cached_health_initially_unknown() {
        let toml = r#"
[profile.default]
transport = "rest"
"#;
        let file = parse_profile_file(toml);
        let pm = ProfileManager::from_profile_file(file).unwrap();

        let health = pm.cached_health("default").unwrap();
        assert_eq!(health.status, ProfileHealthStatus::Unknown);
        assert!(!pm.is_any_connected());
    }

    #[test]
    fn test_health_status_display() {
        assert_eq!(ProfileHealthStatus::Unknown.to_string(), "unknown");
        assert_eq!(ProfileHealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(ProfileHealthStatus::Degraded.to_string(), "degraded");
        assert_eq!(ProfileHealthStatus::Unreachable.to_string(), "unreachable");
    }

    #[test]
    fn test_get_config_by_name() {
        let toml = r#"
[profile.default]
transport = "rest"

[profile.grpc]
transport = "grpc"
"#;
        let file = parse_profile_file(toml);
        let pm = ProfileManager::from_profile_file(file).unwrap();

        assert!(pm.get_config("default").is_some());
        assert!(pm.get_config("grpc").is_some());
        assert!(pm.get_config("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_probe_health_unreachable() {
        let toml = r#"
[profile.unreachable]

[profile.unreachable.orchestration]
base_url = "http://127.0.0.1:19999"

[profile.unreachable.worker]
base_url = "http://127.0.0.1:19998"
"#;
        let file = parse_profile_file(toml);
        let mut pm = ProfileManager::from_profile_file(file).unwrap();
        pm.set_health_probe_timeout_ms(1000); // Short timeout for tests

        let snapshot = pm.probe_health("unreachable").await.unwrap();
        assert_eq!(snapshot.status, ProfileHealthStatus::Unreachable);
        assert_eq!(snapshot.orchestration_healthy, Some(false));
        assert_eq!(snapshot.worker_healthy, Some(false));
        assert!(snapshot.last_checked.is_some());

        // Verify cached
        let cached = pm.cached_health("unreachable").unwrap();
        assert_eq!(cached.status, ProfileHealthStatus::Unreachable);
    }

    #[tokio::test]
    async fn test_probe_health_nonexistent_profile() {
        let toml = r#"
[profile.default]
transport = "rest"
"#;
        let file = parse_profile_file(toml);
        let mut pm = ProfileManager::from_profile_file(file).unwrap();

        let result = pm.probe_health("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_probe_all_health() {
        let toml = r#"
[profile.one]
[profile.one.orchestration]
base_url = "http://127.0.0.1:19999"
[profile.one.worker]
base_url = "http://127.0.0.1:19997"

[profile.two]
[profile.two.orchestration]
base_url = "http://127.0.0.1:19998"
[profile.two.worker]
base_url = "http://127.0.0.1:19996"
"#;
        let file = parse_profile_file(toml);
        let mut pm = ProfileManager::from_profile_file(file).unwrap();
        pm.set_health_probe_timeout_ms(1000);

        let results = pm.probe_all_health().await;
        assert_eq!(results.len(), 2);

        let names: Vec<&str> = results.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"one"));
        assert!(names.contains(&"two"));

        // Both should be unreachable since no services are running
        for (_, snapshot) in &results {
            assert_eq!(snapshot.status, ProfileHealthStatus::Unreachable);
        }
    }

    #[test]
    fn test_set_health_probe_timeout() {
        let mut pm = ProfileManager::offline();
        assert_eq!(
            pm.health_probe_timeout_ms,
            ProfileManager::DEFAULT_HEALTH_PROBE_TIMEOUT_MS
        );

        pm.set_health_probe_timeout_ms(10_000);
        assert_eq!(pm.health_probe_timeout_ms, 10_000);
    }

    #[test]
    fn test_first_profile_becomes_active_when_no_default() {
        let toml = r#"
[profile.staging]
transport = "grpc"

[profile.production]
transport = "grpc"
"#;
        let file = parse_profile_file(toml);
        let pm = ProfileManager::from_profile_file(file).unwrap();

        // Entries are sorted alphabetically — "production" comes first
        assert_eq!(pm.active_profile_name(), "production");
    }
}
