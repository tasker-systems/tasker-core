# TAS-357: S1 SecretsProvider Foundation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the `tasker-secure` crate with secrets resolution layer — `SecretValue`, `SecretsProvider` trait, `EnvSecretsProvider`, `ChainedSecretsProvider`, `SopsSecretsProvider`, `ConfigString` type, and annotated config documentation.

**Architecture:** Strategy pattern for secrets resolution. `SecretValue` wraps `secrecy::SecretBox<str>` (v0.10) for zeroize-on-drop protection. Providers implement the `SecretsProvider` async trait. The crate has no dependency on `tasker-shared` — it is framework-agnostic and independently testable. Feature gates isolate optional backends (`sops`, future `vault`/`aws-*`).

**Tech Stack:** Rust, secrecy 0.10, zeroize 1.x, rops 0.1 (feature-gated), async-trait, tokio, thiserror, serde/serde_json/serde_yaml, cargo-make, nextest

---

## Reference Documents

- Design spec: `docs/research/security-and-secrets/01-secrets-and-credential-injection.md`
- Problem statement: `docs/research/security-and-secrets/00-problem-statement.md`
- Crate proposal: `docs/research/security-and-secrets/05-tasker-secure-crate-proposal.md`
- Annotated configs: `docs/generated/annotated-production.toml`, `docs/generated/annotated-development.toml`
- Config loader: `crates/tasker-shared/src/config/config_loader.rs`

## Key API Decisions

- **secrecy 0.10.x** (not 0.8 as in original design doc): `SecretString = SecretBox<str>`, `expose_secret()` returns `&str`, Debug prints `SecretBox<str>([REDACTED])`. Our `SecretValue` wrapper adds `Display` (→ `[REDACTED]`) and a convenience constructor.
- **rops 0.1.7**: 0% docs.rs coverage. API exploration required during Task 6. The crate supports age, AWS KMS, GCP KMS backends and reads standard SOPS-format files.
- **No tasker-shared dependency**: `tasker-secure` is framework-agnostic. `ConfigString` lives in `tasker-secure` for now; integration into `tasker-shared` config loading is deferred to S2.

---

## Task 1: Crate Scaffolding

**Files:**
- Create: `crates/tasker-secure/Cargo.toml`
- Create: `crates/tasker-secure/src/lib.rs`
- Create: `crates/tasker-secure/src/secrets/mod.rs`
- Create: `crates/tasker-secure/src/secrets/value.rs`
- Create: `crates/tasker-secure/src/resource/mod.rs`
- Create: `crates/tasker-secure/src/classification/mod.rs`
- Create: `crates/tasker-secure/src/encryption/mod.rs`
- Create: `crates/tasker-secure/src/testing/mod.rs`
- Create: `crates/tasker-secure/Makefile.toml`
- Modify: `Cargo.toml` (root workspace — add member + workspace deps)

**Step 1: Create `crates/tasker-secure/Cargo.toml`**

```toml
[package]
name = "tasker-secure"
version = "0.1.6"
edition = "2021"
description = "Strategy-pattern secrets resolution, resource lifecycle, and data protection for Tasker workflows"
readme = "README.md"
repository = "https://github.com/tasker-systems/tasker-core"
license = "MIT"
keywords = ["secrets", "security", "credentials", "workflow", "orchestration"]
categories = ["authentication", "cryptography"]

[lib]
crate-type = ["rlib"]
name = "tasker_secure"

[features]
default = []

# Secrets provider backends
sops = ["dep:rops", "dep:serde_yaml"]

# Testing utilities
test-utils = []

[dependencies]
# Always required
serde = { workspace = true }
serde_json = { workspace = true }
async-trait = { workspace = true }
tokio = { workspace = true, features = ["rt", "sync"] }
thiserror = { workspace = true }
tracing = { workspace = true }

# SecretValue — always required
secrecy = { workspace = true }
zeroize = { workspace = true }

# SOPS file support (feature-gated)
rops = { version = "0.1", optional = true }
serde_yaml = { workspace = true, optional = true }

[dev-dependencies]
tokio = { workspace = true, features = ["full", "test-util"] }
serde_json = { workspace = true }

[lints]
workspace = true
```

**Step 2: Add workspace dependencies to root `Cargo.toml`**

Add these entries to the `[workspace.dependencies]` section in the root `Cargo.toml`:

```toml
# SecretValue support (tasker-secure)
secrecy = { version = "0.10", features = ["serde"] }
zeroize = "1.8"
```

Add `"crates/tasker-secure"` to the `[workspace] members` list, after the `tasker-grammar` entry.

**Step 3: Create `crates/tasker-secure/src/lib.rs`**

```rust
//! Strategy-pattern secrets resolution for Tasker workflow orchestration.
//!
//! This crate provides the secrets management layer that capability executors
//! (`acquire`, `persist`, `emit`) use to access external systems without
//! credentials entering data paths. Secrets are resolved at startup through
//! provider implementations and never appear in composition configs, jaq
//! contexts, step results, or trace spans.
//!
//! # Module structure
//!
//! - [`secrets`] — `SecretsProvider` trait, `SecretValue`, provider implementations
//! - [`config`] — `ConfigString` type for secrets-aware configuration values
//! - [`resource`] — stub module (implementations in S2)
//! - [`classification`] — stub module (implementations in S4)
//! - [`encryption`] — stub module (implementations in S3)
//! - [`testing`] — test utilities (feature-gated)
//!
//! # Design principles
//!
//! - **No dependency on `tasker-shared`**: framework-agnostic, independently testable
//! - **Strategy pattern**: organizations bring their own secrets backend
//! - **Opaque secret values**: `SecretValue` prevents accidental exposure via Display/Debug
//! - **Resolve once at startup**: capability executors receive handles, not secrets

pub mod secrets;

pub mod config;

/// Resource lifecycle management (S2).
pub mod resource;

/// Data classification for trace/log safety (S4).
pub mod classification;

/// Encryption at rest for task data (S3).
pub mod encryption;

/// Test utilities for downstream crates.
#[cfg(any(test, feature = "test-utils"))]
pub mod testing;

// Re-exports
pub use config::ConfigString;
pub use secrets::{
    ChainedSecretsProvider, EnvSecretsProvider, SecretValue, SecretsError, SecretsProvider,
};

#[cfg(feature = "sops")]
pub use secrets::SopsSecretsProvider;
```

**Step 4: Create stub modules**

