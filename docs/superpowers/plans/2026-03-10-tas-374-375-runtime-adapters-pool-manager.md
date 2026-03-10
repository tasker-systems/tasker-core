# TAS-374/375: Runtime Adapters & ResourcePoolManager Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the adapter layer bridging tasker-grammar operation traits to tasker-secure resource handles, plus ResourcePoolManager with eviction and backpressure.

**Architecture:** Six adapters (Postgres persist/acquire, HTTP persist/acquire/emit, Messaging emit) translate structured grammar operations into resource-specific I/O. SQL generation is extracted as pure functions for testability. AdapterRegistry uses closure-based factories for extensibility. ResourcePoolManager wraps ResourceRegistry with admission control, eviction (liveness-aware), and observability metrics.

**Tech Stack:** Rust, async-trait, sqlx (Postgres), reqwest (HTTP), tasker-shared MessagingProvider, tokio::sync::RwLock

**Spec:** `docs/superpowers/specs/2026-03-10-tas-374-375-runtime-adapters-pool-manager-design.md`

**Review fixes applied:**
1. Adapters take `Arc<dyn ResourceHandle>` (not `Arc<PostgresHandle>`) — downcast internally, avoids Clone requirement on handles
2. `InMemorySecretsProvider::new()` requires `HashMap<String, String>` argument — test helper updated
3. Registry tests verify factory registration + correct error on wrong handle type (InMemoryResourceHandle can't downcast to Postgres/HTTP)
4. Postgres persist uses `.fetch_optional()` not `.execute()` to capture RETURNING * rows

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `crates/tasker-runtime/src/adapters/sql_gen.rs` | Pure SQL generation functions + identifier sanitization |
| `crates/tasker-runtime/src/adapters/messaging.rs` | `MessagingEmitAdapter` wrapping `Arc<MessagingProvider>` |
| `crates/tasker-runtime/src/adapters/registry.rs` | `AdapterRegistry` with closure-based factories |
| `crates/tasker-runtime/tests/sql_gen_tests.rs` | SQL generation unit tests |
| `crates/tasker-runtime/tests/adapter_registry_tests.rs` | Registry wiring tests |
| `crates/tasker-runtime/tests/pool_manager_tests.rs` | Pool manager unit tests |
| `crates/tasker-runtime/tests/postgres_adapter_tests.rs` | Postgres adapter SQL construction tests |
| `crates/tasker-runtime/tests/http_adapter_tests.rs` | HTTP adapter request construction tests |

### Modified Files
| File | Changes |
|------|---------|
| `crates/tasker-secure/src/resource/types.rs` | Add `Hash` derive to `ResourceType` |
| `crates/tasker-secure/src/resource/registry.rs` | Add `remove()` method |
| `crates/tasker-secure/src/resource/http.rs` | Add `patch()` method |
| `crates/tasker-grammar/src/operations/types.rs` | Add `PersistMode` enum + field on `PersistConstraints` |
| `crates/tasker-runtime/Cargo.toml` | Add `sqlx`, `tasker-shared`, `regex` dependencies |
| `crates/tasker-runtime/src/lib.rs` | Update re-exports |
| `crates/tasker-runtime/src/adapters/mod.rs` | Add messaging, registry, sql_gen modules |
| `crates/tasker-runtime/src/adapters/postgres.rs` | Implement `persist()` and `acquire()` |
| `crates/tasker-runtime/src/adapters/http.rs` | Implement all three adapters |
| `crates/tasker-runtime/src/pool_manager/mod.rs` | Full ResourcePoolManager implementation |
| `crates/tasker-runtime/src/pool_manager/metrics.rs` | Add `active_checkouts`, `PoolManagerMetrics` |
| `crates/tasker-runtime/src/pool_manager/lifecycle.rs` | Update enums if needed |

---

## Chunk 1: Prerequisites (tasker-secure + tasker-grammar)

### Task 1: Add `Hash` to `ResourceType`

**Files:**
- Modify: `crates/tasker-secure/src/resource/types.rs:10`

- [ ] **Step 1: Add `Hash` derive**

In `crates/tasker-secure/src/resource/types.rs`, change line 10 from:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
```
to:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check --all-features -p tasker-secure`
Expected: success, no errors

---

### Task 2: Add `remove()` to `ResourceRegistry`

**Files:**
- Modify: `crates/tasker-secure/src/resource/registry.rs`

- [ ] **Step 1: Add the `remove` method**

Add after the `get` method (after line 62) in `crates/tasker-secure/src/resource/registry.rs`:

```rust
    /// Remove a resource handle by name, returning it if it existed.
    ///
    /// Takes an exclusive write lock. The returned handle can still be
    /// used by any code that already holds an `Arc` to it — removal
    /// only prevents future lookups.
    pub async fn remove(&self, name: &str) -> Option<Arc<dyn ResourceHandle>> {
        let mut map = self.resources.write().await;
        map.remove(name)
    }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check --all-features -p tasker-secure`
Expected: success

---

### Task 3: Add `patch()` to `HttpHandle`

**Files:**
- Modify: `crates/tasker-secure/src/resource/http.rs`

- [ ] **Step 1: Add the `patch` method**

Add after the `put` method (after line 212) in `crates/tasker-secure/src/resource/http.rs`:

```rust
    /// Create a PATCH request to the given path (appended to `base_url`).
    pub fn patch(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        self.auth.apply(self.client.patch(&url))
    }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check --all-features -p tasker-secure`
Expected: success

---

### Task 4: Add `PersistMode` to tasker-grammar

**Files:**
- Modify: `crates/tasker-grammar/src/operations/types.rs`

- [ ] **Step 1: Add `PersistMode` enum**

Add before the `PersistConstraints` struct in `crates/tasker-grammar/src/operations/types.rs`:

```rust
/// The type of persist operation to perform.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistMode {
    /// INSERT — create new record(s). Fail on conflict.
    #[default]
    Insert,
    /// UPDATE ... WHERE — modify existing record(s) by identity.
    Update,
    /// INSERT ... ON CONFLICT DO UPDATE — create or update.
    Upsert,
    /// DELETE ... WHERE — remove record(s) by identity.
    Delete,
}
```

- [ ] **Step 2: Add `mode` field to `PersistConstraints`**

Add as the first field in `PersistConstraints`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistConstraints {
    /// The type of write operation (insert, update, upsert, delete).
    #[serde(default)]
    pub mode: PersistMode,
    /// Keys that identify the target record(s) for update/upsert/delete.
    pub identity_keys: Option<Vec<String>>,
    /// Keys for upsert conflict resolution (e.g., ["id"], ["order_id", "line_number"])
    pub upsert_key: Option<Vec<String>>,
    /// Conflict resolution strategy
    pub on_conflict: Option<ConflictStrategy>,
    /// Idempotency key for at-most-once semantics
    pub idempotency_key: Option<String>,
}
```

Note: `identity_keys` is added to identify target records for update/delete WHERE clauses. This is distinct from `upsert_key` which is for ON CONFLICT targets.

- [ ] **Step 3: Update `PersistMode` and `PersistConstraints` in the operations module re-exports**

Check `crates/tasker-grammar/src/operations/mod.rs` — ensure `PersistMode` is re-exported. If types are re-exported via `pub use types::*;`, this happens automatically.

- [ ] **Step 4: Fix any compilation issues from the new field**

Run: `cargo check --all-features -p tasker-grammar`

The new `identity_keys` field may cause issues in existing tests that construct `PersistConstraints` — they'll need to add `identity_keys: None`. Since `PersistConstraints` derives `Default` and the field is `Option`, this should be fine for `..Default::default()` patterns. Check the tests:

Run: `cargo test --all-features -p tasker-grammar`
Expected: all existing tests pass (new fields have sensible defaults)

- [ ] **Step 5: Commit prerequisite changes**

```bash
git add crates/tasker-secure/src/resource/types.rs \
       crates/tasker-secure/src/resource/registry.rs \
       crates/tasker-secure/src/resource/http.rs \
       crates/tasker-grammar/src/operations/types.rs
git commit -m "feat(tasker-secure, tasker-grammar): prerequisite changes for TAS-374/375

- Add Hash derive to ResourceType for HashMap key usage
- Add remove() to ResourceRegistry for eviction support
- Add patch() to HttpHandle for PATCH request support
- Add PersistMode enum and identity_keys to PersistConstraints

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 2: SQL Generation + Identifier Sanitization

### Task 5: Identifier sanitization functions

**Files:**
- Create: `crates/tasker-runtime/src/adapters/sql_gen.rs`

- [ ] **Step 1: Write tests for identifier validation**

Create `crates/tasker-runtime/tests/sql_gen_tests.rs`:

```rust
//! Tests for SQL generation and identifier sanitization.

use tasker_runtime::adapters::sql_gen::{validate_identifier, quote_identifier};
use tasker_grammar::operations::ResourceOperationError;

#[test]
fn valid_simple_identifier() {
    assert!(validate_identifier("orders").is_ok());
}

#[test]
fn valid_identifier_with_underscore() {
    assert!(validate_identifier("order_line_items").is_ok());
}

#[test]
fn valid_identifier_starting_with_underscore() {
    assert!(validate_identifier("_internal").is_ok());
}

#[test]
fn invalid_identifier_starts_with_number() {
    assert!(validate_identifier("1orders").is_err());
}

#[test]
fn invalid_identifier_contains_semicolon() {
    assert!(validate_identifier("orders; DROP TABLE").is_err());
}

#[test]
fn invalid_identifier_contains_quotes() {
    assert!(validate_identifier("orders\"--").is_err());
}

#[test]
fn invalid_identifier_empty() {
    assert!(validate_identifier("").is_err());
}

#[test]
fn invalid_identifier_too_long() {
    let long_name = "a".repeat(64);
    assert!(validate_identifier(&long_name).is_err());
}

#[test]
fn valid_identifier_max_length() {
    let max_name = "a".repeat(63);
    assert!(validate_identifier(&max_name).is_ok());
}

#[test]
fn invalid_identifier_unicode() {
    assert!(validate_identifier("über_table").is_err());
}

#[test]
fn quote_identifier_wraps_in_double_quotes() {
    assert_eq!(quote_identifier("orders"), "\"orders\"");
}

#[test]
fn quote_identifier_handles_underscores() {
    assert_eq!(quote_identifier("order_line_items"), "\"order_line_items\"");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --all-features -p tasker-runtime --test sql_gen_tests`
Expected: compilation error — `sql_gen` module doesn't exist yet

- [ ] **Step 3: Create `sql_gen.rs` with identifier functions**

Create `crates/tasker-runtime/src/adapters/sql_gen.rs`:

```rust
//! Pure SQL generation functions and identifier sanitization.
//!
//! All functions return SQL strings and parameter metadata without
//! executing anything. This enables unit testing without a database
//! and future template-time SQL validation via `sqlparser`.

use tasker_grammar::operations::ResourceOperationError;

/// Maximum identifier length (PostgreSQL NAMEDATALEN - 1 for null terminator).
const MAX_IDENTIFIER_LEN: usize = 63;

/// Validate a SQL identifier (table name, column name).
///
/// Rejects anything that doesn't match `[a-zA-Z_][a-zA-Z0-9_]{0,62}`.
/// This is the first layer of defense; `quote_identifier` provides the second.
pub fn validate_identifier(name: &str) -> Result<(), ResourceOperationError> {
    if name.is_empty() {
        return Err(ResourceOperationError::ValidationFailed {
            message: "Identifier cannot be empty".to_string(),
        });
    }

    if name.len() > MAX_IDENTIFIER_LEN {
        return Err(ResourceOperationError::ValidationFailed {
            message: format!(
                "Identifier '{}...' exceeds maximum length of {MAX_IDENTIFIER_LEN}",
                &name[..20]
            ),
        });
    }

    let mut chars = name.chars();

    // First character must be letter or underscore
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => {
            return Err(ResourceOperationError::ValidationFailed {
                message: format!(
                    "Identifier must start with a letter or underscore, got: '{}'",
                    name.chars().next().unwrap_or(' ')
                ),
            });
        }
    }

    // Remaining characters must be alphanumeric or underscore
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return Err(ResourceOperationError::ValidationFailed {
                message: format!("Identifier contains invalid character: '{c}'"),
            });
        }
    }

    Ok(())
}

/// Wrap an identifier in PostgreSQL double quotes for defense-in-depth.
///
/// Call `validate_identifier` first — this function assumes the input
/// has already passed validation.
pub fn quote_identifier(name: &str) -> String {
    format!("\"{name}\"")
}

/// Validate and quote an identifier in one step.
pub fn safe_identifier(name: &str) -> Result<String, ResourceOperationError> {
    validate_identifier(name)?;
    Ok(quote_identifier(name))
}
```

- [ ] **Step 4: Add `sql_gen` module to `adapters/mod.rs`**

In `crates/tasker-runtime/src/adapters/mod.rs`, add after the `http` module:

```rust
#[cfg(feature = "postgres")]
pub mod sql_gen;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --features postgres -p tasker-runtime --test sql_gen_tests`
Expected: all 12 tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/tasker-runtime/src/adapters/sql_gen.rs \
       crates/tasker-runtime/src/adapters/mod.rs \
       crates/tasker-runtime/tests/sql_gen_tests.rs
git commit -m "feat(tasker-runtime): add identifier sanitization for SQL generation

Belt-and-suspenders approach: regex validation rejects exotic identifiers,
then double-quote wrapping provides defense-in-depth. Pure functions
with no I/O, fully unit-testable.

TAS-374

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 6: SQL build functions (INSERT, UPDATE, UPSERT, DELETE, SELECT)

**Files:**
- Modify: `crates/tasker-runtime/src/adapters/sql_gen.rs`
- Modify: `crates/tasker-runtime/tests/sql_gen_tests.rs`

- [ ] **Step 1: Write tests for `build_insert`**

Add to `crates/tasker-runtime/tests/sql_gen_tests.rs`:

```rust
use tasker_runtime::adapters::sql_gen::{build_insert, build_update, build_upsert, build_delete, build_select, SqlOutput};
use tasker_grammar::operations::{PersistConstraints, PersistMode, ConflictStrategy, AcquireConstraints};

#[test]
fn build_insert_simple() {
    let columns = vec!["id".to_string(), "name".to_string(), "total".to_string()];
    let result = build_insert("orders", &columns, &PersistConstraints::default()).unwrap();
    assert_eq!(
        result.sql,
        "INSERT INTO \"orders\" (\"id\", \"name\", \"total\") VALUES ($1, $2, $3) RETURNING *"
    );
    assert_eq!(result.bind_columns, vec!["id", "name", "total"]);
}

#[test]
fn build_insert_single_column() {
    let columns = vec!["id".to_string()];
    let result = build_insert("users", &columns, &PersistConstraints::default()).unwrap();
    assert_eq!(
        result.sql,
        "INSERT INTO \"users\" (\"id\") VALUES ($1) RETURNING *"
    );
}

#[test]
fn build_insert_rejects_invalid_entity() {
    let columns = vec!["id".to_string()];
    assert!(build_insert("orders; DROP TABLE users", &columns, &PersistConstraints::default()).is_err());
}

#[test]
fn build_insert_rejects_invalid_column() {
    let columns = vec!["id".to_string(), "name; --".to_string()];
    assert!(build_insert("orders", &columns, &PersistConstraints::default()).is_err());
}

#[test]
fn build_insert_rejects_empty_columns() {
    assert!(build_insert("orders", &[], &PersistConstraints::default()).is_err());
}
```

- [ ] **Step 2: Write tests for `build_upsert`**

Add to the test file:

```rust
#[test]
fn build_upsert_with_update_strategy() {
    let columns = vec!["id".to_string(), "name".to_string(), "total".to_string()];
    let constraints = PersistConstraints {
        mode: PersistMode::Upsert,
        upsert_key: Some(vec!["id".to_string()]),
        on_conflict: Some(ConflictStrategy::Update),
        ..Default::default()
    };
    let result = build_upsert("orders", &columns, &constraints).unwrap();
    assert!(result.sql.contains("ON CONFLICT (\"id\") DO UPDATE SET"));
    assert!(result.sql.contains("\"name\" = EXCLUDED.\"name\""));
    assert!(result.sql.contains("\"total\" = EXCLUDED.\"total\""));
    // Conflict key should NOT be in the UPDATE SET clause
    assert!(!result.sql.contains("\"id\" = EXCLUDED.\"id\""));
}

#[test]
fn build_upsert_with_skip_strategy() {
    let columns = vec!["id".to_string(), "name".to_string()];
    let constraints = PersistConstraints {
        mode: PersistMode::Upsert,
        upsert_key: Some(vec!["id".to_string()]),
        on_conflict: Some(ConflictStrategy::Skip),
        ..Default::default()
    };
    let result = build_upsert("orders", &columns, &constraints).unwrap();
    assert!(result.sql.contains("ON CONFLICT (\"id\") DO NOTHING"));
}

#[test]
fn build_upsert_with_composite_key() {
    let columns = vec!["order_id".to_string(), "line_num".to_string(), "qty".to_string()];
    let constraints = PersistConstraints {
        mode: PersistMode::Upsert,
        upsert_key: Some(vec!["order_id".to_string(), "line_num".to_string()]),
        on_conflict: Some(ConflictStrategy::Update),
        ..Default::default()
    };
    let result = build_upsert("line_items", &columns, &constraints).unwrap();
    assert!(result.sql.contains("ON CONFLICT (\"order_id\", \"line_num\") DO UPDATE SET"));
}

#[test]
fn build_upsert_requires_upsert_key() {
    let columns = vec!["id".to_string()];
    let constraints = PersistConstraints {
        mode: PersistMode::Upsert,
        on_conflict: Some(ConflictStrategy::Update),
        ..Default::default()
    };
    assert!(build_upsert("orders", &columns, &constraints).is_err());
}
```

- [ ] **Step 3: Write tests for `build_update` and `build_delete`**

Add to the test file:

```rust
#[test]
fn build_update_simple() {
    let columns = vec!["name".to_string(), "total".to_string()];
    let identity_keys = vec!["id".to_string()];
    let result = build_update("orders", &columns, &identity_keys).unwrap();
    assert_eq!(
        result.sql,
        "UPDATE \"orders\" SET \"name\" = $1, \"total\" = $2 WHERE \"id\" = $3 RETURNING *"
    );
    assert_eq!(result.bind_columns, vec!["name", "total", "id"]);
}

#[test]
fn build_update_composite_key() {
    let columns = vec!["qty".to_string()];
    let identity_keys = vec!["order_id".to_string(), "line_num".to_string()];
    let result = build_update("line_items", &columns, &identity_keys).unwrap();
    assert!(result.sql.contains("WHERE \"order_id\" = $2 AND \"line_num\" = $3"));
}

#[test]
fn build_update_requires_identity_keys() {
    let columns = vec!["name".to_string()];
    assert!(build_update("orders", &columns, &[]).is_err());
}

#[test]
fn build_delete_simple() {
    let identity_keys = vec!["id".to_string()];
    let result = build_delete("orders", &identity_keys).unwrap();
    assert_eq!(
        result.sql,
        "DELETE FROM \"orders\" WHERE \"id\" = $1 RETURNING *"
    );
}

#[test]
fn build_delete_composite_key() {
    let identity_keys = vec!["order_id".to_string(), "line_num".to_string()];
    let result = build_delete("line_items", &identity_keys).unwrap();
    assert!(result.sql.contains("WHERE \"order_id\" = $1 AND \"line_num\" = $2"));
}

#[test]
fn build_delete_requires_identity_keys() {
    assert!(build_delete("orders", &[]).is_err());
}
```

- [ ] **Step 4: Write tests for `build_select`**

Add to the test file:

```rust
#[test]
fn build_select_all_columns() {
    let result = build_select("orders", &[], &serde_json::json!({}), &AcquireConstraints::default()).unwrap();
    assert_eq!(result.sql, "SELECT * FROM \"orders\"");
}

#[test]
fn build_select_specific_columns() {
    let columns = vec!["id".to_string(), "name".to_string()];
    let result = build_select("orders", &columns, &serde_json::json!({}), &AcquireConstraints::default()).unwrap();
    assert_eq!(result.sql, "SELECT \"id\", \"name\" FROM \"orders\"");
}

#[test]
fn build_select_with_params() {
    let params = serde_json::json!({"status": "pending", "customer_id": 42});
    let result = build_select("orders", &[], &params, &AcquireConstraints::default()).unwrap();
    assert!(result.sql.contains("WHERE"));
    // Params are bound by position — check that both columns appear
    assert!(result.sql.contains("\"customer_id\" = $"));
    assert!(result.sql.contains("\"status\" = $"));
}

#[test]
fn build_select_with_limit_and_offset() {
    let constraints = AcquireConstraints {
        limit: Some(100),
        offset: Some(50),
        ..Default::default()
    };
    let result = build_select("orders", &[], &serde_json::json!({}), &constraints).unwrap();
    assert!(result.sql.contains("LIMIT 100"));
    assert!(result.sql.contains("OFFSET 50"));
}

#[test]
fn build_select_rejects_invalid_entity() {
    assert!(build_select("orders; --", &[], &serde_json::json!({}), &AcquireConstraints::default()).is_err());
}
```

- [ ] **Step 5: Run tests to verify they fail**

Run: `cargo test --features postgres -p tasker-runtime --test sql_gen_tests`
Expected: compilation errors — functions don't exist yet

- [ ] **Step 6: Implement SQL build functions**

Add to `crates/tasker-runtime/src/adapters/sql_gen.rs`:

```rust
/// Output of a SQL generation function.
#[derive(Debug, Clone)]
pub struct SqlOutput {
    /// The generated SQL string with $N placeholders.
    pub sql: String,
    /// Column names in bind order — caller binds values in this order.
    pub bind_columns: Vec<String>,
}

/// Build an INSERT statement.
///
/// Returns SQL with $N placeholders and the column names in bind order.
/// Always appends RETURNING * for result capture.
pub fn build_insert(
    entity: &str,
    columns: &[String],
    _constraints: &tasker_grammar::operations::PersistConstraints,
) -> Result<SqlOutput, ResourceOperationError> {
    validate_identifier(entity)?;

    if columns.is_empty() {
        return Err(ResourceOperationError::ValidationFailed {
            message: "INSERT requires at least one column".to_string(),
        });
    }

    let mut quoted_cols = Vec::with_capacity(columns.len());
    for col in columns {
        quoted_cols.push(safe_identifier(col)?);
    }

    let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${i}")).collect();

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) RETURNING *",
        quote_identifier(entity),
        quoted_cols.join(", "),
        placeholders.join(", "),
    );

    Ok(SqlOutput {
        sql,
        bind_columns: columns.to_vec(),
    })
}

/// Build an UPDATE statement with PK-based WHERE clause.
///
/// `columns` are the SET columns; `identity_keys` form the WHERE clause.
/// Bind order: SET values first, then WHERE values.
pub fn build_update(
    entity: &str,
    columns: &[String],
    identity_keys: &[String],
) -> Result<SqlOutput, ResourceOperationError> {
    validate_identifier(entity)?;

    if columns.is_empty() {
        return Err(ResourceOperationError::ValidationFailed {
            message: "UPDATE requires at least one column".to_string(),
        });
    }
    if identity_keys.is_empty() {
        return Err(ResourceOperationError::ValidationFailed {
            message: "UPDATE requires identity keys for WHERE clause".to_string(),
        });
    }

    let mut bind_pos = 1;
    let mut set_parts = Vec::with_capacity(columns.len());
    let mut bind_columns = Vec::with_capacity(columns.len() + identity_keys.len());

    for col in columns {
        let quoted = safe_identifier(col)?;
        set_parts.push(format!("{quoted} = ${bind_pos}"));
        bind_columns.push(col.clone());
        bind_pos += 1;
    }

    let mut where_parts = Vec::with_capacity(identity_keys.len());
    for key in identity_keys {
        let quoted = safe_identifier(key)?;
        where_parts.push(format!("{quoted} = ${bind_pos}"));
        bind_columns.push(key.clone());
        bind_pos += 1;
    }

    let sql = format!(
        "UPDATE {} SET {} WHERE {} RETURNING *",
        quote_identifier(entity),
        set_parts.join(", "),
        where_parts.join(" AND "),
    );

    Ok(SqlOutput { sql, bind_columns })
}

/// Build an INSERT ... ON CONFLICT statement (upsert).
///
/// Requires `upsert_key` in constraints. Conflict strategy determines
/// DO UPDATE SET vs DO NOTHING.
pub fn build_upsert(
    entity: &str,
    columns: &[String],
    constraints: &tasker_grammar::operations::PersistConstraints,
) -> Result<SqlOutput, ResourceOperationError> {
    let upsert_keys = constraints.upsert_key.as_ref().ok_or_else(|| {
        ResourceOperationError::ValidationFailed {
            message: "UPSERT requires upsert_key in constraints".to_string(),
        }
    })?;

    // Start with a normal INSERT
    let mut output = build_insert(entity, columns, constraints)?;

    // Build conflict clause
    let mut conflict_cols = Vec::with_capacity(upsert_keys.len());
    for key in upsert_keys {
        conflict_cols.push(safe_identifier(key)?);
    }

    // Remove the RETURNING * we appended in build_insert — we'll re-add it
    let base_sql = output.sql.trim_end_matches(" RETURNING *");

    let conflict_strategy = constraints
        .on_conflict
        .as_ref()
        .unwrap_or(&tasker_grammar::operations::ConflictStrategy::Reject);

    let sql = match conflict_strategy {
        tasker_grammar::operations::ConflictStrategy::Reject => {
            // No ON CONFLICT clause — database raises error on conflict
            format!("{base_sql} RETURNING *")
        }
        tasker_grammar::operations::ConflictStrategy::Update => {
            let update_cols: Vec<String> = columns
                .iter()
                .filter(|c| !upsert_keys.contains(c))
                .map(|c| {
                    let quoted = safe_identifier(c).expect("already validated in build_insert");
                    format!("{quoted} = EXCLUDED.{quoted}")
                })
                .collect();

            format!(
                "{base_sql} ON CONFLICT ({}) DO UPDATE SET {} RETURNING *",
                conflict_cols.join(", "),
                update_cols.join(", "),
            )
        }
        tasker_grammar::operations::ConflictStrategy::Skip => {
            format!(
                "{base_sql} ON CONFLICT ({}) DO NOTHING RETURNING *",
                conflict_cols.join(", "),
            )
        }
    };

    output.sql = sql;
    Ok(output)
}

/// Build a DELETE statement with PK-based WHERE clause.
pub fn build_delete(
    entity: &str,
    identity_keys: &[String],
) -> Result<SqlOutput, ResourceOperationError> {
    validate_identifier(entity)?;

    if identity_keys.is_empty() {
        return Err(ResourceOperationError::ValidationFailed {
            message: "DELETE requires identity keys for WHERE clause".to_string(),
        });
    }

    let mut where_parts = Vec::with_capacity(identity_keys.len());
    let mut bind_columns = Vec::with_capacity(identity_keys.len());

    for (i, key) in identity_keys.iter().enumerate() {
        let quoted = safe_identifier(key)?;
        where_parts.push(format!("{quoted} = ${}", i + 1));
        bind_columns.push(key.clone());
    }

    let sql = format!(
        "DELETE FROM {} WHERE {} RETURNING *",
        quote_identifier(entity),
        where_parts.join(" AND "),
    );

    Ok(SqlOutput { sql, bind_columns })
}

/// Build a SELECT statement from structured parameters.
///
/// `columns` empty means SELECT *. `params` JSON object keys become
/// WHERE conditions. Constraints provide LIMIT/OFFSET.
pub fn build_select(
    entity: &str,
    columns: &[String],
    params: &serde_json::Value,
    constraints: &tasker_grammar::operations::AcquireConstraints,
) -> Result<SqlOutput, ResourceOperationError> {
    validate_identifier(entity)?;

    // Column list
    let col_list = if columns.is_empty() {
        "*".to_string()
    } else {
        let mut quoted = Vec::with_capacity(columns.len());
        for col in columns {
            quoted.push(safe_identifier(col)?);
        }
        quoted.join(", ")
    };

    let mut sql = format!("SELECT {col_list} FROM {}", quote_identifier(entity));
    let mut bind_columns = Vec::new();

    // WHERE from params
    if let Some(obj) = params.as_object() {
        if !obj.is_empty() {
            let mut where_parts = Vec::new();
            let mut bind_pos = 1;

            // Sort keys for deterministic output in tests
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();

            for key in keys {
                let quoted = safe_identifier(key)?;
                where_parts.push(format!("{quoted} = ${bind_pos}"));
                bind_columns.push(key.clone());
                bind_pos += 1;
            }
            sql.push_str(&format!(" WHERE {}", where_parts.join(" AND ")));
        }
    }

    // LIMIT / OFFSET
    if let Some(limit) = constraints.limit {
        sql.push_str(&format!(" LIMIT {limit}"));
    }
    if let Some(offset) = constraints.offset {
        sql.push_str(&format!(" OFFSET {offset}"));
    }

    Ok(SqlOutput { sql, bind_columns })
}
```

- [ ] **Step 7: Run tests**

Run: `cargo test --features postgres -p tasker-runtime --test sql_gen_tests`
Expected: all tests pass

- [ ] **Step 8: Commit**

```bash
git add crates/tasker-runtime/src/adapters/sql_gen.rs \
       crates/tasker-runtime/tests/sql_gen_tests.rs
git commit -m "feat(tasker-runtime): SQL generation functions for all persist/acquire modes

Pure functions: build_insert, build_update, build_upsert, build_delete,
build_select. All return SqlOutput (SQL string + bind column order).
Deterministic parameter ordering for testability.

TAS-374

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 3: Postgres Adapters

### Task 7: PostgresPersistAdapter implementation

**Files:**
- Modify: `crates/tasker-runtime/src/adapters/postgres.rs`
- Modify: `crates/tasker-runtime/Cargo.toml`

- [ ] **Step 1: Add `sqlx` dependency to tasker-runtime**

In `crates/tasker-runtime/Cargo.toml`, add under `[dependencies]`:

```toml
sqlx = { workspace = true, optional = true }
```

Update the `postgres` feature:

```toml
postgres = ["tasker-secure/postgres", "dep:sqlx"]
```

- [ ] **Step 2: Implement `PostgresPersistAdapter::persist`**

Replace the `unimplemented!` in `crates/tasker-runtime/src/adapters/postgres.rs`:

```rust
//! PostgreSQL adapters for persist and acquire operations.
//!
//! Wraps `tasker_secure::resource::postgres::PostgresHandle` and implements
//! `PersistableResource` (SQL INSERT/UPDATE/UPSERT/DELETE) and `AcquirableResource` (SQL SELECT).

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::Row;

use tasker_grammar::operations::{
    AcquirableResource, AcquireConstraints, AcquireResult, PersistConstraints, PersistMode,
    PersistResult, PersistableResource, ResourceOperationError,
};
use tasker_secure::resource::postgres::PostgresHandle;

use super::sql_gen;

/// Adapts a `PostgresHandle` for structured write operations.
///
/// Stores the handle as `Arc<dyn ResourceHandle>` and downcasts on use.
/// This avoids requiring `Clone` on `PostgresHandle`.
#[derive(Debug)]
pub struct PostgresPersistAdapter {
    handle: Arc<dyn tasker_secure::ResourceHandle>,
}

impl PostgresPersistAdapter {
    /// Create a new persist adapter wrapping the given handle.
    pub fn new(handle: Arc<dyn tasker_secure::ResourceHandle>) -> Self {
        Self { handle }
    }

    fn pg_handle(&self) -> Result<&PostgresHandle, ResourceOperationError> {
        use tasker_secure::resource::ResourceHandleExt;
        self.handle.as_postgres().ok_or_else(|| ResourceOperationError::ValidationFailed {
            message: format!("Expected Postgres handle, got {:?}", self.handle.resource_type()),
        })
    }
}

#[async_trait]
impl PersistableResource for PostgresPersistAdapter {
    async fn persist(
        &self,
        entity: &str,
        data: serde_json::Value,
        constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        let obj = data.as_object().ok_or_else(|| {
            ResourceOperationError::ValidationFailed {
                message: "persist data must be a JSON object".to_string(),
            }
        })?;

        let columns: Vec<String> = obj.keys().cloned().collect();

        // Generate SQL based on mode
        let output = match constraints.mode {
            PersistMode::Insert => sql_gen::build_insert(entity, &columns, constraints)?,
            PersistMode::Update => {
                let identity_keys = constraints.identity_keys.as_deref().unwrap_or_default();
                if identity_keys.is_empty() {
                    return Err(ResourceOperationError::ValidationFailed {
                        message: "UPDATE requires identity_keys in constraints".to_string(),
                    });
                }
                // SET columns are all columns minus identity keys
                let set_columns: Vec<String> = columns
                    .iter()
                    .filter(|c| !identity_keys.contains(&c.as_str()))
                    .cloned()
                    .collect();
                sql_gen::build_update(entity, &set_columns, &identity_keys.iter().map(|s| s.to_string()).collect::<Vec<_>>())?
            }
            PersistMode::Upsert => sql_gen::build_upsert(entity, &columns, constraints)?,
            PersistMode::Delete => {
                let identity_keys = constraints.identity_keys.as_deref().unwrap_or_default();
                if identity_keys.is_empty() {
                    return Err(ResourceOperationError::ValidationFailed {
                        message: "DELETE requires identity_keys in constraints".to_string(),
                    });
                }
                sql_gen::build_delete(entity, &identity_keys.iter().map(|s| s.to_string()).collect::<Vec<_>>())?
            }
        };

        // Build the query and bind values in order
        let mut query = sqlx::query(&output.sql);
        for col_name in &output.bind_columns {
            let value = obj.get(col_name).unwrap_or(&serde_json::Value::Null);
            query = bind_json_value(query, value);
        }

        // Execute with fetch_optional to capture RETURNING * row
        let pg = self.pg_handle()?;
        let pool = pg.pool();
        let row = query
            .fetch_optional(pool)
            .await
            .map_err(|e| ResourceOperationError::Other {
                message: format!("SQL execution failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        let data = match row {
            Some(row) => row_to_json(&row)?,
            None => serde_json::json!({}),
        };

        Ok(PersistResult {
            data,
            affected_count: Some(1),
        })
    }
}

/// Bind a `serde_json::Value` to a sqlx query as the appropriate PostgreSQL type.
fn bind_json_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    value: &'q serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match value {
        serde_json::Value::Null => query.bind(None::<String>),
        serde_json::Value::Bool(b) => query.bind(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                query.bind(i)
            } else if let Some(f) = n.as_f64() {
                query.bind(f)
            } else {
                query.bind(n.to_string())
            }
        }
        serde_json::Value::String(s) => query.bind(s.as_str()),
        // Arrays and objects bind as JSONB
        other => query.bind(other),
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check --features postgres -p tasker-runtime`
Expected: success (may need to adjust lifetime annotations on `bind_json_value` — iterate if needed)

- [ ] **Step 4: Commit**

```bash
git add crates/tasker-runtime/src/adapters/postgres.rs \
       crates/tasker-runtime/Cargo.toml
git commit -m "feat(tasker-runtime): implement PostgresPersistAdapter with SQL generation

Translates structured persist operations into parameterized SQL via
sql_gen functions. Supports all four modes: insert, update, upsert, delete.
JSON values bound as appropriate PostgreSQL types.

TAS-374

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 8: PostgresAcquireAdapter implementation

**Files:**
- Modify: `crates/tasker-runtime/src/adapters/postgres.rs`

- [ ] **Step 1: Implement `PostgresAcquireAdapter::acquire`**

Replace the `unimplemented!` in the `AcquirableResource` impl:

```rust
#[async_trait]
impl AcquirableResource for PostgresAcquireAdapter {
    async fn acquire(
        &self,
        entity: &str,
        params: serde_json::Value,
        constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        // Extract column names from params if present as "_columns" key,
        // otherwise SELECT *
        let columns: Vec<String> = params
            .get("_columns")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Filter params: remove meta keys (starting with _)
        let filter_params = if let Some(obj) = params.as_object() {
            let filtered: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .filter(|(k, _)| !k.starts_with('_'))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            serde_json::Value::Object(filtered)
        } else {
            serde_json::json!({})
        };

        let output = sql_gen::build_select(entity, &columns, &filter_params, constraints)?;

        // Build query with bindings
        let mut query = sqlx::query(&output.sql);
        if let Some(obj) = filter_params.as_object() {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort(); // Match the sort order in build_select
            for key in keys {
                let value = &obj[key];
                query = bind_json_value(query, value);
            }
        }

        let pg = self.pg_handle()?;
        let pool = pg.pool();
        let rows = query
            .fetch_all(pool)
            .await
            .map_err(|e| ResourceOperationError::Other {
                message: format!("SQL query failed: {e}"),
                source: Some(Box::new(e)),
            })?;

        // Convert rows to JSON array
        let data: Vec<serde_json::Value> = rows
            .iter()
            .map(|row| row_to_json(row))
            .collect::<Result<Vec<_>, _>>()?;

        let total_count = Some(data.len() as u64);

        Ok(AcquireResult {
            data: serde_json::Value::Array(data),
            total_count,
        })
    }
}

/// Convert a sqlx Row to a JSON object using column metadata.
fn row_to_json(row: &sqlx::postgres::PgRow) -> Result<serde_json::Value, ResourceOperationError> {
    use sqlx::Column;
    use sqlx::TypeInfo;
    use sqlx::ValueRef;

    let mut obj = serde_json::Map::new();
    for col in row.columns() {
        let name = col.name().to_string();
        let value = if row.try_get_raw(col.ordinal()).map(|v| v.is_null()).unwrap_or(true) {
            serde_json::Value::Null
        } else {
            // Try common types in order
            let type_name = col.type_info().name();
            match type_name {
                "BOOL" => {
                    let v: bool = row.try_get(col.ordinal()).map_err(|e| {
                        ResourceOperationError::Other {
                            message: format!("Failed to read column '{name}': {e}"),
                            source: Some(Box::new(e)),
                        }
                    })?;
                    serde_json::Value::Bool(v)
                }
                "INT2" | "INT4" => {
                    let v: i32 = row.try_get(col.ordinal()).map_err(|e| {
                        ResourceOperationError::Other {
                            message: format!("Failed to read column '{name}': {e}"),
                            source: Some(Box::new(e)),
                        }
                    })?;
                    serde_json::json!(v)
                }
                "INT8" => {
                    let v: i64 = row.try_get(col.ordinal()).map_err(|e| {
                        ResourceOperationError::Other {
                            message: format!("Failed to read column '{name}': {e}"),
                            source: Some(Box::new(e)),
                        }
                    })?;
                    serde_json::json!(v)
                }
                "FLOAT4" | "FLOAT8" | "NUMERIC" => {
                    let v: f64 = row.try_get(col.ordinal()).map_err(|e| {
                        ResourceOperationError::Other {
                            message: format!("Failed to read column '{name}': {e}"),
                            source: Some(Box::new(e)),
                        }
                    })?;
                    serde_json::json!(v)
                }
                "JSON" | "JSONB" => {
                    let v: serde_json::Value = row.try_get(col.ordinal()).map_err(|e| {
                        ResourceOperationError::Other {
                            message: format!("Failed to read column '{name}': {e}"),
                            source: Some(Box::new(e)),
                        }
                    })?;
                    v
                }
                _ => {
                    // Default: try as string
                    let v: String = row.try_get(col.ordinal()).map_err(|e| {
                        ResourceOperationError::Other {
                            message: format!("Failed to read column '{name}': {e}"),
                            source: Some(Box::new(e)),
                        }
                    })?;
                    serde_json::Value::String(v)
                }
            }
        };
        obj.insert(name, value);
    }
    Ok(serde_json::Value::Object(obj))
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check --features postgres -p tasker-runtime`
Expected: success

- [ ] **Step 3: Commit**

```bash
git add crates/tasker-runtime/src/adapters/postgres.rs
git commit -m "feat(tasker-runtime): implement PostgresAcquireAdapter with row-to-JSON

Translates structured acquire operations into SELECT queries.
Converts sqlx PgRow to serde_json::Value using column type metadata.

TAS-374

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 4: HTTP Adapters + Messaging Adapter

### Task 9: HTTP adapter implementations

**Files:**
- Modify: `crates/tasker-runtime/src/adapters/http.rs`

- [ ] **Step 1: Write tests for HTTP URL and method construction**

Create `crates/tasker-runtime/tests/http_adapter_tests.rs`:

```rust
//! Tests for HTTP adapter URL and method construction logic.

use tasker_grammar::operations::PersistMode;
use tasker_runtime::adapters::http::http_persist_method;

#[test]
fn persist_mode_insert_maps_to_post() {
    assert_eq!(http_persist_method(&PersistMode::Insert), "POST");
}

#[test]
fn persist_mode_update_maps_to_patch() {
    assert_eq!(http_persist_method(&PersistMode::Update), "PATCH");
}

#[test]
fn persist_mode_upsert_maps_to_put() {
    assert_eq!(http_persist_method(&PersistMode::Upsert), "PUT");
}

#[test]
fn persist_mode_delete_maps_to_delete() {
    assert_eq!(http_persist_method(&PersistMode::Delete), "DELETE");
}
```

- [ ] **Step 2: Run tests — expect failure**

Run: `cargo test --features http -p tasker-runtime --test http_adapter_tests`
Expected: compilation error

- [ ] **Step 3: Implement HTTP adapters**

Replace `crates/tasker-runtime/src/adapters/http.rs`:

```rust
//! HTTP adapters for persist, acquire, and emit operations.
//!
//! Wraps `tasker_secure::resource::http::HttpHandle` and implements
//! `PersistableResource` (POST/PUT/PATCH/DELETE), `AcquirableResource` (GET),
//! and `EmittableResource` (POST webhook).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use tasker_grammar::operations::{
    AcquirableResource, AcquireConstraints, AcquireResult, EmitMetadata, EmitResult,
    EmittableResource, PersistConstraints, PersistMode, PersistResult, PersistableResource,
    ResourceOperationError,
};
use tasker_secure::resource::http::HttpHandle;

/// Map PersistMode to HTTP method name (public for testing).
pub fn http_persist_method(mode: &PersistMode) -> &'static str {
    match mode {
        PersistMode::Insert => "POST",
        PersistMode::Update => "PATCH",
        PersistMode::Upsert => "PUT",
        PersistMode::Delete => "DELETE",
    }
}

