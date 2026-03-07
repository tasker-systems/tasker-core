# Concern 2: Resource Registry

*How named resources decouple capability configs from credentials, and how handles are managed*

*Research spike — March 2026*

---

## The Role of the Resource Registry

The `SecretsProvider` resolves credential values. The `ResourceRegistry` is what those values are used for: initializing connection pools, authenticated HTTP clients, and messaging clients that capability executors can use without ever seeing the underlying credentials.

The registry is the boundary between the secrets layer (credentials as values, resolved once at startup) and the execution layer (capability executors operating on handles with no awareness of credentials). The name of a resource — `"orders-db"`, `"fulfillment-api"`, `"events-bus"` — is what appears in composition configs. The connection details and credentials are resolved separately.

The resource registry pattern is not novel. It is how most mature application frameworks handle database connections (Spring DataSource beans, Rails database.yml, Django DATABASES settings). Tasker's instantiation of the pattern is shaped by two constraints specific to this context: (1) capability executors are grammar-composed and discoverable through MCP, so resource names must be inspectable without exposing configs; and (2) resources are used across namespaces via composition queues, so pool management must work at the worker level, not per-namespace.

---

## ResourceDefinition: Config Without Credentials

The resource definition is what a platform engineer writes in `worker.toml` (or a sidecar config file). It specifies everything needed to initialize the resource except the secret values themselves, which are referenced by path.

```rust
/// A configured resource: everything needed to initialize it, with
/// secret values referenced by path rather than embedded as literals.
#[derive(Debug, Clone, Deserialize)]
pub struct ResourceDefinition {
    /// The name by which capability configs reference this resource.
    /// This is what appears in composition YAML: `resource: {ref: "orders-db"}`
    pub name: String,

    /// What kind of external system this resource connects to.
    pub resource_type: ResourceType,

    /// Configuration parameters — mix of literals and secret references.
    /// Secret references are resolved through SecretsProvider at init time.
    pub config: ResourceConfig,

    /// Optional: which SecretsProvider to use for resolving this resource's secrets.
    /// Defaults to the worker's primary provider.
    pub secrets_provider: Option<String>,
}

/// The type of external system.
/// This determines which ResourceHandle implementation is used.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResourceType {
    /// PostgreSQL database (via sqlx)
    Postgres,
    /// MySQL database (via sqlx)
    MySql,
    /// SQLite (via sqlx) — for local development/testing
    Sqlite,
    /// HTTP/HTTPS API endpoint
    Http,
    /// PGMQ message queue — for emit capability targeting Tasker's own bus
    Pgmq,
    /// Custom resource type — implemented by the worker binary
    Custom { type_name: String },
}

/// A configuration value — either a literal string or a reference to a secret.
///
/// In TOML config:
///   host = "orders-db.internal"          → Literal
///   password = {secret_ref = "/prod/tasker/orders-db/password"}    → SecretRef
///   api_key = {env = "FULFILLMENT_API_KEY"}     → EnvRef (shorthand for EnvSecretsProvider)
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    /// A literal value — not a secret, safe to log.
    Literal(String),

    /// A reference to a secret resolved through the SecretsProvider.
    SecretRef { secret_ref: String },

    /// An environment variable reference — shorthand, resolved by EnvSecretsProvider.
    /// Supported for backward compatibility; semantically identical to
    /// SecretRef with an EnvSecretsProvider.
    EnvRef { env: String },
}
```

**Example worker.toml resource section**:

```toml
[[resources]]
name = "orders-db"
type = "postgres"

[resources.config]
host = "orders-db.internal"
port = "5432"
database = "orders"
user = {secret_ref = "/production/tasker/orders-db/user"}
password = {secret_ref = "/production/tasker/orders-db/password"}
# Pool sizing — these are not secrets
max_connections = "20"
min_connections = "2"
acquire_timeout_seconds = "10"

[[resources]]
name = "fulfillment-api"
type = "http"

[resources.config]
base_url = "https://api.fulfillment.internal"
# The authenticated HTTP client will add this header to all requests
auth_header = "X-API-Key"
auth_value = {secret_ref = "/production/tasker/fulfillment-api/key"}
timeout_ms = "5000"
max_connections_per_host = "50"

[[resources]]
name = "events-bus"
type = "pgmq"
# Uses the same database as Tasker's primary database
# (no additional credentials needed — inherits worker DB config)
```

---

## ResourceHandle: The Abstraction Capability Executors Use

The `ResourceHandle` trait is what capability executors receive. It represents an already-initialized, ready-to-use connection to an external resource. The executor doesn't know or care whether it's a PostgreSQL pool, an HTTP client, or something else — it asks the handle for the specific access type it needs.