`crates/tasker-secure/src/resource/mod.rs`:
```rust
//! Resource registry and handle lifecycle (S2: TAS-358).
//!
//! This module will provide `ResourceHandle`, `ResourceRegistry`, and
//! `ResourceDefinition` types for managing named external resources
//! (database pools, HTTP clients, PGMQ connections) with credentials
//! resolved through `SecretsProvider`.
```

`crates/tasker-secure/src/classification/mod.rs`:
```rust
//! Data classification for trace and log safety (S4: TAS-360).
//!
//! This module will provide `DataClassifier` and `DataClassificationSpec`
//! for field-level PII redaction at observability emission points.
```

`crates/tasker-secure/src/encryption/mod.rs`:
```rust
//! Encryption at rest for task data (S3: TAS-359).
//!
//! This module will provide `EncryptionProvider` trait and implementations
//! for field-level encryption of task context and step results.
```

`crates/tasker-secure/src/testing/mod.rs`:
```rust
//! Test utilities for `tasker-secure` consumers.
//!
//! Provides in-memory secrets providers and fixture helpers for
//! unit testing code that depends on `SecretsProvider`.

mod mock_secrets;

pub use mock_secrets::InMemorySecretsProvider;
```

`crates/tasker-secure/src/secrets/mod.rs` (initial, will be expanded in Tasks 2-5):
```rust
//! Secrets provider trait and implementations.

mod value;

pub use value::SecretValue;
```

`crates/tasker-secure/src/secrets/value.rs` (empty placeholder):
```rust
//! SecretValue type — opaque wrapper for sensitive strings.
```

`crates/tasker-secure/src/config/mod.rs` (empty placeholder):
```rust
//! Configuration value types with secrets resolution support.
```

`crates/tasker-secure/src/testing/mock_secrets.rs` (empty placeholder):
```rust
//! In-memory secrets provider for testing.
```

**Step 5: Create `crates/tasker-secure/Makefile.toml`**

```toml
# =============================================================================
# tasker-secure - cargo-make Task Definitions
# =============================================================================
#
# Secrets resolution, resource lifecycle, and data protection.
# Core module has no infrastructure dependencies; SOPS feature requires
# test fixtures only.
#
# Quick Start:
#   cargo make check    # Run all quality checks
#   cargo make test     # Run tests (default features)
#   cargo make test-all # Run tests including feature-gated providers
#   cargo make fix      # Auto-fix issues
#
# =============================================================================

extend = "../../tools/cargo-make/base-tasks.toml"

[config]
default_to_workspace = false

[env]
CRATE_NAME = "tasker-secure"

# =============================================================================
# Main Tasks
# =============================================================================

[tasks.default]
alias = "check"

[tasks.check]
description = "Run quality checks"
dependencies = ["format-check", "lint", "test"]

[tasks.format-check]
extend = "base-rust-format"

[tasks.format-fix]
extend = "base-rust-format-fix"

[tasks.lint]
extend = "base-rust-lint"

[tasks.lint-fix]
extend = "base-rust-lint-fix"

[tasks.test]
extend = "base-rust-test"
description = "Run tasker-secure tests (default features)"
args = ["nextest", "run", "-p", "${CRATE_NAME}"]

[tasks.test-all]
description = "Run tasker-secure tests with all features"
command = "cargo"
args = ["nextest", "run", "-p", "${CRATE_NAME}", "--all-features"]

[tasks.fix]
description = "Fix all fixable issues"
dependencies = ["format-fix", "lint-fix"]

[tasks.clean]
description = "Clean build artifacts"
command = "cargo"
args = ["clean", "-p", "${CRATE_NAME}"]
```

**Step 6: Verify scaffolding compiles**

Run: `cargo check -p tasker-secure`
Expected: Compiles successfully (empty stubs, no logic yet)

Run: `cargo check -p tasker-secure --all-features`
Expected: Compiles successfully with sops feature

**Step 7: Commit**

```bash
git add crates/tasker-secure/ Cargo.toml Cargo.lock
git commit -m "feat(TAS-357): scaffold tasker-secure crate with module structure

New workspace member at crates/tasker-secure/ with secrets, resource,
classification, encryption, and testing module stubs. Feature gates
for sops backend. No implementation yet — structure only."
```

---

## Task 2: SecretValue Type

**Files:**
- Modify: `crates/tasker-secure/src/secrets/value.rs`
- Create: `crates/tasker-secure/tests/secret_value_test.rs`

**Step 1: Write the failing tests**

Create `crates/tasker-secure/tests/secret_value_test.rs`:

```rust
use tasker_secure::SecretValue;

#[test]
fn display_is_redacted() {
    let val = SecretValue::new("super-secret-password");
    assert_eq!(format!("{val}"), "[REDACTED]");
}

#[test]
fn debug_is_redacted() {
    let val = SecretValue::new("super-secret-password");
    let debug_output = format!("{val:?}");
    assert!(!debug_output.contains("super-secret-password"));
    assert!(debug_output.contains("REDACTED"));
}

#[test]
fn expose_secret_returns_actual_value() {
    let val = SecretValue::new("super-secret-password");
    assert_eq!(val.expose_secret(), "super-secret-password");
}

#[test]
fn secret_value_from_string() {
    let val = SecretValue::from(String::from("from-owned-string"));
    assert_eq!(val.expose_secret(), "from-owned-string");
}

#[test]
fn secret_value_debug_never_leaks_in_struct() {
    let val = SecretValue::new("my-api-key");
    let debug = format!("{val:?}");
    assert!(!debug.contains("my-api-key"), "Debug output must never contain the secret value");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p tasker-secure --test secret_value_test`
Expected: FAIL — `SecretValue` type doesn't exist yet

**Step 3: Implement SecretValue**

Update `crates/tasker-secure/src/secrets/value.rs`:

```rust
//! `SecretValue` — opaque wrapper for sensitive strings.
//!
//! Wraps `secrecy::SecretString` (which is `SecretBox<str>`) to ensure
//! secret values are zeroized on drop and never accidentally exposed
//! through `Display` or `Debug` formatting.

use std::fmt;

use secrecy::{ExposeSecret, SecretString};

/// An opaque wrapper around a secret string value.
///
/// - `Display` and `Debug` both emit `[REDACTED]`
/// - The only way to access the underlying value is `expose_secret()`
/// - Memory is zeroized on drop (via `secrecy` + `zeroize`)
///
/// The method name `expose_secret()` creates intentional friction at code
/// review — any diff adding it in a logging context is immediately visible
/// as a potential credential leak.
pub struct SecretValue(SecretString);

impl SecretValue {
    /// Create a new `SecretValue` from a string slice.
    pub fn new(secret: &str) -> Self {
        Self(SecretString::from(Box::<str>::from(secret)))
    }

    /// Access the underlying secret value.
    ///
    /// This method name is intentionally conspicuous to make code review
    /// of credential access points obvious.
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl From<String> for SecretValue {
    fn from(s: String) -> Self {
        Self(SecretString::from(Box::<str>::from(s)))
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SecretValue").field(&"[REDACTED]").finish()
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p tasker-secure --test secret_value_test`
Expected: All 5 tests PASS

