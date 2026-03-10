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

// ---------------------------------------------------------------------------
// PostgresPersistAdapter
// ---------------------------------------------------------------------------

/// Adapts a [`PostgresHandle`] for structured write operations.
///
/// Stores the handle as `Arc<dyn ResourceHandle>` and downcasts on use,
/// which lets the [`AdapterRegistry`](super::AdapterRegistry) wrap any
/// handle without knowing its concrete type at registration time.
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
        self.handle
            .as_postgres()
            .ok_or_else(|| ResourceOperationError::ValidationFailed {
                message: format!(
                    "Expected Postgres handle, got {:?}",
                    self.handle.resource_type()
                ),
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
        let obj = data
            .as_object()
            .ok_or_else(|| ResourceOperationError::ValidationFailed {
                message: "persist data must be a JSON object".to_string(),
            })?;

        let columns: Vec<String> = obj.keys().cloned().collect();

        // Generate SQL based on the requested mode.
        let output = match constraints.mode {
            PersistMode::Insert => sql_gen::build_insert(entity, &columns, constraints)?,
            PersistMode::Update => {
                let identity_keys = constraints.identity_keys.as_deref().unwrap_or_default();
                if identity_keys.is_empty() {
                    return Err(ResourceOperationError::ValidationFailed {
                        message: "UPDATE requires identity_keys in constraints".to_string(),
                    });
                }
                let id_set: std::collections::HashSet<&str> =
                    identity_keys.iter().map(|s| s.as_str()).collect();
                let set_columns: Vec<String> = columns
                    .iter()
                    .filter(|c| !id_set.contains(c.as_str()))
                    .cloned()
                    .collect();
                let id_keys: Vec<String> = identity_keys.iter().map(|s| s.to_string()).collect();
                sql_gen::build_update(entity, &set_columns, &id_keys)?
            }
            PersistMode::Upsert => sql_gen::build_upsert(entity, &columns, constraints)?,
            PersistMode::Delete => {
                let identity_keys = constraints.identity_keys.as_deref().unwrap_or_default();
                if identity_keys.is_empty() {
                    return Err(ResourceOperationError::ValidationFailed {
                        message: "DELETE requires identity_keys in constraints".to_string(),
                    });
                }
                let id_keys: Vec<String> = identity_keys.iter().map(|s| s.to_string()).collect();
                sql_gen::build_delete(entity, &id_keys)?
            }
        };

        // Build a dynamic query and bind values in generation order.
        let mut query = sqlx::query(&output.sql);
        for col_name in &output.bind_columns {
            let value = obj.get(col_name).unwrap_or(&serde_json::Value::Null);
            query = bind_json_value(query, value.clone());
        }

        // Execute — RETURNING * gives us the row back.
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
            Some(ref r) => row_to_json(r)?,
            None => serde_json::json!({}),
        };

        Ok(PersistResult {
            data,
            affected_count: Some(1),
        })
    }
}

// ---------------------------------------------------------------------------
// PostgresAcquireAdapter
// ---------------------------------------------------------------------------

/// Adapts a [`PostgresHandle`] for structured read operations (SELECT).
#[derive(Debug)]
pub struct PostgresAcquireAdapter {
    handle: Arc<dyn tasker_secure::ResourceHandle>,
}

impl PostgresAcquireAdapter {
    /// Create a new acquire adapter wrapping the given handle.
    pub fn new(handle: Arc<dyn tasker_secure::ResourceHandle>) -> Self {
        Self { handle }
    }

    fn pg_handle(&self) -> Result<&PostgresHandle, ResourceOperationError> {
        use tasker_secure::resource::ResourceHandleExt;
        self.handle
            .as_postgres()
            .ok_or_else(|| ResourceOperationError::ValidationFailed {
                message: format!(
                    "Expected Postgres handle, got {:?}",
                    self.handle.resource_type()
                ),
            })
    }
}

