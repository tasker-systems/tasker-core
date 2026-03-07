# tasker-secure: Crate Proposal

*Structure, traits, dependencies, feature gates, and placement in the workspace*

*Research spike — March 2026*

---

## Crate Purpose and Scope

`tasker-secure` is a strategy-pattern library providing:

1. **Secrets resolution** — `SecretsProvider` trait + implementations for env vars, SOPS/rops, Vault, AWS SSM, AWS Secrets Manager
2. **Resource lifecycle** — `ResourceRegistry`, `ResourceDefinition`, `ResourceHandle` trait + implementations for PostgreSQL, HTTP, PGMQ
3. **Observability protection** — `DataClassifier`, field redaction at trace/log emission points
4. **Encryption at rest** — `EncryptionProvider` trait + implementations for local AES-GCM (dev), AWS KMS, Vault Transit

It does **not**:
- Store secrets
- Manage encryption keys
- Replace external secrets managers or KMS systems
- Apply to domain handlers (those remain opt-in or independent)

---

## Crate Location in the Workspace

`tasker-secure` is a new workspace member at `tasker-secure/`. It follows the same pattern as the existing crates: thin public interface, clear responsibility boundary, minimal feature gates.

```
tasker-core/
├── tasker-pgmq/
├── tasker-shared/
├── tasker-sdk/
├── tasker-secure/          ← new
├── tasker-grammar/         ← planned (Phase 1)
├── tasker-orchestration/
├── tasker-worker/
├── tasker-client/
├── tasker-ctl/
├── tasker-mcp/
└── workers/
    ├── composition/        ← planned
    ├── python/
    ├── ruby/
    ├── rust/
    └── typescript/
```

---

## Module Structure

```
tasker-secure/
├── Cargo.toml
└── src/
    ├── lib.rs                  # Re-exports; feature gate documentation
    │
    ├── secrets/
    │   ├── mod.rs              # SecretsProvider trait, SecretsError
    │   ├── value.rs            # SecretValue (wraps secrecy::SecretString)
    │   ├── env.rs              # EnvSecretsProvider
    │   ├── chained.rs          # ChainedSecretsProvider
    │   ├── sops.rs             # SopsSecretsProvider [feature = "sops"]
    │   ├── vault.rs            # VaultSecretsProvider [feature = "vault"]
    │   ├── aws_ssm.rs          # AwsSsmProvider [feature = "aws-ssm"]
    │   └── aws_secrets.rs      # AwsSecretsManagerProvider [feature = "aws-secrets"]
    │
    ├── resource/
    │   ├── mod.rs              # ResourceHandle trait, ResourceType, ResourceError
    │   ├── definition.rs       # ResourceDefinition, ConfigValue
    │   ├── registry.rs         # ResourceRegistry
    │   ├── summary.rs          # ResourceSummary (for MCP discoverability)
    │   ├── postgres.rs         # PostgresHandle [feature = "postgres"]
    │   ├── http.rs             # HttpHandle, HttpAuthStrategy [feature = "http"]
    │   └── pgmq.rs             # PgmqHandle [feature = "pgmq"]
    │
    ├── classification/
    │   ├── mod.rs              # DataClassification, DataScope
    │   ├── spec.rs             # ClassificationRule, FieldEncryptionSpec (parsed from TaskTemplate)
    │   ├── classifier.rs       # DataClassifier (redact at observability boundary)
    │   └── path.rs             # JsonPath matching for classification rules
    │
    ├── encryption/
    │   ├── mod.rs              # EncryptionProvider trait, EncryptedValue, EncryptionError
    │   ├── aes_gcm.rs          # AesGcmEncryptionProvider (dev/test) [feature = "encryption"]
    │   ├── kms.rs              # AwsKmsEncryptionProvider [feature = "aws-kms"]
    │   └── vault_transit.rs    # VaultTransitEncryptionProvider [feature = "vault"]
    │
    └── testing/
        ├── mod.rs              # Test utilities: in-memory handles, fixture registry
        ├── mock_secrets.rs     # InMemorySecretsProvider for unit tests
        └── mock_resources.rs   # InMemoryResourceHandle, test_registry_with_fixtures()
```

---

## Cargo.toml — Dependencies and Feature Gates