**Step 5: Run clippy**

Run: `cargo clippy -p tasker-secure --all-targets --all-features`
Expected: Zero warnings

**Step 6: Commit**

```bash
git add crates/tasker-secure/src/secrets/value.rs crates/tasker-secure/tests/secret_value_test.rs
git commit -m "feat(TAS-357): implement SecretValue with redacted Display/Debug

Wraps secrecy::SecretString (SecretBox<str>) for zeroize-on-drop.
Display and Debug both emit [REDACTED]. expose_secret() is the only
access path, creating intentional friction at code review."
```

---

## Task 3: SecretsProvider Trait and SecretsError

**Files:**
- Modify: `crates/tasker-secure/src/secrets/mod.rs`
- Create: `crates/tasker-secure/src/secrets/error.rs`

**Step 1: Create SecretsError**

Create `crates/tasker-secure/src/secrets/error.rs`:

```rust
//! Error types for secrets resolution.

use std::fmt;

/// Errors that can occur when resolving secrets through a provider.
#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    /// The requested secret path does not exist in the provider.
    #[error("secret not found: {path}")]
    NotFound {
        /// The secret path that was not found.
        path: String,
    },

    /// The caller does not have permission to access this secret.
    #[error("access denied for secret: {path}")]
    AccessDenied {
        /// The secret path that was denied.
        path: String,
    },

    /// The secrets provider is unavailable (network error, auth failure, etc.).
    #[error("secrets provider unavailable: {message}")]
    ProviderUnavailable {
        /// Description of the unavailability.
        message: String,
    },

    /// The secret path is malformed or invalid for this provider.
    #[error("invalid secret path '{path}': {reason}")]
    InvalidPath {
        /// The invalid path.
        path: String,
        /// Why the path is invalid.
        reason: String,
    },
}
```

**Step 2: Define SecretsProvider trait**

Update `crates/tasker-secure/src/secrets/mod.rs`:

```rust
//! Secrets provider trait and implementations.
//!
//! The `SecretsProvider` trait defines how Tasker resolves secret references
//! to their values. Implementations talk to specific backends (environment
//! variables, SOPS files, Vault, AWS SSM, etc.).
//!
//! Secrets are resolved at startup or pool initialization time, not during
//! step execution. Capability executors never receive secret values — they
//! receive already-initialized resource handles.

mod chained;
mod env;
mod error;
mod value;

#[cfg(feature = "sops")]
pub mod sops;

use std::collections::HashMap;
use std::fmt;

pub use chained::ChainedSecretsProvider;
pub use env::EnvSecretsProvider;
pub use error::SecretsError;
pub use value::SecretValue;

#[cfg(feature = "sops")]
pub use self::sops::SopsSecretsProvider;

/// A secrets provider resolves named secret references to their values.
///
/// Implementations talk to specific backends (Vault, AWS SSM, SOPS files,
/// environment variables, etc.). Tasker does not store secrets — it resolves
/// references through provider implementations.
///
/// Secrets are resolved at startup or pool initialization time, not during
/// step execution. Capability executors never receive secret values — they
/// receive already-initialized resource handles.
#[async_trait::async_trait]
pub trait SecretsProvider: Send + Sync + fmt::Debug {
    /// Resolve a single secret by its path or identifier.
    ///
    /// Path format is provider-defined:
    ///   - Env var:   `"DATABASE_PASSWORD"`
    ///   - SOPS:      `"database.password"`
    ///   - Vault:     `"secret/data/production/tasker/orders-db#password"`
    ///   - AWS SSM:   `"/production/tasker/orders-db/password"`
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError>;

    /// Resolve multiple secrets in a single call.
    ///
    /// Default implementation calls `get_secret` for each path.
    /// Implementations should override to batch round trips where the backend supports it.
    async fn get_secrets(
        &self,
        paths: &[&str],
    ) -> Result<HashMap<String, SecretValue>, SecretsError> {
        let mut results = HashMap::with_capacity(paths.len());
        for &path in paths {
            let value = self.get_secret(path).await?;
            results.insert(path.to_string(), value);
        }
        Ok(results)
    }

    /// Provider identity — used in diagnostic messages only.
    fn provider_name(&self) -> &str;

    /// Verify the provider is reachable and the configuration is valid.
    /// Called at worker startup before resource initialization begins.
    async fn health_check(&self) -> Result<(), SecretsError>;
}
```

**Step 3: Create empty implementation files (to satisfy `mod` declarations)**

Create `crates/tasker-secure/src/secrets/env.rs`:
```rust
//! Environment variable secrets provider.
```

Create `crates/tasker-secure/src/secrets/chained.rs`:
```rust
//! Chained secrets provider — tries providers in priority order.
```

Create `crates/tasker-secure/src/secrets/sops.rs`:
```rust
//! SOPS-encrypted file secrets provider using the `rops` crate.
```

**Step 4: Verify compilation**

Run: `cargo check -p tasker-secure --all-features`
Expected: Compiles (trait defined, empty impl files)

**Step 5: Commit**

```bash
git add crates/tasker-secure/src/secrets/
git commit -m "feat(TAS-357): define SecretsProvider trait and SecretsError

Object-safe async trait with get_secret, get_secrets (with default batch
impl), provider_name, and health_check. SecretsError has NotFound,
AccessDenied, ProviderUnavailable, and InvalidPath variants."
```

---

## Task 4: EnvSecretsProvider

**Files:**
- Modify: `crates/tasker-secure/src/secrets/env.rs`
- Create: `crates/tasker-secure/tests/env_provider_test.rs`

**Step 1: Write the failing tests**

Create `crates/tasker-secure/tests/env_provider_test.rs`:

