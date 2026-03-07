# Concern 1: Secrets and Credential Injection

*How grammar capability executors access external systems without credentials entering data paths*

*Research spike — March 2026*

---

## The Core Problem

The `acquire`, `persist`, and `emit` capability executors need connections to external systems. Those connections require credentials. The question is where credentials live and when they are resolved.

There are four candidate approaches, in increasing correctness:

| Approach | Where credentials live | When resolved | Why it's wrong |
|----------|----------------------|---------------|----------------|
| Inline in composition config | TaskTemplate YAML | Composition load time | Credentials in the database, in MCP output, in `tasker-ctl` output |
| Environment variables | Process environment | Config load time | Process-wide access, spill risk in logs/traces, no scoping, no rotation without restart |
| Task context injection | Composition context `.context` | Per-task execution | Credentials in jaq filter surface, in step results, in checkpoint data |
| Resource registry with resolved handles | Worker startup | Pool initialization time | **This is the right answer** |

The resource registry approach is correct because it resolves secrets exactly once, at worker startup, into already-initialized connection pools or authenticated clients. Capability executors receive handles — not secrets. From the point of resolution forward, the credential value never appears in any code path that intersects with data, logs, or traces.

---

## The SecretsProvider Strategy

The strategy pattern is the right design because organizations have different secrets management requirements and Tasker should not mandate one. A startup using SOPS-encrypted dotfiles and a regulated financial institution using Vault AppRole auth and a cloud-native team using AWS Secrets Manager all have valid, idiomatic approaches. Tasker's role is to provide a clean interface that each backend can implement.

### The Trait

```rust
/// A secrets provider resolves named secret references to their values.
///
/// Implementations talk to specific backends (Vault, AWS SSM, SOPS files,
/// environment variables, etc.). Tasker does not store secrets — it resolves
/// references through provider implementations.
///
/// Secrets are resolved at startup or pool initialization time, not during
/// step execution. Capability executors never receive secret values — they
/// receive already-initialized resource handles.
#[async_trait]
pub trait SecretsProvider: Send + Sync + fmt::Debug {
    /// Resolve a single secret by its path or identifier.
    ///
    /// Path format is provider-defined:
    ///   - SOPS/rops:            "config/production.enc.yaml#database.password"
    ///   - Vault KV v2:          "secret/data/production/tasker/orders-db#password"
    ///   - AWS SSM:              "/production/tasker/orders-db/password"
    ///   - AWS Secrets Manager:  "prod/tasker/orders-db-credentials#password"
    ///   - Env var:              "DATABASE_PASSWORD"
    async fn get_secret(&self, path: &str) -> Result<SecretValue, SecretsError>;

    /// Resolve multiple secrets in a single call.
    /// Implementations should batch round trips where the backend supports it.
    async fn get_secrets(
        &self,
        paths: &[&str],
    ) -> Result<HashMap<String, SecretValue>, SecretsError>;

    /// Provider identity — used in diagnostic messages only.
    fn provider_name(&self) -> &str;

    /// Verify the provider is reachable and the configuration is valid.
    /// Called at worker startup before resource initialization begins.
    async fn health_check(&self) -> Result<(), SecretsError>;
}
```

### SecretValue — The Opaque Type

The `SecretValue` type exists to make accidental exposure of secret values structurally difficult. It should not be rolled from scratch. The `secrecy` crate by Tony Arcieri (iqlusion) provides exactly the right abstraction:

```rust
// This is what we use — wraps secrecy::SecretString
pub struct SecretValue(secrecy::SecretString);

// Display and Debug both emit "[REDACTED]" — this is enforced by secrecy
// fmt::Display: impl Display for SecretString -> "[REDACTED]"
// fmt::Debug:   impl Debug for SecretString -> "[REDACTED]"

// The only way to get the value:
impl SecretValue {
    /// Intentional, explicit access. The method name signals intent at code review.
    pub fn expose_secret(&self) -> &str {
        use secrecy::ExposeSecret;
        self.0.expose_secret()
    }
}

// On drop, the underlying memory is zeroized (via zeroize crate).
```

The `secrecy` crate is used by `rustls`, `ring`, and the broader Rust crypto ecosystem. It has received security review. We should use it directly rather than reimplement its behavior.

**Why the method name matters**: `expose_secret()` is immediately conspicuous in code review. A diff that adds `.expose_secret()` in a context where the result could be logged is a red flag. The name creates friction that discourages careless use.

---

## Provider Implementations

### EnvSecretsProvider

The simplest implementation. Maps secret paths to environment variable names.

```rust
pub struct EnvSecretsProvider {
    /// Optional prefix stripped from paths before env var lookup.
    /// With prefix "TASKER_SECRET_":
    ///   path "orders-db/password" -> env var "TASKER_SECRET_ORDERS_DB_PASSWORD"
    prefix: Option<String>,
    /// Normalize path separators to underscores and uppercase.
    normalize: bool,
}
```

Use cases:
- Local development
- Docker Compose and simple container environments
- Migration path from existing env-var-only configurations