```toml
[package]
name = "tasker-secure"
version = "0.1.0"
edition = "2021"

[features]
default = ["postgres", "http"]

# Resource handle types
postgres = ["dep:sqlx"]
http = ["dep:reqwest"]
pgmq = ["dep:tasker-pgmq"]

# Secrets provider backends (each is optional — use only what the deployment needs)
sops = ["dep:rops"]
vault = ["dep:vaultrs"]
aws-ssm = ["dep:aws-sdk-ssm", "dep:aws-config"]
aws-secrets = ["dep:aws-sdk-secretsmanager", "dep:aws-config"]

# Encryption provider backends
encryption = []                           # base encryption support (types + AES-GCM local)
aws-kms = ["encryption", "dep:aws-sdk-kms", "dep:aws-config"]
vault-transit = ["encryption", "vault"]  # reuses vaultrs from vault feature

# Testing utilities — for use in test configurations only
test-utils = []

[dependencies]
# Always required
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
async-trait = { workspace = true }
tokio = { workspace = true, features = ["rt", "sync"] }
thiserror = { workspace = true }
tracing = { workspace = true }

# SecretValue — always required
secrecy = { version = "0.8", features = ["serde"] }
zeroize = { version = "1.7" }

# JsonPath for DataClassifier field matching
jsonpath-rust = { version = "0.3" }

# AES-GCM encryption — always included when `encryption` feature is on
aes-gcm = { version = "0.10", optional = true, features = ["aes"] }
rand = { version = "0.8", optional = true }
base64 = { version = "0.21", optional = true }

# Resource handle dependencies
sqlx = { workspace = true, optional = true, features = ["postgres", "runtime-tokio-rustls"] }
reqwest = { version = "0.11", optional = true, features = ["json", "rustls-tls"] }
tasker-pgmq = { path = "../tasker-pgmq", optional = true }

# Secrets provider dependencies
rops = { version = "0.2", optional = true }          # SOPS file support
vaultrs = { version = "0.7", optional = true }        # Vault client
aws-sdk-ssm = { version = "1", optional = true }
aws-sdk-secretsmanager = { version = "1", optional = true }
aws-sdk-kms = { version = "1", optional = true }
aws-config = { version = "1", optional = true }

[dev-dependencies]
tokio = { workspace = true, features = ["full", "test-utils"] }
```

---

## Dependency Graph: Where tasker-secure Sits

```
tasker-pgmq          ← unchanged
    │
tasker-shared        ← unchanged (core types, no security concerns)
    │
tasker-secure        ← NEW (no dependency on tasker-shared; this is intentional)
    │
    ├──→ tasker-grammar    ← no dependency on tasker-secure (grammar stays pure)
    │
    ├──→ tasker-worker     ← depends on tasker-secure for:
    │                           ResourceRegistry (ExecutionContext)
    │                           DataClassifier (trace/log emission)
    │                           EncryptionProvider (step result storage)
    │
    ├──→ tasker-orchestration ← depends on tasker-secure for:
    │                              EncryptionProvider (task context storage)
    │                              (DataClassifier and ResourceRegistry are worker concerns)
    │
    └──→ tasker-sdk         ← depends on tasker-secure for:
                                DataClassificationSpec parsing (part of TaskTemplate)
```

**Why tasker-secure does NOT depend on tasker-shared**: The `SecretValue`, `SecretsProvider`, `ResourceHandle`, and `EncryptionProvider` types are framework-agnostic — they know nothing about `Task`, `WorkflowStep`, `TaskTemplate`, or any Tasker domain model. Keeping the dependency clean means `tasker-secure` can be used independently of the Tasker orchestration domain and tested without a database.

The exception: `PgmqHandle` (feature-gated `pgmq`) does depend on `tasker-pgmq`. This is acceptable because `tasker-pgmq` is also a thin, domain-independent crate.

---

## Public API Summary

The public surface of `tasker-secure` is intentionally narrow. Implementation details are private; the trait boundaries are what consumers program against.

### Secrets Layer