```rust
use tasker_secure::{EnvSecretsProvider, SecretsError, SecretsProvider};

#[tokio::test]
async fn resolves_env_var_without_prefix() {
    std::env::set_var("TEST_SECRET_NO_PREFIX", "my-secret-value");
    let provider = EnvSecretsProvider::new(None);
    let result = provider.get_secret("TEST_SECRET_NO_PREFIX").await.unwrap();
    assert_eq!(result.expose_secret(), "my-secret-value");
    std::env::remove_var("TEST_SECRET_NO_PREFIX");
}

#[tokio::test]
async fn resolves_env_var_with_prefix() {
    std::env::set_var("TASKER_SECRET_DB_PASSWORD", "s3cret");
    let provider = EnvSecretsProvider::with_prefix("TASKER_SECRET_");
    let result = provider.get_secret("DB_PASSWORD").await.unwrap();
    assert_eq!(result.expose_secret(), "s3cret");
    std::env::remove_var("TASKER_SECRET_DB_PASSWORD");
}

#[tokio::test]
async fn normalizes_path_separators() {
    std::env::set_var("TASKER_SECRET_ORDERS_DB_PASSWORD", "normalized");
    let provider = EnvSecretsProvider::with_prefix("TASKER_SECRET_");
    let result = provider
        .get_secret("orders-db/password")
        .await
        .unwrap();
    assert_eq!(result.expose_secret(), "normalized");
    std::env::remove_var("TASKER_SECRET_ORDERS_DB_PASSWORD");
}

#[tokio::test]
async fn missing_var_returns_not_found() {
    let provider = EnvSecretsProvider::new(None);
    let result = provider.get_secret("DEFINITELY_NOT_SET_12345").await;
    assert!(matches!(result, Err(SecretsError::NotFound { .. })));
}

#[tokio::test]
async fn health_check_always_ok() {
    let provider = EnvSecretsProvider::new(None);
    assert!(provider.health_check().await.is_ok());
}

#[tokio::test]
async fn provider_name_is_env() {
    let provider = EnvSecretsProvider::new(None);
    assert_eq!(provider.provider_name(), "env");
}

#[tokio::test]
async fn get_secrets_batch() {
    std::env::set_var("BATCH_A", "val_a");
    std::env::set_var("BATCH_B", "val_b");
    let provider = EnvSecretsProvider::new(None);
    let results = provider.get_secrets(&["BATCH_A", "BATCH_B"]).await.unwrap();
    assert_eq!(results["BATCH_A"].expose_secret(), "val_a");
    assert_eq!(results["BATCH_B"].expose_secret(), "val_b");
    std::env::remove_var("BATCH_A");
    std::env::remove_var("BATCH_B");
}

#[tokio::test]
async fn normalization_handles_mixed_case_and_hyphens() {
    std::env::set_var("TASKER_SECRET_MY_API_KEY", "key-value");
    let provider = EnvSecretsProvider::with_prefix("TASKER_SECRET_");
    let result = provider.get_secret("my-api-key").await.unwrap();
    assert_eq!(result.expose_secret(), "key-value");
    std::env::remove_var("TASKER_SECRET_MY_API_KEY");
}

#[tokio::test]
async fn resolves_database_url_from_env() {
    let url = "postgresql://tasker:tasker@localhost:5432/tasker_test";
    std::env::set_var("TEST_DATABASE_URL_FOR_SECURE", url);
    let provider = EnvSecretsProvider::new(None);
    let result = provider.get_secret("TEST_DATABASE_URL_FOR_SECURE").await.unwrap();
    assert_eq!(result.expose_secret(), url);
    std::env::remove_var("TEST_DATABASE_URL_FOR_SECURE");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p tasker-secure --test env_provider_test`
Expected: FAIL — `EnvSecretsProvider` not implemented

**Step 3: Implement EnvSecretsProvider**

Update `crates/tasker-secure/src/secrets/env.rs`:

```rust
//! Environment variable secrets provider.
//!
//! Maps secret paths to environment variable names. Optional prefix
//! stripping and path normalization (uppercase, `/` and `-` → `_`).
//!
//! Use cases:
//! - Local development
//! - Docker Compose and simple container environments
//! - Migration path from existing env-var-only configurations

use super::{SecretValue, SecretsError, SecretsProvider};

/// Reads secrets from environment variables.
///
/// With prefix `"TASKER_SECRET_"`:
///   path `"orders-db/password"` → env var `"TASKER_SECRET_ORDERS_DB_PASSWORD"`
#[derive(Debug)]
pub struct EnvSecretsProvider {
    prefix: Option<String>,
}

impl EnvSecretsProvider {
    /// Create a provider that reads env vars directly (no prefix, no normalization).
    pub fn new(prefix: Option<String>) -> Self {
        Self { prefix }
    }

    /// Create a provider with a prefix. Paths are normalized (uppercase, `/` and `-` → `_`)
    /// before the prefix is prepended.
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            prefix: Some(prefix.to_string()),
        }
    }

    /// Normalize a secret path to an environment variable name.
    ///
    /// Rules:
    /// - Replace `/`, `-`, and `.` with `_`
    /// - Convert to uppercase
    /// - Prepend prefix if configured
    fn normalize_path(&self, path: &str) -> String {
        match &self.prefix {
            Some(prefix) => {
                let normalized = path
                    .replace(['/', '-', '.'], "_")
                    .to_uppercase();
                format!("{prefix}{normalized}")
            }
            None => path.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl SecretsProvider for EnvSecretsProvider {
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError> {
        let var_name = self.normalize_path(path);
        match std::env::var(&var_name) {
            Ok(value) => Ok(SecretValue::from(value)),
            Err(std::env::VarError::NotPresent) => Err(SecretsError::NotFound {
                path: path.to_string(),
            }),
            Err(std::env::VarError::NotUnicode(_)) => Err(SecretsError::InvalidPath {
                path: path.to_string(),
                reason: format!("environment variable '{var_name}' contains invalid UTF-8"),
            }),
        }
    }

    fn provider_name(&self) -> &str {
        "env"
    }

    async fn health_check(&self) -> Result<(), SecretsError> {
        Ok(())
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p tasker-secure --test env_provider_test`
Expected: All 9 tests PASS

**Step 5: Run clippy**

Run: `cargo clippy -p tasker-secure --all-targets --all-features`
Expected: Zero warnings

**Step 6: Commit**

```bash
git add crates/tasker-secure/src/secrets/env.rs crates/tasker-secure/tests/env_provider_test.rs
git commit -m "feat(TAS-357): implement EnvSecretsProvider

Reads secrets from environment variables with optional prefix and
path normalization (uppercase, separators → underscore). health_check
always Ok — env is always available."
```

---

## Task 5: ChainedSecretsProvider

