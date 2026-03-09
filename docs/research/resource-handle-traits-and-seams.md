# Resource Handle Traits and Architectural Seams

*Design for operation-level resource traits, crate topology, and composition worker segmentation*

*March 2026 — Research and Design*

*Branch: `jcoletaylor/resource-handle-traits-and-seams`*

---

## Problem Statement

The Tasker Action Grammar project (Phase 1C) defines three side-effecting capability executors — `persist`, `acquire`, and `emit` — that interact with external resources (databases, APIs, message buses). The `tasker-secure` crate (TAS-357, TAS-358) delivers the `ResourceHandle` abstraction and `ResourceRegistry` for named resource lifecycle. The question is: **where does the actual I/O logic live, how are resource pools managed at runtime, and how does this decompose into crate responsibilities?**

This question becomes urgent for two reasons:

1. **Generative workflows** create task templates at runtime. Templates reference resources via `resource_ref` that may not have existed when the worker booted. This breaks the static "initialize all resources at startup" assumption in the current `ResourceRegistry` design.

2. **Compositional action grammar handlers** represent a fundamentally different workload profile than domain handler dispatch. Domain handlers delegate I/O to the codebase where Tasker is embedded (Rails, Django, etc.). Grammar-composed handlers delegate I/O to Tasker itself — meaning Tasker must own connection pooling, backpressure, and resource lifecycle at the infrastructure layer.

### What This Document Covers

- Three distinct concerns and where each lives (identity vs. operations vs. implementation)
- Operation-level traits that grammar capability executors call through
- Adapter pattern bridging grammar operations to secure resource handles
- Crate topology: tasker-secure, tasker-grammar, tasker-runtime, tasker-worker, tasker-rs
- ResourcePoolManager design for dynamic resource lifecycle
- Worker segmentation: composition workers vs. domain workers
- StepContext / CompositionExecutionContext boundary for handler dispatch
- Backpressure and pool management policies
- Relationship to existing tickets and what changes

### Related Documents

- `docs/research/security-and-secrets/00-problem-statement.md` — The three security concerns
- `docs/research/security-and-secrets/02-resource-registry.md` — ResourceHandle and ResourceRegistry design
- `docs/action-grammar/grammar-trait-boundary.md` — Grammar trait system and integration with handler dispatch
- `docs/action-grammar/actions-traits-and-capabilities.md` — Foundational grammar architecture
- `docs/action-grammar/transform-revised-grammar.md` — 6-capability model with jaq-core
- `docs/action-grammar/implementation-phases.md` — Phase 1 roadmap

### Related Tickets

- **TAS-357** (S1: SecretsProvider Foundation) — In Review. Credential resolution layer.
- **TAS-358** (S2: ResourceRegistry and ResourceHandle) — Done. Named resource lifecycle types.
- **TAS-369** (ConfigString into tasker-shared) — Backlog. Milestone 1.5 integration.
- **TAS-370** (ExecutionContext in tasker-worker) — Backlog. Milestone 1.5 integration. Design revised by this document.
- **TAS-330** (persist capability executor) — Backlog. Phase 1C grammar work. Informed by this document.
- **TAS-331** (acquire capability executor) — Backlog. Phase 1C grammar work. Informed by this document.
- **TAS-332** (emit capability executor) — Backlog. Phase 1C grammar work. Informed by this document.

---

## Key Insight: The Action Is the Orchestration, Not the I/O

A `persist` capability executor does six things:

1. Parse and validate the capability config (pure)
2. Evaluate the `data` jaq expression against the composition envelope (pure)
3. Look up the operation interface from the execution context (lookup)
4. Execute the write through the operation trait (I/O, behind abstraction)
5. Evaluate the `validate_success` expression against the result (pure)
6. Evaluate the `result_shape` expression to extract the output (pure)

Steps 1, 2, 5, and 6 are pure data transformations — jaq evaluation, config parsing, JSON manipulation. Step 3 is a lookup. Step 4 is the one I/O operation — and it happens entirely behind a trait boundary.

The critical insight: **step 4 doesn't require the capability executor to know anything about SQL, HTTP, or message protocols.** The executor asks a trait method to perform a structured operation — "persist this data to this entity with these constraints" — and the implementation behind the trait translates that into resource-specific I/O.

This is the same separation that exists in every mature application framework. A Rails controller calls `@order.save!` — the controller orchestrates (validate, persist, respond), ActiveRecord generates SQL, and the database adapter manages the connection. The "save" action isn't "spread across" the controller and the adapter — it's cleanly separated by abstraction level.

The grammar capability executor owns the **full action** — not a stub, not a delegation, but the complete orchestration pipeline from config parsing through result shaping. The resource operation trait provides a **capability** (structured I/O against a specific backend) that the executor calls through.

---

## Three Concerns, Three Crates

The architecture separates three distinct responsibilities that initially appear entangled but have fundamentally different reasons to exist and different reasons to change:

### Concern 1: Resource Identity, Credentials, and Connection Lifecycle (tasker-secure)

**Question answered**: "How do I get a handle to a resource that requires secure access?"

This is what TAS-357 and TAS-358 built. `tasker-secure` manages:

- **Credential resolution**: `SecretsProvider`, `SecretValue`, `ChainedSecretsProvider`, `EnvSecretsProvider`, `SopsSecretsProvider`
- **Resource registration**: `ResourceDefinition`, `ConfigValue`, `ResourceRegistry`
- **Connection handles**: `PostgresHandle` (wraps `PgPool`), `HttpHandle` (wraps `reqwest::Client`), `PgmqHandle` (wraps `PgmqClient`)
- **Handle abstraction**: `ResourceHandle` trait (health check, credential refresh, downcast), `ResourceHandleExt` for typed downcasts (`as_postgres()`, `as_http()`, `as_pgmq()`)

`PostgresHandle` gives you a connection pool. `HttpHandle` gives you an authenticated HTTP client. These handles don't know or care what you're going to do with them. They are infrastructure concerns — identity, credentials, connectivity.

### Concern 2: Operation Contracts (tasker-grammar)

**Question answered**: "What kinds of structured operations can grammar capability executors ask resources to perform?"

These are *grammar-shaped* abstractions. They exist because `persist`, `acquire`, and `emit` capability executors need a contract to call against. If the grammar didn't exist, these traits wouldn't need to exist. They describe the grammar's requirements of resources, not the resources' own nature.

`PersistableResource`, `AcquirableResource`, and `EmittableResource` are the grammar's language for resource interaction. They live in `tasker-grammar` alongside the capability executors that call through them, because:

- The trait signatures are shaped by what the grammar needs (entity, structured data, constraints, metadata) — not by what the resource provides (a connection pool, an HTTP client)
- The constraint and result types (`PersistConstraints`, `AcquireResult`, `EmitMetadata`) are grammar concepts — they describe the vocabulary of operations that grammar compositions can express
- The in-memory test implementations are grammar test utilities — they verify that the executor orchestration pipeline works correctly, independent of any real I/O
- If the grammar's needs evolve (new constraint types, richer result shapes), the traits evolve with it — without touching tasker-secure

### Concern 3: Adapters, Lifecycle, and Runtime Execution (tasker-runtime)

**Question answered**: "How does a grammar operation actually execute against a live resource, and who manages the resource pools at runtime?"

This is the bridge between grammar abstractions and secure infrastructure. `tasker-runtime` provides:

- **Adapters**: `PostgresPersistAdapter` wraps a `PostgresHandle` and implements `PersistableResource` by generating SQL. `HttpAcquireAdapter` wraps an `HttpHandle` and implements `AcquirableResource` by constructing HTTP requests. Each adapter translates the grammar's structured operation into resource-specific I/O.
- **Pool lifecycle**: `ResourcePoolManager` wraps `ResourceRegistry` with eviction, backpressure, and lazy initialization for dynamic resources.
- **Context construction**: `CompositionExecutionContext` wiring — resolving resource_ref names to operation trait objects via adapters.
- **Runtime policy**: Eviction strategies, connection budgets, admission control.

The name `tasker-runtime` captures that this is where grammar concepts meet operational reality — the execution runtime for grammar compositions. It is distinct from `tasker-worker` (which is the step lifecycle runtime for all handlers).

---

## Operation-Level Resource Traits

### Trait Definitions (in tasker-grammar)

These traits define the grammar's interface for resource operations. They are what capability executors call through. They know nothing about PostgreSQL, HTTP, or any specific backend — they speak the grammar's language of entities, structured data, and constraints.

```rust
// tasker-grammar::operations

/// A resource operation that can accept structured write operations.
///
/// Grammar capability executors (PersistExecutor) call through this trait.
/// Implementations live in tasker-runtime as adapters wrapping tasker-secure
/// handles. Test implementations live in tasker-grammar as in-memory fixtures.
#[async_trait]
pub trait PersistableResource: Send + Sync {
    /// Execute a write operation.
    ///
    /// # Arguments
    /// * `entity` - The target (table name, API path, queue name, etc.)
    /// * `data` - The data to persist, constructed by jaq expression
    /// * `constraints` - Operational constraints (upsert keys, conflict resolution)
    ///
    /// # Returns
    /// A structured result representing the operation outcome — this becomes
    /// the input to `validate_success` and `result_shape` expressions.
    async fn persist(
        &self,
        entity: &str,
        data: serde_json::Value,
        constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError>;
}

/// A resource operation that can serve structured read operations.
///
/// Grammar capability executors (AcquireExecutor) call through this trait.
#[async_trait]
pub trait AcquirableResource: Send + Sync {
    /// Execute a read operation.
    ///
    /// # Arguments
    /// * `entity` - The source (table name, API path, etc.)
    /// * `params` - Query parameters, constructed by jaq expression
    /// * `constraints` - Operational constraints (pagination, timeouts, limits)
    async fn acquire(
        &self,
        entity: &str,
        params: serde_json::Value,
        constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError>;
}

/// A resource operation that can accept event/message publication.
///
/// Grammar capability executors (EmitExecutor) call through this trait.
#[async_trait]
pub trait EmittableResource: Send + Sync {
    /// Publish an event or message.
    ///
    /// # Arguments
    /// * `topic` - The destination (queue name, webhook path, topic ARN, etc.)
    /// * `payload` - The event payload, constructed by jaq expression
    /// * `metadata` - Event metadata (correlation, idempotency key, headers)
    async fn emit(
        &self,
        topic: &str,
        payload: serde_json::Value,
        metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError>;
}
```

### Constraint and Result Types (in tasker-grammar)

These types are grammar vocabulary — they describe what a composition can express about its resource operations.

```rust
// tasker-grammar::operations::types

/// Constraints for persist operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistConstraints {
    /// Keys for upsert conflict resolution (e.g., ["id"], ["order_id", "line_number"])
    pub upsert_key: Option<Vec<String>>,
    /// Conflict resolution strategy
    pub on_conflict: Option<ConflictStrategy>,
    /// Idempotency key for at-most-once semantics
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictStrategy {
    /// Error on conflict (default)
    Reject,
    /// Update existing record
    Update,
    /// Skip the conflicting row
    Skip,
}

/// Result of a persist operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistResult {
    /// The raw result data (affected rows, returned record, API response)
    pub data: serde_json::Value,
    /// Number of rows/records affected
    pub affected_count: Option<u64>,
}

/// Constraints for acquire operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquireConstraints {
    /// Maximum number of records to return
    pub limit: Option<u64>,
    /// Pagination offset
    pub offset: Option<u64>,
    /// Request timeout override
    pub timeout_ms: Option<u64>,
}

/// Result of an acquire operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquireResult {
    /// The acquired data
    pub data: serde_json::Value,
    /// Total count if available (for pagination)
    pub total_count: Option<u64>,
}

/// Metadata for emit operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmitMetadata {
    /// Correlation ID for event tracing
    pub correlation_id: Option<String>,
    /// Idempotency key for at-most-once delivery
    pub idempotency_key: Option<String>,
    /// Additional headers/attributes for the event
    pub attributes: Option<HashMap<String, String>>,
}

/// Result of an emit operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmitResult {
    /// The publish confirmation data (message ID, timestamp, etc.)
    pub data: serde_json::Value,
    /// Whether delivery was confirmed by the target
    pub confirmed: bool,
}

/// Errors from resource operations.
///
/// Distinct from tasker-secure's ResourceError (which covers initialization,
/// health check, and credential refresh). These errors describe operation-level
/// failures — the things that go wrong when you try to use a resource, not
/// when you try to connect to it.
#[derive(Debug, thiserror::Error)]
pub enum ResourceOperationError {
    #[error("Entity not found: {entity}")]
    EntityNotFound { entity: String },

    #[error("Conflict on persist to {entity}: {reason}")]
    Conflict { entity: String, reason: String },

    #[error("Authorization failed for {operation} on {entity}")]
    AuthorizationFailed { operation: String, entity: String },

    #[error("Resource unavailable: {message}")]
    Unavailable { message: String },

    #[error("Operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("Resource operation error: {message}")]
    Other {
        message: String,
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}
```

### In-Memory Test Implementation (in tasker-grammar)

The grammar crate provides its own test implementation of the operation traits. This is not the `InMemoryResourceHandle` from tasker-secure (which is a `ResourceHandle` — a connection abstraction). This is an operation-level test double that the grammar's own test suite uses.

