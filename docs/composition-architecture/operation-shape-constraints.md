# Operation Shape Constraints

*Design constraints for persist and acquire operations in the grammar capability system*

*March 2026*

---

## Guiding Principle

The grammar system's power lives in **composition and transformation**, not in I/O complexity. The `persist` and `acquire` operations are intentionally shape-constrained: they express predictable, structured interactions with resources. The `transform` capability (jaq-core) handles reshaping, filtering, aggregation, and computation on the results.

This is a deliberate architectural choice:

- **I/O operations are dumb and predictable.** An adapter can generate SQL or HTTP requests from structured declarations without parsing or interpreting user-authored query logic.
- **Analytical work happens in a sandboxed expression engine.** jaq-core is safe by construction — no I/O, execution timeouts, output size limits.
- **Complex multi-stage work uses composition.** Batch processes chain multiple acquire → transform steps. The grammar's value is making these compositions declarative, not making individual operations more powerful.

The grammar is meant to make it possible to do **predictable-shape things**, not to execute arbitrary or questionable-provenance operations.

---

## persist: Structured Write Operations

### Operation Modes

`persist` supports four distinct modes, each with clear semantics:

| Mode | SQL Equivalent | HTTP Equivalent | Semantics |
|------|---------------|-----------------|-----------|
| `insert` | INSERT | POST | Create new record(s). Fail on conflict. |
| `update` | UPDATE ... WHERE | PUT / PATCH | Modify existing record(s). Fail if not found (configurable). |
| `upsert` | INSERT ... ON CONFLICT DO UPDATE | PUT | Create or update. Requires conflict key declaration. |
| `delete` | DELETE ... WHERE | DELETE | Remove record(s). Requires explicit target identification. |

The mode is declared in the capability config, not inferred:

```yaml
- capability: persist
  config:
    resource:
      ref: "orders-db"
      entity: orders
    mode: upsert
    data:
      expression: "{id: .prev.order_id, total: .prev.computed_total, status: \"confirmed\"}"
    identity:
      primary_key: ["id"]
    constraints:
      on_conflict: update    # update | skip | reject
```

### Data Shape Declarations

The data arriving at `persist` is always a JSON object (or array of objects for batch). The adapter translates this directly to columns and values:

```json
{
  "id": 123,
  "customer_name": "Acme Corp",
  "total": 45.67,
  "status": "confirmed"
}
```

→ Adapter generates: `INSERT INTO orders (id, customer_name, total, status) VALUES ($1, $2, $3, $4)`

**No column mapping or renaming in the adapter.** If the JSON keys don't match the column names, that's a `transform` step's job before the persist. The adapter is a transparent translation layer.

### Identity Declarations

Every persist operation that targets existing records must declare how records are identified:

```yaml
identity:
  primary_key: ["id"]                    # Single PK
  primary_key: ["order_id", "line_num"]  # Composite PK
```

- **insert**: PK declaration is optional (database may auto-generate)
- **update**: PK declaration required — used in WHERE clause
- **upsert**: PK declaration required — used in ON CONFLICT target
- **delete**: PK declaration required — used in WHERE clause

### Nested Relationships

For related table operations, the data shape declares the relationship structure:

```yaml
- capability: persist
  config:
    resource:
      ref: "orders-db"
      entity: orders
    mode: insert
    data:
      expression: |
        {
          id: .prev.order_id,
          customer_id: .prev.customer_id,
          total: .prev.computed_total,
          line_items: [.prev.items[] | {
            order_id: .prev.order_id,
            product_id: .product_id,
            quantity: .quantity,
            price: .price
          }]
        }
    identity:
      primary_key: ["id"]
    relationships:
      line_items:
        entity: order_line_items
        foreign_key: ["order_id"]
        references: ["id"]
        mode: insert
```

The adapter sees:
1. A parent object with scalar fields → INSERT into `orders`
2. A nested array `line_items` with a declared relationship → INSERT each into `order_line_items` with the FK value from the parent

**Relationship operations are always one level of nesting.** If you need deeper nesting (grandchildren), use separate persist steps in the composition. This keeps the adapter's SQL generation simple and predictable.