#[async_trait]
impl AcquirableResource for PostgresAcquireAdapter {
    async fn acquire(
        &self,
        entity: &str,
        params: serde_json::Value,
        constraints: &AcquireConstraints,
    ) -> Result<AcquireResult, ResourceOperationError> {
        // Extract column list from a special `_columns` key.
        let columns: Vec<String> = params
            .get("_columns")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Everything except underscore-prefixed keys becomes a WHERE filter.
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

        let mut query = sqlx::query(&output.sql);
        // Bind filter values in sorted-key order (matching sql_gen).
        if let Some(obj) = filter_params.as_object() {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            for key in keys {
                let value = &obj[key];
                query = bind_json_value(query, value.clone());
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

        let data: Vec<serde_json::Value> = rows
            .iter()
            .map(row_to_json)
            .collect::<Result<Vec<_>, _>>()?;

        let total_count = Some(data.len() as u64);

        Ok(AcquireResult {
            data: serde_json::Value::Array(data),
            total_count,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Bind a [`serde_json::Value`] to a sqlx query as the appropriate PostgreSQL type.
///
/// Takes ownership of the value so lifetimes stay simple.
fn bind_json_value(
    query: sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments>,
    value: serde_json::Value,
) -> sqlx::query::Query<'_, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match value {
        serde_json::Value::Null => query.bind(None::<String>),
        serde_json::Value::Bool(b) => query.bind(b),
        serde_json::Value::Number(ref n) => {
            if let Some(i) = n.as_i64() {
                query.bind(i)
            } else if let Some(f) = n.as_f64() {
                query.bind(f)
            } else {
                query.bind(n.to_string())
            }
        }
        serde_json::Value::String(s) => query.bind(s),
        // Arrays and objects bind as JSONB.
        other => query.bind(other),
    }
}

/// Convert a sqlx [`PgRow`](sqlx::postgres::PgRow) to a JSON object using column metadata.
fn row_to_json(row: &sqlx::postgres::PgRow) -> Result<serde_json::Value, ResourceOperationError> {
    use sqlx::Column;
    use sqlx::TypeInfo;
    use sqlx::ValueRef;

    let mut obj = serde_json::Map::new();
    for col in row.columns() {
        let name = col.name().to_string();
        let value = if row
            .try_get_raw(col.ordinal())
            .map(|v| v.is_null())
            .unwrap_or(true)
        {
            serde_json::Value::Null
        } else {
            let type_name = col.type_info().name();
            match type_name {
                "BOOL" => {
                    let v: bool =
                        row.try_get(col.ordinal())
                            .map_err(|e| ResourceOperationError::Other {
                                message: format!("Failed to read column '{name}': {e}"),
                                source: Some(Box::new(e)),
                            })?;
                    serde_json::Value::Bool(v)
                }
                "INT2" | "INT4" => {
                    let v: i32 =
                        row.try_get(col.ordinal())
                            .map_err(|e| ResourceOperationError::Other {
                                message: format!("Failed to read column '{name}': {e}"),
                                source: Some(Box::new(e)),
                            })?;
                    serde_json::json!(v)
                }
                "INT8" => {
                    let v: i64 =
                        row.try_get(col.ordinal())
                            .map_err(|e| ResourceOperationError::Other {
                                message: format!("Failed to read column '{name}': {e}"),
                                source: Some(Box::new(e)),
                            })?;
                    serde_json::json!(v)
                }
                "FLOAT4" | "FLOAT8" | "NUMERIC" => {
                    let v: f64 =
                        row.try_get(col.ordinal())
                            .map_err(|e| ResourceOperationError::Other {
                                message: format!("Failed to read column '{name}': {e}"),
                                source: Some(Box::new(e)),
                            })?;
                    serde_json::json!(v)
                }
                "JSON" | "JSONB" => {
                    let v: serde_json::Value =
                        row.try_get(col.ordinal())
                            .map_err(|e| ResourceOperationError::Other {
                                message: format!("Failed to read column '{name}': {e}"),
                                source: Some(Box::new(e)),
                            })?;
                    v
                }
                _ => {
                    // Fall back to String for TEXT, VARCHAR, UUID, TIMESTAMP, etc.
                    let v: String =
                        row.try_get(col.ordinal())
                            .map_err(|e| ResourceOperationError::Other {
                                message: format!("Failed to read column '{name}': {e}"),
                                source: Some(Box::new(e)),
                            })?;
                    serde_json::Value::String(v)
                }
            }
        };
        obj.insert(name, value);
    }
    Ok(serde_json::Value::Object(obj))
}