```rust
// tasker-grammar::operations::testing

/// In-memory implementation of all operation traits for grammar testing.
///
/// Provides fixture data for acquire operations and capture lists
/// for persist and emit operations. Used by capability executor tests
/// to verify the full orchestration pipeline with zero I/O.
pub struct InMemoryOperations {
    /// Canned responses for acquire operations, keyed by entity name
    fixture_data: HashMap<String, Vec<serde_json::Value>>,
    /// Captured persist operations for test assertions
    persisted: Arc<Mutex<Vec<CapturedPersist>>>,
    /// Captured emit operations for test assertions
    emitted: Arc<Mutex<Vec<CapturedEmit>>>,
}

#[derive(Debug, Clone)]
pub struct CapturedPersist {
    pub entity: String,
    pub data: serde_json::Value,
    pub constraints: PersistConstraints,
}

#[derive(Debug, Clone)]
pub struct CapturedEmit {
    pub topic: String,
    pub payload: serde_json::Value,
    pub metadata: EmitMetadata,
}

#[async_trait]
impl PersistableResource for InMemoryOperations {
    async fn persist(
        &self,
        entity: &str,
        data: serde_json::Value,
        constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        self.persisted.lock().await.push(CapturedPersist {
            entity: entity.to_string(),
            data: data.clone(),
            constraints: constraints.clone(),
        });
        Ok(PersistResult {
            data,
            affected_count: Some(1),
        })
    }
}

#[async_trait]
impl AcquirableResource for InMemoryOperations {
    async fn acquire(
        &self,
        entity: &str,
        _params: serde_json::Value,
        constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        let records = self.fixture_data.get(entity)
            .ok_or(ResourceOperationError::EntityNotFound {
                entity: entity.to_string(),
            })?;
        let data = serde_json::Value::Array(
            records.iter()
                .take(constraints.limit.unwrap_or(u64::MAX) as usize)
                .cloned()
                .collect()
        );
        Ok(AcquireResult {
            data,
            total_count: Some(records.len() as u64),
        })
    }
}

#[async_trait]
impl EmittableResource for InMemoryOperations {
    async fn emit(
        &self,
        topic: &str,
        payload: serde_json::Value,
        metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError> {
        self.emitted.lock().await.push(CapturedEmit {
            topic: topic.to_string(),
            payload,
            metadata: metadata.clone(),
        });
        Ok(EmitResult {
            data: serde_json::json!({"message_id": "test-msg-001"}),
            confirmed: true,
        })
    }
}

/// Convenience builder for test contexts with in-memory operations.
pub fn test_operations_with_fixtures(
    fixtures: HashMap<String, Vec<serde_json::Value>>,
) -> InMemoryOperations {
    InMemoryOperations {
        fixture_data: fixtures,
        persisted: Arc::new(Mutex::new(Vec::new())),
        emitted: Arc::new(Mutex::new(Vec::new())),
    }
}
```

---

## Adapters: Bridging Grammar Operations to Secure Handles (tasker-runtime)

The adapter pattern is the core of `tasker-runtime`. Each adapter wraps a `tasker-secure` handle and implements a `tasker-grammar` operation trait, translating the grammar's structured operations into resource-specific I/O.

### PostgresPersistAdapter

```rust
// tasker-runtime::adapters::postgres

use tasker_secure::resource::PostgresHandle;
use tasker_grammar::operations::{
    PersistableResource, AcquirableResource,
    PersistConstraints, PersistResult,
    AcquireConstraints, AcquireResult,
    ResourceOperationError,
};

/// Adapts a PostgresHandle to the grammar's PersistableResource interface.
///
/// Translates structured persist operations into parameterized SQL.
/// Owns the SQL generation logic — the grammar executor never sees SQL,
/// and the PostgresHandle never sees grammar concepts.
pub struct PostgresPersistAdapter {
    handle: Arc<PostgresHandle>,
}

#[async_trait]
impl PersistableResource for PostgresPersistAdapter {
    async fn persist(
        &self,
        entity: &str,
        data: serde_json::Value,
        constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        let pool = self.handle.pool();

        // Translate grammar's structured operation into SQL
        let obj = data.as_object()
            .ok_or(ResourceOperationError::ValidationFailed {
                message: "persist data must be a JSON object".into(),
            })?;

        let columns: Vec<&String> = obj.keys().collect();
        let placeholders: Vec<String> = (1..=columns.len())
            .map(|i| format!("${i}"))
            .collect();

        let mut sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            entity,  // Note: entity validation/sanitization needed
            columns.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", "),
            placeholders.join(", "),
        );

        // Handle upsert constraints
        if let Some(ref upsert_keys) = constraints.upsert_key {
            let conflict_cols = upsert_keys.join(", ");
            let update_cols: Vec<String> = columns.iter()
                .filter(|c| !upsert_keys.contains(c))
                .map(|c| format!("{c} = EXCLUDED.{c}"))
                .collect();
            match constraints.on_conflict.as_ref().unwrap_or(&ConflictStrategy::Reject) {
                ConflictStrategy::Reject => {
                    // Default INSERT behavior — conflict raises error
                }
                ConflictStrategy::Update => {
                    sql.push_str(&format!(
                        " ON CONFLICT ({conflict_cols}) DO UPDATE SET {}",
                        update_cols.join(", ")
                    ));
                }
                ConflictStrategy::Skip => {
                    sql.push_str(&format!(
                        " ON CONFLICT ({conflict_cols}) DO NOTHING"
                    ));
                }
            }
        }

        sql.push_str(" RETURNING *");

        // Bind values and execute
        // (simplified — actual implementation handles type mapping)
        let result = sqlx::query(&sql)
            .execute(pool)
            .await
            .map_err(|e| ResourceOperationError::Other {
                message: e.to_string(),
                source: Some(Box::new(e)),
            })?;

        Ok(PersistResult {
            data: serde_json::json!({}), // RETURNING * row would be mapped here
            affected_count: Some(result.rows_affected()),
        })
    }
}
```

### HttpAcquireAdapter

```rust
// tasker-runtime::adapters::http

use tasker_secure::resource::HttpHandle;
use tasker_grammar::operations::{
    AcquirableResource, AcquireConstraints, AcquireResult,
    ResourceOperationError,
};

/// Adapts an HttpHandle to the grammar's AcquirableResource interface.
///
/// Translates structured acquire operations into HTTP GET requests.
pub struct HttpAcquireAdapter {
    handle: Arc<HttpHandle>,
}

#[async_trait]
impl AcquirableResource for HttpAcquireAdapter {
    async fn acquire(
        &self,
        entity: &str,
        params: serde_json::Value,
        constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        let mut request = self.handle.get(entity);

        // Map params to query string
        if let Some(obj) = params.as_object() {
            for (key, value) in obj {
                let str_val = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                request = request.query(&[(key.as_str(), str_val.as_str())]);
            }
        }

        // Apply constraints
        if let Some(limit) = constraints.limit {
            request = request.query(&[("limit", limit.to_string().as_str())]);
        }
        if let Some(offset) = constraints.offset {
            request = request.query(&[("offset", offset.to_string().as_str())]);
        }
        if let Some(timeout) = constraints.timeout_ms {
            request = request.timeout(Duration::from_millis(timeout));
        }

        let response = request.send().await
            .map_err(|e| ResourceOperationError::Unavailable {
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            if response.status() == reqwest::StatusCode::UNAUTHORIZED
                || response.status() == reqwest::StatusCode::FORBIDDEN
            {
                return Err(ResourceOperationError::AuthorizationFailed {
                    operation: "acquire".into(),
                    entity: entity.into(),
                });
            }
            return Err(ResourceOperationError::Other {
                message: format!("HTTP {}", response.status()),
                source: None,
            });
        }

        let data: serde_json::Value = response.json().await
            .map_err(|e| ResourceOperationError::Other {
                message: e.to_string(),
                source: Some(Box::new(e)),
            })?;

        Ok(AcquireResult {
            data,
            total_count: None, // Could parse from headers (X-Total-Count, etc.)
        })
    }
}
```

### Adapter Matrix

| Adapter | Wraps | Implements | Translation |
|---------|-------|------------|-------------|
| `PostgresPersistAdapter` | `PostgresHandle` | `PersistableResource` | Structured data → INSERT/UPSERT SQL |
| `PostgresAcquireAdapter` | `PostgresHandle` | `AcquirableResource` | Params → SELECT SQL |
| `HttpPersistAdapter` | `HttpHandle` | `PersistableResource` | Structured data → POST/PUT request |
| `HttpAcquireAdapter` | `HttpHandle` | `AcquirableResource` | Params → GET request |
| `HttpEmitAdapter` | `HttpHandle` | `EmittableResource` | Payload → POST (webhook) |
| `PgmqEmitAdapter` | `PgmqHandle` | `EmittableResource` | Payload → pgmq send |

