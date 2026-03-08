# tasker-secure

Strategy-pattern secrets resolution, resource lifecycle, and data protection for Tasker workflows.

## Overview

`tasker-secure` provides pluggable secret backends via the `SecretsProvider` trait, with built-in implementations for environment variables, SOPS-encrypted files, and priority-ordered chained resolution. All secret values are wrapped in `SecretValue`, which uses the `secrecy` crate to prevent accidental logging or display of sensitive data.

This crate has **no infrastructure dependencies** — no database, messaging, or orchestration coupling. It can be tested standalone with `cargo test -p tasker-secure`.

## Features

### Secrets Resolution (S1 — TAS-357)

- **`SecretValue`** — Wrapper around `secrecy::SecretString` with redacted `Debug`/`Display` and zeroize-on-drop
- **`SecretsProvider`** — Async trait for pluggable secret backends
- **`EnvSecretsProvider`** — Resolves secrets from environment variables with optional prefix and path normalization (dots, hyphens, slashes → underscores, uppercased)
- **`ChainedSecretsProvider`** — Priority-ordered fallback across multiple providers (first success wins)
- **`SopsSecretsProvider`** — Decrypts [SOPS](https://github.com/getsops/sops)-encrypted YAML files using age keys via the `rops` crate (feature-gated: `sops`)
- **`ConfigString`** — Serde-deserializable enum supporting plain strings, `{ secret_ref = "path" }` for provider-backed resolution, and `{ env_ref = "VAR", default = "..." }` for environment variable references
- **`InMemorySecretsProvider`** — Test utility (feature-gated: `test-utils`)

### Future Modules

- **Resource lifecycle** (S2 — TAS-358) — Automatic credential rotation and connection management
- **Field-level encryption** (TAS-359) — Encrypt sensitive fields in workflow data
- **Data classification** (TAS-360) — Sensitivity tagging and policy enforcement

## Feature Flags

| Feature | Description |
|---------|-------------|
| `sops` | Enables `SopsSecretsProvider` with `rops` and `serde_yaml` dependencies |
| `test-utils` | Enables `InMemorySecretsProvider` for testing |

## Usage

```rust
use tasker_secure::{EnvSecretsProvider, SecretsProvider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Resolve secrets from environment variables with a prefix
    let provider = EnvSecretsProvider::with_prefix("MYAPP");
    let db_password = provider.get_secret("database.password").await?;
    // Looks up MYAPP_DATABASE_PASSWORD in the environment
    println!("Connected with secret: {}", db_password); // prints [REDACTED]
    Ok(())
}
```

### SOPS-encrypted files

```rust
use tasker_secure::{SopsSecretsProvider, SecretsProvider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Requires ROPS_AGE env var or ~/.config/rops/age_keys
    let provider = SopsSecretsProvider::from_path("config/secrets.enc.yaml").await?;
    let api_key = provider.get_secret("api.stripe.secret_key").await?;
    Ok(())
}
```

### Chained resolution (fallback order)

```rust
use std::sync::Arc;
use tasker_secure::{ChainedSecretsProvider, EnvSecretsProvider, SecretsProvider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain = ChainedSecretsProvider::new(vec![
        Arc::new(EnvSecretsProvider::with_prefix("MYAPP")),
        Arc::new(EnvSecretsProvider::new()),  // fallback: no prefix
    ]);
    let secret = chain.get_secret("database.password").await?;
    Ok(())
}
```

### ConfigString in TOML

```toml
# Plain string (backward compatible)
database_url = "postgresql://localhost/mydb"

# Secret reference (resolved via SecretsProvider)
database_url = { secret_ref = "database.url" }

# Environment variable with default
database_url = { env_ref = "DATABASE_URL", default = "postgresql://localhost/mydb" }
```

## Testing

```bash
# All tests (no infrastructure needed)
cargo test -p tasker-secure --all-features

# Without SOPS tests
cargo test -p tasker-secure
```

SOPS tests require the `ROPS_AGE` environment variable set to the test age key. This is configured automatically via `config/dotenv/test.env` and the CI setup-env action.

## License

MIT
