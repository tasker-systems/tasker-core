# Case Study: Grammar Proposals for Contrib Workflow Handlers

*Expressing the internal logic of each handler as a grammar composition*

*March 2026 — Research Spike (revised for transform-centric 6-capability model)*

---

## Approach

Each contrib example workflow has 4-8 handlers, each implemented identically across Rust, Python, Ruby, and TypeScript. For each handler, we analyze the internal logic and propose a grammar composition that would express the same operations using capability vocabularies. Where a handler is too simple or too domain-specific for composition, we note why.

The grammar categories used here follow the taxonomy from `actions-traits-and-capabilities.md`: **Acquire** (retrieve), **Transform** (reshape/enrich/compute/evaluate), **Validate** (check/assert), **Persist** (write/commit), **Emit** (domain event publication). The 6-capability model (`transform`, `validate`, `assert`, `persist`, `acquire`, `emit`) uses jaq (Rust-native jq) as the unified expression language for all data transformation — see `transform-revised-grammar.md` for the full design rationale.

---

## Workflow 1: E-commerce Order Processing

### Handler: validate_cart

**Internal logic** (all languages):
1. Validate cart items have required fields (SKU, name, quantity > 0, price > 0)
2. Calculate line totals per item (price x quantity)
3. Sum subtotal across items
4. Calculate tax (8-8.75% of subtotal)
5. Determine shipping (free if subtotal >= threshold, else flat rate)
6. Round all monetary values to 2 decimals

**Grammar proposal**:

```yaml
grammar: Validate
compose:
  - capability: validate
    config:
      schema:
        sku: { type: string, required: true }
        name: { type: string, required: true }
        quantity: { type: integer, required: true, min: 1 }
        unit_price: { type: number, required: true, min: 0.01 }
      coercion: permissive
      unknown_fields: passthrough
      on_failure: collect

  - capability: transform
    output:
      type: object
      required: [items, subtotal, tax, free_shipping, shipping, total]
      properties:
        items: { type: array, items: { type: object, properties: { line_total: { type: number } } } }
        subtotal: { type: number }
        tax: { type: number }
        free_shipping: { type: boolean }
        shipping: { type: number }
        total: { type: number }
    filter: |
      .prev
      | .items |= map(. + {line_total: ((.quantity * .unit_price) * 100 | round / 100)})
      | . + {subtotal: ([.items[].line_total] | add)}
      | . + {tax: ((.subtotal * 0.0875) * 100 | round / 100)}
      | . + {free_shipping: (.subtotal >= 75.00)}
      | . + {shipping: (if .free_shipping then 0 else 9.99 end)}
      | . + {total: ((.subtotal + .tax + .shipping) * 100 | round / 100)}
```

**Observations**: Two capabilities — validate (boundary gate) and transform (arithmetic, boolean evaluation, and conditional derivation in a single jaq filter). The `validate` capability is the trust boundary: it asserts that cart items have required fields with correct types, permissively coerces (e.g., string `"2"` to integer `2`), passes unknown fields through (items may have extra attributes), and collects all violations rather than failing on the first. The `transform` does all the work that previously required three separate capabilities (compute, evaluate, compute): per-item line totals, aggregate subtotal/tax, the boolean `free_shipping` determination, conditional shipping cost, and the final total — all in one jaq expression. The `|=` (update) and `| . +` (merge) patterns thread state through the computation naturally. Note that jq uses IEEE 754 floats; the `(. * 100 | round / 100)` pattern provides 2-decimal rounding for monetary values.

### Handler: process_payment

**Internal logic**:
1. Extract total from cart dependency
2. Validate amount (reject if > $10,000)
3. Check for test/decline tokens
4. Generate transaction ID, authorization code
5. Calculate processing fee (2.9% of total)
6. Return payment record

**Grammar proposal**:

The gateway charge is a domain operation — it cannot be a capability within a grammar composition. A virtual handler cannot invoke a domain handler; that breaks responsibility separation. Instead, the TaskTemplate DAG should split this into two steps:

**Step 1: `prepare_payment` (virtual handler — grammar-composed)**:

```yaml
grammar: Validate
compose:
  - capability: transform
    output:
      type: object
      required: [total, payment_token]
      properties:
        total: { type: number }
        payment_token: { type: string }
    filter: |
      {
        total: .deps.validate_cart.total,
        payment_token: .context.payment_token
      }

  - capability: validate
    config:
      schema:
        total: { type: number, required: true, min: 0.01, max: 10000 }
        payment_token: { type: string, required: true }
      coercion: strict
      unknown_fields: drop
      on_failure: fail

  - capability: assert
    filter: '.prev.payment_token | IN("tok_test_declined","tok_test_insufficient_funds") | not'
    error: "Payment token rejected"
```

**Step 2: `process_payment` (domain handler — traditional curated code)**:

Depends on `prepare_payment`. Consumes the validated, asserted `result_schema` from step 1. Handles gateway-specific API calls, processing fee calculation, PCI compliance, nuanced error handling (declined vs. insufficient funds vs. timeout), and idempotency key management. This is irreducibly domain-specific logic.

**Observations**: The virtual handler (step 1) does the grammar-composable work: transform to project from dependencies and task context, validate the payment data at the trust boundary, and assert the token precondition. The assert's jaq filter directly evaluates the boolean condition — no separate evaluate capability is needed because jq's `IN()` operator handles membership testing natively. Its `result_schema` becomes the input to the domain handler (step 2). The TaskTemplate DAG freely mixes virtual handlers and domain handlers — the composition boundary is the `result_schema` of the StepDefinition. No capability within a grammar composition may invoke domain handler logic.

### Handler: update_inventory

**Internal logic**:
1. Extract validated items from cart dependency
2. For each item: select warehouse based on quantity, generate reservation ID
3. Simulate stock levels, calculate new stock after reservation
4. Track total reserved, set expiry (30 min)

**Grammar proposal**:

The inventory reservation is a domain operation. The TaskTemplate DAG could optionally split this into a virtual handler that prepares data, followed by the domain handler that performs the reservation:

**Option A: Single domain handler step** (current approach — the projection is trivial enough to stay in handler code)

**Option B: Split into two steps**:

Step 1: `prepare_inventory_update` (virtual handler):

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [validated_items, item_skus, item_quantities]
      properties:
        validated_items: { type: array, items: { type: object } }
        item_skus: { type: array, items: { type: string } }
        item_quantities: { type: array, items: { type: integer } }
    filter: |
      {
        validated_items: .deps.validate_cart.validated_items,
        item_skus: [.deps.validate_cart.validated_items[].sku],
        item_quantities: [.deps.validate_cart.validated_items[].quantity]
      }