Each adapter is a focused translation layer. It knows exactly two things: the grammar's operation contract and the specific handle's I/O protocol. It doesn't know about jaq expressions, composition context, checkpointing, or capability config — those are the executor's concern. It doesn't know about credentials, health checks, or pool sizing — those are the handle's concern.

### Adapter Registration and Discovery

Adapters are registered at startup in `tasker-runtime`, mapping `(ResourceType, OperationTrait)` pairs to adapter constructors:

```rust
// tasker-runtime::adapters::registry

/// Maps resource types to their available operation adapters.
///
/// When the CompositionExecutionContext resolves a resource_ref,
/// it uses this registry to wrap the ResourceHandle in the
/// appropriate adapter for the requested operation.
pub struct AdapterRegistry {
    persist_adapters: HashMap<ResourceType, Arc<dyn PersistAdapterFactory>>,
    acquire_adapters: HashMap<ResourceType, Arc<dyn AcquireAdapterFactory>>,
    emit_adapters: HashMap<ResourceType, Arc<dyn EmitAdapterFactory>>,
}

pub trait PersistAdapterFactory: Send + Sync {
    fn create(&self, handle: Arc<dyn ResourceHandle>) -> Arc<dyn PersistableResource>;
}

impl AdapterRegistry {
    pub fn standard() -> Self {
        let mut registry = Self::new();
        // Register built-in adapters
        registry.register_persist(ResourceType::Postgres, PostgresPersistAdapterFactory);
        registry.register_persist(ResourceType::Http, HttpPersistAdapterFactory);
        registry.register_acquire(ResourceType::Postgres, PostgresAcquireAdapterFactory);
        registry.register_acquire(ResourceType::Http, HttpAcquireAdapterFactory);
        registry.register_emit(ResourceType::Pgmq, PgmqEmitAdapterFactory);
        registry.register_emit(ResourceType::Http, HttpEmitAdapterFactory);
        registry
    }

    /// Resolve a resource handle into a PersistableResource.
    /// Returns None if no adapter exists for this resource type.
    pub fn as_persistable(
        &self,
        handle: Arc<dyn ResourceHandle>,
    ) -> Option<Arc<dyn PersistableResource>> {
        self.persist_adapters
            .get(handle.resource_type())
            .map(|factory| factory.create(handle))
    }

    // ... as_acquirable(), as_emittable() similarly
}
```

This is extensible. Organizations using `tasker-runtime` can register custom adapters for custom resource types — the same build-from-source extensibility model as the grammar categories and capability executors.

---

## Crate Topology

### Detailed Responsibilities

```
tasker-secure               "resource identity, credentials, connection lifecycle"
  ├── secrets/              SecretsProvider, SecretValue, ChainedSecretsProvider,
  │                         EnvSecretsProvider, SopsSecretsProvider
  ├── resource/             ResourceHandle trait, ResourceRegistry, ResourceDefinition,
  │                         ConfigValue, ResourceSummary, ResourceType
  ├── handles/              PostgresHandle (wraps PgPool), HttpHandle (wraps reqwest::Client),
  │                         PgmqHandle (wraps PgmqClient)
  │                         Each exposes its inner connection for adapter use:
  │                         PostgresHandle::pool(), HttpHandle::get/post(), etc.
  ├── testing/              InMemoryResourceHandle (implements ResourceHandle trait,
  │                         provides fixture data and capture lists at the handle level)
  └── lib.rs                Re-exports, feature documentation

tasker-grammar              "grammar types, operation contracts, capability execution"
  ├── types/                GrammarCategory, CapabilityDeclaration, CompositionSpec,
  │                         CompositionStep, OutcomeDeclaration
  ├── expression/           ExpressionEngine (jaq-core wrapper, sandboxing)
  ├── operations/           PersistableResource, AcquirableResource, EmittableResource
  │   ├── traits.rs         The operation trait definitions
  │   ├── types.rs          PersistConstraints, AcquireConstraints, EmitMetadata,
  │   │                     PersistResult, AcquireResult, EmitResult,
  │   │                     ResourceOperationError, ConflictStrategy
  │   └── testing.rs        InMemoryOperations (implements all operation traits
  │                         with fixtures and capture lists — grammar-level test double)
  ├── capabilities/         PersistExecutor, AcquireExecutor, EmitExecutor,
  │                         TransformExecutor, ValidateExecutor, AssertExecutor
  │                         Each owns the FULL action orchestration pipeline:
  │                         config parse → expression eval → operation call →
  │                         result validation → output shaping
  ├── validation/           CompositionValidator (contract chaining, schema checks)
  ├── executor/             CompositionExecutor (standalone, not a StepHandler)
  └── context.rs            OperationProvider trait — the interface that executors
                            use to obtain operation trait objects (see below)

tasker-runtime              "adapters, pool lifecycle, composition worker runtime"
  ├── adapters/             PostgresPersistAdapter, PostgresAcquireAdapter,
  │                         HttpPersistAdapter, HttpAcquireAdapter,
  │                         HttpEmitAdapter, PgmqEmitAdapter,
  │                         AdapterRegistry (maps ResourceType → adapter factory)
  ├── pool_manager/         ResourcePoolManager, EvictionConfig, BackpressurePolicy,
  │                         PoolAdmissionControl, ResourceAccessMetrics, ResourceOrigin
  ├── sources/              ResourceDefinitionSource trait, SopsFileWatcher,
  │                         StaticConfigSource, ChainedDefinitionSource
  └── context/              CompositionExecutionContext construction — wires
                            ResourcePoolManager + AdapterRegistry into the
                            OperationProvider interface that tasker-grammar expects

tasker-worker               "step lifecycle, handler dispatch, FFI bridges"
  ├── dispatch/             StepHandler trait, HandlerDispatchService, ResolverChain
  ├── context/              StepContext (renamed from TaskSequenceStep)
  ├── ffi/                  Ruby, Python, TypeScript dispatch bridges
  ├── lifecycle/            Step state machine, graceful shutdown, retry semantics
  └── events/               In-process event bus, PGMQ/RabbitMQ boundaries
  Unchanged — domain handler dispatch envelope. No grammar knowledge.

tasker-rs                   "composition-capable worker binary"
  ├── GrammarActionResolver (registered in ResolverChain at priority 15)
  ├── Worker startup: init ResourcePoolManager, register adapters,
  │   register grammar resolvers, subscribe to composition queues
  └── Binary that composes tasker-worker + tasker-runtime
```

### Dependency Graph

```
                    tasker-secure
                   /             \
           tasker-grammar    tasker-worker
                 \               /
               tasker-runtime   /
                    \          /
                    tasker-rs (binary)
```

Key properties:
- **tasker-grammar** depends on `tasker-secure` only for `ResourceHandle`, `ResourceType`, and `ResourceHandleExt` — the types it needs to express "give me a handle and tell me what type it is." It does NOT depend on any feature-gated I/O deps (sqlx, reqwest). Its own operation traits and test doubles are self-contained.
- **tasker-grammar** does NOT depend on `tasker-worker` or `tasker-runtime`.
- **tasker-worker** does NOT depend on `tasker-grammar` or `tasker-runtime`.
- **tasker-runtime** depends on both `tasker-secure` (for concrete handles) and `tasker-grammar` (for operation traits it implements).
- **tasker-rs** composes everything into the composition-capable worker.
- Domain worker binaries depend only on `tasker-worker` (and domain crates).

