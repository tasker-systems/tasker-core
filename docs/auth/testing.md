# Auth Testing

E2E test infrastructure for validating authentication and permission enforcement.

---

## Test Organization

```
tasker-orchestration/tests/web/auth/
├── mod.rs                  # Module declarations
├── common.rs               # AuthWebTestClient, token generators, constants
├── tasks.rs                # Task endpoint auth tests
├── workflow_steps.rs       # Step resolution auth tests
├── dlq.rs                  # DLQ endpoint auth tests
├── handlers.rs             # Handler registry auth tests
├── analytics.rs            # Analytics endpoint auth tests
├── config.rs               # Config endpoint auth tests
├── health.rs               # Health endpoint public access tests
└── api_keys.rs             # API key auth tests (full/read/tasks/none)
```

All tests are feature-gated: `#[cfg(feature = "test-services")]`

---

## Running Auth Tests

```bash
# Run all auth E2E tests (requires database running)
cargo make test-auth-e2e    # or: cargo make tae

# Run a specific test file
cargo nextest run --features test-services \
  -E 'test(auth::tasks)' \
  --package tasker-orchestration

# Run with output
cargo nextest run --features test-services \
  -E 'test(auth::)' \
  --package tasker-orchestration \
  --nocapture
```

---

## Test Infrastructure

### AuthTestServer and AuthWebTestClient

Tests use a two-part setup: `AuthTestServer` starts an auth-enabled Axum server, and `AuthWebTestClient` provides HTTP methods to interact with it:

```rust
use crate::web::auth_test_helpers::*;

#[tokio::test]
async fn test_example() {
    let server = AuthTestServer::start()
        .await
        .expect("Failed to start auth test server");
    let mut client = AuthWebTestClient::for_server(&server);

    // Use client for requests...
    let response = client.get("/v1/tasks").await.expect("request failed");

    server.shutdown().await.expect("shutdown failed");
}
```

`AuthTestServer::start()` does:

1. Allocates a dynamic port (`127.0.0.1:0`)
2. Sets `TASKER_CONFIG_PATH` and `TASKER_JWT_PUBLIC_KEY_PATH`
3. Creates `SystemContext` + `OrchestrationCore` + `AppState`
4. Starts Axum with auth middleware active

`AuthWebTestClient::for_server(&server)` creates an HTTP client configured with the server's base URL. It supports auth modes via builder methods: `with_jwt()`, `with_api_key()`, and `without_auth()`.

### Token Generators

```rust
use crate::web::auth::common::{generate_jwt, generate_expired_jwt, generate_jwt_wrong_issuer};

// Valid token with specific permissions
let token = generate_jwt(&["tasks:create", "tasks:read"]);

// Expired token (1 hour ago)
let token = generate_expired_jwt(&["tasks:create"]);

// Wrong issuer (won't validate)
let token = generate_jwt_wrong_issuer(&["tasks:create"]);
```

Token generation uses the test RSA private key (`tests/fixtures/auth/jwt-private-key-test.pem`) embedded as a constant.

### API Key Constants

```rust
use crate::web::auth::common::{
    TEST_API_KEY_FULL,             // permissions: ["*"]
    TEST_API_KEY_READ_ONLY,        // permissions: tasks/steps/dlq read + system read
    TEST_API_KEY_TASKS_ONLY,       // permissions: ["tasks:*"]
    TEST_API_KEY_NO_PERMS,         // permissions: []
    INVALID_API_KEY,               // not registered
};
```

These match the keys configured in `config/tasker/generated/auth-test.toml`.

---

## Test Configuration

### `config/tasker/generated/auth-test.toml`

A copy of `complete-test.toml` with auth overrides:

```toml
[orchestration.web.auth]
enabled = true
jwt_issuer = "tasker-core-test"
jwt_audience = "tasker-api-test"
jwt_verification_method = "public_key"
jwt_public_key_path = ""  # Set via TASKER_JWT_PUBLIC_KEY_PATH at runtime
api_keys_enabled = true
strict_validation = false

[[orchestration.web.auth.api_keys]]
key = "test-api-key-full-access"
permissions = ["*"]

[[orchestration.web.auth.api_keys]]
key = "test-api-key-read-only"
permissions = ["tasks:read", "tasks:list", "steps:read", ...]

# ... more keys ...
```

### Test Fixture Keys

```
tests/fixtures/auth/
├── jwt-private-key-test.pem   # RSA private key (for token generation in tests)
└── jwt-public-key-test.pem    # RSA public key (loaded by SecurityService)
```