**Files:**
- Modify: `crates/tasker-secure/src/secrets/chained.rs`
- Create: `crates/tasker-secure/tests/chained_provider_test.rs`
- Modify: `crates/tasker-secure/src/testing/mock_secrets.rs`

**Step 1: Implement InMemorySecretsProvider (test utility)**

Update `crates/tasker-secure/src/testing/mock_secrets.rs`:

```rust
//! In-memory secrets provider for testing.

use std::collections::HashMap;

use crate::secrets::{SecretValue, SecretsError, SecretsProvider};

/// A secrets provider backed by an in-memory HashMap.
///
/// Useful for unit testing code that depends on `SecretsProvider`
/// without requiring real secrets infrastructure.
#[derive(Debug)]
pub struct InMemorySecretsProvider {
    secrets: HashMap<String, String>,
    name: String,
}

impl InMemorySecretsProvider {
    /// Create a new in-memory provider with the given secrets.
    pub fn new(secrets: HashMap<String, String>) -> Self {
        Self {
            secrets,
            name: "in-memory".to_string(),
        }
    }

    /// Create a provider with a custom name (useful for chaining tests).
    pub fn with_name(name: &str, secrets: HashMap<String, String>) -> Self {
        Self {
            secrets,
            name: name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl SecretsProvider for InMemorySecretsProvider {
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError> {
        self.secrets
            .get(path)
            .map(|v| SecretValue::from(v.clone()))
            .ok_or_else(|| SecretsError::NotFound {
                path: path.to_string(),
            })
    }

    fn provider_name(&self) -> &str {
        &self.name
    }

    async fn health_check(&self) -> Result<(), SecretsError> {
        Ok(())
    }
}
```

**Step 2: Write the failing tests for ChainedSecretsProvider**

Create `crates/tasker-secure/tests/chained_provider_test.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;

use tasker_secure::{ChainedSecretsProvider, SecretsError, SecretsProvider};
use tasker_secure::testing::InMemorySecretsProvider;

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
```

**Step 3: Run tests to verify they fail**

Run: `cargo nextest run -p tasker-secure --test chained_provider_test`
Expected: FAIL — `ChainedSecretsProvider` not implemented

**Step 4: Implement ChainedSecretsProvider**

Update `crates/tasker-secure/src/secrets/chained.rs`:

```rust
//! Chained secrets provider — tries providers in priority order.
//!
//! First success wins; all must fail for an error to be returned.
//! The last error in the chain is the one surfaced to the caller.
//!
//! Use cases:
//! - **Migration**: env vars → Vault transition (try Vault first, fall back to env)
//! - **Multi-environment**: SOPS in dev, AWS SSM in prod
//! - **Defense in depth**: primary + backup provider

use std::collections::HashMap;
use std::sync::Arc;

use super::{SecretValue, SecretsError, SecretsProvider};

/// Tries providers in priority order. First success wins.
#[derive(Debug)]
pub struct ChainedSecretsProvider {
    providers: Vec<Arc<dyn SecretsProvider>>,
    name: String,
}

impl ChainedSecretsProvider {
    /// Create a chained provider from a list of providers in priority order.
    pub fn new(providers: Vec<Arc<dyn SecretsProvider>>) -> Self {
        let name = format!(
            "chain[{}]",
            providers
                .iter()
                .map(|p| p.provider_name().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        Self { providers, name }
    }
}

#[async_trait::async_trait]
impl SecretsProvider for ChainedSecretsProvider {
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError> {
        if self.providers.is_empty() {
            return Err(SecretsError::ProviderUnavailable {
                message: "no providers configured in chain".to_string(),
            });
        }

        let mut last_error = None;
        for provider in &self.providers {
            match provider.get_secret(path).await {
                Ok(value) => return Ok(value),
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error.expect("at least one provider exists"))
    }

    async fn get_secrets(
        &self,
        paths: &[&str],
    ) -> Result<HashMap<String, SecretValue>, SecretsError> {
        // Resolve each path independently through the chain
        let mut results = HashMap::with_capacity(paths.len());
        for &path in paths {
            let value = self.get_secret(path).await?;
            results.insert(path.to_string(), value);
        }
        Ok(results)
    }

    fn provider_name(&self) -> &str {
        &self.name
    }

    async fn health_check(&self) -> Result<(), SecretsError> {
        if self.providers.is_empty() {
            return Err(SecretsError::ProviderUnavailable {
                message: "no providers configured in chain".to_string(),
            });
        }

        // At least one provider must be healthy
        let mut last_error = None;
        for provider in &self.providers {
            match provider.health_check().await {
                Ok(()) => return Ok(()),
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error.expect("at least one provider exists"))
    }
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p tasker-secure --test chained_provider_test`
Expected: All 6 tests PASS

**Step 6: Run clippy**

Run: `cargo clippy -p tasker-secure --all-targets --all-features`
Expected: Zero warnings

**Step 7: Commit**

```bash
git add crates/tasker-secure/src/secrets/chained.rs \
       crates/tasker-secure/src/testing/mock_secrets.rs \
       crates/tasker-secure/tests/chained_provider_test.rs
git commit -m "feat(TAS-357): implement ChainedSecretsProvider and InMemorySecretsProvider

ChainedSecretsProvider tries providers in priority order — first
success wins, all must fail for error. InMemorySecretsProvider
provides test-utils for downstream crate testing."
```

---

## Task 6: SopsSecretsProvider (feature-gated)

**Files:**
- Modify: `crates/tasker-secure/src/secrets/sops.rs`
- Create: `crates/tasker-secure/tests/sops_provider_test.rs`
- Create: `crates/tasker-secure/tests/fixtures/` (test SOPS files)

**Important note:** The `rops` crate has 0% docs.rs documentation. The implementer must explore the `rops` API by reading its source (`cargo doc -p rops --open` or browsing the GitHub repo). The key type is `RopsFile` in `rops::file`. The crate supports YAML, JSON, and TOML formats with age, AWS KMS, and GCP KMS backends.

**Step 1: Explore the `rops` API**