```rust
// Core trait
pub trait SecretsProvider: Send + Sync + fmt::Debug {
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError>;
    async fn get_secrets(&self, paths: &[&str]) -> Result<HashMap<String, SecretValue>, SecretsError>;
    fn provider_name(&self) -> &str;
    async fn health_check(&self) -> Result<(), SecretsError>;
}

// Opaque value type — Display/Debug emit "[REDACTED]"
pub struct SecretValue(secrecy::SecretString);

// Built-in providers
pub struct EnvSecretsProvider { ... }
pub struct ChainedSecretsProvider { ... }
#[cfg(feature = "sops")] pub struct SopsSecretsProvider { ... }
#[cfg(feature = "vault")] pub struct VaultSecretsProvider { ... }
#[cfg(feature = "aws-ssm")] pub struct AwsSsmProvider { ... }
#[cfg(feature = "aws-secrets")] pub struct AwsSecretsManagerProvider { ... }
```

### Resource Layer

```rust
// Core trait
pub trait ResourceHandle: Send + Sync + fmt::Debug {
    fn resource_name(&self) -> &str;
    fn resource_type(&self) -> &ResourceType;
    async fn refresh_credentials(&self, secrets: &dyn SecretsProvider) -> Result<(), ResourceError>;
    async fn health_check(&self) -> Result<(), ResourceError>;
    fn as_any(&self) -> &dyn Any;
}

// Registry
pub struct ResourceRegistry { ... }
impl ResourceRegistry {
    pub async fn initialize_all(
        secrets: Arc<dyn SecretsProvider>,
        definitions: Vec<ResourceDefinition>,
    ) -> Result<Self, ResourceError>;
    pub fn get(&self, name: &str) -> Option<Arc<dyn ResourceHandle>>;
    pub async fn refresh_resource(&self, name: &str) -> Result<(), ResourceError>;
    pub fn list_resources(&self) -> Vec<ResourceSummary>;
}

// Concrete handles
#[cfg(feature = "postgres")] pub struct PostgresHandle { ... }
#[cfg(feature = "http")] pub struct HttpHandle { ... }
#[cfg(feature = "pgmq")] pub struct PgmqHandle { ... }

// Config types
pub struct ResourceDefinition { pub name, pub resource_type, pub config, pub secrets_provider }
pub enum ConfigValue { Literal(String), SecretRef { secret_ref: String }, EnvRef { env: String } }
pub struct ResourceSummary { pub name, pub resource_type, pub healthy }
```

### Classification Layer

```rust
pub struct DataClassifier { ... }
impl DataClassifier {
    pub fn from_spec(spec: &DataClassificationSpec) -> Self;
    pub fn redact(&self, value: &Value, scope: DataScope) -> Value;
    pub fn is_classified(&self, path: &str, scope: DataScope) -> bool;
    pub fn redact_span_attributes(&self, attrs: HashMap<String, String>, scope: DataScope)
        -> HashMap<String, String>;
}

pub struct DataClassificationSpec { pub context_fields: Vec<ClassificationRule>, pub result_fields: Vec<ClassificationRule> }
pub struct ClassificationRule { pub path, pub classification, pub trace_behavior, pub log_behavior, pub encrypt_at_rest }
pub enum DataClassification { Pii, PaymentCard, HealthcarePhi, Credential, Custom(String) }
pub enum DataScope { TaskContext, DependencyResult, CapabilityOutput, StepResult, CheckpointData }
```

### Encryption Layer

```rust
// Core trait
pub trait EncryptionProvider: Send + Sync + fmt::Debug {
    async fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedValue, EncryptionError>;
    async fn decrypt(&self, ciphertext: &EncryptedValue) -> Result<Vec<u8>, EncryptionError>;
    async fn encrypt_fields(&self, value: &Value, specs: &[FieldEncryptionSpec]) -> Result<Value, EncryptionError>;
    async fn decrypt_fields(&self, value: &Value, specs: &[FieldEncryptionSpec]) -> Result<Value, EncryptionError>;
    fn provider_name(&self) -> &str;
    async fn health_check(&self) -> Result<(), EncryptionError>;
}

pub struct EncryptedValue { pub version, pub algorithm, pub dek_encrypted, pub iv, pub ciphertext }
pub struct FieldEncryptionSpec { pub path: String, pub classification: DataClassification }

// Implementations
#[cfg(feature = "encryption")] pub struct AesGcmEncryptionProvider { ... }
#[cfg(feature = "aws-kms")] pub struct AwsKmsEncryptionProvider { ... }
#[cfg(all(feature = "vault", feature = "encryption"))] pub struct VaultTransitEncryptionProvider { ... }
```

### Testing Utilities

