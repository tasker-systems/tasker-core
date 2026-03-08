# TAS-358: S2 ResourceRegistry and ResourceHandle — Design

*March 2026 — Revised scope (ExecutionContext and ConfigString integration descoped to Milestone 1.5)*

## Purpose

Deliver the `ResourceHandle` trait, `ResourceRegistry`, concrete handles (`PostgresHandle`, `HttpHandle`), and `InMemoryResourceHandle` test utility within the `tasker-secure` crate. These types unblock Phase 1C grammar executor stubs (`acquire`, `persist`, `emit`) which compile against `Arc<dyn ResourceHandle>` and `ResourceRegistry::get()`.

## Design Decisions

1. **Credential rotation**: Executor-driven refresh. Capability executors call `context.resources.refresh_resource(name)` on auth errors and return a retriable error. Proactive refresh deferred.
2. **Scope boundary**: All work stays within `crates/tasker-secure/`. No changes to tasker-worker, tasker-shared, or any other crate. Integration deferred to TAS-369 (ConfigString) and TAS-370 (ExecutionContext).
3. **ConfigValue vs ConfigString**: The resource module defines its own `ConfigValue` enum (Literal/SecretRef/EnvRef) for resource config fields. This is structurally similar to `ConfigString` from the config module but serves a different purpose — it resolves individual config parameters within a `ResourceDefinition`, not top-level TOML config fields. Both resolve through `SecretsProvider`.
4. **Feature gates**: `postgres` (sqlx dependency), `http` (reqwest dependency), `test-utils` (InMemoryResourceHandle). Core traits and registry are always available.

## Architecture

```
ResourceDefinition (TOML config)
  │
  ├── name: "orders-db"
  ├── resource_type: Postgres
  └── config: HashMap<String, ConfigValue>
          │
          ▼ resolve through SecretsProvider
          │
ResourceRegistry::initialize_all()
  │
  ├── constructs PostgresHandle / HttpHandle / etc.
  ├── calls health_check() on each
  └── stores Arc<dyn ResourceHandle> by name
          │
          ▼
ResourceRegistry::get("orders-db")
  → Arc<dyn ResourceHandle>
    → handle.as_postgres() → &PostgresHandle → .pool()
```

## Types

### Core (always available)

- `ResourceHandle` trait — `resource_name()`, `resource_type()`, `refresh_credentials()`, `health_check()`, `as_any()`
- `ResourceHandleExt` trait — typed downcasts: `as_postgres()`, `as_http()`, `as_pgmq()`
- `ResourceType` enum — `Postgres`, `Http`, `Pgmq`, `Custom { type_name }`
- `ResourceError` — `InitializationFailed`, `HealthCheckFailed`, `CredentialRefreshFailed`, `ResourceNotFound`, `WrongResourceType`
- `ResourceDefinition` — TOML-deserializable resource config
- `ConfigValue` — `Literal(String)`, `SecretRef { secret_ref }`, `EnvRef { env }` with `resolve()`
- `ResourceConfig` — `HashMap<String, ConfigValue>` with typed accessor helpers
- `ResourceRegistry` — `initialize_all()`, `get()`, `refresh_resource()`, `list_resources()`
- `ResourceSummary` — name + type + healthy (safe for MCP exposure)

### Feature-gated

- `PostgresHandle` (feature: `postgres`) — wraps `sqlx::PgPool`
- `HttpHandle` (feature: `http`) — wraps `reqwest::Client` with `HttpAuthStrategy`
- `HttpAuthStrategy` trait — `apply()` + `refresh()`
- `ApiKeyAuthStrategy`, `BearerTokenAuthStrategy` — built-in strategies
- `InMemoryResourceHandle` (feature: `test-utils`) — fixture data + capture lists
- `test_registry_with_fixtures()` — test helper

## Acceptance Criteria

- `cargo test -p tasker-secure --features test-utils` passes in-memory
- `cargo test -p tasker-secure --features postgres,test-services` passes against local PostgreSQL
- `cargo test -p tasker-secure --features http` passes against local mock HTTP server
- `InMemoryResourceHandle` provides fixture data for acquire, capture lists for persist/emit
- `list_resources()` never exposes host, port, credentials, or secret paths
- All existing tasker-secure S1 tests continue to pass

## Descoped (Milestone 1.5)

- TAS-369: ConfigString into tasker-shared config loading
- TAS-370: ExecutionContext in tasker-worker
- TAS-371: tasker-cfg crate extraction