These are deterministic test keys committed to the repository. They are only used in tests and have no security value.

---

## Test Patterns

### Pattern: No Credentials → 401

```rust
#[tokio::test]
async fn test_no_credentials_returns_401() {
    let server = AuthTestServer::start().await.expect("Failed to start");
    let mut client = AuthWebTestClient::for_server(&server);
    client.without_auth();

    let response = client.get("/v1/tasks").await.expect("request failed");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    server.shutdown().await.expect("shutdown failed");
}
```

### Pattern: Valid JWT with Required Permission → 200

```rust
#[tokio::test]
async fn test_jwt_with_permission_succeeds() {
    let server = AuthTestServer::start().await.expect("Failed to start");
    let mut client = AuthWebTestClient::for_server(&server);

    let token = generate_jwt(&["tasks:list"]);
    client.with_jwt(&token);

    let response = client.get("/v1/tasks").await.expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);

    server.shutdown().await.expect("shutdown failed");
}
```

### Pattern: Valid JWT Missing Permission → 403

```rust
#[tokio::test]
async fn test_jwt_without_permission_returns_403() {
    let server = AuthTestServer::start().await.expect("Failed to start");
    let mut client = AuthWebTestClient::for_server(&server);

    let token = generate_jwt(&["tasks:read"]);  // missing tasks:create
    client.with_jwt(&token);

    let body = serde_json::json!({ /* ... */ });
    let response = client.post_json("/v1/tasks", &body).await.expect("request failed");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    server.shutdown().await.expect("shutdown failed");
}
```

### Pattern: API Key with Permissions → 200

```rust
#[tokio::test]
async fn test_api_key_full_access() {
    let server = AuthTestServer::start().await.expect("Failed to start");
    let mut client = AuthWebTestClient::for_server(&server);
    client.with_api_key(TEST_API_KEY_FULL);

    let response = client.get("/v1/tasks").await.expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);

    server.shutdown().await.expect("shutdown failed");
}
```

### Pattern: Health Always Public

```rust
#[tokio::test]
async fn test_health_no_auth_required() {
    let server = AuthTestServer::start().await.expect("Failed to start");
    let mut client = AuthWebTestClient::for_server(&server);
    client.without_auth();

    let response = client.get("/health").await.expect("request failed");
    assert_eq!(response.status(), StatusCode::OK);

    server.shutdown().await.expect("shutdown failed");
}
```

---

## Test Coverage Matrix

| Scenario | Expected | Test File |
|----------|----------|-----------|
| No credentials on protected routes | 401 | All files |
| JWT with exact permission | 200 | tasks, dlq, handlers, analytics, config |
| JWT with resource wildcard (`tasks:*`) | 200 | tasks |
| JWT with global wildcard (`*`) | 200 | All files |
| JWT missing required permission | 403 | tasks, dlq, handlers, analytics |
| JWT wrong issuer | 401 | tasks |
| JWT wrong audience | 401 | tasks |
| Expired JWT | 401 | tasks |
| Malformed JWT | 401 | tasks |
| API key full access | 200 | api_keys |
| API key read-only | 200/403 | api_keys |
| API key tasks-only | 200/403 | api_keys |
| API key no permissions | 403 | api_keys |
| Invalid API key | 401 | api_keys |
| Health endpoints without auth | 200 | health |

---

## CI Compatibility

Auth tests are compatible with CI without special environment setup:

- **Dynamic port allocation**: `TcpListener::bind("127.0.0.1:0")` avoids port conflicts
- **Self-configuring paths**: Uses `CARGO_MANIFEST_DIR` to resolve fixture paths at compile time
- **No external services**: Auth validation is in-process (no external JWKS/IdP needed)
- **Nextest isolation**: Each test runs in its own process, preventing env var conflicts

---

## Adding New Auth Tests

1. Identify the endpoint and required permission (see [Permissions](permissions.md))
2. Add tests to the appropriate file (by resource) or create a new one
3. Test at minimum: no credentials (401), correct permission (200), wrong permission (403)
4. For POST/PATCH endpoints, use a valid request body (deserialization runs before permission check)
5. Run `cargo make test-auth-e2e` to verify

---

## Related

- [Permissions](permissions.md) — Full permission vocabulary and endpoint mapping
- [Configuration](configuration.md) — Auth config reference
- [`config/tasker/generated/auth-test.toml`](../../config/tasker/generated/auth-test.toml) — Test auth configuration