Explicitly documented as "not recommended for production deployments that have real secrets management requirements." This is not deprecation — it is honest documentation about the tradeoff.

**Dog-fooding connection**: This provider is also how `DATABASE_URL`, `PGMQ_DATABASE_URL`, `REDIS_URL`, etc. would be resolved when the Tasker config layer is extended to support `{secret_ref: ...}` syntax. The env var approach remains valid; it just becomes one strategy among several.

### SopsSecretsProvider

SOPS (Secrets OPerationS) is a tool for encrypting configuration files in place — the encrypted file lives in version control, decryption happens at startup using a key from an external KMS or a local age key. The `rops` crate is the Rust-native SOPS implementation.

```rust
pub struct SopsSecretsProvider {
    /// Path to the SOPS-encrypted file to load at startup.
    /// Can be YAML, JSON, or TOML — rops handles all three.
    encrypted_file_path: PathBuf,

    /// Age private key or KMS configuration for decryption.
    /// This is where the "trust anchor" lives — the thing that can decrypt the file.
    decryption_config: SopsDecryptionConfig,

    /// Cached decrypted values — resolved once at startup.
    /// The decrypted file is kept in memory (via SecretValue wrapping).
    cache: Arc<RwLock<HashMap<String, SecretValue>>>,
}

pub enum SopsDecryptionConfig {
    /// Age private key (PEM format). For development and simpler setups.
    AgeKey { key_path: PathBuf },
    /// AWS KMS key ARN. IAM role provides decryption permission.
    AwsKms { key_arn: String, region: String },
    /// GCP KMS resource name. Service account provides decryption permission.
    GcpKms { resource_name: String },
    /// PGP key fingerprint. For organizations using PGP-based secrets.
    Pgp { fingerprint: String },
}
```

Path format: `"section.subsection.key"` — dot-separated navigation into the decrypted YAML/JSON structure. Example: `"database.orders.password"` resolves to the value at that path in the decrypted file.

The appeal of SOPS for GitOps-style deployments: encrypted secrets live in the same repository as configuration, reviewed via the same PR process, with access controlled by who holds the decryption key. Teams deploying with Helm/Flux/ArgoCD often already use SOPS.

**Important**: `rops` is the Rust implementation of the SOPS format. It supports age, AWS KMS, and GCP KMS backends, and reads files in the same format as the Go `sops` CLI. Teams using SOPS today can use `rops` without changing their encrypted files or key management setup.

### VaultSecretsProvider

HashiCorp Vault is the most common enterprise secrets manager. The `vaultrs` crate provides async Rust Vault client support.

```rust
pub struct VaultSecretsProvider {
    client: Arc<VaultClient>,
    auth: VaultAuth,
    /// KV mount path (defaults to "secret")
    mount: String,
    /// Lease renewal configuration for leased secrets
    lease_manager: Option<Arc<LeaseManager>>,
}

pub enum VaultAuth {
    /// Token auth — typically a root token for development only
    Token(SecretValue),
    /// AppRole — the standard for machine-to-machine auth
    AppRole { role_id: String, secret_id: SecretValue },
    /// Kubernetes service account auth — for deployments in k8s
    Kubernetes { role: String, jwt_path: PathBuf },
}
```

Path format follows Vault KV v2: `"secret/data/production/tasker/orders-db#password"` — the path before `#` is the Vault secret path, the fragment is the key within the secret's data map.

Vault has a feature no other provider has: **secret leases with automatic renewal**. A database credential can be a Vault dynamic secret — generated on demand, valid for a lease period, automatically rotated by Vault. The `VaultSecretsProvider` needs a `LeaseManager` that renews leases before they expire and triggers resource pool reinitialization when a credential changes. This is the subject of an open research question (see `06-research-spikes.md`).

### AwsSsmProvider and AwsSecretsManagerProvider

Two separate implementations for AWS's two secrets storage services:

**AWS SSM Parameter Store**: Path-based, hierarchical, with `SecureString` type that encrypts at rest using AWS KMS. IAM policies control access by path prefix. Best for simple string secrets.

```rust
pub struct AwsSsmProvider {
    client: Arc<SsmClient>,
    /// AWS region
    region: String,
    /// Optional path prefix stripped before lookup
    /// With prefix "/production/tasker":
    ///   path "orders-db/password" -> SSM path "/production/tasker/orders-db/password"
    path_prefix: Option<String>,
    /// Whether to decrypt SecureString values (almost always true)
    with_decryption: bool,
}
```

**AWS Secrets Manager**: JSON-payload-capable, rotation-aware, more expensive than SSM. Best for structured credentials (JSON containing username, password, host together).

```rust
pub struct AwsSecretsManagerProvider {
    client: Arc<SecretsManagerClient>,
    region: String,
    path_prefix: Option<String>,
}
```

Path format for both: `"/production/tasker/orders-db/password"` for SSM, `"prod/tasker/orders-db-credentials#password"` for Secrets Manager (where `#password` navigates into the parsed JSON payload).

### ChainedSecretsProvider

Tries providers in priority order. A resolution succeeds if any provider returns a value; it only fails if all providers return errors.