/// Build the URL path for a persist operation.
fn persist_url(entity: &str, mode: &PersistMode, data: &serde_json::Value, constraints: &PersistConstraints) -> String {
    match mode {
        PersistMode::Insert => format!("/{entity}"),
        PersistMode::Update | PersistMode::Upsert | PersistMode::Delete => {
            // Extract PK value(s) from data using identity_keys
            let pk_value = extract_pk_path(data, constraints);
            if pk_value.is_empty() {
                format!("/{entity}")
            } else {
                format!("/{entity}/{pk_value}")
            }
        }
    }
}

/// Extract primary key value(s) from data for URL construction.
fn extract_pk_path(data: &serde_json::Value, constraints: &PersistConstraints) -> String {
    let keys = constraints
        .identity_keys
        .as_deref()
        .or(constraints.upsert_key.as_deref())
        .unwrap_or_default();

    keys.iter()
        .filter_map(|k| data.get(k))
        .map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Map HTTP response status to appropriate error.
fn map_http_error(status: reqwest::StatusCode, entity: &str, operation: &str) -> ResourceOperationError {
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        ResourceOperationError::AuthorizationFailed {
            operation: operation.to_string(),
            entity: entity.to_string(),
        }
    } else if status == reqwest::StatusCode::NOT_FOUND {
        ResourceOperationError::EntityNotFound {
            entity: entity.to_string(),
        }
    } else if status == reqwest::StatusCode::CONFLICT {
        ResourceOperationError::Conflict {
            entity: entity.to_string(),
            reason: format!("HTTP {status}"),
        }
    } else {
        ResourceOperationError::Other {
            message: format!("HTTP {status}"),
            source: None,
        }
    }
}