```

Step 2: `reserve_inventory` (domain handler): Consumes the projected data and handles warehouse selection logic, distributed reservation protocols, stock management, and TTL enforcement.

**Observations**: This handler is simple enough that Option A (single domain handler) is preferable. The projection is a single jaq object construction — creating a virtual handler step for it adds DAG complexity without meaningful benefit. The grammar composition boundary should be drawn where it adds value, not applied mechanically to every step.

### Handler: create_order

**Internal logic**:
1. Extract results from three dependencies (cart, payment, inventory)
2. Generate order ID and order number
3. Calculate estimated delivery date
4. Aggregate items with line totals from cart validation
5. Copy financial totals, payment references, inventory status

**Grammar proposal**:

```yaml
grammar: Persist
compose:
  - capability: transform
    output:
      type: object
      required: [validated_items, subtotal, tax, shipping, total, payment_id, transaction_id, updated_products, estimated_delivery]
      properties:
        validated_items: { type: array, items: { type: object } }
        subtotal: { type: number }
        tax: { type: number }
        shipping: { type: number }
        total: { type: number }
        payment_id: { type: string }
        transaction_id: { type: string }
        updated_products: { type: array }
        expedited_shipping: { type: boolean }
        estimated_delivery: { type: string }
    filter: |
      {
        validated_items: .deps.validate_cart.validated_items,
        subtotal: .deps.validate_cart.subtotal,
        tax: .deps.validate_cart.tax,
        shipping: .deps.validate_cart.shipping,
        total: .deps.validate_cart.total,
        payment_id: .deps.process_payment.payment_id,
        transaction_id: .deps.process_payment.transaction_id,
        updated_products: .deps.update_inventory.updated_products,
        expedited_shipping: (.deps.validate_cart.shipping > 0),
        estimated_delivery: (if .deps.validate_cart.shipping > 0 then "3_days" else "5_days" end)
      }

  - capability: persist
    config:
      resource:
        type: database
        entity: orders
      constraints:
        unique_key: order_ref
        id_pattern: "ORD-{YYYYMMDD}-{hex}"
      validate_success:
        order_id: { type: string, required: true }
      result_shape: [order_id, order_ref, created_at, estimated_delivery]
    data: |
      {
        order_ref: .prev.order_ref,
        customer_email: .context.customer_email,
        items: .prev.validated_items,
        total: .prev.total,
        estimated_delivery: .prev.estimated_delivery
      }
    checkpoint: true
```

**Observations**: Two capabilities — transform (project from three dependencies plus derive delivery estimate) and persist. The single `transform` replaces what was previously three separate capabilities (reshape, evaluate, compute): it projects fields from three upstream dependency outputs, derives the boolean `expedited_shipping`, and computes the delivery estimate — all in one jaq object construction. The `persist` capability expresses the (action, resource, context) triple: write to the orders entity with uniqueness constraints, validate that an order_id was generated, and return a specific result shape for downstream use. The jaq `data` filter maps from `.prev` (the transform's output) into the persistence shape.

### Handler: send_confirmation

**Internal logic**:
1. Extract order details from dependency
2. Build email template data (customer email, items, totals, delivery estimate)
3. Generate message ID
4. Set email provider and template name

**Grammar proposal**:

```yaml
grammar: Emit
compose:
  - capability: transform
    output:
      type: object
      required: [order_number, total, customer_email]
      properties:
        order_number: { type: string }
        total: { type: number }
        estimated_delivery: { type: string }
        customer_email: { type: string }
    filter: |
      {
        order_number: .deps.create_order.order_number,
        total: .deps.create_order.total,
        estimated_delivery: .deps.create_order.estimated_delivery,
        customer_email: .deps.create_order.customer_email
      }

  - capability: emit
    config:
      event_name: "order.confirmed"
      event_version: "1.0"
      delivery_mode: durable
      condition: success
    payload: |
      {
        order_number: .prev.order_number,
        total: .prev.total,
        estimated_delivery: .prev.estimated_delivery,
        customer_email: .prev.customer_email
      }
    schema:
      type: object
      required: [order_number, total, customer_email]
      properties:
        order_number: { type: string }
        total: { type: number }
        customer_email: { type: string }
        estimated_delivery: { type: string }
```

**Observations**: This is an `Emit` grammar — the outcome is a domain event. Two capabilities: transform (gather from dependencies), emit (fire domain event). The `emit` fires an `order.confirmed` event with the order data as payload. What happens downstream — sending a confirmation email, updating a CRM, triggering a webhook — is entirely outside the grammar's concern. Some downstream consumer reads from the `{namespace}_domain_events` PGMQ queue and acts on the event. This is much cleaner than the previous model where the grammar tried to construct email subjects and bodies — content construction is the consumer's responsibility.

---

## Workflow 2: Data Pipeline Analytics

### Handlers: extract_sales_data, extract_inventory_data, extract_customer_data

**Internal logic** (all three follow same pattern):
1. Read configuration (source, date range, granularity)
2. Generate deterministic sample data (seeded from config hash)
3. Iterate records, computing per-record fields
4. Aggregate totals across records

**Grammar proposal** (representative — `extract_sales_data`):

```yaml
grammar: Acquire
compose:
  - capability: acquire
    config:
      resource:
        type: api
        endpoint: "/api/sales"
        method: GET
      constraints:
        timeout_ms: 5000
      validate_success:
        status: { in: [200] }
      result_shape: [data.sales_records]
    params: |
      {
        source: .context.source,
        date_range_start: .context.date_range_start,
        date_range_end: .context.date_range_end
      }

  - capability: transform
    output:
      type: object
      required: [records, total_revenue, total_quantity, record_count]
      properties:
        records: { type: array, items: { type: object, properties: { revenue: { type: number } } } }
        total_revenue: { type: number }
        total_quantity: { type: integer }
        record_count: { type: integer }
    filter: |
      .prev.data.sales_records
      | map({date, sku, quantity, unit_price, revenue: ((.quantity * .unit_price) * 100 | round / 100)})
      | {
          records: .,
          total_revenue: ([.[].revenue] | add),
          total_quantity: ([.[].quantity] | add),
          record_count: length
        }