### Why tasker-grammar Depends on tasker-secure at All

The grammar's operation traits don't reference `PostgresHandle` or `PgPool` — they're purely abstract. However, the grammar's `OperationProvider` interface (see next section) needs to be able to look up operation trait objects by resource name, which requires knowing about `ResourceType` (to distinguish "this is a postgres resource" from "this is an HTTP resource" in error messages). It also helps for the `InMemoryResourceHandle` in tasker-secure's test-utils to remain usable as a building block.

If this dependency feels too heavy, an alternative is to extract just the `ResourceType` enum and the `ResourceHandle` trait into a tiny `tasker-resource-api` crate that both tasker-grammar and tasker-secure depend on. This is an option to evaluate if the dependency weight becomes a concern — but for now, tasker-grammar depending on tasker-secure for a handful of trait definitions is lightweight and pragmatic.

### The OperationProvider Interface

The grammar executors need a way to obtain operation trait objects without knowing about adapters, handles, or the pool manager. This is the seam between "what the grammar needs" and "how the runtime provides it":

```rust
// tasker-grammar::context

/// The interface that grammar capability executors use to obtain
/// operation trait objects for named resources.
///
/// Implemented by tasker-runtime's CompositionExecutionContext,
/// which resolves resource_refs through the ResourcePoolManager
/// and wraps handles in the appropriate adapters.
///
/// Implemented by tasker-grammar's test utilities with InMemoryOperations.
#[async_trait]
pub trait OperationProvider: Send + Sync {
    /// Get a PersistableResource for a named resource.
    /// Returns an error if the resource doesn't exist or doesn't
    /// support persist operations.
    async fn get_persistable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError>;

    /// Get an AcquirableResource for a named resource.
    async fn get_acquirable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError>;

    /// Get an EmittableResource for a named resource.
    async fn get_emittable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn EmittableResource>, ResourceOperationError>;
}
```

In tests:

```rust
// tasker-grammar test — provides InMemoryOperations directly
let ops = test_operations_with_fixtures(fixtures);
let provider = InMemoryOperationProvider::new(ops);
let context = test_execution_context(provider);

// PersistExecutor calls context.get_persistable("orders-db")
// → returns the InMemoryOperations instance
// → persist() pushes to capture list
```

At runtime (in tasker-runtime):

```rust
// tasker-runtime — resolves through pool manager + adapter registry
impl OperationProvider for RuntimeOperationProvider {
    async fn get_persistable(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> {
        let handle = self.pool_manager
            .get_or_initialize(resource_ref)
            .await
            .map_err(/* ... */)?;

        self.adapter_registry
            .as_persistable(handle)
            .ok_or(ResourceOperationError::ValidationFailed {
                message: format!(
                    "Resource '{}' (type {:?}) does not support persist operations",
                    resource_ref,
                    handle.resource_type(),
                ),
            })
    }
}
```

This is the clean seam. The grammar executor calls `context.get_persistable("orders-db")` and gets back `Arc<dyn PersistableResource>`. It has no idea whether that's an `InMemoryOperations` in a test or a `PostgresPersistAdapter` wrapping a live `PostgresHandle` in production. The orchestration is the same either way.

---

## ResourcePoolManager Design

### Purpose

The `ResourcePoolManager` wraps `ResourceRegistry` with lifecycle management for dynamic resource pools. It lives in `tasker-runtime` because it is a runtime operational concern — when to initialize pools, when to evict them, and when to refuse new ones. The registry in `tasker-secure` handles the actual initialization and lookup mechanics; the pool manager handles the policy.

### Structure

```rust
// tasker-runtime::pool_manager

/// Manages the lifecycle of resource pools within a composition worker.
///
/// Static resources (from worker.toml) are initialized at startup and never evicted.
/// Dynamic resources (from runtime template resource_refs) are lazily initialized
/// on first access and subject to eviction policies.
pub struct ResourcePoolManager {
    /// The underlying registry — handles actual init/lookup
    registry: Arc<ResourceRegistry>,

    /// The secrets provider for initializing new resources
    secrets: Arc<dyn SecretsProvider>,

    /// Known resource definitions — both static (boot config) and dynamic (runtime)
    definitions: RwLock<HashMap<String, ResourceDefinition>>,

    /// Access tracking for eviction decisions
    access_metrics: RwLock<HashMap<String, ResourceAccessMetrics>>,

    /// Sources for resolving new resource definitions at runtime
    definition_sources: Vec<Arc<dyn ResourceDefinitionSource>>,

    /// Pool management configuration
    config: PoolManagerConfig,

    /// Aggregate connection tracking for budget enforcement
    connection_budget: AtomicU64,
}

/// How a resource entered the pool manager.
#[derive(Debug, Clone)]
pub enum ResourceOrigin {
    /// From worker.toml [[resources]] — never evicted, always refreshed on failure
    Static,
    /// From runtime template resource_ref resolution
    Dynamic {
        first_seen: Instant,
        source_template: Option<String>,
    },
}

/// Per-resource usage tracking for eviction decisions.
#[derive(Debug)]
pub struct ResourceAccessMetrics {
    pub origin: ResourceOrigin,
    pub last_accessed: RwLock<Instant>,
    pub access_count: AtomicU64,
    pub created_at: Instant,
    pub estimated_connections: u32,
}
```

### Configuration

```rust
/// Configuration for pool lifecycle management.
#[derive(Debug, Clone, Deserialize)]
pub struct PoolManagerConfig {
    /// Hard ceiling on distinct managed pools (static + dynamic)
    pub max_pools: usize,

    /// Aggregate connection budget across all pools
    pub max_total_connections: u64,

    /// How long an unused dynamic resource pool lives before eviction
    pub idle_timeout: Duration,

    /// How often to run the eviction sweep
    pub sweep_interval: Duration,

    /// Eviction strategy when at capacity and a new pool is requested
    pub eviction_strategy: EvictionStrategy,

    /// What to do when a new pool would exceed the connection budget
    pub budget_exceeded_strategy: BudgetExceededStrategy,
}

#[derive(Debug, Clone, Deserialize)]
pub enum EvictionStrategy {
    /// Evict the least recently used dynamic pool
    Lru,
    /// Evict the least recently used, weighted by access frequency
    LruWeighted,
}

#[derive(Debug, Clone, Deserialize)]
pub enum BudgetExceededStrategy {
    /// Initialize the new pool with fewer connections than requested
    ReducePoolSize,
    /// Reject the request with a retriable error
    Reject,
}
```

Example `worker.toml` configuration:

```toml
[worker.resource_pool_manager]
max_pools = 25
max_total_connections = 500
idle_timeout_seconds = 300
sweep_interval_seconds = 60
eviction_strategy = "lru"
budget_exceeded_strategy = "reduce_pool_size"
```

### Core Operations