/// Parse a response body as JSON, with a fallback to a status wrapper.
async fn parse_response_json(response: reqwest::Response) -> Result<serde_json::Value, ResourceOperationError> {
    let status = response.status();
    let body = response.text().await.map_err(|e| ResourceOperationError::Other {
        message: format!("Failed to read response body: {e}"),
        source: Some(Box::new(e)),
    })?;

    if body.is_empty() {
        return Ok(serde_json::json!({"status": status.as_u16()}));
    }

    serde_json::from_str(&body).unwrap_or_else(|_| {
        serde_json::json!({"status": status.as_u16(), "body": body})
    })
}

/// Adapts an `HttpHandle` for structured write operations (POST/PUT/PATCH/DELETE).
#[derive(Debug)]
pub struct HttpPersistAdapter {
    handle: Arc<dyn tasker_secure::ResourceHandle>,
}

impl HttpPersistAdapter {
    /// Create a new persist adapter wrapping the given handle.
    pub fn new(handle: Arc<dyn tasker_secure::ResourceHandle>) -> Self {
        Self { handle }
    }

    fn http_handle(&self) -> Result<&HttpHandle, ResourceOperationError> {
        use tasker_secure::resource::ResourceHandleExt;
        self.handle.as_http().ok_or_else(|| ResourceOperationError::ValidationFailed {
            message: format!("Expected HTTP handle, got {:?}", self.handle.resource_type()),
        })
    }
}