Run: `cargo doc -p rops --open --all-features` (or browse [github.com/gibbz00/rops](https://github.com/gibbz00/rops))

Key things to discover:
- How to parse a SOPS-encrypted file from a string
- How to decrypt using an age key
- How to access the decrypted data as `serde_json::Value` or equivalent
- Error types

If the `rops` API turns out to be too complex or unstable, fall back to **shelling out to the `sops` CLI** via `tokio::process::Command` as a pragmatic alternative. Document the decision.

If `rops` integration proves infeasible in S1, an alternative approach is acceptable:
- Implement `SopsSecretsProvider` as a thin wrapper that:
  1. Reads the SOPS-encrypted file
  2. Uses `sops` CLI (if available) or `age` crate directly for decryption
  3. Parses the decrypted YAML/JSON/TOML into `serde_json::Value`
  4. Navigates dot-separated paths to resolve individual secrets

**Step 2: Create test fixtures**

Create `crates/tasker-secure/tests/fixtures/test-secrets.yaml` — a plain YAML file representing what a decrypted SOPS file would contain:

```yaml
database:
  orders:
    password: "orders-db-secret"
    username: "orders-user"
  analytics:
    password: "analytics-db-secret"
api:
  stripe:
    secret_key: "sk_test_12345"
  sendgrid:
    api_key: "SG.test-key"
```

For actual SOPS encryption testing, the implementer should:
1. Generate an age key pair: `age-keygen -o tests/fixtures/test-age-key.txt`
2. Encrypt the test file: `sops --encrypt --age <public-key> tests/fixtures/test-secrets.yaml > tests/fixtures/test-secrets.enc.yaml`
3. Add `tests/fixtures/test-age-key.txt` to `.gitignore` if it contains a real private key (or use a test-only key committed to the repo)

If `sops` CLI is not available in the test environment, provide a pre-encrypted fixture with the age key committed (acceptable for test-only keys).

**Step 3: Write the failing tests**

Create `crates/tasker-secure/tests/sops_provider_test.rs`:

```rust
#![cfg(feature = "sops")]

use std::path::PathBuf;
use tasker_secure::{SecretsError, SecretsProvider, SopsSecretsProvider};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

// Test with a plain (unencrypted) YAML file to validate path navigation
// without requiring SOPS encryption infrastructure
#[tokio::test]
async fn resolves_nested_path_from_yaml() {
    let provider = SopsSecretsProvider::from_plaintext_yaml(
        fixtures_dir().join("test-secrets.yaml"),
    )
    .await
    .unwrap();

    let result = provider.get_secret("database.orders.password").await.unwrap();
    assert_eq!(result.expose_secret(), "orders-db-secret");
}

#[tokio::test]
async fn resolves_deeply_nested_path() {
    let provider = SopsSecretsProvider::from_plaintext_yaml(
        fixtures_dir().join("test-secrets.yaml"),
    )
    .await
    .unwrap();

    let result = provider.get_secret("api.stripe.secret_key").await.unwrap();
    assert_eq!(result.expose_secret(), "sk_test_12345");
}

#[tokio::test]
async fn missing_path_returns_not_found() {
    let provider = SopsSecretsProvider::from_plaintext_yaml(
        fixtures_dir().join("test-secrets.yaml"),
    )
    .await
    .unwrap();

    let result = provider.get_secret("database.nonexistent.key").await;
    assert!(matches!(result, Err(SecretsError::NotFound { .. })));
}

#[tokio::test]
async fn path_to_object_returns_not_found() {
    let provider = SopsSecretsProvider::from_plaintext_yaml(
        fixtures_dir().join("test-secrets.yaml"),
    )
    .await
    .unwrap();

    // "database.orders" is an object, not a string
    let result = provider.get_secret("database.orders").await;
    assert!(matches!(result, Err(SecretsError::InvalidPath { .. })));
}

#[tokio::test]
async fn provider_name_is_sops() {
    let provider = SopsSecretsProvider::from_plaintext_yaml(
        fixtures_dir().join("test-secrets.yaml"),
    )
    .await
    .unwrap();

    assert_eq!(provider.provider_name(), "sops");
}

#[tokio::test]
async fn health_check_ok_when_loaded() {
    let provider = SopsSecretsProvider::from_plaintext_yaml(
        fixtures_dir().join("test-secrets.yaml"),
    )
    .await
    .unwrap();

    assert!(provider.health_check().await.is_ok());
}

#[tokio::test]
async fn nonexistent_file_returns_error() {
    let result = SopsSecretsProvider::from_plaintext_yaml(
        fixtures_dir().join("nonexistent.yaml"),
    )
    .await;

    assert!(result.is_err());
}
```

**Step 4: Run tests to verify they fail**

Run: `cargo nextest run -p tasker-secure --features sops --test sops_provider_test`
Expected: FAIL — `SopsSecretsProvider` not implemented

**Step 5: Implement SopsSecretsProvider**

The implementation approach depends on what the `rops` API exploration reveals. The core pattern is:

Update `crates/tasker-secure/src/secrets/sops.rs`:

```rust
//! SOPS-encrypted file secrets provider.
//!
//! Uses the `rops` crate (Rust SOPS implementation) or falls back to
//! parsing decrypted YAML/JSON files. Values are cached in memory as
//! `SecretValue` after initial load.
//!
//! Path format: dot-separated key navigation — `"database.orders.password"`
//! resolves to the value at that path in the decrypted structure.

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
    /// In production, use `from_encrypted_yaml` with a decryption config.
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

    // TODO(S1): Add `from_encrypted` constructor using rops crate for actual
    // SOPS decryption with age/KMS keys. Requires rops API exploration.
    // See: https://github.com/gibbz00/rops
}

/// Flatten a nested JSON value into dot-separated paths.
///
/// Only leaf string values are included. Objects create path segments;
/// arrays and non-string leaves are skipped (they're not secret values).
fn flatten_value(value: &serde_json::Value, prefix: String, out: &mut HashMap<String, SecretValue>) {
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
        // Numbers, bools, arrays — convert to string for retrieval
        serde_json::Value::Number(n) => {
            out.insert(prefix, SecretValue::from(n.to_string()));
        }
        serde_json::Value::Bool(b) => {
            out.insert(prefix, SecretValue::from(b.to_string()));
        }
        serde_json::Value::Null | serde_json::Value::Array(_) => {
            // Skip null and array values — not representable as single secrets
        }
    }
}

#[async_trait::async_trait]
impl SecretsProvider for SopsSecretsProvider {
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError> {
        let cache = self.cache.read().await;

        // Check if this path points to an object (has children but no direct value)
        let has_children = cache.keys().any(|k| k.starts_with(&format!("{path}.")));
        if has_children && !cache.contains_key(path) {
            return Err(SecretsError::InvalidPath {
                path: path.to_string(),
                reason: "path refers to an object, not a leaf value".to_string(),
            });
        }

        cache.get(path).map(|v| SecretValue::new(v.expose_secret())).ok_or_else(|| {
            SecretsError::NotFound {
                path: path.to_string(),
            }
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
```

**Step 6: Run tests to verify they pass**

Run: `cargo nextest run -p tasker-secure --features sops --test sops_provider_test`
Expected: All 7 tests PASS

**Step 7: Run clippy**

Run: `cargo clippy -p tasker-secure --all-targets --all-features`
Expected: Zero warnings

**Step 8: Commit**

```bash
git add crates/tasker-secure/src/secrets/sops.rs \
       crates/tasker-secure/tests/sops_provider_test.rs \
       crates/tasker-secure/tests/fixtures/
git commit -m "feat(TAS-357): implement SopsSecretsProvider with YAML loading

Loads secrets from YAML files with dot-separated path navigation.
from_plaintext_yaml constructor for dev/test; encrypted file support
via rops to be added as follow-up. Values cached in memory at load time."
```

---

## Task 7: ConfigString Type

**Files:**
- Create: `crates/tasker-secure/src/config/mod.rs` (replace stub)
- Create: `crates/tasker-secure/tests/config_string_test.rs`

**Step 1: Write the failing tests**

Create `crates/tasker-secure/tests/config_string_test.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;

use tasker_secure::config::ConfigString;
use tasker_secure::testing::InMemorySecretsProvider;
use tasker_secure::SecretsProvider;

fn test_provider() -> Arc<dyn SecretsProvider> {
    let mut secrets = HashMap::new();
    secrets.insert(
        "/production/tasker/database/url".to_string(),
        "postgresql://prod:secret@db.example.com/tasker".to_string(),
    );
    secrets.insert(
        "redis/url".to_string(),
        "redis://cache.example.com:6379".to_string(),
    );
    Arc::new(InMemorySecretsProvider::new(secrets))
}

#[tokio::test]
async fn literal_resolves_to_itself() {
    let config = ConfigString::Literal("hello".to_string());
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await.unwrap();
    assert_eq!(result, "hello");
}

#[tokio::test]
async fn secret_ref_resolves_through_provider() {
    let config = ConfigString::SecretRef {
        path: "/production/tasker/database/url".to_string(),
        provider: None,
    };
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await.unwrap();
    assert_eq!(result, "postgresql://prod:secret@db.example.com/tasker");
}

#[tokio::test]
async fn env_ref_resolves_from_env() {
    std::env::set_var("TEST_CONFIG_STRING_VAR", "env-value");
    let config = ConfigString::EnvRef {
        var: "TEST_CONFIG_STRING_VAR".to_string(),
        default: None,
    };
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await.unwrap();
    assert_eq!(result, "env-value");
    std::env::remove_var("TEST_CONFIG_STRING_VAR");
}

#[tokio::test]
async fn env_ref_uses_default_when_unset() {
    let config = ConfigString::EnvRef {
        var: "DEFINITELY_NOT_SET_CONFIG_STRING".to_string(),
        default: Some("fallback-value".to_string()),
    };
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await.unwrap();
    assert_eq!(result, "fallback-value");
}

#[tokio::test]
async fn env_ref_fails_without_default_when_unset() {
    let config = ConfigString::EnvRef {
        var: "DEFINITELY_NOT_SET_CONFIG_STRING_2".to_string(),
        default: None,
    };
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn secret_ref_fails_when_not_found() {
    let config = ConfigString::SecretRef {
        path: "nonexistent/path".to_string(),
        provider: None,
    };
    let provider = test_provider();
    let result = config.resolve(provider.as_ref()).await;
    assert!(result.is_err());
}

#[test]
fn deserialize_literal_string() {
    let toml_str = r#"value = "postgresql://localhost/tasker""#;
    let parsed: TestConfig = toml::from_str(toml_str).unwrap();
    assert!(matches!(parsed.value, ConfigString::Literal(_)));
}

#[test]
fn deserialize_secret_ref() {
    let toml_str = r#"
[value]
secret_ref = "/production/tasker/database/url"
"#;
    let parsed: TestConfig = toml::from_str(toml_str).unwrap();
    assert!(matches!(parsed.value, ConfigString::SecretRef { .. }));
}

#[test]
fn deserialize_secret_ref_with_provider() {
    let toml_str = r#"
[value]
secret_ref = "prod/tasker/db-url"
provider = "vault"
"#;
    let parsed: TestConfig = toml::from_str(toml_str).unwrap();
    match &parsed.value {
        ConfigString::SecretRef { path, provider } => {
            assert_eq!(path, "prod/tasker/db-url");
            assert_eq!(provider.as_deref(), Some("vault"));
        }
        _ => panic!("expected SecretRef"),
    }
}

#[derive(serde::Deserialize)]
struct TestConfig {
    value: ConfigString,
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p tasker-secure --test config_string_test`
Expected: FAIL — `ConfigString` not implemented

**Step 3: Add `toml` to dev-dependencies**

Add to `crates/tasker-secure/Cargo.toml` `[dev-dependencies]`:
```toml
toml = { workspace = true }
```

**Step 4: Implement ConfigString**

Replace `crates/tasker-secure/src/config/mod.rs`:

```rust
//! Configuration value types with secrets resolution support.
//!
//! `ConfigString` extends Tasker's configuration layer to support
//! `{secret_ref: "..."}` alongside existing `${VAR:-default}` interpolation.
//! This enables teams with proper secrets management (Vault, SOPS, AWS SSM)
//! to deploy Tasker without wrapper scripts that materialize env vars.

use std::fmt;

use serde::Deserialize;

use crate::secrets::{SecretsError, SecretsProvider};

/// A configuration value that may be a literal, a secret reference, or an
/// environment variable reference.
///
/// Backward-compatible: existing `${VAR:-default}` deployments continue
/// unchanged. The `EnvSecretsProvider` is the default so existing env-var-based
/// deployments work without any config change.
///
/// # TOML syntax
///
/// ```toml
/// # Literal string (existing, always valid):
/// url = "postgresql://localhost/tasker"
///
/// # Secrets manager reference (new):
/// url = {secret_ref = "/production/tasker/database/url"}
///
/// # With a named provider:
/// url = {secret_ref = "prod/tasker/db-url", provider = "vault"}
/// ```
///
/// The `EnvRef` variant is constructed programmatically when parsing
/// `${VAR:-default}` syntax from existing config files (not yet integrated
/// into the config loader — deferred to S2).
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
    ///
    /// - `Literal`: returns the value directly
    /// - `SecretRef`: resolves through the provided `SecretsProvider`
    /// - `EnvRef`: reads from environment, falls back to default
    pub async fn resolve(&self, secrets: &dyn SecretsProvider) -> Result<String, ConfigError> {
        match self {
            Self::Literal(s) => Ok(s.clone()),

            Self::SecretRef { path, .. } => {
                let value = secrets.get_secret(path).await.map_err(|e| {
                    ConfigError::SecretResolution {
                        path: path.clone(),
                        source: e,
                    }
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

        // ConfigString can be either a plain string or an object with secret_ref
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

                let path = secret_ref.ok_or_else(|| {
                    de::Error::missing_field("secret_ref")
                })?;

                Ok(ConfigString::SecretRef { path, provider })
            }
        }

        deserializer.deserialize_any(ConfigStringVisitor)
    }
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo nextest run -p tasker-secure --test config_string_test`
Expected: All 9 tests PASS

**Step 6: Run clippy**

Run: `cargo clippy -p tasker-secure --all-targets --all-features`
Expected: Zero warnings

**Step 7: Commit**

```bash
git add crates/tasker-secure/src/config/ \
       crates/tasker-secure/tests/config_string_test.rs \
       crates/tasker-secure/Cargo.toml
git commit -m "feat(TAS-357): implement ConfigString with secret_ref deserialization

ConfigString supports Literal, SecretRef, and EnvRef variants.
TOML deserialization handles both plain strings and {secret_ref = ...}
objects. resolve() delegates to SecretsProvider for secret refs."
```

---

## Task 8: Annotated Config Documentation Update

**Files:**
- Modify: `docs/generated/annotated-production.toml`
- Modify: `docs/generated/annotated-development.toml`

**Step 1: Identify the five sensitive config values**

The five values to document (from the config loader allowlist):
1. `common.database.url` — `DATABASE_URL`
2. `common.pgmq_database.url` — `PGMQ_DATABASE_URL`
3. `common.cache.redis.url` — `REDIS_URL`
4. `common.queues.rabbitmq.url` — `RABBITMQ_URL`
5. `orchestration.web.auth.jwt_public_key` / `worker.web.auth.jwt_public_key` — `TASKER_JWT_PUBLIC_KEY`

**Step 2: Add secret_ref documentation to each sensitive value**

For each of the five values in both annotated config files, add a comment block showing both syntaxes. Example for `common.database.url`:

Find the existing line:
```toml
url = "${DATABASE_URL:-postgresql://localhost/tasker}"
```

Add comments above it:
```toml
# Connection URL for the primary PostgreSQL database.
#
# Environment variable (existing, still valid):
#   url = "${DATABASE_URL:-postgresql://localhost/tasker}"
#
# Secrets manager reference (requires tasker-secure SecretsProvider):
#   url = {secret_ref = "/production/tasker/database/url"}
#
# With a named provider:
#   url = {secret_ref = "prod/tasker/database-url", provider = "vault"}
url = "${DATABASE_URL:-postgresql://localhost/tasker}"
```

Apply the same pattern to all five values, adjusting the example `secret_ref` paths to match the domain:
- Database: `/production/tasker/database/url`
- PGMQ: `/production/tasker/pgmq-database/url`
- Redis: `/production/tasker/cache/redis-url`
- RabbitMQ: `/production/tasker/messaging/rabbitmq-url`
- JWT key: `/production/tasker/auth/jwt-public-key`

**Step 3: Verify the config files are valid TOML**

Run: `python3 -c "import tomllib; tomllib.load(open('docs/generated/annotated-production.toml', 'rb'))"` (or equivalent TOML validation)

**Step 4: Commit**

```bash
git add docs/generated/annotated-production.toml docs/generated/annotated-development.toml
git commit -m "docs(TAS-357): add secret_ref syntax to annotated config reference

Document {secret_ref = \"...\"} alternative alongside existing
\${VAR:-default} syntax for all five sensitive config values:
DATABASE_URL, PGMQ_DATABASE_URL, REDIS_URL, RABBITMQ_URL,
TASKER_JWT_PUBLIC_KEY."
```

---

## Task 9: Final Verification and Cleanup

**Files:**
- Possibly modify any files with clippy/fmt issues

**Step 1: Run full test suite**

Run: `cargo nextest run -p tasker-secure`
Expected: All default-feature tests pass

Run: `cargo nextest run -p tasker-secure --all-features`
Expected: All tests pass (including sops feature-gated tests)

**Step 2: Run full quality checks**

Run: `cargo clippy -p tasker-secure --all-targets --all-features`
Expected: Zero warnings

Run: `cargo fmt -p tasker-secure --check`
Expected: No formatting issues

**Step 3: Verify workspace build**

Run: `cargo check --all-features`
Expected: Entire workspace compiles including new tasker-secure crate

**Step 4: Verify documentation builds**

Run: `cargo doc -p tasker-secure --all-features --no-deps`
Expected: Documentation generates without warnings

**Step 5: Review acceptance criteria checklist**

- [ ] `cargo test -p tasker-secure` passes with no external secrets backend required
- [ ] `cargo test -p tasker-secure --features sops` passes with test YAML fixture
- [ ] `SecretValue`'s Display and Debug never expose the secret value
- [ ] `EnvSecretsProvider` resolves env vars correctly
- [ ] `ConfigString::resolve()` handles all three variants
- [ ] Annotated config files show both syntaxes for all five sensitive values
- [ ] `tasker-secure` compiles with `--features sops` and with default features only

**Step 6: Final commit if any cleanup was needed**

```bash
git add -A
git commit -m "chore(TAS-357): final cleanup for S1 SecretsProvider foundation"
```

---

## Summary

| Task | What | Tests | Commit |
|------|------|-------|--------|
| 1 | Crate scaffolding + workspace integration | Compilation check | `feat: scaffold tasker-secure` |
| 2 | `SecretValue` type | 5 tests | `feat: SecretValue with redacted Display/Debug` |
| 3 | `SecretsProvider` trait + `SecretsError` | Compilation check | `feat: SecretsProvider trait and SecretsError` |
| 4 | `EnvSecretsProvider` | 9 tests | `feat: EnvSecretsProvider` |
| 5 | `ChainedSecretsProvider` + `InMemorySecretsProvider` | 6 tests | `feat: ChainedSecretsProvider` |
| 6 | `SopsSecretsProvider` (feature-gated) | 7 tests | `feat: SopsSecretsProvider` |
| 7 | `ConfigString` type | 9 tests | `feat: ConfigString` |
| 8 | Annotated config documentation | TOML validation | `docs: secret_ref syntax` |
| 9 | Final verification | Full suite | Cleanup if needed |

**Total: ~36 tests across 9 tasks**