```rust
/// A handle to an initialized external resource.
///
/// Capability executors receive `Arc<dyn ResourceHandle>` from the
/// ResourceRegistry. They never see credentials — the handle was
/// initialized with resolved credentials at worker startup.
#[async_trait]
pub trait ResourceHandle: Send + Sync + fmt::Debug {
    /// Human-readable name of the resource this handle represents.
    fn resource_name(&self) -> &str;

    /// The resource type.
    fn resource_type(&self) -> &ResourceType;

    /// Attempt to refresh credentials from the SecretsProvider.
    ///
    /// Called by the ResourceRegistry when an auth error is detected
    /// (connection pool exhausted due to auth failures, HTTP 401/403 responses).
    /// The handle reinitializes itself with freshly resolved credentials.
    ///
    /// Returns Ok if credentials were refreshed successfully.
    /// Returns Err if refresh itself fails (SecretsProvider unavailable, etc.).
    async fn refresh_credentials(
        &self,
        secrets: &dyn SecretsProvider,
    ) -> Result<(), ResourceError>;

    /// Check whether this handle is healthy (pool has usable connections,
    /// client can reach the endpoint, etc.).
    async fn health_check(&self) -> Result<(), ResourceError>;

    /// Downcast to a concrete handle type.
    /// Capability executors use this to get the specific access they need.
    fn as_any(&self) -> &dyn Any;
}

/// Convenience extension: downcast ResourceHandle to a concrete type.
/// Used by capability executors that know what they need.
///
/// Example:
///   let pool = handle.as_postgres()
///       .ok_or_else(|| CapabilityError::WrongResourceType { ... })?;
pub trait ResourceHandleExt: ResourceHandle {
    fn as_postgres(&self) -> Option<&PostgresHandle>;
    fn as_http(&self) -> Option<&HttpHandle>;
    fn as_pgmq(&self) -> Option<&PgmqHandle>;
}
```

### Concrete Handle Types

**PostgresHandle**: Wraps a `sqlx::PgPool`. The `persist` and `acquire` capabilities with `resource.type: database` use this.

```rust
pub struct PostgresHandle {
    name: String,
    pool: Arc<PgPool>,
    config: PostgresResourceConfig,
    /// Stored so credentials can be refreshed if auth fails
    credential_paths: PostgresCredentialPaths,
}

impl PostgresHandle {
    /// Get the connection pool for direct use in sqlx queries.
    pub fn pool(&self) -> &PgPool { &self.pool }
}
```

**HttpHandle**: Wraps a `reqwest::Client` configured with base URL, auth headers, timeouts, and connection pool settings. The `acquire` capability with `resource.type: api` uses this.

```rust
pub struct HttpHandle {
    name: String,
    client: Arc<reqwest::Client>,
    base_url: String,
    config: HttpResourceConfig,
    /// Auth injection strategy — token added to every request
    auth: Arc<dyn HttpAuthStrategy>,
}

pub trait HttpAuthStrategy: Send + Sync {
    /// Apply auth to an outgoing request builder.
    fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder;
    /// Refresh auth (e.g., if token expired). Called on 401 responses.
    async fn refresh(&self, secrets: &dyn SecretsProvider) -> Result<(), ResourceError>;
}

impl HttpHandle {
    /// Build a request with auth pre-applied.
    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        let builder = self.client.get(format!("{}{}", self.base_url, path));
        self.auth.apply(builder)
    }

    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        let builder = self.client.post(format!("{}{}", self.base_url, path));
        self.auth.apply(builder)
    }
}
```

**PgmqHandle**: Wraps a `PgmqClient` (from `tasker-pgmq`) for the `emit` capability when targeting Tasker's own event bus.

---

## ResourceRegistry: Initialization and Lookup

The registry is initialized at worker startup, before any steps are claimed or executed. All resources are initialized (or the startup fails loudly — fail loudly is a Tasker core tenet).

```rust
pub struct ResourceRegistry {
    /// The secrets provider for resolving credential references.
    secrets: Arc<dyn SecretsProvider>,

    /// Initialized resource handles, keyed by resource name.
    resources: RwLock<HashMap<String, Arc<dyn ResourceHandle>>>,

    /// Resource definitions — kept for credential refresh operations.
    definitions: HashMap<String, ResourceDefinition>,
}

impl ResourceRegistry {
    /// Initialize all resources from their definitions.
    /// Called once at worker startup.
    /// Fails loudly if any resource cannot be initialized — a worker
    /// that cannot connect to required resources should not start.
    pub async fn initialize_all(
        secrets: Arc<dyn SecretsProvider>,
        definitions: Vec<ResourceDefinition>,
    ) -> Result<Self, ResourceError>;

    /// Initialize a single resource.
    /// Called for lazy initialization or re-initialization after credential refresh.
    pub async fn initialize_resource(
        &self,
        definition: &ResourceDefinition,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceError>;

    /// Look up an initialized resource by name.
    /// Returns None if the resource is not registered or not yet initialized.
    /// Capability executors use this at the start of their execute() call.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ResourceHandle>>;

    /// Trigger credential refresh for a resource.
    /// Called when a capability executor encounters an auth failure.
    /// Reinitializes the resource handle with freshly resolved credentials.
    pub async fn refresh_resource(
        &self,
        name: &str,
    ) -> Result<(), ResourceError>;

    /// List all registered resource names and their types.
    /// Used by MCP discovery tools — exposes names and types, NOT configs or credentials.
    pub fn list_resources(&self) -> Vec<ResourceSummary>;
}

/// Public-facing resource information for MCP discoverability.
/// Contains name and type only — no configuration or credential details.
pub struct ResourceSummary {
    pub name: String,
    pub resource_type: ResourceType,
    pub description: Option<String>,
    pub healthy: bool,
}
```