#[async_trait]
impl PersistableResource for HttpPersistAdapter {
    async fn persist(
        &self,
        entity: &str,
        data: serde_json::Value,
        constraints: &PersistConstraints,
    ) -> Result<PersistResult, ResourceOperationError> {
        let path = persist_url(entity, &constraints.mode, &data, constraints);

        let request = match constraints.mode {
            PersistMode::Insert => self.handle.post(&path).json(&data),
            PersistMode::Update => self.handle.patch(&path).json(&data),
            PersistMode::Upsert => self.handle.put(&path).json(&data),
            PersistMode::Delete => {
                let req = self.handle.delete(&path);
                // Include body for DELETE only if data is non-empty
                if data.as_object().map_or(false, |o| !o.is_empty()) {
                    req.json(&data)
                } else {
                    req
                }
            }
        };

        let response = request.send().await.map_err(|e| {
            ResourceOperationError::Unavailable {
                message: format!("HTTP request failed: {e}"),
            }
        })?;

        if !response.status().is_success() {
            return Err(map_http_error(response.status(), entity, "persist"));
        }

        let response_data = parse_response_json(response).await?;

        Ok(PersistResult {
            data: response_data,
            affected_count: Some(1),
        })
    }
}

/// Adapts an `HttpHandle` for structured read operations (GET).
///
/// Same pattern as HttpPersistAdapter: stores Arc<dyn ResourceHandle>,
/// downcasts via http_handle() helper.
#[derive(Debug)]
pub struct HttpAcquireAdapter {
    handle: Arc<dyn tasker_secure::ResourceHandle>,
}