```rust
impl ResourcePoolManager {
    /// Initialize from static config at worker startup.
    /// Fails loudly if any static resource cannot be initialized.
    pub async fn initialize(
        secrets: Arc<dyn SecretsProvider>,
        static_definitions: Vec<ResourceDefinition>,
        definition_sources: Vec<Arc<dyn ResourceDefinitionSource>>,
        config: PoolManagerConfig,
    ) -> Result<Self, ResourceError>;

    /// Get a resource handle, lazily initializing if needed.
    ///
    /// Fast path: resource already initialized → touch metrics, return handle.
    /// Slow path: resolve definition → admission check → initialize → return.
    pub async fn get_or_initialize(
        &self,
        resource_ref: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceError>;

    /// Refresh credentials for a resource (on auth failure).
    pub async fn refresh_resource(
        &self,
        name: &str,
    ) -> Result<(), ResourceError>;

    /// List managed resources (safe for MCP exposure).
    pub fn list_resources(&self) -> Vec<ResourceSummary>;

    /// Run one eviction sweep. Called periodically by a background task.
    async fn eviction_sweep(&self);

    /// Check whether a new pool can be admitted.
    fn admission_check(
        &self,
        definition: &ResourceDefinition,
    ) -> Result<(), ResourceError>;
}
```

### Backpressure and Admission Control

Two ceilings operate at different levels:

**Per-pool ceiling** (already exists): `max_connections` on PostgresHandle, `max_connections_per_host` on HttpHandle. This is the `sqlx::PgPool` / `reqwest::Client` connection pool sizing. No new work needed.

**Worker-level pool ceiling** (new): How many distinct managed resource pools a single worker can sustain. Bounded by file descriptors, memory, and aggregate connection pressure.

Backpressure cascades:

1. **Admission control on `get_or_initialize()`**: When at the pool ceiling and a new pool is requested, the manager either evicts an idle dynamic pool (if one exists) or returns `ResourceError::PoolCapacityExhausted`. This retriable error flows through the step state machine as a normal retry.

2. **Eviction under pressure**: The idle-timeout sweep handles steady-state cleanup. Under capacity pressure (at ceiling with new pool requested), LRU eviction of dynamic pools is triggered. Static pools are never evicted.

3. **Aggregate connection budget**: Even below the pool count ceiling, aggregate connections across all pools may exceed what the worker should consume. The `max_total_connections` budget constrains the sum. When a new pool would exceed budget, the manager either reduces the requested pool size (`ReducePoolSize`) or rejects until budget frees (`Reject`).

4. **Validation-time signal**: The `CompositionValidator` can check `resource_ref` references against a "resource budget" declaration, rejecting compositions that would require more pools than the worker can support — fail at validation time rather than execution time.

### ResourceDefinitionSource Trait

For dynamic resource resolution — where does a new `ResourceDefinition` come from when a template references a resource_ref not in the boot config?

```rust
/// A source that can resolve resource definitions by name.
/// Tried in priority order (like ChainedSecretsProvider).
#[async_trait]
pub trait ResourceDefinitionSource: Send + Sync {
    /// Try to resolve a resource definition by name.
    /// Returns None if this source doesn't know about this resource.
    async fn resolve(
        &self,
        name: &str,
    ) -> Result<Option<ResourceDefinition>, ResourceError>;

    /// Watch for new or changed definitions.
    /// Sources that don't support watching return None.
    async fn watch(
        &self,
    ) -> Option<tokio::sync::mpsc::Receiver<ResourceDefinitionEvent>>;

    fn source_name(&self) -> &str;
}

pub enum ResourceDefinitionEvent {
    Added(ResourceDefinition),
    Updated(ResourceDefinition),
    Removed { name: String },
}
```

Sources (in priority order):
1. **StaticConfigSource** — reads from `worker.toml` `[[resources]]` sections. No watch.
2. **SopsFileWatcher** — watches a mounted volume for SOPS-encrypted YAML files. Each file can define one or more resources. New files trigger `Added` events; changes trigger `Updated`. This enables real-time deployment of new resource definitions via volume mounts.
3. **Future: RegistryServiceSource** — a centralized resource registry API for multi-worker coordination. Not in initial scope.

---

## StepContext and ExecutionContext Boundary

### The Naming Alignment

`TaskSequenceStep` in Rust should be renamed to `StepContext` to align with the FFI crates (tasker-py, tasker-rb, tasker-js) which already use this name. This is a naming change in `tasker-worker`, not a structural change.

### Two Context Types, Not One

TAS-370 originally proposed a single `ExecutionContext` that replaces `StepContext` everywhere. This design revises that:

**StepContext** (in tasker-worker): The DTO hydrated for domain handlers. Contains task inputs, step configuration, correlation context, dependency results. Crosses the FFI boundary. No resource handles, no pool references, no composition state. This is what `StepHandler::call()` receives.

**CompositionExecutionContext** (in tasker-runtime): The enriched context for grammar capability executors. Contains everything in StepContext plus the `OperationProvider` (backed by ResourcePoolManager + AdapterRegistry), composition envelope (`.context`, `.deps`, `.prev`, `.step`), checkpoint state, and data classifier reference. This never crosses FFI.

```rust
// tasker-worker — what domain handlers see (crosses FFI)
pub struct StepContext {
    pub step_uuid: Uuid,
    pub correlation_id: String,
    pub task_input: serde_json::Value,
    pub step_inputs: serde_json::Value,
    pub step_config: serde_json::Value,
    pub dependency_results: HashMap<String, serde_json::Value>,
}

// tasker-runtime — what capability executors see (internal)
pub struct CompositionExecutionContext {
    pub step: Arc<StepContext>,
    pub operations: Arc<dyn OperationProvider>,
    pub classifier: Option<Arc<DataClassifier>>,
    pub checkpoint: Arc<CheckpointService>,
    pub checkpoint_state: Option<CheckpointRecord>,
    pub composition_envelope: CompositionEnvelope,
}
```

The capability executors interact with resources through `context.operations.get_persistable("orders-db")` — they never see handles, pools, or adapters.

### Handler Trait Separation

This resolves TAS-370's "Option A vs Option B" design decision: **Option B** — a parallel trait for grammar-composed handlers, keeping `StepHandler` unchanged for domain handlers.

The `GrammarActionResolver` (in tasker-rs) resolves `"grammar:*"` callables into a `GrammarResolvedHandler` which wraps the `CompositionExecutor`. This handler implements `StepHandler` (so it fits the existing dispatch pipeline) but internally constructs a `CompositionExecutionContext` and delegates to the grammar execution pipeline.

Domain `StepHandler` implementations continue receiving `StepContext` through the existing dispatch path. No changes needed.

---

## Worker Segmentation

### Two Worker Types

**Domain workers** (existing): Binary depends on `tasker-worker` + domain crates. Subscribes to namespace queues. Dispatches to domain handlers via FFI or native Rust callables. No grammar knowledge, no resource pool management.

**Composition workers** (tasker-rs): Binary depends on `tasker-worker` + `tasker-runtime`. Subscribes to namespace queues AND shared composition queues. Registers `GrammarActionResolver` in the `ResolverChain`. Initializes `ResourcePoolManager` and `AdapterRegistry` at startup. Handles grammar-composed steps and (optionally) domain handler steps.

### Interoperability

Task templates freely mix grammar-composed and domain handler steps. Queue routing handles the segmentation:

- Steps with `callable: "grammar:persist_order"` route to queues that composition workers subscribe to.
- Steps with `callable: "OrderProcessingHandler"` route to queues that domain workers subscribe to.
- The orchestrator doesn't know or care about the distinction — it sees steps with dependencies and lifecycles.

A composition worker *can* also register domain handlers (it has the full dispatch pipeline from tasker-worker). But the operational recommendation is separate fleets — composition workers tuned for pool management and resource pressure, domain workers tuned for handler throughput and FFI concurrency.