### Startup Behavior

The startup sequence for a worker with resources:

1. Load `SecretsProvider` configuration and initialize the provider (verify it's reachable via `health_check()`)
2. Load `ResourceDefinition` list from config
3. For each definition:
   a. Resolve `ConfigValue::SecretRef` values through the `SecretsProvider`
   b. Construct the appropriate handle type with resolved credentials
   c. Call `health_check()` on the new handle (verify connectivity)
   d. Register in the `ResourceRegistry`
4. If any step fails: log the error, **do not start**

This is the "fail loudly" principle applied to infrastructure: a worker that cannot reach its configured resources is broken and should not silently proceed to claim steps that will fail.

**The health check trade-off**: A comprehensive startup health check (verify every database connection, every API endpoint) increases startup time and can cause deployment failures during infrastructure maintenance windows. The health check depth should be configurable:

```toml
[worker.resource_health_check]
# "ping" — just verify connectivity (fast, lightweight)
# "query" — run a test query/request (slower, more thorough)
mode = "ping"
# Retry failed health checks before aborting startup
max_retries = 3
retry_delay_ms = 1000
```

---

## Composition Config → Resource Lookup

When a capability executor runs, the flow from composition config to resource handle is:

```
CompositionStep config:
  resource:
    ref: "orders-db"       ← this is the resource name
    entity: orders          ← entity is resource-specific config

        │
        ▼

CapabilityExecutor::execute(input, config, context)
  ↓
  let resource_name = config["resource"]["ref"].as_str()?;
  let handle = context.resources.get(resource_name)
      .ok_or(CapabilityError::ResourceNotFound { name: resource_name })?;
  let pg_handle = handle.as_postgres()
      .ok_or(CapabilityError::WrongResourceType { ... })?;
  let pool = pg_handle.pool();
  // ... execute the query using pool
```

The `ExecutionContext` passed to every capability executor includes the `ResourceRegistry`:

```rust
pub struct ExecutionContext {
    pub step_uuid: Uuid,
    pub correlation_id: String,
    pub checkpoint: Arc<CheckpointService>,
    pub checkpoint_state: Option<CheckpointRecord>,
    pub step_config: serde_json::Value,
    /// Resource registry — capability executors look up handles here
    pub resources: Arc<ResourceRegistry>,
}
```

---

## MCP Discoverability Boundary

The MCP tool `capability_inspect` shows what a capability can do and what resources it needs. The `grammar_list_resources` tool (added as part of tasker-secure integration) shows what resources are available.

The boundary: resource **names and types** are discoverable. Resource **configs and credentials** are never exposed through MCP, `tasker-ctl`, or any other external interface.

```
grammar_list_resources response:
{
  "resources": [
    {"name": "orders-db", "type": "postgres", "healthy": true},
    {"name": "fulfillment-api", "type": "http", "healthy": true},
    {"name": "events-bus", "type": "pgmq", "healthy": true}
  ]
}

NOT included:
  - Connection strings
  - Hostnames
  - Credentials
  - Secret paths
  - Any config values
```

An agent composing a workflow can discover that `orders-db` exists and is a postgres resource. It cannot discover the host, port, database name, username, or password. This is the correct level of exposure.

---

## What This Means for Capability Executor Stubs in Phase 1C

The Phase 1C grammar testing uses stub implementations that don't need real external infrastructure. The stub design is shaped by the `ResourceHandle` interface established here:

```rust
/// Stub implementation for grammar testing — no real connections.
pub struct InMemoryResourceHandle {
    name: String,
    resource_type: ResourceType,
    /// Canned responses for acquire operations
    fixture_data: HashMap<String, Value>,
    /// Captured persist operations for test assertions
    persisted: Arc<Mutex<Vec<Value>>>,
    /// Captured emit operations for test assertions
    emitted: Arc<Mutex<Vec<Value>>>,
}

impl ResourceHandle for InMemoryResourceHandle { ... }

/// Test registry populated with in-memory handles.
/// Used in grammar-level tests with no database.
pub fn test_registry_with_fixtures(fixtures: Vec<ResourceFixture>) -> ResourceRegistry;
```

This is the specific interface the grammar executor stubs depend on. The design here — `ResourceHandle` trait, `ResourceRegistry::get()`, `ExecutionContext::resources` — must be stable before Phase 1C begins.

---

*Read next: `03-trace-and-log-safety.md` for how sensitive data in legitimate step contexts is protected at the observability boundary.*