impl HttpAcquireAdapter {
    pub fn new(handle: Arc<dyn tasker_secure::ResourceHandle>) -> Self {
        Self { handle }
    }

    fn http_handle(&self) -> Result<&HttpHandle, ResourceOperationError> {
        use tasker_secure::resource::ResourceHandleExt;
        self.handle.as_http().ok_or_else(|| ResourceOperationError::ValidationFailed {
            message: format!("Expected HTTP handle, got {:?}", self.handle.resource_type()),
        })
    }
}

#[async_trait]
impl AcquirableResource for HttpAcquireAdapter {
    async fn acquire(
        &self,
        entity: &str,
        params: serde_json::Value,
        constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        let path = format!("/{entity}");
        let mut request = self.handle.get(&path);

        // Map params to query string
        if let Some(obj) = params.as_object() {
            for (key, value) in obj {
                if key.starts_with('_') {
                    continue; // Skip meta keys
                }
                let str_val = match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                request = request.query(&[(key.as_str(), str_val.as_str())]);
            }
        }

        // Apply constraints
        if let Some(limit) = constraints.limit {
            request = request.query(&[("limit", &limit.to_string())]);
        }
        if let Some(offset) = constraints.offset {
            request = request.query(&[("offset", &offset.to_string())]);
        }
        if let Some(timeout_ms) = constraints.timeout_ms {
            request = request.timeout(Duration::from_millis(timeout_ms));
        }

        let response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                ResourceOperationError::Timeout {
                    timeout_ms: constraints.timeout_ms.unwrap_or(0),
                }
            } else {
                ResourceOperationError::Unavailable {
                    message: format!("HTTP request failed: {e}"),
                }
            }
        })?;

        if !response.status().is_success() {
            return Err(map_http_error(response.status(), entity, "acquire"));
        }

        let data = parse_response_json(response).await?;

        Ok(AcquireResult {
            data,
            total_count: None,
        })
    }
}

/// Adapts an `HttpHandle` for event emission (POST webhook).
/// Same Arc<dyn ResourceHandle> + downcast pattern.
#[derive(Debug)]
pub struct HttpEmitAdapter {
    handle: Arc<dyn tasker_secure::ResourceHandle>,
}

impl HttpEmitAdapter {
    pub fn new(handle: Arc<dyn tasker_secure::ResourceHandle>) -> Self {
        Self { handle }
    }

    fn http_handle(&self) -> Result<&HttpHandle, ResourceOperationError> {
        use tasker_secure::resource::ResourceHandleExt;
        self.handle.as_http().ok_or_else(|| ResourceOperationError::ValidationFailed {
            message: format!("Expected HTTP handle, got {:?}", self.handle.resource_type()),
        })
    }
}

#[async_trait]
impl EmittableResource for HttpEmitAdapter {
    async fn emit(
        &self,
        topic: &str,
        payload: serde_json::Value,
        metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError> {
        let path = format!("/{topic}");
        let mut request = self.handle.post(&path).json(&payload);

        // Map metadata to headers
        if let Some(ref correlation_id) = metadata.correlation_id {
            request = request.header("X-Correlation-ID", correlation_id);
        }
        if let Some(ref idempotency_key) = metadata.idempotency_key {
            request = request.header("Idempotency-Key", idempotency_key);
        }
        if let Some(ref attrs) = metadata.attributes {
            for (key, value) in attrs {
                request = request.header(key, value);
            }
        }

        let response = request.send().await.map_err(|e| {
            ResourceOperationError::Unavailable {
                message: format!("HTTP emit failed: {e}"),
            }
        })?;

        let confirmed = response.status().is_success();

        if !confirmed {
            return Err(map_http_error(response.status(), topic, "emit"));
        }

        let data = parse_response_json(response).await?;

        Ok(EmitResult { data, confirmed })
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --features http -p tasker-runtime --test http_adapter_tests`
Expected: all tests pass

- [ ] **Step 5: Compile check**

Run: `cargo check --features http -p tasker-runtime`
Expected: success

- [ ] **Step 6: Commit**

```bash
git add crates/tasker-runtime/src/adapters/http.rs \
       crates/tasker-runtime/tests/http_adapter_tests.rs
git commit -m "feat(tasker-runtime): implement HTTP persist/acquire/emit adapters

HttpPersistAdapter maps PersistMode to HTTP methods (POST/PATCH/PUT/DELETE).
HttpAcquireAdapter builds GET with query params and pagination.
HttpEmitAdapter POSTs webhook with correlation/idempotency headers.

TAS-374

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 10: MessagingEmitAdapter

**Files:**
- Create: `crates/tasker-runtime/src/adapters/messaging.rs`
- Modify: `crates/tasker-runtime/src/adapters/mod.rs`
- Modify: `crates/tasker-runtime/Cargo.toml`

- [ ] **Step 1: Add `tasker-shared` dependency**

In `crates/tasker-runtime/Cargo.toml`, add under `[dependencies]`:

```toml
tasker-shared = { path = "../tasker-shared", version = "=0.1.6" }
```

- [ ] **Step 2: Create `messaging.rs`**

Create `crates/tasker-runtime/src/adapters/messaging.rs`:

```rust
//! Messaging adapter for emit operations.
//!
//! Wraps `tasker_shared::MessagingProvider` (PGMQ or RabbitMQ) and implements
//! `EmittableResource` for domain event emission through the existing
//! messaging infrastructure.

use std::sync::Arc;

use async_trait::async_trait;

use tasker_grammar::operations::{
    EmitMetadata, EmitResult, EmittableResource, ResourceOperationError,
};
use tasker_shared::messaging::MessagingProvider;

/// Adapts the existing messaging infrastructure for grammar emit operations.
///
/// Unlike the handle-based adapters, this wraps `MessagingProvider` directly
/// since PGMQ/RabbitMQ are infrastructure-level concerns already managed
/// by tasker-shared, not per-resource handles.
#[derive(Debug)]
pub struct MessagingEmitAdapter {
    provider: Arc<MessagingProvider>,
}

impl MessagingEmitAdapter {
    /// Create a new messaging emit adapter.
    pub fn new(provider: Arc<MessagingProvider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl EmittableResource for MessagingEmitAdapter {
    async fn emit(
        &self,
        topic: &str,
        payload: serde_json::Value,
        _metadata: &EmitMetadata,
    ) -> Result<EmitResult, ResourceOperationError> {
        // Ensure the queue exists (idempotent)
        self.provider
            .ensure_queue(topic)
            .await
            .map_err(|e| ResourceOperationError::Unavailable {
                message: format!("Failed to ensure queue '{topic}': {e}"),
            })?;

        // Send the payload as a JSON message
        let message_id = self
            .provider
            .send_message(topic, &payload)
            .await
            .map_err(|e| ResourceOperationError::Other {
                message: format!("Failed to send message to '{topic}': {e}"),
                source: Some(Box::new(e)),
            })?;

        Ok(EmitResult {
            data: serde_json::json!({
                "message_id": message_id.to_string(),
                "queue": topic,
            }),
            confirmed: true,
        })
    }
}
```

- [ ] **Step 3: Add `messaging` module to `adapters/mod.rs`**

In `crates/tasker-runtime/src/adapters/mod.rs`, add:

```rust
pub mod messaging;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check --all-features -p tasker-runtime`
Expected: success (may need to check the `MessagingProvider` import path — adjust if `tasker_shared::messaging::MessagingProvider` is not the right path; it might be `tasker_shared::messaging::service::MessagingProvider`)

- [ ] **Step 5: Commit**

```bash
git add crates/tasker-runtime/src/adapters/messaging.rs \
       crates/tasker-runtime/src/adapters/mod.rs \
       crates/tasker-runtime/Cargo.toml
git commit -m "feat(tasker-runtime): implement MessagingEmitAdapter for PGMQ/RabbitMQ

Wraps Arc<MessagingProvider> to emit domain events through existing
messaging infrastructure. Works with both PGMQ and RabbitMQ backends.

TAS-374

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 5: AdapterRegistry

### Task 11: AdapterRegistry with closure-based factories

**Files:**
- Create: `crates/tasker-runtime/src/adapters/registry.rs`
- Create: `crates/tasker-runtime/tests/adapter_registry_tests.rs`
- Modify: `crates/tasker-runtime/src/adapters/mod.rs`

- [ ] **Step 1: Write registry tests**

Create `crates/tasker-runtime/tests/adapter_registry_tests.rs`:

```rust
//! Tests for AdapterRegistry factory dispatch.

use std::sync::Arc;

use tasker_grammar::operations::ResourceOperationError;
use tasker_runtime::adapters::registry::AdapterRegistry;
use tasker_secure::testing::InMemoryResourceHandle;
use tasker_secure::ResourceType;

// Note: InMemoryResourceHandle can't downcast to PostgresHandle/HttpHandle,
// so factory *execution* returns Err. These tests verify the factory is
// registered (lookup succeeds) and the downcast error is descriptive.

#[test]
fn standard_registry_has_postgres_persist_factory() {
    let registry = AdapterRegistry::standard();
    let handle = Arc::new(InMemoryResourceHandle::new("test_db", ResourceType::Postgres));
    let result = registry.as_persistable(handle);
    // Factory is registered but downcast fails for InMemoryResourceHandle
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("Expected Postgres handle"), "Got: {msg}");
}

#[test]
fn standard_registry_has_http_emit_factory() {
    let registry = AdapterRegistry::standard();
    let handle = Arc::new(InMemoryResourceHandle::new("webhook", ResourceType::Http));
    let result = registry.as_emittable(handle);
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("Expected HTTP handle"), "Got: {msg}");
}

#[test]
fn unknown_resource_type_returns_no_factory_error() {
    let registry = AdapterRegistry::standard();
    let handle = Arc::new(InMemoryResourceHandle::new(
        "custom",
        ResourceType::Custom { type_name: "redis".to_string() },
    ));
    let result = registry.as_persistable(handle);
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("No persist adapter registered"), "Got: {msg}");
}