```rust
pub struct ChainedSecretsProvider {
    providers: Vec<Arc<dyn SecretsProvider>>,
}
```

Use cases:
- **Migration**: Old system uses env vars, new system uses Vault. The chain tries Vault first (by path), falls back to env if the Vault path isn't found yet.
- **Multi-environment**: Development uses SOPS files with age keys; production uses AWS KMS. The chain selects the right provider based on which one succeeds.
- **Defense in depth**: Primary provider + backup provider for availability during secrets manager outages.

---

## Dog-Fooding: Tasker's Own Configuration

The current Tasker config layer uses shell-style variable interpolation for its own secrets:

```toml
[common.database]
url = "${DATABASE_URL:-postgresql://localhost/tasker}"

[common.pgmq_database]
url = "${PGMQ_DATABASE_URL:-}"

[common.cache.redis]
url = "${REDIS_URL:-redis://localhost:6379}"

[common.queues.rabbitmq]
url = "${RABBITMQ_URL:-amqp://guest:guest@localhost:5672/%2F}"

[orchestration.web.auth]
jwt_public_key = "${TASKER_JWT_PUBLIC_KEY:-}"
```

The dog-fooding proposal: extend the configuration layer to support a `{secret_ref: "path"}` syntax alongside the existing `${VAR:-default}` syntax. Resolution is delegated to the configured `SecretsProvider`.

```toml
# Current (still valid):
[common.database]
url = "${DATABASE_URL:-postgresql://localhost/tasker}"

# New (alternative for teams with proper secrets management):
[common.database]
url = {secret_ref = "/production/tasker/database/url"}

# Or with a named provider:
[common.database]
url = {secret_ref = "prod/tasker/database-url#connection_string", provider = "vault"}
```

The config loading layer resolves `{secret_ref: ...}` values through the `SecretsProvider` registered during bootstrap. The `EnvSecretsProvider` remains the default, meaning existing deployments continue to work without change. Teams that have proper secrets management can opt into it.

**Why this matters**: It prevents a class of deployment anti-patterns where Tasker's own credentials (particularly `DATABASE_URL` for the orchestration DB) end up materialized as plaintext in CI/CD pipelines, deployment manifests, or container environment variable dumps.

**Implementation note**: The config resolution happens before any subsystem initialization, so the `SecretsProvider` for Tasker's own config must be bootstrap-able from minimal config (a provider type + provider-specific config, without depending on a database connection or existing secrets). SOPS files and env vars are fully self-contained. Vault requires network access to a Vault server. AWS SSM/SM require IAM credentials. These are all appropriate at bootstrap time for a production system.

---

## The Credential Rotation Problem

This is an open research question explicitly called out for Spike S2 (see `06-research-spikes.md`), but the design must accommodate it.

**The scenario**: Worker has been running for 6 hours. A database password in Vault was rotated by the organization's credential rotation policy (30-day rotation). The `ResourceRegistry` initialized a connection pool with the old credentials. New connections to that pool start failing with auth errors.

**The necessary behavior**: The `ResourceRegistry` must detect auth failures, re-resolve the credential through the `SecretsProvider`, and reinitialize the affected resource pool. Capability executors experience this as a transient error on one step execution; the step retries and succeeds with the new pool.

**Implication for the ResourceHandle trait**:

```rust
pub trait ResourceHandle: Send + Sync + fmt::Debug {
    /// Attempt to refresh credentials from the SecretsProvider.
    /// Called by the ResourceRegistry when an auth error is detected.
    /// Returns Ok if credentials were refreshed; Err if refresh itself fails.
    async fn refresh_credentials(&self, secrets: &dyn SecretsProvider) -> Result<(), ResourceError>;

    /// Check if this handle is healthy (connection pool has usable connections).
    async fn health_check(&self) -> Result<(), ResourceError>;
}
```

The rotation problem is particularly acute with Vault dynamic secrets, where credentials are intentionally short-lived (e.g., 1-hour TTLs for database credentials generated by Vault's database secrets engine). The `VaultSecretsProvider`'s `LeaseManager` handles renewal proactively, but `ResourceHandle::refresh_credentials` is the recovery path when renewal fails and a connection error occurs.

---

## What Stays Out of Scope

**jaq-accessible secrets**: No mechanism will be provided for jaq filters to access secret values directly. If a `params` filter for an `acquire` step needs to include an API key as a query parameter, the solution is for the `acquire` resource handle (an authenticated HTTP client) to inject the key at request time, not for the filter to reference it. The rule is absolute: nothing that flows through jaq is a credential.

**Handler author secrets**: Domain handler authors continue to manage their own credentials however they currently do. The `SecretsProvider` and `ResourceRegistry` are available for them to use if they want to adopt the pattern, but this is not mandated.

**Secrets discovery**: No mechanism for Tasker to discover what secrets an organization has or enumerate available paths. The `SecretsProvider` is pull-only.

---

*Read next: `02-resource-registry.md` for how the `SecretsProvider` connects to capability executor infrastructure.*