```rust
#[cfg(feature = "test-utils")]
pub mod testing {
    pub struct InMemorySecretsProvider { ... }  // pre-loaded key-value secrets
    pub struct InMemoryResourceHandle { ... }   // fixture data + captured operations
    pub fn test_registry_with_fixtures(fixtures: Vec<ResourceFixture>) -> ResourceRegistry;
}
```

---

## Integration Points in Existing Crates

### tasker-worker: ExecutionContext

The `ExecutionContext` passed to all `CapabilityExecutor::execute()` calls gains `resources` and `classifier` fields:

```rust
// In tasker-worker (references tasker-secure types)
pub struct ExecutionContext {
    pub step_uuid: Uuid,
    pub correlation_id: String,
    pub checkpoint: Arc<CheckpointService>,
    pub checkpoint_state: Option<CheckpointRecord>,
    pub step_config: serde_json::Value,
    pub resources: Arc<tasker_secure::ResourceRegistry>,
    pub classifier: Option<Arc<tasker_secure::DataClassifier>>,
}
```

The `ResourceRegistry` and optional `DataClassifier` are constructed at worker startup and passed through to every capability execution. The `DataClassifier` is `None` when the task template has no `data_classification` section (the common case — no overhead for workflows that don't opt in).

### tasker-worker: WorkerBootstrap

Worker startup gains a `tasker-secure` initialization phase:

```rust
// In WorkerBootstrap (tasker-worker)
pub struct WorkerBootstrap {
    // ...existing fields...
    secrets_provider: Option<Arc<dyn SecretsProvider>>,
    resource_definitions: Vec<ResourceDefinition>,
    encryption_provider: Option<Arc<dyn EncryptionProvider>>,
}

impl WorkerBootstrap {
    pub fn with_secrets_provider(mut self, provider: Arc<dyn SecretsProvider>) -> Self { ... }
    pub fn with_resources(mut self, definitions: Vec<ResourceDefinition>) -> Self { ... }
    pub fn with_encryption_provider(mut self, provider: Arc<dyn EncryptionProvider>) -> Self { ... }
}
```

### tasker-shared: Config Layer (Dog-Fooding)

The config layer in `tasker-shared` (or wherever the TOML resolution lives) gains support for `{secret_ref: ...}` in string values:

```rust
// In tasker-shared config resolution
pub enum ConfigString {
    Literal(String),
    SecretRef { path: String, provider: Option<String> },
    EnvRef { var: String, default: Option<String> },
}

impl ConfigString {
    pub async fn resolve(&self, secrets: &dyn SecretsProvider) -> Result<String, ConfigError>;
}
```

This is backward-compatible: `"${DATABASE_URL:-...}"` continues to work as before (via the `EnvRef` variant). The new `{secret_ref: "..."}` syntax enables opt-in secrets management for Tasker's own credentials.

### tasker-sdk: DataClassificationSpec Parsing

The `TaskTemplate` parser in `tasker-sdk` gains understanding of the `data_classification:` section:

```rust
// In tasker-sdk (references tasker-secure::DataClassificationSpec)
pub struct TaskTemplate {
    // ...existing fields...
    pub data_classification: Option<tasker_secure::DataClassificationSpec>,
}
```

---

## What the Composition-Only Worker Needs

The composition-only worker (`workers/composition`) is the minimal Rust binary for executing grammar-composed steps. It needs:

```toml
# workers/composition/Cargo.toml
[dependencies]
tasker-worker = { path = "../../tasker-worker", default-features = true }
tasker-secure = { path = "../../tasker-secure", features = ["postgres", "http", "pgmq"] }
# No sops/vault/aws-* by default — deployment adds what it needs
```

Production deployment adds features appropriate for the secrets backend in use:

```toml
# For an AWS-native deployment:
tasker-secure = { path = "../../tasker-secure", features = ["postgres", "http", "pgmq", "aws-ssm", "aws-kms"] }

# For a Vault-native deployment:
tasker-secure = { path = "../../tasker-secure", features = ["postgres", "http", "pgmq", "vault"] }
```

This is the build-from-source extensibility model — organizations compile the worker binary with the feature flags appropriate for their infrastructure, same as they already do for custom handler registration.

---

*Read next: `06-research-spikes.md` for the phased spike plan and acceptance criteria.*