```

**Observations**: Two capabilities — acquire and transform. The `acquire` is the I/O boundary (true acquisition), with a jaq `params` filter that navigates to `.context` for the request parameters. The `transform` replaces what was previously a reshape followed by a compute: it projects the relevant fields from the API response, enriches each record with a computed `revenue` field, and aggregates totals — all in a single jaq pipeline. The `map()` + object construction pattern naturally combines field selection and per-record arithmetic. Note that jq uses IEEE 754 floats; the `(. * 100 | round / 100)` pattern provides 2-decimal rounding.

All three extract handlers have the same structure (acquire, transform) but different configurations. This is exactly where grammar composition shines: **same grammar, same capabilities, different config**.

### Handlers: transform_sales, transform_inventory, transform_customers

**Internal logic** (all three follow same pattern):
1. Read upstream extract results from dependency
2. Group records by one or more dimensions (category, warehouse, tier)
3. Calculate per-group aggregates (sum, average, count)
4. Identify top/best performers
5. Calculate cross-group metrics

**Grammar proposal** (representative — `transform_sales`):

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [groups, top_category, total_revenue, total_categories]
      properties:
        groups:
          type: array
          items:
            type: object
            properties:
              category: { type: string }
              revenue: { type: number }
              quantity: { type: integer }
              transaction_count: { type: integer }
              avg_revenue: { type: number }
        top_category:
          type: object
          properties:
            category: { type: string }
            revenue: { type: number }
        total_revenue: { type: number }
        total_categories: { type: integer }
    filter: |
      .deps.extract_sales_data.records
      | group_by(.category)
      | map({
          category: .[0].category,
          revenue: ([.[].revenue] | add),
          quantity: ([.[].quantity] | add),
          transaction_count: length,
          avg_revenue: (([.[].revenue] | add) / length * 100 | round / 100)
        })
      | {
          groups: .,
          top_category: (sort_by(.revenue) | reverse | .[0]),
          total_revenue: ([.[].revenue] | add),
          total_categories: length
        }
```

**Observations**: A single `transform` replaces what was previously three separate capabilities (group_by, rank, compute). jq's native `group_by(.category)` handles dimensional grouping, `sort_by(.revenue) | reverse` handles ranking, and the `map()` with arithmetic handles per-group aggregation — all within one expression. The `output` JSON Schema declares the full contract (groups array, top category, totals), enabling static analysis of downstream dependencies without executing the filter. Different transform handlers would use the same single-transform pattern with different grouping dimensions and metrics.

### Handler: aggregate_metrics

**Internal logic**:
1. Verify all three transform dependencies present
2. Extract key metrics from each (revenue, inventory, customer counts)
3. Calculate cross-source derived metrics (revenue_per_customer, inventory_turnover)
4. Build summary structures per source

**Grammar proposal**:

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [total_revenue, record_count, total_quantity, reorder_alerts, total_customers, total_ltv, revenue_per_customer, inventory_turnover]
      properties:
        total_revenue: { type: number }
        record_count: { type: integer }
        total_quantity: { type: integer }
        reorder_alerts: { type: integer }
        total_customers: { type: integer }
        total_ltv: { type: number }
        revenue_per_customer: { type: number }
        inventory_turnover: { type: number }
    filter: |
      {
        total_revenue: .deps.transform_sales.total_revenue,
        record_count: .deps.transform_sales.record_count,
        total_quantity: .deps.transform_inventory.total_quantity,
        reorder_alerts: .deps.transform_inventory.reorder_alerts,
        total_customers: .deps.transform_customers.total_customers,
        total_ltv: .deps.transform_customers.total_ltv
      }
      | . + {
          revenue_per_customer: ((.total_revenue / .total_customers) * 100 | round / 100),
          inventory_turnover: ((.total_revenue / .total_quantity) * 100 | round / 100)
        }
```

**Observations**: A single `transform` replaces what was previously two capabilities (reshape then compute). The jaq filter first constructs an object projecting from three dependency outputs, then derives cross-source ratios by referencing the fields just projected — the `| . +` pattern threads state through the computation naturally. No separate reshape is needed because jq's object construction is already projection, and no separate compute is needed because arithmetic operators work inline.

### Handler: generate_insights

**Internal logic**:
1. Evaluate multiple threshold-based rules against aggregate metrics
2. Generate insight recommendations per domain (revenue, inventory, customer)
3. Calculate composite health score (0-100) from multiple weighted factors
4. Classify health rating (Excellent/Good/Fair/Needs Improvement)

**Grammar proposal**:

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [insights, has_best_source, high_volume, low_stock_warning, health_score, health_rating]
      properties:
        insights:
          type: array
          items:
            type: object
            properties:
              type: { type: string }
              severity: { type: string }
              recommendation: { type: string }
        has_best_source: { type: boolean }
        high_volume: { type: boolean }
        low_stock_warning: { type: boolean }
        health_score: { type: integer }
        health_rating: { type: string, enum: [excellent, good, fair, needs_improvement] }
    filter: |
      .deps.aggregate_metrics
      | . + {
          insights: [
            (if .revenue_per_customer < 500 then {type: "revenue", severity: "medium", recommendation: "Focus on upselling"} else empty end),
            (if .reorder_alerts > 5 then {type: "inventory", severity: "high", recommendation: "Immediate reorder needed"} else empty end),
            (if .total_ltv / .total_customers > 3000 then {type: "customer", severity: "low", recommendation: "Retention program effective"} else empty end)
          ]
        }
      | . + {
          has_best_source: (has("best_source")),
          high_volume: (.record_count > 50),
          low_stock_warning: (.reorder_alerts > 5)
        }
      | . + {
          health_score: (
            75
            + (if .has_best_source then 5 else 0 end)
            + (if .high_volume then 10 else 0 end)
            + (if .low_stock_warning then -15 else 0 end)
            | if . > 100 then 100 elif . < 0 then 0 else . end
          )
        }
      | . + {
          health_rating: (
            if .health_score >= 80 then "excellent"
            elif .health_score >= 60 then "good"
            elif .health_score >= 40 then "fair"
            else "needs_improvement" end
          )
        }
```

**Observations**: A single `transform` replaces what was previously four separate capabilities (evaluate_rules, evaluate, compute, evaluate). The jaq filter chains four computation stages using `| . +`: (1) rule-based insight generation using conditional `if-then-else` with `empty` to produce variable-length arrays, (2) boolean factor derivation, (3) weighted health score calculation with clamping, and (4) rating classification via `if-elif-else`. Each stage adds fields to the accumulating object. This demonstrates that the semantic distinctions between "rule evaluation", "boolean evaluation", "arithmetic computation", and "classification" are conventions about what the jq filter does — not different execution models requiring separate capabilities.

This handler is a strong composition candidate because the rules, scoring weights, and rating brackets are pure configuration — different deployments would have different thresholds and weights.

---

## Workflow 3: Microservices User Registration

### Handler: create_user_account

**Internal logic**:
1. Validate email (regex), name (length >= 2), username (not reserved)
2. Generate user ID, internal ID, API key, verification token
3. Set initial status = "pending_verification"

**Grammar proposal**:

```yaml
grammar: Persist
compose:
  - capability: validate
    config:
      schema:
        email: { type: string, required: true, pattern: "^[^@]+@[^@]+$" }
        full_name: { type: string, required: true, min_length: 2 }
        username: { type: string, required: false, not_in: [admin, root, system, api] }
      coercion: strict
      unknown_fields: drop
      on_failure: collect

  - capability: persist
    config:
      resource:
        type: database
        entity: user_accounts
      constraints:
        unique_key: email
      validate_success:
        user_id: { type: string, required: true }
      result_shape: [user_id, email, status, created_at]
    data: |
      {
        email: .prev.email,
        full_name: .prev.full_name,
        username: .prev.username,
        status: "pending_verification"
      }
    checkpoint: true
```

**Observations**: Two capabilities — validate, persist. The `validate` capability is the trust boundary: strict coercion (exact types required for user registration), drop unknown fields (defensive filtering at account creation boundary), and collect all violations for user-facing error reporting. The `persist` expresses the full (action, resource, context) triple: write to user_accounts with uniqueness constraint on email, validate that a user_id was generated, and return a specific result shape. The jaq `data` filter maps from `.prev` (the validated output) and adds the literal `status` value. **Note**: The original handler also generates credentials (user_id, API key, verification token) — this is security-sensitive logic that should remain as traditional handler code. A grammar-composed version would handle the validate then persist flow, while credential generation would either be a pre-step in a domain handler or a separate workflow step.

### Handler: setup_billing

**Internal logic**:
1. Extract user_id and plan from user dependency
2. Look up billing tier config (price, trial_days, limits)
3. If billing required (price > 0): set billing dates and status
4. If free plan: skip billing, set status = "skipped_free_plan"
5. Generate billing_id, subscription_id

**Grammar proposal**:

```yaml
grammar: Persist
compose:
  - capability: acquire
    config:
      resource:
        type: config
        table: billing_tiers
        key_field: plan
      constraints:
        required: true
      result_shape: [price, trial_days, api_limit, storage_gb]
    params: |
      {
        plan: .deps.create_user_account.plan
      }

  - capability: transform
    output:
      type: object
      required: [user_id, plan, billing_status, billing_required]
      properties:
        user_id: { type: string }
        plan: { type: string }
        price: { type: number }
        trial_days: { type: integer }
        billing_required: { type: boolean }
        billing_status: { type: string }
        next_billing_date: { type: string }
    filter: |
      .prev + {
        user_id: .deps.create_user_account.user_id,
        plan: .deps.create_user_account.plan,
        billing_required: (.prev.price > 0),
        billing_status: (if .prev.price > 0 then "active" else "skipped_free_plan" end),
        next_billing_date: (if .prev.price > 0 then "30_days_from_now" else null end)
      }

  - capability: persist
    config:
      resource:
        type: database
        entity: billing_profiles
      constraints:
        id_prefix: "bill_"
      validate_success:
        billing_id: { type: string, required: true }
      result_shape: [billing_id, billing_status, next_billing_date]
    data: |
      {
        user_id: .prev.user_id,
        plan: .prev.plan,
        billing_status: .prev.billing_status,
        next_billing_date: .prev.next_billing_date,
        trial_days: .prev.trial_days
      }
    checkpoint: true
```

**Observations**: Three capabilities — acquire (config lookup), transform, persist. The `acquire` expresses (action, resource, context): read billing tier configuration by plan key, requiring a match, and returning specific fields. The jaq `params` filter navigates to `.deps` for the plan value. The `transform` replaces what was previously two separate capabilities (evaluate then compute): it merges the acquire output with dependency data, derives the boolean `billing_required`, and conditionally computes billing status and next billing date — all in one jaq expression using `if-then-else`. The `persist` writes the billing profile with the full (action, resource, context) surface: resource target, jaq data mapping from `.prev`, constraints, success validation, and result shape.

### Handler: initialize_preferences

**Internal logic**:
1. Extract plan from user dependency
2. Build default preferences based on plan tier
3. Deep merge custom preferences over defaults
4. Count defaults applied and customizations

**Grammar proposal**:

```yaml
grammar: Persist
compose:
  - capability: acquire
    config:
      resource:
        type: config
        table: plan_defaults
        key_field: plan
      constraints:
        required: true
      result_shape: [notifications, ui_settings, feature_flags]
    params: |
      {
        plan: .deps.create_user_account.plan
      }

  - capability: transform
    output:
      type: object
      required: [user_id, notifications, ui_settings, feature_flags]
      properties:
        user_id: { type: string }
        notifications: { type: object }
        ui_settings: { type: object }
        feature_flags: { type: object }
    filter: |
      {
        user_id: .deps.create_user_account.user_id,
        notifications: (.prev.notifications * (.context.metadata.custom_preferences.notifications // {})),
        ui_settings: (.prev.ui_settings * (.context.metadata.custom_preferences.ui_settings // {})),
        feature_flags: .prev.feature_flags
      }

  - capability: persist
    config:
      resource:
        type: database
        entity: user_preferences
      validate_success:
        preferences_id: { type: string, required: true }
      result_shape: [preferences_id, defaults_applied_count, customizations_count]
    data: |
      {
        user_id: .prev.user_id,
        notifications: .prev.notifications,
        ui_settings: .prev.ui_settings,
        feature_flags: .prev.feature_flags
      }
    checkpoint: true
```

**Observations**: The `acquire` reads plan defaults from a config store — expressing (action, resource, context) clearly. The `transform` handles the deep merge pattern using jq's `*` (recursive merge) operator — custom preferences override defaults, with `// {}` providing an empty fallback when no customizations exist. This replaces the previous `reshape` with `merge_strategy: overlay` using a standard jq idiom that is well-documented and widely understood. The `persist` writes the merged preferences with full (action, resource, context): resource target, jaq data mapping, success validation, and result shape.

### Handler: send_welcome_sequence

**Internal logic**:
1. Verify all three dependencies present (user, billing, preferences)
2. Check notification preferences to decide channels
3. Build message list conditionally (welcome email if opted in, verification always, onboarding always, trial notification if applicable)
4. Count messages and channels

**Grammar proposal**:

```yaml
grammar: Emit
compose:
  - capability: transform
    output:
      type: object
      required: [user_id, email, notifications]
      properties:
        user_id: { type: string }
        email: { type: string }
        trial_end: { type: string }
        notifications:
          type: array
          items:
            type: object
            properties:
              type: { type: string }
              channel: { type: string }
    filter: |
      {
        user_id: .deps.create_user_account.user_id,
        email: .deps.create_user_account.email,
        trial_end: .deps.setup_billing.trial_end,
        email_updates: .deps.initialize_preferences.notifications.email_updates
      }
      | . + {
          notifications: [
            (if .email_updates then {type: "welcome_email", channel: "email"} else empty end),
            {type: "verification_email", channel: "email"},
            {type: "onboarding_guide", channel: "in_app"},
            (if .trial_end != null then {type: "trial_notification", channel: "email"} else empty end)
          ]
        }

  - capability: emit
    config:
      event_name: "user.welcome_sequence"
      event_version: "1.0"
      delivery_mode: durable
      condition: success
    payload: |
      {
        user_id: .prev.user_id,
        email: .prev.email,
        trial_end: .prev.trial_end,
        notifications: .prev.notifications
      }
    schema:
      type: object
      required: [user_id, email, notifications]
      properties:
        user_id: { type: string }
        email: { type: string }
        notifications: { type: array }
```

