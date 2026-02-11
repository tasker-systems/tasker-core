# Authentication & Authorization

API-level security for Tasker's orchestration and worker HTTP endpoints, providing JWT bearer token and API key authentication with permission-based access control.

---

## Architecture

```
                         ┌──────────────────────────────┐
Request ──►  Middleware  │  SecurityService              │
             (per-route) │  ├─ JwtAuthenticator          │
                         │  ├─ JwksKeyStore (optional)   │
                         │  └─ ApiKeyRegistry (optional) │
                         └───────────┬──────────────────┘
                                     │
                                     ▼
                           SecurityContext
                           (injected into request extensions)
                                     │
                                     ▼
                         ┌───────────────────────┐
                         │  authorize() wrapper  │
                         │  Resource + Action    │
                         └───────────┬───────────┘
                                     │
                           ┌─────────┴─────────┐
                           ▼                   ▼
                     Body parsing        403 (denied)
                         │
                         ▼
                    Handler body
                         │
                         ▼
                   200 (success)
```

### Key Components

| Component | Location | Role |
|-----------|----------|------|
| `SecurityService` | `tasker-shared/src/services/security_service.rs` | Unified auth backend: validates JWTs (static key or JWKS) and API keys |
| `SecurityContext` | `tasker-shared/src/types/security.rs` | Per-request identity + permissions, extracted by handlers |
| `Permission` enum | `tasker-shared/src/types/permissions.rs` | Compile-time permission vocabulary (`resource:action`) |
| `Resource`, `Action` | `tasker-shared/src/types/resources.rs` | Resource-based authorization types |
| `authorize()` wrapper | `tasker-shared/src/web/authorize.rs` | Handler wrapper for declarative permission checks |
| Auth middleware | `*/src/web/middleware/auth.rs` | Axum middleware injecting `SecurityContext` |
| `require_permission()` | `*/src/web/middleware/permission.rs` | Legacy per-handler permission gate (still available) |

### Request Flow

1. **Middleware** (`conditional_auth`) runs on protected routes
2. If auth disabled → injects `SecurityContext::disabled_context()` (all permissions)
3. If auth enabled → extracts Bearer token or API key from headers
4. **`SecurityService`** validates credentials, returns `SecurityContext`
5. **`authorize()` wrapper** checks permission BEFORE body deserialization → 403 if denied
6. Body deserialization and handler execution proceed if authorized

### Route Layers

Routes are split into **public** (never require auth) and **protected** (auth middleware applied):

**Orchestration (port 8080):**
- Public: `/health/*`, `/metrics`, `/api-docs/*`
- Protected: `/v1/*`, `/config` (opt-in)

**Worker (port 8081):**
- Public: `/health/*`, `/metrics`, `/api-docs/*`
- Protected: `/v1/templates/*`, `/config` (opt-in)

---

## Quick Start

```bash
# 1. Generate RSA key pair
cargo run --bin tasker-ctl -- auth generate-keys --output-dir ./keys

# 2. Generate a token
cargo run --bin tasker-ctl -- auth generate-token \
  --private-key ./keys/jwt-private-key.pem \
  --permissions "tasks:create,tasks:read,tasks:list" \
  --subject my-service \
  --expiry-hours 24

# 3. Enable auth in config (orchestration.toml)
# [web.auth]
# enabled = true
# jwt_public_key_path = "./keys/jwt-public-key.pem"

# 4. Use the token
curl -H "Authorization: Bearer <token>" http://localhost:8080/v1/tasks
```

---

## Documentation Index

| Document | Contents |
|----------|----------|
| [Permissions](permissions.md) | Permission vocabulary, route mapping, wildcards, role patterns |
| [Configuration](configuration.md) | TOML config, environment variables, deployment patterns |
| [Testing](testing.md) | E2E test infrastructure, cargo-make tasks, writing auth tests |

### Cross-References

| Document | Contents |
|----------|----------|
| [API Security Guide](../guides/api-security.md) | Quick start, CLI commands, error responses, observability |
| [Auth Integration Guide](../guides/auth-integration.md) | JWKS, Auth0, Keycloak, Okta configuration |

---

## Design Decisions

### Auth Disabled by Default

Security is opt-in (`enabled = false` default). Existing deployments are unaffected. When disabled, all handlers receive a `SecurityContext` with `AuthMethod::Disabled` and `permissions: ["*"]`.

### Config Endpoint Opt-In

The `/config` endpoint exposes runtime configuration (secrets redacted). It is controlled by a separate toggle (`config_endpoint_enabled`, default `false`). When disabled, the route is not registered (404, not 401).

### Resource-Based Authorization

Permission checks happen at the route level via `authorize()` wrappers BEFORE body deserialization:

```rust
.route("/tasks", post(authorize(Resource::Tasks, Action::Create, create_task)))
```

This approach:
- Rejects unauthorized requests before parsing request bodies
- Provides a declarative, visible permission model at the route level
- Is protocol-agnostic (same `Resource`/`Action` types work for REST and gRPC)
- Documents permissions in OpenAPI via `x-required-permission` extensions

The legacy `require_permission()` function is still available for cases where permission checks need to happen inside handler logic.

### Credential Priority (Client)

The `tasker-client` library resolves credentials in this order:
1. Endpoint-specific token (`TASKER_ORCHESTRATION_AUTH_TOKEN` / `TASKER_WORKER_AUTH_TOKEN`)
2. Global token (`TASKER_AUTH_TOKEN`)
3. API key (`TASKER_API_KEY`)
4. JWT generation from private key (if configured)

---

## Known Limitations

- ~~**Body-before-permission** ordering for POST/PATCH endpoints~~ — Resolved by resource-based authorization
- **No token refresh** — tokens are stateless; clients must generate new tokens before expiry
- **API keys have no expiration** — rotate by removing from config and redeploying
