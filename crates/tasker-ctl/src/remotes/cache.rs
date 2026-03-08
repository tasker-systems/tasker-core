//! Cache management for remote git repositories.
//!
//! Remotes are cached at `~/.cache/tasker-ctl/remotes/<name>/`.
//! A `.tasker-last-fetch` timestamp file tracks when the cache was last updated.
//! Stale caches produce warnings but still work (offline-friendly).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::cli_config::RemoteConfig;

use super::error::RemoteError;
use super::git;

const CACHE_DIR_NAME: &str = "tasker-ctl";
const REMOTES_DIR_NAME: &str = "remotes";
const LAST_FETCH_FILE: &str = ".tasker-last-fetch";

/// Manages cached clones of remote git repositories.
#[derive(Debug)]
pub(crate) struct RemoteCache;

impl RemoteCache {
    /// Base directory for all cached remotes: `~/.cache/tasker-ctl/remotes/`
    pub(crate) fn cache_base() -> Result<PathBuf, RemoteError> {
        let cache_dir = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
            PathBuf::from(xdg)
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".cache")
        } else {
            return Err(RemoteError::CacheError(
                "Cannot determine cache directory: neither XDG_CACHE_HOME nor HOME is set"
                    .to_string(),
            ));
        };

        Ok(cache_dir.join(CACHE_DIR_NAME).join(REMOTES_DIR_NAME))
    }

    /// Cache directory for a named remote.
    pub(crate) fn cache_dir(name: &str) -> Result<PathBuf, RemoteError> {
        Ok(Self::cache_base()?.join(name))
    }

    /// Resolve a remote to a local cached path.
    ///
    /// - If cached and fresh: return path immediately.
    /// - If cached but stale: warn and return path.
    /// - If not cached: clone and return path.
    /// - If clone fails but stale cache exists: warn and return stale path (offline mode).
    pub(crate) fn resolve(
        remote: &RemoteConfig,
        max_age_hours: u64,
    ) -> Result<PathBuf, RemoteError> {
        let cache = Self::cache_dir(&remote.name)?;

        if cache.exists() {
            // Cache exists — check staleness
            if Self::is_stale(&cache, max_age_hours) {
                tracing::warn!(
                    remote = %remote.name,
                    "Remote cache is stale (older than {}h). Run `tasker-ctl remote update {}` to refresh.",
                    max_age_hours,
                    remote.name
                );
            }
            return Ok(cache);
        }

        // No cache — clone
        Self::ensure_git()?;
        std::fs::create_dir_all(Self::cache_base()?)?;

        match git::shallow_clone(&remote.url, &remote.git_ref, &cache) {
            Ok(()) => {
                Self::write_timestamp(&cache)?;
                Ok(cache)
            }
            Err(e) => {
                // Clean up partial clone
                let _ = std::fs::remove_dir_all(&cache);
                Err(e)
            }
        }
    }

    /// Force fetch and checkout latest for a remote.
    pub(crate) fn update(remote: &RemoteConfig) -> Result<PathBuf, RemoteError> {
        let cache = Self::cache_dir(&remote.name)?;

        Self::ensure_git()?;

        if cache.exists() {
            git::fetch_and_checkout(&cache, &remote.git_ref)?;
            Self::write_timestamp(&cache)?;
            Ok(cache)
        } else {
            // Not cached yet — do initial clone
            std::fs::create_dir_all(Self::cache_base()?)?;
            git::shallow_clone(&remote.url, &remote.git_ref, &cache)?;
            Self::write_timestamp(&cache)?;
            Ok(cache)
        }
    }

    /// Resolve an ad-hoc URL (not configured in .tasker-ctl.toml) to a cached path.
    /// Hashes the URL to a deterministic cache directory name.
    pub(crate) fn resolve_url(
        url: &str,
        git_ref: Option<&str>,
        max_age_hours: u64,
    ) -> Result<PathBuf, RemoteError> {
        let effective_ref = git_ref.unwrap_or("main");

        // Hash URL to deterministic name
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();
        let cache_name = format!("_url_{:016x}", hash);

        let remote = RemoteConfig {
            name: cache_name,
            url: url.to_string(),
            git_ref: effective_ref.to_string(),
            ..RemoteConfig::default()
        };

        Self::resolve(&remote, max_age_hours)
    }

    /// Remove a cached remote.
    pub(crate) fn remove(name: &str) -> Result<(), RemoteError> {
        let cache = Self::cache_dir(name)?;
        if cache.exists() {
            std::fs::remove_dir_all(&cache)?;
        }
        Ok(())
    }

    /// List all cached remote names.
    #[expect(dead_code, reason = "Available for remote list/diagnostics")]
    pub(crate) fn list_cached() -> Result<Vec<String>, RemoteError> {
        let base = Self::cache_base()?;
        if !base.exists() {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        for entry in std::fs::read_dir(base)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Check whether the cached remote has a last-fetch timestamp older than max_age.
    fn is_stale(cache_dir: &Path, max_age_hours: u64) -> bool {
        let timestamp_file = cache_dir.join(LAST_FETCH_FILE);
        match std::fs::read_to_string(&timestamp_file) {
            Ok(contents) => {
                let ts: u64 = contents.trim().parse().unwrap_or(0);
                let fetch_time = SystemTime::UNIX_EPOCH + Duration::from_secs(ts);
                let age = SystemTime::now()
                    .duration_since(fetch_time)
                    .unwrap_or(Duration::MAX);
                age > Duration::from_secs(max_age_hours * 3600)
            }
            Err(_) => true, // No timestamp means stale
        }
    }

    /// Write the current timestamp to the cache's last-fetch file.
    fn write_timestamp(cache_dir: &Path) -> Result<(), RemoteError> {
        let timestamp_file = cache_dir.join(LAST_FETCH_FILE);
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        std::fs::write(&timestamp_file, now.to_string())?;
        Ok(())
    }

    /// Check that git is available, returning a clear error if not.
    fn ensure_git() -> Result<(), RemoteError> {
        if !git::git_available() {
            return Err(RemoteError::CacheError(
                "git is not installed or not in PATH. Install git to use remote repositories."
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// Get the last fetch time for a cache directory, if available.
    pub(crate) fn last_fetch_time(cache_dir: &Path) -> Option<SystemTime> {
        let timestamp_file = cache_dir.join(LAST_FETCH_FILE);
        let contents = std::fs::read_to_string(timestamp_file).ok()?;
        let ts: u64 = contents.trim().parse().ok()?;
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(ts))
    }
}