**Observations**: Two capabilities — transform (project and evaluate rules), emit (fire domain event). The single `transform` replaces what was previously three separate capabilities (reshape, evaluate_rules, and implicit merging): it projects relevant fields from three dependencies, then builds the conditional notification array using jq's `if-then-else` with `empty` to produce variable-length arrays. This is more expressive than the previous `evaluate_rules` approach because jq's conditional array construction handles both "always include" (no condition) and "conditionally include" (with `if`) naturally in the same array literal. The `emit` fires a `user.welcome_sequence` domain event with the matched notification rules as payload. Downstream consumers decide how to deliver each notification — the grammar's job ends at expressing what happened and what data is relevant.

### Handler: update_user_status

**Internal logic**:
1. Verify all four dependencies present
2. Build registration summary from all upstream data
3. Conditionally include billing_id if plan != "free"
4. Set status = "active", list services completed

**Grammar proposal**:

```yaml
grammar: Persist
compose:
  - capability: transform
    output:
      type: object
      required: [user_id, email, plan, include_billing]
      properties:
        user_id: { type: string }
        email: { type: string }
        plan: { type: string }
        billing_id: { type: string }
        preferences_count: { type: integer }
        channels_used: { type: array }
        messages_sent: { type: integer }
        include_billing: { type: boolean }
    filter: |
      {
        user_id: .deps.create_user_account.user_id,
        email: .deps.create_user_account.email,
        plan: .deps.create_user_account.plan,
        billing_id: .deps.setup_billing.billing_id,
        preferences_count: .deps.initialize_preferences.preferences_count,
        channels_used: .deps.send_welcome_sequence.channels_used,
        messages_sent: .deps.send_welcome_sequence.messages_sent,
        include_billing: (.deps.create_user_account.plan != "free")
      }

  - capability: persist
    config:
      resource:
        type: database
        entity: user_accounts
      constraints:
        key: user_id
        operation: update
      validate_success:
        status: { equals: active }
      result_shape: [user_id, status, updated_at]
    data: |
      {
        status: "active",
        billing_id: (if .prev.include_billing then .prev.billing_id else null end),
        preferences_count: .prev.preferences_count,
        channels_used: .prev.channels_used,
        messages_sent: .prev.messages_sent
      }
    checkpoint: true
```

**Observations**: Two capabilities — transform (project from four dependencies plus boolean derivation) and persist. The single `transform` replaces the previous reshape then evaluate chain: it projects relevant fields from all upstream dependencies and derives the boolean `include_billing` — all in one jaq object construction. The `persist` uses a jaq `data` filter with an inline `if-then-else` to conditionally include billing_id based on the transform's output, writing the status update with full (action, resource, context): the resource target (user_accounts), the data to write, update constraints, success validation, and result shape.

---

## Workflow 4: Payments Refund Processing

### Handler: validate_payment_eligibility

**Internal logic**:
1. Validate refund amount > 0
2. Simulate original amount lookup
3. Calculate refund percentage
4. Compute fraud score from hash of order reference + email
5. Reject if fraud score > 85

**Grammar proposal**:

The fraud check is a domain operation. The TaskTemplate DAG should split this:

**Step 1: `validate_payment_data` (virtual handler — grammar-composed)**:

```yaml
grammar: Validate
compose:
  - capability: validate
    config:
      schema:
        refund_amount: { type: number, required: true, min: 0.01 }
      coercion: permissive
      unknown_fields: passthrough
      on_failure: fail

  - capability: acquire
    config:
      resource:
        type: api
        endpoint: payment_service
        key_field: payment_id
      constraints:
        required: true
      result_shape: [original_amount, payment_method, payment_status]
    params: |
      {
        payment_id: .prev.payment_id
      }

  - capability: transform
    output:
      type: object
      required: [refund_amount, original_amount, refund_percentage]
      properties:
        refund_amount: { type: number }
        original_amount: { type: number }
        payment_method: { type: string }
        payment_status: { type: string }
        refund_percentage: { type: number }
    filter: |
      .prev + {
        refund_amount: .context.refund_amount,
        refund_percentage: ((.context.refund_amount / .prev.original_amount * 100) * 100 | round / 100)
      }
```

**Step 2: `check_fraud_score` (domain handler — traditional curated code)**:

Depends on `validate_payment_data`. Consumes the validated, computed `result_schema` from step 1. Runs the proprietary fraud scoring algorithm — hash-based scoring, model inference, or third-party API integration. The fraud check's "how" is irreducibly domain-specific (algorithm selection, seed field handling, threshold behavior, rejection semantics).

**Observations**: The virtual handler (step 1) does the grammar-composable work: validate refund amount at the trust boundary, acquire the original payment data, and compute the refund percentage via a `transform`. The `transform` merges the acquire output with context data and derives the percentage in one expression. The domain handler (step 2) consumes this prepared data and runs the opaque fraud scoring. The `result_schema` of step 1 becomes the upstream dependency for step 2. This respects the architectural boundary: no capability within a grammar composition invokes domain handler logic. Note that jq uses IEEE 754 floats for the percentage calculation; the rounding pattern ensures 2-decimal precision.

### Handler: process_gateway_refund

**Internal logic**:
1. Verify upstream eligibility
2. Generate refund ID, gateway transaction ID, settlement ID
3. Calculate authorization code (hash-derived)
4. Calculate estimated arrival (5 business days)
5. Record gateway response

**Grammar proposal**:

The gateway refund is a domain operation. The TaskTemplate DAG should split this:

**Step 1: `validate_refund_eligibility` (virtual handler — grammar-composed)**:

```yaml
grammar: Validate
compose:
  - capability: assert
    filter: '.deps.check_fraud_score.payment_status == "completed"'
    error: "Payment not validated"
```

**Step 2: `process_gateway_refund` (domain handler — traditional curated code)**:

Depends on `validate_refund_eligibility`. Consumes the validated, asserted `result_schema` from step 1. Handles Stripe/provider-specific API calls, refund ID generation, authorization code computation, settlement delay calculation, and gateway response recording. This is irreducibly domain-specific — every payment provider has different refund APIs, error codes, and idempotency semantics.

**Observations**: The virtual handler (step 1) is now a single assert — the jaq filter directly evaluates the payment status condition against the dependency output. No separate evaluate capability is needed because the assert's filter can navigate to `.deps` and perform the boolean comparison inline. This is the simplest possible grammar composition: one capability, one boolean gate. The gateway interaction is pure domain handler code. The TaskTemplate DAG connects them through dependency declarations.