### HTTP Persist Mapping

For HTTP resources, the mapping is straightforward:

| Mode | HTTP Method | URL Pattern | Body |
|------|------------|-------------|------|
| `insert` | POST | `{base_url}/{entity}` | JSON body |
| `update` | PATCH | `{base_url}/{entity}/{pk_value}` | JSON body (partial) |
| `upsert` | PUT | `{base_url}/{entity}/{pk_value}` | JSON body (full) |
| `delete` | DELETE | `{base_url}/{entity}/{pk_value}` | None or JSON body |

PK values are extracted from the data object using the `identity.primary_key` declaration.

### What persist Does NOT Do

- **No raw SQL.** The adapter generates SQL from structured declarations.
- **No arbitrary WHERE clauses.** Updates and deletes target records by declared identity (PK).
- **No multi-table transactions.** Each persist call (including nested relationships) is a single logical operation. Multi-step atomicity is a future concern (see [resource-handle-traits-and-seams.md §Open Questions](../research/resource-handle-traits-and-seams.md)).
- **No stored procedure calls.** If you need a stored procedure, use a domain handler.
- **No DDL.** persist operates on data, not schema.

---

## acquire: Declarative Read Operations

### Column and Table Selection

`acquire` declares what to read using structured column and table specifications, not SQL:

```yaml
- capability: acquire
  config:
    resource:
      ref: "orders-db"
      entity: orders
    select:
      columns: ["id", "customer_id", "total", "status", "created_at"]
    filter:
      status:
        eq: "pending"
      created_at:
        gte: "2026-01-01"
    params:
      expression: "{customer_id: .context.customer_id}"
    constraints:
      limit: 100
      offset: 0
      order_by: ["created_at:desc"]
```

The adapter generates: `SELECT id, customer_id, total, status, created_at FROM orders WHERE status = $1 AND created_at >= $2 AND customer_id = $3 ORDER BY created_at DESC LIMIT 100 OFFSET 0`

### Filter Operators

A fixed set of declarative filter operators — no arbitrary expressions:

| Operator | SQL | Example |
|----------|-----|---------|
| `eq` | `=` | `status: { eq: "pending" }` |
| `neq` | `!=` | `status: { neq: "cancelled" }` |
| `gt` | `>` | `total: { gt: 100 }` |
| `gte` | `>=` | `created_at: { gte: "2026-01-01" }` |
| `lt` | `<` | `total: { lt: 1000 }` |
| `lte` | `<=` | `priority: { lte: 3 }` |
| `in` | `IN (...)` | `status: { in: ["pending", "processing"] }` |
| `not_in` | `NOT IN (...)` | `status: { not_in: ["cancelled", "refunded"] }` |
| `is_null` | `IS NULL` | `deleted_at: { is_null: true }` |
| `like` | `LIKE` | `name: { like: "%acme%" }` |

Filters declared in `filter` are static (part of the capability config). Dynamic values come through `params.expression` — a jaq expression evaluated against the composition envelope that produces a JSON object of key-value pairs merged into the WHERE clause.

### Joined Queries via Relationship Declarations

For queries spanning related tables, relationships are declared structurally:

```yaml
- capability: acquire
  config:
    resource:
      ref: "orders-db"
      entity: orders
    select:
      columns: ["id", "total", "status"]
      include:
        customer:
          entity: customers
          foreign_key: ["customer_id"]
          references: ["id"]
          columns: ["name", "email"]
        line_items:
          entity: order_line_items
          foreign_key: ["order_id"]    # FK is on the child table
          references: ["id"]           # References parent PK
          columns: ["product_id", "quantity", "price"]
    filter:
      status:
        eq: "pending"
```

The adapter has two strategies for resolving this:

**Strategy A — Separate queries (default):** Execute the parent query, then fetch related records in batch. This is predictable, avoids cartesian explosion, and the result shape is clean:

```json
[
  {
    "id": 123,
    "total": 45.67,
    "status": "pending",
    "customer": { "name": "Acme Corp", "email": "orders@acme.com" },
    "line_items": [
      { "product_id": 1, "quantity": 2, "price": 10.00 },
      { "product_id": 2, "quantity": 1, "price": 25.67 }
    ]
  }
]
```

**Strategy B — JOIN (opt-in):** For belongs-to relationships (customer above), a JOIN may be more efficient. The adapter can JOIN when the relationship is many-to-one (parent has FK pointing to related table). One-to-many relationships (line_items above) always use separate queries to avoid row multiplication.

The result is always nested JSON matching the declared structure. The subsequent `transform` step can reshape, filter, aggregate, or compute on this data using jaq-core.

### HTTP Acquire Mapping

For HTTP resources, acquire maps to GET requests:

```yaml
- capability: acquire
  config:
    resource:
      ref: "catalog-api"
      entity: "products"          # → GET {base_url}/products
    params:
      expression: "{category: .context.category}"
    filter:
      active:
        eq: true
    constraints:
      limit: 50
```

- `entity` → URL path segment
- `filter` + `params` → query string parameters
- `constraints.limit/offset` → pagination query parameters
- `constraints.timeout_ms` → request timeout

The response body (JSON) is returned directly as the acquire result. No structural assumptions about the API response shape — that's for `transform` to handle.

### What acquire Does NOT Do

- **No arbitrary SQL.** The adapter generates SELECT statements from declarations.
- **No subqueries.** If you need the result of one query as input to another, use two acquire steps in a composition.
- **No aggregation in the query.** GROUP BY, HAVING, COUNT, SUM — these happen in `transform` via jaq-core after the data is acquired. For large datasets requiring database-side aggregation, use a domain handler or a database view as the entity.
- **No CTEs or window functions.** Same reasoning — if the query shape goes beyond "select columns from tables with filters," it belongs in a domain handler or a pre-built view.
- **No writes.** acquire is read-only. No INSERT ... RETURNING used as a read mechanism.
- **No raw WHERE clauses.** All filtering is through the declarative operator set.

---

## The Composition Pattern for Complex Work

The constraints above are not limitations — they're a design that pushes complexity into the right places:

### Simple case: single acquire + transform

```yaml
steps:
  - capability: acquire
    config:
      resource: { ref: "orders-db", entity: orders }
      select: { columns: ["id", "total", "status", "created_at"] }
      filter: { status: { eq: "pending" } }

  - capability: transform
    config:
      expression: |
        .prev | group_by(.status) | map({
          status: .[0].status,
          count: length,
          total_value: map(.total) | add
        })
```

The acquire gets flat rows. The transform does the aggregation. Clean separation.

### Complex case: multi-stage with multiple sources

```yaml
steps:
  - capability: acquire
    name: get_orders
    config:
      resource: { ref: "orders-db", entity: orders }
      select: { columns: ["id", "customer_id", "total"] }
      filter: { status: { eq: "pending" }, total: { gt: 100 } }

  - capability: acquire
    name: get_customers
    config:
      resource: { ref: "orders-db", entity: customers }
      select: { columns: ["id", "name", "tier"] }
      params:
        expression: "{ id: { in: [.deps.get_orders[].customer_id] } }"

  - capability: transform
    name: enrich_orders
    config:
      expression: |
        .deps.get_orders | map(. + {
          customer: (.deps.get_customers[] | select(.id == .customer_id))
        })

  - capability: persist
    config:
      resource: { ref: "orders-db", entity: enriched_order_summaries }
      mode: upsert
      identity: { primary_key: ["order_id"] }
      data:
        expression: ".prev"
```

Each acquire is simple and predictable. The transform step does the join logic in jaq-core. The persist writes the result. The composition is the orchestration layer.

### When to use a domain handler instead

If the operation requires:
- Stored procedures or database functions
- Complex SQL (CTEs, window functions, recursive queries)
- External service calls with protocol-specific logic (SOAP, GraphQL, gRPC)
- Business logic that can't be expressed as data transformation
- Multi-table transactions with rollback semantics