### Scaling Implications

Composition workers scale based on:
- Number of managed resource pools (pool count ceiling)
- Aggregate connection budget across pools
- jaq expression evaluation throughput (CPU-bound)
- Composition complexity (steps per composition, checkpoint frequency)

Domain workers scale based on:
- Step claim rate and handler concurrency
- FFI runtime overhead (Ruby/Python GIL, V8 event loop)
- Handler-specific I/O patterns (managed by the domain codebase)

Independent scaling prevents composition resource pressure from affecting domain handler throughput and vice versa.

---

## Full Example: persist Capability Executor

To illustrate the complete architecture, here is how a `persist` action flows through the system:

```yaml
# In a composition spec within a task template
- capability: persist
  config:
    resource:
      ref: "orders-db"
      entity: orders
    data:
      expression: "{id: .prev.order_id, total: .prev.computed_total, status: \"confirmed\"}"
    constraints:
      upsert_key: ["id"]
    validate_success:
      expression: ".affected_count > 0"
    result_shape:
      expression: "{persisted_id: .data.id, timestamp: .data.created_at}"
```

**Execution flow**:

1. **CompositionExecutor** (tasker-grammar) receives the step, checks checkpoint state for resume.

2. **PersistExecutor** (tasker-grammar) is invoked with the composition envelope as input:
   - Parses `PersistConfig` from the config JSON
   - Evaluates the `data.expression` jaq filter against the envelope: produces `{"id": 123, "total": 45.67, "status": "confirmed"}`
   - Calls `context.operations.get_persistable("orders-db")` → gets `Arc<dyn PersistableResource>`
   - Calls `persistable.persist("orders", data_value, &constraints)`

3. **RuntimeOperationProvider** (tasker-runtime) handles the `get_persistable("orders-db")` call:
   - Calls `pool_manager.get_or_initialize("orders-db")` → gets `Arc<dyn ResourceHandle>`
   - Calls `adapter_registry.as_persistable(handle)` → wraps in `PostgresPersistAdapter`
   - Returns the adapter as `Arc<dyn PersistableResource>`

4. **PostgresPersistAdapter** (tasker-runtime) receives the `persist()` call:
   - Accesses `handle.pool()` from the wrapped `PostgresHandle` (tasker-secure)
   - Constructs: `INSERT INTO orders (id, total, status) VALUES ($1, $2, $3) ON CONFLICT (id) DO UPDATE SET total = $2, status = $3`
   - Executes via `sqlx::query()` against the managed `PgPool`
   - Returns `PersistResult { data: {...}, affected_count: Some(1) }`

5. **PersistExecutor** (back in tasker-grammar) continues:
   - Evaluates `validate_success.expression` against the result: `.affected_count > 0` → true
   - Evaluates `result_shape.expression`: produces `{"persisted_id": 123, "timestamp": "2026-03-08T..."}`
   - Returns this as the capability output → becomes `.prev` for the next composition step

6. **CompositionExecutor** checkpoints the result (persist is a mutating capability) and proceeds.

**In tests** (same flow, different provider):

1–2. Same as above — PersistExecutor parses config, evaluates jaq expressions.

3. **InMemoryOperationProvider** (tasker-grammar testing) handles `get_persistable("orders-db")`:
   - Returns the `InMemoryOperations` instance directly as `Arc<dyn PersistableResource>`

4. **InMemoryOperations** (tasker-grammar testing) receives the `persist()` call:
   - Pushes `CapturedPersist { entity: "orders", data: {...}, constraints: {...} }` to capture list
   - Returns `PersistResult { data: {...}, affected_count: Some(1) }`

5–6. Same as production — validate_success, result_shape, checkpoint.

The grammar crate tests the **complete orchestration** — config parsing through output shaping — with zero I/O and zero dependency on tasker-runtime or any concrete handles.

---

## Parallel Workstreams

Given this architecture, the work segments into independently progressable streams:

### Workstream A: Operation Traits and Grammar Capability Executors

**Scope**: Define `PersistableResource`, `AcquirableResource`, `EmittableResource` traits and constraint/result types in tasker-grammar. Implement `InMemoryOperations` test double. Define `OperationProvider` interface. Implement `PersistExecutor`, `AcquireExecutor`, `EmitExecutor` with full orchestration pipelines.

**Dependencies**: TAS-322 (scaffold, done), TAS-323 (core types). Minimal dependency on tasker-secure (just `ResourceType` and `ResourceHandle` for the `OperationProvider` interface).

**I/O**: None — all tests use `InMemoryOperations`.

**Tickets**: TAS-330 (persist), TAS-331 (acquire), TAS-332 (emit) need revision to reference the operation traits and `OperationProvider` pattern rather than direct handle access. May also want a predecessor ticket for the operation trait definitions themselves.

**Status**: Can proceed immediately.

### Workstream B: Runtime Adapters

**Scope**: Scaffold `tasker-runtime` crate. Implement `PostgresPersistAdapter`, `PostgresAcquireAdapter`, `HttpPersistAdapter`, `HttpAcquireAdapter`, `HttpEmitAdapter`, `PgmqEmitAdapter`. Implement `AdapterRegistry`. Implement `RuntimeOperationProvider` bridging the adapter registry to the `OperationProvider` interface.

**Dependencies**: Workstream A (operation traits to implement), TAS-357/358 (secure handles to wrap).

**New tickets needed**: New project scope — likely a "Tasker Runtime" project or added to "Tasker Secure Foundations" as Milestone 2.

**Status**: Can begin scaffolding immediately; adapter implementations after Workstream A delivers traits.

### Workstream C: ResourcePoolManager

**Scope**: Implement `ResourcePoolManager` in tasker-runtime with eviction, backpressure, admission control, connection budget tracking. Implement `ResourceDefinitionSource` trait and `StaticConfigSource`. `SopsFileWatcher` can follow.

**Dependencies**: TAS-358 (done — ResourceRegistry to wrap).

**New tickets needed**: New project scope within Tasker Runtime.

**Status**: Can proceed immediately — depends only on the existing ResourceRegistry.

### Workstream D: StepContext Rename

**Scope**: Rename `TaskSequenceStep` to `StepContext` in tasker-worker. Align Rust naming with FFI crates.

**Dependencies**: None (naming change only).

**Revision of**: TAS-370. The `ExecutionContext` portion moves to tasker-runtime as `CompositionExecutionContext`. TAS-370 should be revised to cover only the rename and handler trait separation.

**Status**: Can proceed independently.

### Workstream E: ConfigString Integration (TAS-369)

**Scope**: Dog-food `ConfigString` into tasker-shared config loading.

**Dependencies**: TAS-358 (done).

**Status**: Unchanged — fully independent.

### Workstream F: tasker-rs Worker Binary

**Scope**: Scaffold the composition-capable worker binary. Register `GrammarActionResolver`, initialize `ResourcePoolManager` and `AdapterRegistry`, subscribe to composition queues, construct `CompositionExecutionContext` for grammar dispatch.

**Dependencies**: Workstreams A (executors), B (adapters), C (pool manager), D (StepContext).

**New tickets needed**: New project scope, likely Phase 3 of Tasker Action Grammar.

**Status**: Blocked until workstreams A–D converge.

### Dependency Graph