### Handler: update_payment_records

**Internal logic**:
1. Verify refund was processed
2. Create two ledger entries (debit refunds_payable, credit accounts_receivable)
3. Generate record ID, journal ID
4. Calculate fiscal period
5. Set reconciliation status

**Grammar proposal**:

```yaml
grammar: Persist
compose:
  - capability: assert
    filter: '.deps.process_gateway_refund.refund_status == "processed"'
    error: "Refund must be processed before updating records"

  - capability: persist
    config:
      resource:
        type: database
        entity: ledger_entries
      constraints:
        fiscal_period_format: "{year}-{month}"
        idempotency_key: refund_id
      validate_success:
        record_id: { type: string, required: true }
        journal_id: { type: string, required: true }
      result_shape: [record_id, journal_id, fiscal_period, created_at]
    data: |
      {
        entries: [
          {type: "debit", account: "refunds_payable", amount: .deps.process_gateway_refund.refund_amount},
          {type: "credit", account: "accounts_receivable", amount: .deps.process_gateway_refund.refund_amount}
        ],
        refund_id: .deps.process_gateway_refund.refund_id
      }
    checkpoint: true

  - capability: persist
    config:
      resource:
        type: database
        entity: reconciliation_status
      validate_success:
        reconciliation_id: { type: string, required: true }
      result_shape: [reconciliation_id, status]
    data: |
      {
        status: "pending",
        refund_id: .deps.process_gateway_refund.refund_id,
        journal_id: .prev.journal_id
      }
```

**Observations**: Three capabilities — assert, persist (ledger entries), persist (reconciliation status). The assert's jaq filter directly evaluates the refund status condition against the dependency output — no separate evaluate capability is needed. Both `persist` capabilities express the full (action, resource, context) triple. The ledger entry persist's jaq `data` filter constructs the double-entry array structure inline, navigating to `.deps` for the refund amount and ID. The reconciliation persist references `.prev.journal_id` from the first persist's result. Both are `persist` with different resource configurations — domain specificity comes from the configuration envelope, not capability naming.

### Handler: notify_customer

**Internal logic**:
1. Verify refund processed
2. Extract customer email (fallback from multiple sources)
3. Generate message ID
4. Build notification with refund references
5. Set delivery metadata

**Grammar proposal**:

```yaml
grammar: Emit
compose:
  - capability: transform
    output:
      type: object
      required: [customer_email, refund_id, amount_refunded]
      properties:
        customer_email: { type: string }
        refund_id: { type: string }
        amount_refunded: { type: number }
        estimated_arrival: { type: string }
    filter: |
      {
        customer_email: (.context.customer_email // .deps.validate_payment_eligibility.customer_email),
        refund_id: .deps.process_gateway_refund.refund_id,
        amount_refunded: .deps.process_gateway_refund.amount_refunded,
        estimated_arrival: .deps.process_gateway_refund.estimated_arrival
      }

  - capability: emit
    config:
      event_name: "refund.processed"
      event_version: "1.0"
      delivery_mode: durable
      condition: success
    payload: |
      {
        refund_id: .prev.refund_id,
        amount_refunded: .prev.amount_refunded,
        customer_email: .prev.customer_email,
        estimated_arrival: .prev.estimated_arrival
      }
    schema:
      type: object
      required: [refund_id, amount_refunded, customer_email]
      properties:
        refund_id: { type: string }
        amount_refunded: { type: number }
        customer_email: { type: string }
        estimated_arrival: { type: string }
```

**Observations**: Same Emit pattern as e-commerce confirmation — transform, emit. The `transform` uses jq's `//` (alternative) operator to resolve the customer email from multiple sources — this replaces the previous `coalesce()` from the custom expression language with standard jq syntax that is well-documented and widely understood. The `emit` fires a `refund.processed` domain event with the relevant data as payload. Downstream consumers (notification service, CRM, audit trail) decide what to do with the event. The composition structure is identical across Emit grammars: transform gathers the data, emit fires the event. Content construction (email templates, notification formatting) is the downstream consumer's responsibility.

---

## Workflow 5: Customer Success Refund

### Handler: validate_refund_request

**Internal logic**:
1. Validate amount > 0 and <= $10,000
2. Check refund reason against allowed set
3. Determine customer tier from customer_id
4. Generate request ID, validation hash
5. Set purchase date reference

**Grammar proposal — two-step split**:

The original handler mixes trust boundary validation with domain-specific customer classification. `classify_customer` is organization-specific logic (keyword-based tier lookup, CRM-driven segmentation, loyalty program rules) whose "how" cannot be expressed as an (action, resource, context) triple. Split into:

**Step 1: `validate_refund_input`** (virtual handler — grammar composition):

```yaml
grammar: Validate
compose:
  - capability: validate
    config:
      schema:
        refund_amount: { type: number, required: true, min: 0.01, max: 10000 }
        refund_reason: { type: string, required: true, enum: [defective, wrong_item, not_as_described, changed_mind, late_delivery] }
        customer_id: { type: string, required: true }
        payment_id: { type: string, required: true }
      coercion: permissive
      unknown_fields: drop
      on_failure: fail

  - capability: acquire
    config:
      resource:
        type: api
        endpoint: order_service
        key_field: payment_id
      constraints:
        required: true
      result_shape: [original_purchase_date, original_amount]
    params: |
      {
        payment_id: .prev.payment_id
      }

result_schema: { refund_amount, refund_reason, customer_id, payment_id, original_purchase_date, original_amount }
```

**Step 2: `classify_and_enrich_refund`** (domain handler):

Consumes `validate_refund_input.result_schema`. Performs organization-specific customer classification (CRM tier lookup, loyalty program mapping, keyword-based segmentation) and enriches the refund request with `customer_tier`, `request_id`, `validation_hash`. This logic is opaque domain code — tier classification rules, CRM integration patterns, and ID generation strategies vary per organization and cannot be parameterized through a generic configuration surface.

**Observations**: The virtual handler handles the composable work — trust boundary validation and purchase history acquisition. The domain handler handles what grammars cannot express: organization-specific customer classification. The `result_schema` from step 1 provides the validated, enriched input that step 2 consumes.

### Handler: check_refund_policy

**Internal logic**:
1. Verify request was validated
2. Determine approval path based on amount thresholds and reason
3. Look up tier-specific policy (refund window, max amount)
4. Calculate days since purchase
5. Check if within refund window

**Grammar proposal — two-step split**:

The original handler mixes composable precondition checking and approval routing with organization-specific policy window logic. `check_policy_window` involves tier-specific refund windows, max amounts, and date arithmetic against organization-defined policies — the "how" of policy evaluation is opaque domain logic. Split into:

**Step 1: `evaluate_refund_routing`** (virtual handler — grammar composition):

```yaml
grammar: Validate
compose:
  - capability: transform
    output:
      type: object
      required: [validation_status, refund_amount, refund_reason, customer_tier, original_purchase_date]
      properties:
        validation_status: { type: string }
        refund_amount: { type: number }
        refund_reason: { type: string }
        customer_tier: { type: string }
        original_purchase_date: { type: string }
    filter: |
      {
        validation_status: .deps.classify_and_enrich_refund.validation_status,
        refund_amount: .deps.classify_and_enrich_refund.refund_amount,
        refund_reason: .deps.classify_and_enrich_refund.refund_reason,
        customer_tier: .deps.classify_and_enrich_refund.customer_tier,
        original_purchase_date: .deps.classify_and_enrich_refund.original_purchase_date
      }

  - capability: assert
    filter: '.prev.validation_status == "passed"'
    error: "Request must be validated before checking refund policy"

  - capability: transform
    output:
      type: object
      required: [approval_path]
      properties:
        approval_path: { type: string, enum: [auto_approved, manager_review, standard_review] }
    filter: |
      .prev
      | if .refund_amount <= 50 then . + {approval_path: "auto_approved"}
        elif (.refund_reason | IN("defective","wrong_item")) and .refund_amount <= 500 then . + {approval_path: "auto_approved"}
        elif .refund_amount > 500 then . + {approval_path: "manager_review"}
        else . + {approval_path: "standard_review"} end

result_schema: { refund_amount, refund_reason, customer_tier, original_purchase_date, approval_path }
```

**Step 2: `check_refund_policy`** (domain handler):

Consumes `evaluate_refund_routing.result_schema`. Applies organization-specific tier-based policy rules: refund window duration per tier (premium=90d, gold=60d, standard=30d), maximum refund amounts per tier, date arithmetic against purchase date, and any special-case overrides (seasonal policies, loyalty exceptions, escalation paths). This logic is opaque domain code — policy rules, tier thresholds, and window calculations are organization-specific and change independently of the workflow structure.

**Observations**: The virtual handler handles the composable work — precondition assertion via jaq boolean filter, approval path routing via jq `if-elif-else` chain. The assert's filter directly evaluates the validation status — no separate evaluate capability needed. The second transform uses jq's conditional chain to implement first-match rule logic: the `if-elif-else` pattern naturally expresses the same semantics as the previous `evaluate_rules` with `first_match: true`, but using standard jq syntax. The domain handler handles what grammars cannot express: organization-specific policy window evaluation with tier-dependent thresholds.

### Handler: get_manager_approval / approve_refund

**Internal logic**:
1. Check if approval required
2. If yes: generate manager ID, set approval with manager metadata
3. If no: set auto_approved = true, approver = "system"

**Grammar proposal**:

```yaml
grammar: Transform
compose:
  - capability: transform
    output:
      type: object
      required: [approved, approver, approval_type]
      properties:
        approved: { type: boolean }
        approver: { type: string }
        approval_type: { type: string, enum: [manager, auto] }
    filter: |
      .deps.evaluate_refund_routing
      | {
          approved: true,
          approver: (if .requires_approval then "mgr_generated" else "system" end),
          approval_type: (if .requires_approval then "manager" else "auto" end)
        }
```

**Observations**: A single `transform` replaces the previous evaluate then compute chain. The jaq filter navigates to the dependency output and derives all three fields using `if-then-else` — boolean determination and value derivation happen in the same expression. This handler is still very simple, and the composition adds overhead over a single function with a branch. Keep as domain handler.

### Handler: execute_refund_workflow

**Internal logic**:
1. Verify approval obtained
2. Generate delegation IDs
3. Set target namespace and workflow for cross-namespace delegation
4. Mark task as delegated

**Grammar proposal**: **Not a composition candidate.** Cross-namespace task delegation is an orchestration concern. The handler creates a reference to a task in another namespace — this is fundamentally about workflow coordination, not capability execution. The handler should remain a domain handler.

### Handler: update_ticket_status

**Internal logic**:
1. Verify delegation completed
2. Update ticket status from "in_progress" to "resolved"
3. Build resolution note with refund details

**Grammar proposal**:

```yaml
grammar: Persist
compose:
  - capability: assert
    filter: '.deps.execute_refund_workflow.delegation_status == "completed"'
    error: "Task delegation must be completed before updating ticket"

  - capability: transform
    output:
      type: object
      required: [resolution_note, refund_id, correlation_id]
      properties:
        resolution_note: { type: string }
        refund_id: { type: string }
        correlation_id: { type: string }
        estimated_arrival: { type: string }
    filter: |
      {
        resolution_note: "Refund of \(.deps.execute_refund_workflow.refund_amount) processed via \(.deps.execute_refund_workflow.delegated_task_id)",
        refund_id: .deps.execute_refund_workflow.refund_id,
        correlation_id: .deps.execute_refund_workflow.correlation_id,
        estimated_arrival: .deps.execute_refund_workflow.estimated_arrival
      }

  - capability: persist
    config:
      resource:
        type: database
        entity: tickets
      constraints:
        key: ticket_id
        operation: update
      validate_success:
        status: { equals: resolved }
      result_shape: [ticket_id, status, resolution_note, updated_at]
    data: |
      {
        status: "resolved",
        customer_notified: true,
        resolution_note: .prev.resolution_note,
        refund_id: .prev.refund_id,
        correlation_id: .prev.correlation_id
      }
    checkpoint: true
```

**Observations**: Three capabilities — assert, transform, persist. The assert's jaq filter directly evaluates the delegation status condition against the dependency output — no separate evaluate capability needed. The `transform` constructs the resolution note using jq's string interpolation (`\(...)`) and projects the relevant fields from the dependency — this replaces the previous `reshape` with `concat()` from the custom expression language. The `persist` expresses the full (action, resource, context) triple: update the tickets entity keyed by ticket_id, write specific data via jaq filter including the transformed resolution note, validate that status is resolved, and return a specific result shape. The assert then transform then persist pattern is common in case management systems.

---

## Emerging Vocabulary: Capabilities That Appear Repeatedly

| Capability | Occurrences | Grammar(s) | Description |
|-----------|-------------|------------|-------------|
| `transform` | 18 | All | Unified pure data transformation: projection, arithmetic, boolean derivation, conditional logic, grouping, ranking — all via jaq filters with JSON Schema output contracts |
| `persist` | 8 | Persist | Write data to resource target: typed envelope (resource, constraints, validate_success, result_shape) + jaq `data` filter |
| `validate` | 5 | Validate, Persist | Boundary gate: JSON Schema conformance, type coercion, attribute filtering, configurable failure modes |
| `assert` | 5 | Validate, Persist | Execution gate: jaq boolean `filter` + `error` message |
| `acquire` | 5 | Acquire, Validate, Persist | Read data from resource source: typed envelope (resource, constraints, result_shape) + jaq `params` filter |
| `emit` | 3 | Emit | Fire domain event: typed envelope (event_name, delivery_mode, condition) + jaq `payload` filter + payload schema |

