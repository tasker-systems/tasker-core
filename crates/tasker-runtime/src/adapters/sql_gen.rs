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

/// Output of a SQL generation function.
#[derive(Debug, Clone)]
pub struct SqlOutput {
    /// The generated SQL string with $N placeholders.
    pub sql: String,
    /// Column names in bind order — caller binds values in this order.
    pub bind_columns: Vec<String>,
}

/// Build an INSERT statement.
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
    let mut output = build_insert(entity, columns, constraints)?;
    let mut conflict_cols = Vec::with_capacity(upsert_keys.len());
    for key in upsert_keys {
        conflict_cols.push(safe_identifier(key)?);
    }
    let base_sql = output.sql.trim_end_matches(" RETURNING *");
    let conflict_strategy = constraints
        .on_conflict
        .as_ref()
        .unwrap_or(&tasker_grammar::operations::ConflictStrategy::Reject);
    let sql = match conflict_strategy {
        tasker_grammar::operations::ConflictStrategy::Reject => {
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
pub fn build_select(
    entity: &str,
    columns: &[String],
    params: &serde_json::Value,
    constraints: &tasker_grammar::operations::AcquireConstraints,
) -> Result<SqlOutput, ResourceOperationError> {
    validate_identifier(entity)?;
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
    if let Some(obj) = params.as_object() {
        if !obj.is_empty() {
            let mut where_parts = Vec::new();
            let mut bind_pos = 1;
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
    if let Some(limit) = constraints.limit {
        sql.push_str(&format!(" LIMIT {limit}"));
    }
    if let Some(offset) = constraints.offset {
        sql.push_str(&format!(" OFFSET {offset}"));
    }
    Ok(SqlOutput { sql, bind_columns })
}