```
TAS-357 (in review) ──→ Workstream B (adapters, needs secure handles)
                              │
TAS-358 (done) ──────────────┼──→ Workstream C (ResourcePoolManager)
                              │
                              │
        Workstream A (traits + executors, mostly independent)
              │                    │
              └──→ Workstream B    │
                        │          │
                        ├──→ Workstream F (tasker-rs)
                        │          ↑
       Workstream D (StepContext) ─┘
                              │
       Workstream E (ConfigString) — fully independent
```

Workstream A is the critical path — it defines the traits that Workstream B implements and the executors that Workstream F wires up.

Workstreams C, D, and E are fully independent of each other and can proceed in parallel.

Workstream F is the convergence point where everything comes together.

---

## Impact on Existing Tickets

### TAS-330 (persist), TAS-331 (acquire), TAS-332 (emit)

**Status**: Valid but need revision.

**Change**: The executor implementations should use the `OperationProvider` interface (`context.operations.get_persistable(...)`) rather than direct handle access or downcasts. Tests use `InMemoryOperations` from tasker-grammar's testing module, not `InMemoryResourceHandle` from tasker-secure. The capability config parsing, jaq expression evaluation, and result validation remain as specified.

**Action**: Update ticket descriptions to reference this design document, the operation traits, and the `OperationProvider` pattern.

### TAS-370 (ExecutionContext in tasker-worker)

**Status**: Needs significant revision.

**Change**: The single `ExecutionContext` concept splits into `StepContext` (rename in tasker-worker) and `CompositionExecutionContext` (in tasker-runtime). The "Handler Trait Evolution" decision resolves as Option B: parallel handler trait for grammar-composed handlers, `StepHandler` unchanged. The `ResourceRegistry` integration moves from tasker-worker to tasker-runtime.

**Action**: Revise to cover only StepContext rename and handler trait separation. Create new ticket(s) in tasker-runtime for `CompositionExecutionContext`, `RuntimeOperationProvider`, and the `GrammarActionResolver` bridge.

### TAS-369 (ConfigString into tasker-shared)

**Status**: Unchanged. This work is independent of the architecture described here.

### TAS-357, TAS-358

**Status**: Complete (TAS-358) or nearly complete (TAS-357). No changes needed. The `ResourceHandle` trait and concrete handles are exactly what the adapters in tasker-runtime will wrap.

---

## Open Questions

### 1. SQL Generation Scope in PostgresPersistAdapter

How much SQL generation intelligence should the adapter have? Options:

- **Minimal**: Flat key-value inserts/upserts only. Complex queries require a custom adapter or a domain handler.
- **Moderate**: Nested JSON→JSONB, array columns, basic joins for upsert conflict targets.
- **Extensive**: Full query builder with type coercion, CTE support, etc.

Recommendation: Start minimal. The grammar's power is in jaq expression evaluation and composition — the data arriving at `persist()` should already be in the right shape. If complex SQL patterns emerge as common needs, they're better served by new adapter implementations or capability executor registrations than by growing the adapter into a query builder.

### 2. HttpHandle Operation Mapping

How does the HTTP adapter map grammar operations to HTTP semantics?

- `persist()` → POST (create) or PUT (upsert when `upsert_key` present)?
- `acquire()` → GET always, or HEAD for existence checks?
- `emit()` → POST to webhook URL?

This needs a clear convention documented in the adapter implementations.

### 3. Transaction Scope for persist

Should `PersistableResource::persist()` execute within a transaction, or should the caller (composition executor) manage transaction scope? For single-persist compositions this doesn't matter. For compositions with multiple persists (checkpointed), each persist should be its own transaction (consistent with checkpoint semantics). For compositions where multiple writes need atomicity, we may eventually need a `TransactionalPersistableResource` variant.

Recommendation: Each `persist()` call is a self-contained operation (auto-commit or single-statement transaction). Multi-write atomicity is a future concern — flag it but don't solve it in the initial design.

### 4. Adapter Caching in RuntimeOperationProvider

Should the `RuntimeOperationProvider` cache adapter instances, or create a new adapter per `get_persistable()` call? Adapters are lightweight (they hold an `Arc<Handle>` and no state), so creation cost is minimal. But if the same resource is accessed repeatedly in a composition, caching avoids redundant pool manager lookups.

Recommendation: Cache at the `CompositionExecutionContext` level per composition execution. Each composition run creates a context, the context lazily resolves and caches adapters, and everything is dropped at composition completion. No cross-composition caching (that's the pool manager's job at the handle level).

### 5. tasker-grammar's Dependency on tasker-secure

The `OperationProvider` interface needs `ResourceType` for error messages and potentially `ResourceHandle` for the provider to work with. Is this dependency acceptable, or should the shared types be extracted into a `tasker-resource-api` crate?

Recommendation: Accept the dependency for now. tasker-grammar only uses a handful of types from tasker-secure (no feature-gated I/O deps). If the dependency becomes a concern — e.g., tasker-secure grows heavy optional deps that slow down tasker-grammar's compile — extract at that point.

### 6. Connection Budget Enforcement Timing

Should the aggregate connection budget be enforced at pool creation time (preventive) or monitored continuously (reactive)? Preventive enforcement is simpler but requires estimating connections before the pool is created. Reactive enforcement allows the pool to be created and then adjusted.

Recommendation: Preventive at creation time using the pool's `max_connections` config as the estimate. Reactive monitoring as a follow-on for production tuning.

---

## Summary

The key architectural decisions in this document:

1. **Three concerns, three crates**: Resource identity and credentials (tasker-secure) → operation contracts and orchestration (tasker-grammar) → adapters and runtime lifecycle (tasker-runtime). Each concern has a distinct reason to exist and a distinct reason to change.

2. **The action is the orchestration**: Grammar capability executors in tasker-grammar own the full action pipeline (config → expression → operation → validation → result shaping). They call through abstract operation traits (`PersistableResource`, `AcquirableResource`, `EmittableResource`) that they themselves define.

3. **Adapters bridge the seam**: `tasker-runtime` provides adapter implementations that translate grammar operations into resource-specific I/O by wrapping tasker-secure handles. A `PostgresPersistAdapter` wraps a `PostgresHandle` and implements `PersistableResource` by generating SQL. The grammar executor never sees SQL; the handle never sees grammar concepts.

4. **OperationProvider as the clean interface**: Capability executors obtain operation traits through an `OperationProvider` interface. In production, this is backed by `ResourcePoolManager` + `AdapterRegistry`. In tests, it's backed by `InMemoryOperations`. The executors don't know the difference.

5. **ResourcePoolManager** in tasker-runtime handles dynamic resource lifecycle, eviction, and backpressure — the operational concerns of running grammar compositions against live resources with dynamic template creation.

6. **Worker segmentation**: Composition workers (tasker-rs) and domain workers scale independently with different tuning profiles, while task templates freely mix both handler types through namespace queue routing.

7. **StepContext / CompositionExecutionContext split**: Domain handlers see a minimal DTO (StepContext). Grammar capability executors see an enriched context with operation access (CompositionExecutionContext). The boundary is clean and the types never cross FFI.

---

*This document should be read alongside `docs/research/security-and-secrets/02-resource-registry.md` (ResourceHandle design), `docs/action-grammar/grammar-trait-boundary.md` (grammar trait system), and `docs/action-grammar/implementation-phases.md` (Phase 1 roadmap). It supersedes the ExecutionContext design in TAS-370 and informs revisions to TAS-330, TAS-331, and TAS-332.*