**All capabilities express the (action, resource, context) triple.** The vocabulary divides into two groups:

**Pure data capabilities** (no side effects):

| Capability | Role | Mechanism |
|-----------|------|-----------|
| `transform` | Reshape, compute, evaluate, classify, group, rank | jaq filter + JSON Schema output contract |
| `validate` | Verify conformance at trust boundaries | JSON Schema + coercion/filtering/failure config |
| `assert` | Gate execution (proceed or fail) | jaq boolean filter + error message |

The `transform` capability subsumes what was previously six separate capabilities (`reshape`, `compute`, `evaluate`, `evaluate_rules`, `group_by`, `rank`). The semantic distinctions between these operations — "reorganize shape" vs. "derive numeric values" vs. "determine booleans" vs. "match rules" — are conventions for how the jaq filter is written, not different execution models. JSON Schema declares what a step promises to produce; the jaq filter declares how.

**Action capabilities** (side-effecting, typed Rust envelope + jaq data mapping):

| Capability | Action | Config Envelope | jaq Field |
|-----------|--------|----------------|-----------|
| `persist` | Write data to target | `resource`, `constraints`, `validate_success`, `result_shape` | `data` |
| `acquire` | Read data from source | `resource`, `constraints`, `validate_success`, `result_shape` | `params` |
| `emit` | Fire domain event | `event_name`, `event_version`, `delivery_mode`, `condition`, `schema` | `payload` |

The three action capabilities express all side-effecting operations through the (action, resource, context) triple — domain specificity comes from the configuration envelope, not capability naming. The `emit` capability maps directly onto Tasker's existing `DomainEvent` system — it fires domain events with payloads, not emails or notifications. Content construction and delivery are downstream consumer responsibilities.

**What stays outside grammar scope**: Operations where the "how" is irreducibly domain-specific (`fraud_check`, `payment_gateway_charge`, `gateway_refund`, `inventory_reserve`, `classify_customer`, `generate_credentials`, `check_policy_window`) remain as traditional domain handlers.

---

## Assessment: Which Handlers Benefit from Grammar Composition?

### Strong Candidates (configurable, multi-step internal logic)

| Handler | Why | Pattern |
|---------|-----|---------|
| All extract_* handlers | Same structure, different config (source, fields) | acquire, transform |
| All transform_* handlers | Same structure, different dimensions/metrics | transform (single) |
| aggregate_metrics | Cross-source metric derivation | transform (single) |
| generate_insights | Rule evaluation + scoring — pure configuration | transform (single) |
| validate_cart | Multi-step validation pipeline | validate, transform |
| create_user_account | Trust boundary then persist with constraints | validate, persist |
| create_order | Multi-dependency assembly then persist | transform, persist |
| setup_billing | Config lookup then conditional logic then persist | acquire, transform, persist |
| initialize_preferences | Config defaults then merge then persist | acquire, transform, persist |
| update_payment_records | Assert then double-entry persist | assert, persist, persist |
| update_ticket_status | Assert then transform then persist | assert, transform, persist |
| update_user_status | Multi-dependency then conditional persist | transform, persist |
| send_confirmation | Gather then fire domain event | transform, emit |
| send_welcome_sequence | Merge then conditional rules then fire domain event | transform, emit |
| notify_customer | Gather then fire domain event | transform, emit |

### Should Stay as Traditional Domain Handlers

| Handler | Why — (action, resource, context) cannot be deterministically expressed |
|---------|-----|
| process_payment | `payment_gateway_charge` — provider-specific API, PCI compliance, nuanced error handling |
| update_inventory | `inventory_reserve` — warehouse topology, distributed reservation protocols |
| validate_refund_request | Split: virtual handler validates + acquires; domain handler classifies customer (organization-specific) |
| check_refund_policy | Split: virtual handler asserts preconditions + routes approval; domain handler evaluates policy window (organization-specific) |
| execute_refund_workflow | Cross-namespace delegation — orchestration concern |
| get_manager_approval | Single conditional branch — too simple for composition |

### Composable via Virtual Handler Wrappers

| Handler Category | Wrapper Type | Grammar Expresses | Wrapper Provides |
|------------------|-------------|-------------------|------------------|
| All decision handlers | `DecisionCompositionHandler` | Routing logic as JSON (`route`, `steps`) | Translation to `DecisionPointOutcome` |
| All batch analyzers | `BatchAnalyzerCompositionHandler` | Partitioning logic as JSON (`batch_size`, `worker_count`, `cursors`) | Translation to `BatchProcessingOutcome` |
| All batch workers (per-chunk body) | `BatchWorkerCompositionHandler` | Per-chunk transform/persist composition | Cursor iteration + checkpoint_yield lifecycle |
| Aggregators (WithBatches path) | `CompositionHandler` (standard) | Multi-parent merge + metric derivation | Already expressible as standard composition |

Note: aggregator scenario detection (NoBatches vs WithBatches routing) remains outside grammar scope — this is an orchestration concern. The WithBatches aggregation path itself, which merges batch results and derives summary metrics, is already expressible as a standard composition (see `advanced-patterns.md`).

The virtual handler wrapper pattern extends grammar's reach to decision, batch analyzer, and batch worker steps without compromising grammar purity. The grammar composition produces JSON matching a declared output schema; the wrapper in `tasker-worker` translates that JSON to the appropriate orchestration protocol type (`DecisionPointOutcome`, `BatchProcessingOutcome`, etc.). The `tasker-grammar` crate remains entirely unaware of orchestration types. See `transform-revised-grammar.md` §"Open Design: Decision and Batch Outcome Expression" for the full wrapper architecture.

**The dividing line**: handlers whose internal logic can be fully expressed as sequences of (action, resource, context) triples are grammar-composable via `CompositionHandler`. Handlers whose *output* must be orchestration protocol types are *also* composable when a virtual handler wrapper provides the protocol translation — the grammar expresses the logic, the wrapper bridges to the protocol. Handlers containing irreducibly opaque domain operations — where the "how" cannot be parameterized through a generic configuration surface — remain as traditional domain handler code.

---

*This case study should be read alongside `actions-traits-and-capabilities.md` for the grammar architecture, `transform-revised-grammar.md` for the 6-capability model design rationale, `grammar-trait-boundary.md` for the trait design, `composition-validation.md` for how these compositions would be validated, and `virtual-handler-dispatch.md` for how virtual handlers execute within the worker infrastructure.*