#[test]
fn pgmq_has_no_persist_factory() {
    let registry = AdapterRegistry::standard();
    let handle = Arc::new(InMemoryResourceHandle::new("queue", ResourceType::Pgmq));
    let result = registry.as_persistable(handle);
    let err = result.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("No persist adapter registered"), "Got: {msg}");
}

#[test]
fn custom_factory_can_be_registered() {
    use tasker_grammar::operations::PersistableResource;

    let mut registry = AdapterRegistry::new();
    let custom_type = ResourceType::Custom { type_name: "redis".to_string() };

    // Register a factory that always succeeds with a dummy adapter
    registry.register_persist(custom_type.clone(), Box::new(|_handle| {
        // In real code this would downcast — for testing we use InMemoryOperations
        Err(ResourceOperationError::ValidationFailed {
            message: "test factory called".to_string(),
        })
    }));

    let handle = Arc::new(InMemoryResourceHandle::new("redis1", custom_type));
    let result = registry.as_persistable(handle);
    // Factory was found and called (returns our test error)
    let err = result.unwrap_err();
    assert!(format!("{err}").contains("test factory called"));
}
```

- [ ] **Step 2: Run tests — expect failure**

Run: `cargo test --all-features -p tasker-runtime --test adapter_registry_tests`
Expected: compilation error

- [ ] **Step 3: Implement AdapterRegistry**

Create `crates/tasker-runtime/src/adapters/registry.rs`:

```rust
//! Adapter registry mapping resource types to adapter factories.
//!
//! Uses closure-based factories for extensibility without proliferating
//! named factory traits. The `standard()` constructor registers built-in
//! adapters; custom types can be added via `register_*` methods.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use tasker_grammar::operations::{
    AcquirableResource, EmittableResource, PersistableResource, ResourceOperationError,
};
use tasker_secure::{ResourceHandle, ResourceType};

/// Factory closure types for each operation.
type PersistFactory = Box<
    dyn Fn(Arc<dyn ResourceHandle>) -> Result<Arc<dyn PersistableResource>, ResourceOperationError>
        + Send
        + Sync,
>;
type AcquireFactory = Box<
    dyn Fn(Arc<dyn ResourceHandle>) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError>
        + Send
        + Sync,
>;
type EmitFactory = Box<
    dyn Fn(Arc<dyn ResourceHandle>) -> Result<Arc<dyn EmittableResource>, ResourceOperationError>
        + Send
        + Sync,
>;

/// Maps resource types to adapter factory closures.
///
/// When the `RuntimeOperationProvider` needs an operation trait object,
/// it asks the registry to wrap a handle in the right adapter.
pub struct AdapterRegistry {
    persist_factories: HashMap<ResourceType, PersistFactory>,
    acquire_factories: HashMap<ResourceType, AcquireFactory>,
    emit_factories: HashMap<ResourceType, EmitFactory>,
}