Then it belongs in a domain handler, not a grammar composition. The grammar boundary is clear: **if you can express the (action, resource, context) triple as a structured declaration with predictable shape, it's a grammar operation. If you can't, it's a domain handler.**

---

## Impact on Adapter Implementation

These constraints simplify the adapter implementations significantly:

### PostgresPersistAdapter

Needs to handle:
- INSERT with explicit columns and parameterized values
- UPDATE with SET clause and PK-based WHERE
- INSERT ... ON CONFLICT with declared conflict keys and configurable strategy
- DELETE with PK-based WHERE
- Nested relationship inserts (one level, FK propagation)
- RETURNING * for result capture

Does NOT need:
- Query building beyond INSERT/UPDATE/DELETE
- CTE generation, subquery support, or JOIN construction
- Type coercion beyond serde_json::Value → SQL parameter binding
- Transaction management beyond single-statement auto-commit (initially)

### PostgresAcquireAdapter

Needs to handle:
- SELECT with explicit column list
- WHERE clause from declarative filter operators (eq, gt, in, etc.)
- WHERE clause from dynamic params (merged with static filters)
- ORDER BY from constraints
- LIMIT / OFFSET from constraints
- Belongs-to JOINs (optional, many-to-one only)
- Separate batch queries for one-to-many relationships
- Result assembly into nested JSON

Does NOT need:
- GROUP BY, HAVING, aggregate functions
- Subqueries, CTEs, window functions
- Complex JOIN trees (max one level of nesting)
- Raw SQL passthrough

### HttpPersistAdapter / HttpAcquireAdapter

Even simpler — HTTP is already shape-constrained:
- Persist: method selection (POST/PUT/PATCH/DELETE) + JSON body + URL construction
- Acquire: GET + query parameters from filters/params + pagination

---

## Open Design Questions

### 1. Filter Composition (AND vs OR)

The current design implies all filters are AND-combined. Should we support OR grouping?

```yaml
filter:
  any:
    - status: { eq: "pending" }
    - status: { eq: "processing" }
```

Recommendation: Start without OR. The `in` operator handles the most common case (`status: { in: ["pending", "processing"] }`). If OR grouping becomes necessary, add it as a structured declaration, never as raw SQL.

### 2. Pagination Strategy

Should acquire handle cursor-based pagination, or only offset-based?

Recommendation: Start with offset-based (LIMIT/OFFSET). Cursor-based pagination is more efficient for large datasets but requires the adapter to understand the cursor column. Add as a constraint option later if needed:

```yaml
constraints:
  cursor:
    column: "created_at"
    after: "2026-03-01T00:00:00Z"
  limit: 100
```

### 3. Batch persist Size Limits

Should persist enforce a maximum batch size to prevent unbounded INSERT statements?

Recommendation: Yes. Default to 1000 rows per persist call. Larger batches should be chunked in the composition using transform + batch processing patterns. The adapter shouldn't generate a 50,000-row INSERT.

### 4. Nested Relationship Depth

The design says "one level of nesting." Should we enforce this in the schema, or allow deeper nesting with a warning?

Recommendation: Enforce at one level in the schema. Deeper nesting is a composition concern — use multiple persist steps. This keeps the adapter's SQL generation simple and the transaction scope predictable.

### 5. Column Type Hints

Should the acquire declaration include type hints for result columns to avoid ambiguous JSON serialization (e.g., numeric strings vs numbers)?

Recommendation: Defer. PostgreSQL's type system and sqlx's type mapping handle this well for database resources. HTTP resources return JSON with types already determined by the API. If type ambiguity becomes a problem in practice, add optional type hints as a constraint.

---

## Summary

| Concern | Where It Lives | Complexity Budget |
|---------|---------------|-------------------|
| **I/O shape** | persist / acquire declarations | Predictable, structured, declarative |
| **Data transformation** | transform (jaq-core) | Full expression power, sandboxed |
| **Multi-source composition** | Composition steps | Unlimited chaining, checkpoint-safe |
| **Complex domain logic** | Domain handlers | Unconstrained, outside grammar |

The grammar makes the predictable things declarative. Domain handlers handle the unpredictable things. The boundary is clear and intentional.