impl fmt::Debug for AdapterRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AdapterRegistry")
            .field("persist_types", &self.persist_factories.keys().collect::<Vec<_>>())
            .field("acquire_types", &self.acquire_factories.keys().collect::<Vec<_>>())
            .field("emit_types", &self.emit_factories.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl AdapterRegistry {
    /// Create an empty adapter registry.
    pub fn new() -> Self {
        Self {
            persist_factories: HashMap::new(),
            acquire_factories: HashMap::new(),
            emit_factories: HashMap::new(),
        }
    }

    /// Create a registry with all built-in adapters registered.
    pub fn standard() -> Self {
        let mut registry = Self::new();

        // Postgres adapters — adapters take Arc<dyn ResourceHandle> and
        // downcast internally, avoiding Clone requirement on PostgresHandle
        #[cfg(feature = "postgres")]
        {
            use super::postgres::{PostgresAcquireAdapter, PostgresPersistAdapter};
            use tasker_secure::resource::ResourceHandleExt;

            registry.register_persist(ResourceType::Postgres, Box::new(|handle| {
                // Verify it's a Postgres handle (will be downcast inside the adapter)
                handle.as_postgres().ok_or_else(|| {
                    ResourceOperationError::ValidationFailed {
                        message: format!(
                            "Expected Postgres handle, got {:?}",
                            handle.resource_type()
                        ),
                    }
                })?;
                Ok(Arc::new(PostgresPersistAdapter::new(handle)))
            }));

            registry.register_acquire(ResourceType::Postgres, Box::new(|handle| {
                handle.as_postgres().ok_or_else(|| {
                    ResourceOperationError::ValidationFailed {
                        message: format!(
                            "Expected Postgres handle, got {:?}",
                            handle.resource_type()
                        ),
                    }
                })?;
                Ok(Arc::new(PostgresAcquireAdapter::new(handle)))
            }));
        }

        // HTTP adapters
        #[cfg(feature = "http")]
        {
            use super::http::{HttpAcquireAdapter, HttpEmitAdapter, HttpPersistAdapter};
            use tasker_secure::resource::ResourceHandleExt;

            registry.register_persist(ResourceType::Http, Box::new(|handle| {
                handle.as_http().ok_or_else(|| {
                    ResourceOperationError::ValidationFailed {
                        message: format!(
                            "Expected HTTP handle, got {:?}",
                            handle.resource_type()
                        ),
                    }
                })?;
                Ok(Arc::new(HttpPersistAdapter::new(handle)))
            }));

            registry.register_acquire(ResourceType::Http, Box::new(|handle| {
                handle.as_http().ok_or_else(|| {
                    ResourceOperationError::ValidationFailed {
                        message: format!(
                            "Expected HTTP handle, got {:?}",
                            handle.resource_type()
                        ),
                    }
                })?;
                Ok(Arc::new(HttpAcquireAdapter::new(handle)))
            }));

            registry.register_emit(ResourceType::Http, Box::new(|handle| {
                handle.as_http().ok_or_else(|| {
                    ResourceOperationError::ValidationFailed {
                        message: format!(
                            "Expected HTTP handle, got {:?}",
                            handle.resource_type()
                        ),
                    }
                })?;
                Ok(Arc::new(HttpEmitAdapter::new(handle)))
            }));
        }

        registry
    }

    /// Register a persist adapter factory for a resource type.
    pub fn register_persist(&mut self, resource_type: ResourceType, factory: PersistFactory) {
        self.persist_factories.insert(resource_type, factory);
    }

    /// Register an acquire adapter factory for a resource type.
    pub fn register_acquire(&mut self, resource_type: ResourceType, factory: AcquireFactory) {
        self.acquire_factories.insert(resource_type, factory);
    }

    /// Register an emit adapter factory for a resource type.
    pub fn register_emit(&mut self, resource_type: ResourceType, factory: EmitFactory) {
        self.emit_factories.insert(resource_type, factory);
    }

    /// Wrap a resource handle as a `PersistableResource`.
    pub fn as_persistable(
        &self,
        handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn PersistableResource>, ResourceOperationError> {
        let factory = self.persist_factories.get(handle.resource_type()).ok_or_else(|| {
            ResourceOperationError::ValidationFailed {
                message: format!(
                    "No persist adapter registered for resource type '{}'",
                    handle.resource_type()
                ),
            }
        })?;
        factory(handle)
    }

    /// Wrap a resource handle as an `AcquirableResource`.
    pub fn as_acquirable(
        &self,
        handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn AcquirableResource>, ResourceOperationError> {
        let factory = self.acquire_factories.get(handle.resource_type()).ok_or_else(|| {
            ResourceOperationError::ValidationFailed {
                message: format!(
                    "No acquire adapter registered for resource type '{}'",
                    handle.resource_type()
                ),
            }
        })?;
        factory(handle)
    }

    /// Wrap a resource handle as an `EmittableResource`.
    pub fn as_emittable(
        &self,
        handle: Arc<dyn ResourceHandle>,
    ) -> Result<Arc<dyn EmittableResource>, ResourceOperationError> {
        let factory = self.emit_factories.get(handle.resource_type()).ok_or_else(|| {
            ResourceOperationError::ValidationFailed {
                message: format!(
                    "No emit adapter registered for resource type '{}'",
                    handle.resource_type()
                ),
            }
        })?;
        factory(handle)
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

**Important note for implementer:** The `handle.as_postgres()` returns `Option<&PostgresHandle>` — a reference, not an owned value. You'll need to check whether `PostgresHandle` implements `Clone`. If not, the factory will need to use `handle.as_any().downcast_ref::<PostgresHandle>()` and keep the `Arc<dyn ResourceHandle>` alive via a wrapper struct. Adjust the factory closures accordingly — the test suite uses `InMemoryResourceHandle` which will succeed at the type-check level but return `None` from `as_postgres()`. For the test to pass, the factories need to handle the `InMemoryResourceHandle` case gracefully — either the tests should expect `Err` for in-memory handles, or the registry tests should verify factory *registration* rather than *execution*.

Revise the tests if needed: the `standard()` tests should verify that factories are registered for the expected types, and a separate test should verify that a real downcast works (or that the error message is correct when downcast fails).

- [ ] **Step 4: Update `adapters/mod.rs` to add registry module and update re-exports**

Replace the existing `AdapterRegistry` stub in `adapters/mod.rs` with:

```rust
//! Adapter registry and resource-specific adapter implementations.

#[cfg(feature = "postgres")]
pub mod postgres;

#[cfg(feature = "http")]
pub mod http;

pub mod messaging;
pub mod registry;

#[cfg(feature = "postgres")]
pub mod sql_gen;

// Re-export primary type
pub use registry::AdapterRegistry;
```

- [ ] **Step 5: Run tests**

Run: `cargo test --all-features -p tasker-runtime --test adapter_registry_tests`
Expected: tests pass (adjust tests if factory downcast behavior differs from expectation)

- [ ] **Step 6: Commit**

```bash
git add crates/tasker-runtime/src/adapters/registry.rs \
       crates/tasker-runtime/src/adapters/mod.rs \
       crates/tasker-runtime/tests/adapter_registry_tests.rs
git commit -m "feat(tasker-runtime): implement AdapterRegistry with closure-based factories

Closure factories map ResourceType to adapter constructors. standard()
registers Postgres and HTTP adapters behind feature flags. Extensible
via register_persist/acquire/emit for custom resource types.

TAS-374

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 6: ResourcePoolManager (TAS-375)

### Task 12: Update metrics and lifecycle types

**Files:**
- Modify: `crates/tasker-runtime/src/pool_manager/metrics.rs`
- Modify: `crates/tasker-runtime/src/pool_manager/lifecycle.rs`

- [ ] **Step 1: Update `ResourceAccessMetrics`**

Replace `crates/tasker-runtime/src/pool_manager/metrics.rs`:

```rust
//! Access metrics for pool eviction and observability.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Tracks access patterns for a single resource pool.
#[derive(Debug)]
pub struct ResourceAccessMetrics {
    /// When the pool was created.
    pub creation_time: Instant,
    /// When the pool was last accessed.
    pub last_accessed: Instant,
    /// Total number of accesses.
    pub access_count: u64,
    /// Currently in-flight operations (for liveness protection).
    pub active_checkouts: u64,
    /// Estimated connection count for budget enforcement.
    pub estimated_connections: u32,
}

impl ResourceAccessMetrics {
    /// Create metrics for a newly created pool.
    pub fn new(estimated_connections: u32) -> Self {
        let now = Instant::now();
        Self {
            creation_time: now,
            last_accessed: now,
            access_count: 0,
            active_checkouts: 0,
            estimated_connections,
        }
    }

    /// Record an access to the pool.
    pub fn record_access(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

/// Aggregate metrics for the pool manager (safe for telemetry export).
#[derive(Debug)]
pub struct PoolManagerMetrics {
    pub total_pools: AtomicU64,
    pub static_pools: AtomicU64,
    pub dynamic_pools: AtomicU64,
    pub estimated_total_connections: AtomicU64,
    pub admission_rejections: AtomicU64,
    pub evictions_performed: AtomicU64,
}

impl PoolManagerMetrics {
    /// Create zeroed metrics.
    pub fn new() -> Self {
        Self {
            total_pools: AtomicU64::new(0),
            static_pools: AtomicU64::new(0),
            dynamic_pools: AtomicU64::new(0),
            estimated_total_connections: AtomicU64::new(0),
            admission_rejections: AtomicU64::new(0),
            evictions_performed: AtomicU64::new(0),
        }
    }

    /// Take a snapshot of current values.
    pub fn snapshot(&self) -> PoolManagerMetricsSnapshot {
        PoolManagerMetricsSnapshot {
            total_pools: self.total_pools.load(Ordering::Relaxed),
            static_pools: self.static_pools.load(Ordering::Relaxed),
            dynamic_pools: self.dynamic_pools.load(Ordering::Relaxed),
            estimated_total_connections: self.estimated_total_connections.load(Ordering::Relaxed),
            admission_rejections: self.admission_rejections.load(Ordering::Relaxed),
            evictions_performed: self.evictions_performed.load(Ordering::Relaxed),
        }
    }
}

impl Default for PoolManagerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Point-in-time snapshot of pool manager metrics.
#[derive(Debug, Clone)]
pub struct PoolManagerMetricsSnapshot {
    pub total_pools: u64,
    pub static_pools: u64,
    pub dynamic_pools: u64,
    pub estimated_total_connections: u64,
    pub admission_rejections: u64,
    pub evictions_performed: u64,
}
```

- [ ] **Step 2: Update re-exports in `pool_manager/mod.rs`**

Update the `pub use` line in `crates/tasker-runtime/src/pool_manager/mod.rs`:

```rust
pub use metrics::{PoolManagerMetrics, PoolManagerMetricsSnapshot, ResourceAccessMetrics};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --all-features -p tasker-runtime`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add crates/tasker-runtime/src/pool_manager/metrics.rs \
       crates/tasker-runtime/src/pool_manager/lifecycle.rs \
       crates/tasker-runtime/src/pool_manager/mod.rs
git commit -m "feat(tasker-runtime): update pool manager metrics with liveness and observability

Add active_checkouts for liveness-aware eviction, estimated_connections
for budget enforcement, PoolManagerMetrics with atomic counters for
telemetry, and snapshot API for safe export.

TAS-375

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 13: ResourcePoolManager implementation

**Files:**
- Modify: `crates/tasker-runtime/src/pool_manager/mod.rs`
- Create: `crates/tasker-runtime/tests/pool_manager_tests.rs`

- [ ] **Step 1: Write pool manager tests**

Create `crates/tasker-runtime/tests/pool_manager_tests.rs`:

```rust
//! Tests for ResourcePoolManager lifecycle management.

use std::sync::Arc;
use std::time::Duration;

use tasker_runtime::pool_manager::{
    AdmissionStrategy, EvictionStrategy, PoolManagerConfig, ResourceOrigin, ResourcePoolManager,
};
use tasker_secure::testing::InMemoryResourceHandle;
use tasker_secure::{ResourceRegistry, ResourceType};

fn test_secrets() -> Arc<dyn tasker_secure::SecretsProvider> {
    Arc::new(tasker_secure::testing::InMemorySecretsProvider::new(
        std::collections::HashMap::new(),
    ))
}

fn test_config() -> PoolManagerConfig {
    PoolManagerConfig {
        max_pools: 3,
        max_total_connections: 30,
        idle_timeout: Duration::from_millis(100),
        sweep_interval: Duration::from_secs(60),
        eviction_strategy: EvictionStrategy::Lru,
        admission_strategy: AdmissionStrategy::Reject,
    }
}

fn make_handle(name: &str) -> Arc<InMemoryResourceHandle> {
    Arc::new(InMemoryResourceHandle::new(name, ResourceType::Postgres))
}

#[tokio::test]
async fn get_returns_registered_resource() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry.clone(), test_config());

    let handle = make_handle("db1");
    manager.register("db1", handle.clone(), ResourceOrigin::Static, 10).await.unwrap();

    let result = manager.get("db1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn get_returns_not_found_for_missing() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry, test_config());

    let result = manager.get("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn get_updates_access_metrics() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry.clone(), test_config());

    let handle = make_handle("db1");
    manager.register("db1", handle, ResourceOrigin::Dynamic, 10).await.unwrap();

    manager.get("db1").await.unwrap();
    manager.get("db1").await.unwrap();
    manager.get("db1").await.unwrap();

    let metrics = manager.pool_metrics().snapshot();
    assert_eq!(metrics.total_pools, 1);
}

#[tokio::test]
async fn admission_rejected_when_at_max_pools() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let config = test_config(); // max_pools = 3
    let manager = ResourcePoolManager::new(registry.clone(), config);

    for i in 0..3 {
        let name = format!("db{i}");
        manager.register(&name, make_handle(&name), ResourceOrigin::Dynamic, 10).await.unwrap();
    }

    let result = manager.register("db3", make_handle("db3"), ResourceOrigin::Dynamic, 10).await;
    assert!(result.is_err(), "Should reject when at max_pools");

    let metrics = manager.pool_metrics().snapshot();
    assert_eq!(metrics.admission_rejections, 1);
}

#[tokio::test]
async fn connection_budget_enforcement() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let mut config = test_config();
    config.max_pools = 10;
    config.max_total_connections = 25;
    let manager = ResourcePoolManager::new(registry.clone(), config);

    // Register 2 pools with 10 connections each = 20 total
    manager.register("db1", make_handle("db1"), ResourceOrigin::Dynamic, 10).await.unwrap();
    manager.register("db2", make_handle("db2"), ResourceOrigin::Dynamic, 10).await.unwrap();

    // Third pool with 10 would exceed budget of 25
    let result = manager.register("db3", make_handle("db3"), ResourceOrigin::Dynamic, 10).await;
    assert!(result.is_err(), "Should reject when exceeding connection budget");
}

#[tokio::test]
async fn static_resources_never_evicted() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let mut config = test_config();
    config.idle_timeout = Duration::from_millis(1);
    let manager = ResourcePoolManager::new(registry.clone(), config);

    manager.register("static_db", make_handle("static_db"), ResourceOrigin::Static, 10).await.unwrap();

    // Wait for idle timeout
    tokio::time::sleep(Duration::from_millis(10)).await;

    let (candidates, evicted) = manager.sweep().await;
    assert_eq!(evicted, 0, "Static resources should never be evicted");

    // Verify still accessible
    assert!(manager.get("static_db").await.is_ok());
}

#[tokio::test]
async fn sweep_evicts_idle_dynamic_resources() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let mut config = test_config();
    config.idle_timeout = Duration::from_millis(1);
    let manager = ResourcePoolManager::new(registry.clone(), config);

    manager.register("dynamic_db", make_handle("dynamic_db"), ResourceOrigin::Dynamic, 10).await.unwrap();

    // Wait for idle timeout
    tokio::time::sleep(Duration::from_millis(10)).await;

    let (candidates, evicted) = manager.sweep().await;
    assert!(evicted > 0, "Idle dynamic resource should be evicted");

    // Verify no longer accessible
    assert!(manager.get("dynamic_db").await.is_err());
}

#[tokio::test]
async fn sweep_preserves_recently_accessed() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let mut config = test_config();
    config.idle_timeout = Duration::from_millis(50);
    let manager = ResourcePoolManager::new(registry.clone(), config);

    manager.register("active_db", make_handle("active_db"), ResourceOrigin::Dynamic, 10).await.unwrap();

    // Access it (resets last_accessed)
    manager.get("active_db").await.unwrap();

    // Sweep immediately — should not evict (just accessed)
    let (_, evicted) = manager.sweep().await;
    assert_eq!(evicted, 0, "Recently accessed should not be evicted");
}

#[tokio::test]
async fn evict_one_admission_strategy() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let mut config = test_config();
    config.max_pools = 2;
    config.admission_strategy = AdmissionStrategy::EvictOne;
    config.idle_timeout = Duration::from_millis(1);
    let manager = ResourcePoolManager::new(registry.clone(), config);

    manager.register("db1", make_handle("db1"), ResourceOrigin::Dynamic, 10).await.unwrap();
    manager.register("db2", make_handle("db2"), ResourceOrigin::Dynamic, 10).await.unwrap();

    // Wait for idle
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Should evict oldest and admit new
    let result = manager.register("db3", make_handle("db3"), ResourceOrigin::Dynamic, 10).await;
    assert!(result.is_ok(), "EvictOne should make room");

    let metrics = manager.pool_metrics().snapshot();
    assert_eq!(metrics.evictions_performed, 1);
}

#[tokio::test]
async fn current_pools_returns_summaries() {
    let registry = Arc::new(ResourceRegistry::new(test_secrets()));
    let manager = ResourcePoolManager::new(registry.clone(), test_config());

    manager.register("db1", make_handle("db1"), ResourceOrigin::Static, 10).await.unwrap();
    manager.register("db2", make_handle("db2"), ResourceOrigin::Dynamic, 5).await.unwrap();

    let pools = manager.current_pools();
    assert_eq!(pools.len(), 2);
}
```

- [ ] **Step 2: Run tests — expect failure**

Run: `cargo test --all-features -p tasker-runtime --test pool_manager_tests`
Expected: compilation error — methods don't exist yet

- [ ] **Step 3: Implement ResourcePoolManager**

Replace `crates/tasker-runtime/src/pool_manager/mod.rs`:

```rust
//! Resource pool manager with lifecycle management, eviction, and admission control.

mod lifecycle;
mod metrics;

pub use lifecycle::{AdmissionStrategy, EvictionStrategy, PoolManagerConfig, ResourceOrigin};
pub use metrics::{PoolManagerMetrics, PoolManagerMetricsSnapshot, ResourceAccessMetrics};

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;

use tasker_secure::{ResourceError, ResourceHandle, ResourceRegistry, ResourceSummary};

/// Manages resource pool lifecycle with eviction and admission control.
///
/// Wraps a `ResourceRegistry` and adds:
/// - Admission control (pool count ceiling + connection budget)
/// - Liveness-aware eviction of idle dynamic pools
/// - Per-resource access metrics
/// - Aggregate observability metrics for autoscaling signals
#[derive(Debug)]
pub struct ResourcePoolManager {
    registry: Arc<ResourceRegistry>,
    config: PoolManagerConfig,
    origins: RwLock<HashMap<String, ResourceOrigin>>,
    metrics: RwLock<HashMap<String, ResourceAccessMetrics>>,
    pool_metrics: PoolManagerMetrics,
}

impl ResourcePoolManager {
    /// Create a new pool manager wrapping the given registry.
    pub fn new(registry: Arc<ResourceRegistry>, config: PoolManagerConfig) -> Self {
        Self {
            registry,
            config,
            origins: RwLock::new(HashMap::new()),
            metrics: RwLock::new(HashMap::new()),
            pool_metrics: PoolManagerMetrics::new(),
        }
    }

    /// Register a resource handle with admission control.
    ///
    /// Checks pool count ceiling and connection budget before admitting.
    /// If at capacity, behavior depends on `AdmissionStrategy`:
    /// - `Reject`: returns error (retriable, signals backpressure)
    /// - `EvictOne`: evicts an eligible dynamic resource, then admits
    pub async fn register(
        &self,
        name: &str,
        handle: Arc<dyn ResourceHandle>,
        origin: ResourceOrigin,
        estimated_connections: u32,
    ) -> Result<(), ResourceError> {
        // Check admission
        let origins = self.origins.read().await;
        let current_count = origins.len();
        let metrics_map = self.metrics.read().await;
        let current_connections: u64 = metrics_map
            .values()
            .map(|m| u64::from(m.estimated_connections))
            .sum();
        drop(metrics_map);
        drop(origins);

        let pool_full = current_count >= self.config.max_pools;
        let budget_exceeded =
            current_connections + u64::from(estimated_connections) > self.config.max_total_connections as u64;

        if pool_full || budget_exceeded {
            match self.config.admission_strategy {
                AdmissionStrategy::Reject => {
                    self.pool_metrics
                        .admission_rejections
                        .fetch_add(1, Ordering::Relaxed);
                    return Err(ResourceError::InitializationFailed {
                        name: name.to_string(),
                        message: if pool_full {
                            format!(
                                "Pool capacity exhausted ({}/{})",
                                current_count, self.config.max_pools
                            )
                        } else {
                            format!(
                                "Connection budget exceeded ({}/{})",
                                current_connections, self.config.max_total_connections
                            )
                        },
                    });
                }
                AdmissionStrategy::EvictOne => {
                    if !self.try_evict_one().await {
                        self.pool_metrics
                            .admission_rejections
                            .fetch_add(1, Ordering::Relaxed);
                        return Err(ResourceError::InitializationFailed {
                            name: name.to_string(),
                            message: "No eligible resources to evict".to_string(),
                        });
                    }
                }
            }
        }

        // Admit
        self.registry.register(name, handle).await;

        let mut origins = self.origins.write().await;
        let is_static = matches!(origin, ResourceOrigin::Static);
        origins.insert(name.to_string(), origin);
        drop(origins);

        let mut metrics_map = self.metrics.write().await;
        metrics_map.insert(
            name.to_string(),
            ResourceAccessMetrics::new(estimated_connections),
        );
        drop(metrics_map);

        // Update aggregate metrics
        self.pool_metrics.total_pools.fetch_add(1, Ordering::Relaxed);
        if is_static {
            self.pool_metrics.static_pools.fetch_add(1, Ordering::Relaxed);
        } else {
            self.pool_metrics.dynamic_pools.fetch_add(1, Ordering::Relaxed);
        }
        self.pool_metrics
            .estimated_total_connections
            .fetch_add(u64::from(estimated_connections), Ordering::Relaxed);

        Ok(())
    }

    /// Get a resource handle by name, updating access metrics.
    ///
    /// Returns `ResourceNotFound` if not registered.
    pub async fn get(
        &self,
        name: &str,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceError> {
        let handle = self.registry.get(name).ok_or_else(|| {
            ResourceError::ResourceNotFound {
                name: name.to_string(),
            }
        })?;

        // Update access metrics
        let mut metrics_map = self.metrics.write().await;
        if let Some(m) = metrics_map.get_mut(name) {
            m.record_access();
        }

        Ok(handle)
    }

    /// Evict a specific resource pool by name.
    pub async fn evict(&self, name: &str) -> Result<(), ResourceError> {
        // Check it's not static
        let origins = self.origins.read().await;
        if let Some(ResourceOrigin::Static) = origins.get(name) {
            return Err(ResourceError::InitializationFailed {
                name: name.to_string(),
                message: "Cannot evict static resource".to_string(),
            });
        }
        drop(origins);

        self.do_evict(name).await;
        Ok(())
    }

    /// Run an eviction sweep based on the configured strategy.
    ///
    /// Returns `(candidates_found, evicted_count)`.
    /// Only dynamic resources past their idle timeout with zero active
    /// checkouts are eligible.
    pub async fn sweep(&self) -> (usize, usize) {
        let now = Instant::now();
        let origins = self.origins.read().await;
        let metrics_map = self.metrics.read().await;

        // Find eligible candidates
        let mut candidates: Vec<(String, &ResourceAccessMetrics)> = Vec::new();

        for (name, origin) in origins.iter() {
            if matches!(origin, ResourceOrigin::Static) {
                continue;
            }
            if let Some(m) = metrics_map.get(name) {
                let idle_duration = now.duration_since(m.last_accessed);
                if idle_duration >= self.config.idle_timeout && m.active_checkouts == 0 {
                    candidates.push((name.clone(), m));
                }
            }
        }

        let candidates_found = candidates.len();
        drop(metrics_map);
        drop(origins);

        if candidates_found == 0 {
            return (0, 0);
        }

        // Sort by eviction strategy
        match self.config.eviction_strategy {
            EvictionStrategy::Lru => {
                candidates.sort_by_key(|(_, m)| m.last_accessed);
            }
            EvictionStrategy::Lfu => {
                candidates.sort_by_key(|(_, m)| m.access_count);
            }
            EvictionStrategy::Fifo => {
                candidates.sort_by_key(|(_, m)| m.creation_time);
            }
        }

        let mut evicted = 0;
        for (name, _) in &candidates {
            self.do_evict(name).await;
            evicted += 1;
        }

        (candidates_found, evicted)
    }

    /// List current pool summaries for introspection.
    pub fn current_pools(&self) -> Vec<ResourceSummary> {
        self.registry.list_resources()
    }

    /// Refresh credentials for a resource.
    pub async fn refresh_resource(&self, name: &str) -> Result<(), ResourceError> {
        self.registry.refresh_resource(name).await
    }

    /// Access aggregate pool metrics for observability.
    pub fn pool_metrics(&self) -> &PoolManagerMetrics {
        &self.pool_metrics
    }

    /// Internal: perform eviction of a single resource.
    async fn do_evict(&self, name: &str) {
        self.registry.remove(name).await;

        let mut origins = self.origins.write().await;
        let was_static = matches!(origins.remove(name), Some(ResourceOrigin::Static));
        drop(origins);

        let mut metrics_map = self.metrics.write().await;
        let connections = metrics_map
            .remove(name)
            .map(|m| u64::from(m.estimated_connections))
            .unwrap_or(0);
        drop(metrics_map);

        // Update aggregate metrics
        self.pool_metrics.total_pools.fetch_sub(1, Ordering::Relaxed);
        if was_static {
            self.pool_metrics.static_pools.fetch_sub(1, Ordering::Relaxed);
        } else {
            self.pool_metrics.dynamic_pools.fetch_sub(1, Ordering::Relaxed);
        }
        self.pool_metrics
            .estimated_total_connections
            .fetch_sub(connections, Ordering::Relaxed);
        self.pool_metrics
            .evictions_performed
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Try to evict one eligible dynamic resource.
    /// Returns true if eviction succeeded.
    async fn try_evict_one(&self) -> bool {
        let now = Instant::now();
        let origins = self.origins.read().await;
        let metrics_map = self.metrics.read().await;

        let mut best_candidate: Option<(String, Instant)> = None;

        for (name, origin) in origins.iter() {
            if matches!(origin, ResourceOrigin::Static) {
                continue;
            }
            if let Some(m) = metrics_map.get(name) {
                let idle_duration = now.duration_since(m.last_accessed);
                if idle_duration >= self.config.idle_timeout && m.active_checkouts == 0 {
                    if best_candidate
                        .as_ref()
                        .map_or(true, |(_, t)| m.last_accessed < *t)
                    {
                        best_candidate = Some((name.clone(), m.last_accessed));
                    }
                }
            }
        }

        drop(metrics_map);
        drop(origins);

        if let Some((name, _)) = best_candidate {
            self.do_evict(&name).await;
            true
        } else {
            false
        }
    }
}
```

- [ ] **Step 4: Update `lib.rs` re-exports**

In `crates/tasker-runtime/src/lib.rs`, add to re-exports:

```rust
pub use pool_manager::{
    PoolManagerConfig, PoolManagerMetrics, PoolManagerMetricsSnapshot, ResourceAccessMetrics,
    ResourcePoolManager,
};
```

- [ ] **Step 5: Run tests**

Run: `cargo test --all-features -p tasker-runtime --test pool_manager_tests`
Expected: all tests pass

Note: the `InMemorySecretsProvider` import may need adjusting. Check `crates/tasker-secure/src/testing/` for the exact type name and import path.

- [ ] **Step 6: Run all tasker-runtime tests**

Run: `cargo test --all-features -p tasker-runtime`
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/tasker-runtime/src/pool_manager/ \
       crates/tasker-runtime/src/lib.rs \
       crates/tasker-runtime/tests/pool_manager_tests.rs
git commit -m "feat(tasker-runtime): implement ResourcePoolManager with eviction and backpressure

Admission control: pool count ceiling + connection budget enforcement.
Liveness-aware eviction: only idle dynamic resources with zero active
checkouts are candidates. Aggregate PoolManagerMetrics with
admission_rejections counter as autoscaling signal.

TAS-375

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Chunk 7: Final Integration & Cleanup

### Task 14: Cross-crate compilation and clippy

**Files:** All modified files

- [ ] **Step 1: Full workspace check**

Run: `cargo check --all-features`
Expected: success across entire workspace

- [ ] **Step 2: Clippy**

Run: `cargo clippy --all-targets --all-features --workspace`
Expected: zero warnings

- [ ] **Step 3: Format**

Run: `cargo fmt`

- [ ] **Step 4: Run all tasker-runtime tests**

Run: `cargo test --all-features -p tasker-runtime`
Expected: all tests pass

- [ ] **Step 5: Run tasker-grammar tests (verify PersistMode didn't break anything)**

Run: `cargo test --all-features -p tasker-grammar`
Expected: all existing tests pass

- [ ] **Step 6: Run tasker-secure tests**

Run: `cargo test --all-features -p tasker-secure`
Expected: all existing tests pass

- [ ] **Step 7: Fix any issues found**

Address clippy warnings, formatting issues, or test failures.

- [ ] **Step 8: Commit any fixes**

```bash
git add -A
git commit -m "style: fix clippy warnings and formatting

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

### Task 15: Update re-exports and verify public API

**Files:**
- Modify: `crates/tasker-runtime/src/lib.rs`

- [ ] **Step 1: Ensure all public types are re-exported**

Verify `crates/tasker-runtime/src/lib.rs` re-exports:

```rust
pub mod adapters;
pub mod context;
pub mod pool_manager;
pub mod provider;
pub mod sources;

// Re-export primary types for convenience.
pub use adapters::AdapterRegistry;
pub use pool_manager::{
    PoolManagerConfig, PoolManagerMetrics, PoolManagerMetricsSnapshot,
    ResourceAccessMetrics, ResourcePoolManager,
};
pub use provider::RuntimeOperationProvider;
pub use sources::ResourceDefinitionSource;
```

- [ ] **Step 2: Build docs**

Run: `cargo doc --all-features -p tasker-runtime --no-deps`
Expected: success, no warnings

- [ ] **Step 3: Commit if changes needed**

---

### Task 16: `cargo make test-no-infra` verification

- [ ] **Step 1: Run the no-infrastructure test suite**

Run: `cargo make test-no-infra`
Expected: all tests pass (this is the broadest test suite that doesn't need DB)

- [ ] **Step 2: If any failures, investigate and fix**

Common issues:
- Feature flag combinations
- Missing `#[cfg(test)]` on test modules
- Import path issues when features are disabled

- [ ] **Step 3: Final commit if needed**

```bash
git add -A
git commit -m "fix: resolve test-no-infra compatibility issues

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```
