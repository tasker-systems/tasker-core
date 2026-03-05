# Case Study: Grammar Proposals for Contrib Workflow Handlers

*Expressing the internal logic of each handler as a grammar composition*

*March 2026 — Research Spike*

---

## Approach

Each contrib example workflow has 4-8 handlers, each implemented identically across Rust, Python, Ruby, and TypeScript. For each handler, we analyze the internal logic and propose a grammar composition that would express the same operations using capability vocabularies. Where a handler is too simple or too domain-specific for composition, we note why.

The grammar categories used here follow the taxonomy from `actions-traits-and-capabilities.md`: **Acquire** (retrieve/compute), **Transform** (reshape/enrich), **Validate** (check/assert), **Persist** (write/commit), **Emit** (domain event publication).

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
    input_mapping: { type: task_context, path: cart_items }

  - capability: compute
    config:
      operations:
        - select: "items[*]"
          derive:
            line_total: "quantity * unit_price"
          cast: decimal(2)
        - select: "$"
          derive:
            subtotal: "sum(items[*].line_total)"
            tax: "subtotal * 0.0875"
          cast: decimal(2)
    input_mapping: { type: previous }

  - capability: evaluate
    config:
      expressions:
        free_shipping: "subtotal >= 75.00"
    input_mapping: { type: previous }

  - capability: compute
    config:
      operations:
        - select: "$"
          derive:
            shipping: "select(free_shipping, 0, 9.99)"
            total: "subtotal + tax + shipping"
          cast: decimal(2)
    input_mapping: { type: previous }
```

**Observations**: Four capabilities — validate (boundary gate), compute (arithmetic), evaluate (boolean), compute (final derivation). The `validate` capability is the trust boundary: it asserts that cart items have required fields with correct types, permissively coerces (e.g., string `"2"` → integer `2`), passes unknown fields through (items may have extra attributes), and collects all violations rather than failing on the first. The first `compute` handles pure arithmetic: per-item line totals and aggregate subtotal/tax. The `evaluate` capability determines the boolean `free_shipping` field. The second `compute` references that evaluated boolean via `select()` to derive the shipping cost, then sums the total. No checkpoint needed (no mutations).

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
  - capability: reshape
    config:
      fields:
        total: "validate_cart.total"
        payment_token: "$.payment_token"
    input_mapping: { type: task_context }

  - capability: validate
    config:
      schema:
        total: { type: number, required: true, min: 0.01, max: 10000 }
        payment_token: { type: string, required: true }
      coercion: strict
      unknown_fields: drop
      on_failure: fail
    input_mapping: { type: previous }

  - capability: evaluate
    config:
      expressions:
        is_test_decline: "payment_token in ['tok_test_declined', 'tok_test_insufficient_funds']"
    input_mapping: { type: previous }

  - capability: assert
    config:
      conditions:
        - name: not_test_decline
          none: [is_test_decline]
          error: "Payment token rejected"
      on_failure: fail
    input_mapping: { type: previous }
```

**Step 2: `process_payment` (domain handler — traditional curated code)**:

Depends on `prepare_payment`. Consumes the validated, asserted `result_schema` from step 1. Handles gateway-specific API calls, processing fee calculation, PCI compliance, nuanced error handling (declined vs. insufficient funds vs. timeout), and idempotency key management. This is irreducibly domain-specific logic.

**Observations**: The virtual handler (step 1) does the grammar-composable work: reshape from dependencies, validate the payment data at the trust boundary, evaluate whether the token is a test/decline token, and assert the precondition. Its `result_schema` becomes the input to the domain handler (step 2). The TaskTemplate DAG freely mixes virtual handlers and domain handlers — the composition boundary is the `result_schema` of the StepDefinition. No capability within a grammar composition may invoke domain handler logic.

### Handler: update_inventory

**Internal logic**:
1. Extract validated items from cart dependency
2. For each item: select warehouse based on quantity, generate reservation ID
3. Simulate stock levels, calculate new stock after reservation
4. Track total reserved, set expiry (30 min)

**Grammar proposal**:

The inventory reservation is a domain operation. The TaskTemplate DAG could optionally split this into a virtual handler that prepares data, followed by the domain handler that performs the reservation:

**Option A: Single domain handler step** (current approach — the reshape is trivial enough to stay in handler code)

**Option B: Split into two steps**:

Step 1: `prepare_inventory_update` (virtual handler):

```yaml
grammar: Transform
compose:
  - capability: reshape
    config:
      fields:
        validated_items: "validate_cart.validated_items"
        item_skus: "validate_cart.validated_items[*].sku"
        item_quantities: "validate_cart.validated_items[*].quantity"
    input_mapping: { type: task_context }
```

Step 2: `reserve_inventory` (domain handler): Consumes the reshaped data and handles warehouse selection logic, distributed reservation protocols, stock management, and TTL enforcement.

**Observations**: This handler is simple enough that Option A (single domain handler) is preferable. The reshape is a single selector-path projection — creating a virtual handler step for it adds DAG complexity without meaningful benefit. The grammar composition boundary should be drawn where it adds value, not applied mechanically to every step.

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
  - capability: reshape
    config:
      fields:
        validated_items: "validate_cart.validated_items"
        subtotal: "validate_cart.subtotal"
        tax: "validate_cart.tax"
        shipping: "validate_cart.shipping"
        total: "validate_cart.total"
        payment_id: "process_payment.payment_id"
        transaction_id: "process_payment.transaction_id"
        updated_products: "update_inventory.updated_products"
    input_mapping: { type: task_context }

  - capability: evaluate
    config:
      expressions:
        expedited_shipping: "shipping > 0"
    input_mapping: { type: previous }

  - capability: compute
    config:
      operations:
        - select: "$"
          derive:
            estimated_delivery_days: "select(expedited_shipping, 3, 5)"
            estimated_delivery: "date_add(now(), estimated_delivery_days, 'days')"
    input_mapping: { type: previous }

  - capability: persist
    config:
      resource:
        type: database
        entity: orders
      data:
        order_ref: "$.order_ref"
        customer_email: "$.customer_email"
        items: "$.validated_items"
        total: "$.total"
        estimated_delivery: "$.estimated_delivery"
      constraints:
        unique_key: order_ref
        id_pattern: "ORD-{YYYYMMDD}-{hex}"
      validate_success:
        order_id: { type: string, required: true }
      result_shape:
        fields: [order_id, order_ref, created_at, estimated_delivery]
    input_mapping: { type: previous }
    checkpoint: true
```

**Observations**: Four capabilities — reshape (project from three dependencies), evaluate (determine shipping speed), compute (derive delivery estimate), persist. The `reshape` capability uses selector-paths to project fields from three upstream dependency outputs into one flat structure. The `persist` capability expresses the (action, resource, context) triple: write to the orders entity with uniqueness constraints, validate that an order_id was generated, and return a specific result shape for downstream capabilities.

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
  - capability: reshape
    config:
      fields:
        order_number: "create_order.order_number"
        total: "create_order.total"
        estimated_delivery: "create_order.estimated_delivery"
        customer_email: "create_order.customer_email"
    input_mapping: { type: task_context }

  - capability: emit
    config:
      event_name: "order.confirmed"
      event_version: "1.0"
      delivery_mode: durable
      condition: success
      payload:
        order_number: "$.order_number"
        total: "$.total"
        estimated_delivery: "$.estimated_delivery"
        customer_email: "$.customer_email"
      schema:
        type: object
        required: [order_number, total, customer_email]
        properties:
          order_number: { type: string }
          total: { type: number }
          customer_email: { type: string }
          estimated_delivery: { type: string }
    input_mapping: { type: previous }
```

**Observations**: This is an `Emit` grammar — the outcome is a domain event. Two capabilities: reshape (gather from dependencies), emit (fire domain event). The `emit` fires an `order.confirmed` event with the order data as payload. What happens downstream — sending a confirmation email, updating a CRM, triggering a webhook — is entirely outside the grammar's concern. Some downstream consumer reads from the `{namespace}_domain_events` PGMQ queue and acts on the event. This is much cleaner than the previous model where the grammar tried to construct email subjects and bodies — content construction is the consumer's responsibility.

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
        params_from: [source, date_range_start, date_range_end]
      constraints:
        timeout_ms: 5000
      validate_success:
        status: { in: [200] }
      result_shape:
        fields: [data.sales_records]
    input_mapping: { type: task_context }

  - capability: reshape
    config:
      fields:
        records: "$.data.sales_records[*].{date, sku, quantity, unit_price}"
    input_mapping: { type: previous }

  - capability: compute
    config:
      operations:
        - select: "records[*]"
          derive:
            revenue: "quantity * unit_price"
          cast: decimal(2)
        - select: "$"
          derive:
            total_revenue: "sum(records[*].revenue)"
            total_quantity: "sum(records[*].quantity)"
            record_count: "count(records[*])"
    input_mapping: { type: previous }
```

**Observations**: Three capabilities — acquire, reshape, compute. The `acquire` is the I/O boundary (true acquisition). The `reshape` projects the relevant fields from the API response — this is the extraction step, using selector-paths to sub-select the record attributes of concern from the raw response. The `compute` handles per-record enrichment and whole-input aggregation. This three-step pattern (acquire → reshape → compute) separates I/O, projection, and derivation cleanly.

All three extract handlers have the same structure (fetch + reshape + compute) but different configurations. This is exactly where grammar composition shines: **same grammar, same capabilities, different config**.

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
  - capability: group_by
    config:
      dimensions: [category]
      metrics:
        revenue: { sum: revenue }
        quantity: { sum: quantity }
        transaction_count: { count: "*" }
        avg_revenue: { formula: "revenue / transaction_count" }
    input_mapping: { type: previous }

  - capability: rank
    config:
      by: revenue
      direction: desc
      output_field: top_category
      limit: 1
    input_mapping: { type: previous }

  - capability: compute
    config:
      operations:
        - select: "$"
          derive:
            total_revenue: "sum(groups[*].revenue)"
            total_categories: "count(groups[*])"
    input_mapping:
      type: merged
      sources:
        - { type: step_output, index: 0 }
        - { type: step_output, index: 1 }
```

**Observations**: Three capabilities — group, rank, compute. This is a natural `Transform` grammar. The `group_by` and `rank` capabilities are distinct data operations (reshaping vs. ordering). The `compute` capability handles the final aggregation using the same selector-path + expression model as elsewhere. Different transform handlers would use the same capabilities with different dimension/metric configurations.

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
  - capability: reshape
    config:
      fields:
        total_revenue: "transform_sales.total_revenue"
        record_count: "transform_sales.record_count"
        total_quantity: "transform_inventory.total_quantity"
        reorder_alerts: "transform_inventory.reorder_alerts"
        total_customers: "transform_customers.total_customers"
        total_ltv: "transform_customers.total_ltv"
    input_mapping: { type: task_context }

  - capability: compute
    config:
      operations:
        - select: "$"
          derive:
            revenue_per_customer: "total_revenue / total_customers"
            inventory_turnover: "total_revenue / total_quantity"
          cast: decimal(2)
    input_mapping: { type: previous }
```

**Observations**: Two capabilities — reshape (project from three dependency outputs into flat structure), compute (derive cross-source ratios). The `reshape` projects exactly the fields needed from each upstream transform, and the `compute` expressions reference the reshaped field names directly — no nested path traversal needed because `reshape` has already flattened the structure.

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
  - capability: evaluate_rules
    config:
      rules:
        - condition: "revenue_per_customer < 500"
          insight: { type: revenue, severity: medium, recommendation: "Focus on upselling" }
        - condition: "reorder_alerts > 5"
          insight: { type: inventory, severity: high, recommendation: "Immediate reorder needed" }
        - condition: "avg_ltv > 3000"
          insight: { type: customer, severity: low, recommendation: "Retention program effective" }
    input_mapping: { type: previous }

  - capability: evaluate
    config:
      expressions:
        has_best_source: "has(best_source)"
        high_volume: "total_records > 50"
        low_stock_warning: "low_stock_alerts > 5"
    input_mapping:
      type: merged
      sources:
        - { type: step_output, index: 0 }
        - { type: previous }

  - capability: compute
    config:
      operations:
        - select: "$"
          derive:
            health_score_raw: "75 + select(has_best_source, 5, 0) + select(high_volume, 10, 0) + select(low_stock_warning, -15, 0)"
            health_score: "clamp(health_score_raw, 0, 100)"
          cast: integer
    input_mapping: { type: previous }

  - capability: evaluate
    config:
      expressions:
        health_rating:
          case:
            - when: "health_score >= 80"
              then: "excellent"
            - when: "health_score >= 60"
              then: "good"
            - when: "health_score >= 40"
              then: "fair"
            - default: "needs_improvement"
    input_mapping: { type: previous }
```

**Observations**: Four capabilities — rule evaluation, evaluate (boolean factors), compute (score calculation), evaluate (rating classification). The `evaluate_rules` capability maps conditions to insights. The first `evaluate` determines three boolean factors (has best source, high volume, low stock). `compute` then references those booleans via `select()` to calculate the weighted health score. The second `evaluate` classifies the score into a rating using a case expression — this is a selection operation (mapping a numeric range to a label), which is evaluability, not computation.

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
    input_mapping: { type: task_context }

  - capability: persist
    config:
      resource:
        type: database
        entity: user_accounts
      data:
        email: "$.email"
        full_name: "$.full_name"
        username: "$.username"
        status: pending_verification
      constraints:
        unique_key: email
      validate_success:
        user_id: { type: string, required: true }
      result_shape:
        fields: [user_id, email, status, created_at]
    input_mapping: { type: previous }
    checkpoint: true
```

**Observations**: Two capabilities — validate, persist. The `validate` capability is the trust boundary: strict coercion (exact types required for user registration), drop unknown fields (defensive filtering at account creation boundary), and collect all violations for user-facing error reporting. The `persist` expresses the full (action, resource, context) triple: write to user_accounts with uniqueness constraint on email, validate that a user_id was generated, and return a specific result shape. **Note**: The original handler also generates credentials (user_id, API key, verification token) — this is security-sensitive logic that should remain as traditional handler code. A grammar-composed version would handle the validate → persist flow, while credential generation would either be a pre-step in a domain handler or a separate workflow step.

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
      result_shape:
        fields: [price, trial_days, api_limit, storage_gb]
    input_mapping: { type: previous }

  - capability: evaluate
    config:
      expressions:
        billing_required: "price > 0"
    input_mapping: { type: previous }

  - capability: compute
    config:
      operations:
        - select: "$"
          derive:
            billing_status: "select(billing_required, 'active', 'skipped_free_plan')"
            next_billing_date: "select(billing_required, date_add(now(), 30, 'days'), null)"
    input_mapping: { type: previous }

  - capability: persist
    config:
      resource:
        type: database
        entity: billing_profiles
      data:
        user_id: "$.user_id"
        plan: "$.plan"
        billing_status: "$.billing_status"
        next_billing_date: "$.next_billing_date"
        trial_days: "$.trial_days"
      constraints:
        id_prefix: "bill_"
      validate_success:
        billing_id: { type: string, required: true }
      result_shape:
        fields: [billing_id, billing_status, next_billing_date]
    input_mapping: { type: previous }
    checkpoint: true
```

**Observations**: Four capabilities — acquire (config lookup), evaluate, compute, persist. The `acquire` expresses (action, resource, context): read billing tier configuration by plan key, requiring a match, and returning specific fields. The `evaluate` capability determines the boolean `billing_required` field. The `compute` capability then references it via `select()` to derive billing status and next billing date. The `persist` writes the billing profile with all the (action, resource, context) surface: resource target, data mapping, constraints, success validation, and result shape.

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
      result_shape:
        fields: [notifications, ui_settings, feature_flags]
    input_mapping: { type: previous }

  - capability: reshape
    config:
      fields:
        notifications: "$.notifications"
        ui_settings: "$.ui_settings"
        feature_flags: "$.feature_flags"
        custom_notifications: "task_context.metadata.custom_preferences.notifications"
        custom_ui: "task_context.metadata.custom_preferences.ui_settings"
      merge_strategy: overlay  # later fields override earlier when paths overlap
    input_mapping:
      type: merged
      sources:
        - { type: step_output, index: 0 }
        - { type: task_context, path: metadata.custom_preferences }

  - capability: persist
    config:
      resource:
        type: database
        entity: user_preferences
      data:
        user_id: "$.user_id"
        notifications: "$.notifications"
        ui_settings: "$.ui_settings"
        feature_flags: "$.feature_flags"
      validate_success:
        preferences_id: { type: string, required: true }
      result_shape:
        fields: [preferences_id, defaults_applied_count, customizations_count]
    input_mapping: { type: previous }
    checkpoint: true
```

**Observations**: The `acquire` reads plan defaults from a config store — expressing (action, resource, context) clearly. The `reshape` capability handles the deep merge pattern — projecting fields from both the defaults (acquire output) and customizations (task context), with an overlay merge strategy. The `persist` writes the merged preferences with full (action, resource, context): resource target, data mapping, success validation, and result shape. The composition captures the handler's logic (acquire defaults → reshape with overlay → persist) cleanly.

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
  - capability: reshape
    config:
      fields:
        user_id: "create_user_account.user_id"
        email: "create_user_account.email"
        trial_end: "setup_billing.trial_end"
        email_updates: "initialize_preferences.notifications.email_updates"
    input_mapping: { type: task_context }

  - capability: evaluate_rules
    config:
      rules:
        - condition: "email_updates == true"
          emit: { type: welcome_email, channel: email }
        - condition: "true"
          emit: { type: verification_email, channel: email }
        - condition: "true"
          emit: { type: onboarding_guide, channel: in_app }
        - condition: "trial_end != null"
          emit: { type: trial_notification, channel: email }
    input_mapping: { type: previous }

  - capability: emit
    config:
      event_name: "user.welcome_sequence"
      event_version: "1.0"
      delivery_mode: durable
      condition: success
      payload:
        user_id: "$.user_id"
        email: "$.email"
        trial_end: "$.trial_end"
        notifications: "$.matched_rules"
      schema:
        type: object
        required: [user_id, email, notifications]
        properties:
          user_id: { type: string }
          email: { type: string }
          notifications: { type: array }
    input_mapping: { type: previous }
```

**Observations**: Three capabilities — reshape (project relevant fields from three dependencies), evaluate rules (determine which notifications to send), emit (fire domain event). The `reshape` flattens the three dependency outputs into the exact fields needed for rule evaluation — note how the evaluate_rules conditions reference simple field names (`email_updates`, `trial_end`) rather than nested dependency paths. The `emit` fires a `user.welcome_sequence` domain event with the matched notification rules as payload. Downstream consumers decide how to deliver each notification (email, in-app, etc.) — the grammar's job ends at expressing what happened and what data is relevant.

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
  - capability: reshape
    config:
      fields:
        user_id: "create_user_account.user_id"
        email: "create_user_account.email"
        plan: "create_user_account.plan"
        billing_id: "setup_billing.billing_id"
        preferences_count: "initialize_preferences.preferences_count"
        channels_used: "send_welcome_sequence.channels_used"
        messages_sent: "send_welcome_sequence.messages_sent"
    input_mapping: { type: task_context }

  - capability: evaluate
    config:
      expressions:
        include_billing: "plan != 'free'"
    input_mapping: { type: previous }

  - capability: persist
    config:
      resource:
        type: database
        entity: user_accounts
      data:
        status: active
        billing_id: "select(include_billing, $.billing_id, null)"
        preferences_count: "$.preferences_count"
        channels_used: "$.channels_used"
        messages_sent: "$.messages_sent"
      constraints:
        key: user_id
        operation: update
      validate_success:
        status: { equals: active }
      result_shape:
        fields: [user_id, status, updated_at]
    input_mapping: { type: previous }
    checkpoint: true
```

**Observations**: Three capabilities — reshape (project from four dependencies), evaluate (determine conditional inclusion), persist. The `reshape` projects relevant fields from all upstream dependencies into a flat structure. The `evaluate` determines whether billing should be included. The `persist` writes the status update with full (action, resource, context): the resource target (user_accounts), the data to write (using `select()` to conditionally include billing_id based on the evaluate output), update constraints, success validation, and result shape.

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
    input_mapping: { type: task_context }

  - capability: acquire
    config:
      resource:
        type: api
        endpoint: payment_service
        key_field: payment_id
      constraints:
        required: true
      result_shape:
        fields: [original_amount, payment_method, payment_status]
    input_mapping: { type: previous }

  - capability: compute
    config:
      operations:
        - select: "$"
          derive:
            refund_percentage: "(refund_amount / original_amount) * 100"
          cast: decimal(2)
    input_mapping: { type: previous }
```

**Step 2: `check_fraud_score` (domain handler — traditional curated code)**:

Depends on `validate_payment_data`. Consumes the validated, computed `result_schema` from step 1. Runs the proprietary fraud scoring algorithm — hash-based scoring, model inference, or third-party API integration. The fraud check's "how" is irreducibly domain-specific (algorithm selection, seed field handling, threshold behavior, rejection semantics).

**Observations**: The virtual handler (step 1) does the grammar-composable work: validate refund amount at the trust boundary, acquire the original payment data, and compute the refund percentage. The domain handler (step 2) consumes this prepared data and runs the opaque fraud scoring. The `result_schema` of step 1 becomes the upstream dependency for step 2. This respects the architectural boundary: no capability within a grammar composition invokes domain handler logic.

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
  - capability: evaluate
    config:
      expressions:
        payment_validated: "payment_status == 'completed'"
    input_mapping: { type: previous }

  - capability: assert
    config:
      conditions:
        - name: payment_ready
          all: [payment_validated]
          error: "Payment not validated"
      on_failure: fail
    input_mapping: { type: previous }
```

**Step 2: `process_gateway_refund` (domain handler — traditional curated code)**:

Depends on `validate_refund_eligibility`. Consumes the validated, asserted `result_schema` from step 1. Handles Stripe/provider-specific API calls, refund ID generation, authorization code computation, settlement delay calculation, and gateway response recording. This is irreducibly domain-specific — every payment provider has different refund APIs, error codes, and idempotency semantics.

**Observations**: The virtual handler (step 1) gates execution: evaluate the payment status, assert the precondition. Its `result_schema` passes through the validated payment data to the domain handler (step 2). The evaluate → assert pattern is pure grammar composition. The gateway interaction is pure domain handler code. The TaskTemplate DAG connects them through dependency declarations.

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
  - capability: evaluate
    config:
      expressions:
        refund_processed: "refund_status == 'processed'"
    input_mapping: { type: previous }

  - capability: assert
    config:
      conditions:
        - name: refund_confirmed
          all: [refund_processed]
          error: "Refund must be processed before updating records"
      on_failure: fail
    input_mapping: { type: previous }

  - capability: persist
    config:
      resource:
        type: database
        entity: ledger_entries
      data:
        entries:
          - { type: debit, account: refunds_payable, amount: "$.refund_amount" }
          - { type: credit, account: accounts_receivable, amount: "$.refund_amount" }
      constraints:
        fiscal_period_format: "{year}-{month}"
        idempotency_key: "$.refund_id"
      validate_success:
        record_id: { type: string, required: true }
        journal_id: { type: string, required: true }
      result_shape:
        fields: [record_id, journal_id, fiscal_period, created_at]
    input_mapping: { type: previous }
    checkpoint: true

  - capability: persist
    config:
      resource:
        type: database
        entity: reconciliation_status
      data:
        status: pending
        refund_id: "$.refund_id"
        journal_id: "$.journal_id"
      validate_success:
        reconciliation_id: { type: string, required: true }
      result_shape:
        fields: [reconciliation_id, status]
    input_mapping: { type: previous }
```

**Observations**: Four capabilities — evaluate, assert, persist (ledger entries), persist (reconciliation status). The `evaluate` → `assert` pair gates execution with composable boolean evaluation. Both `persist` capabilities express the full (action, resource, context) triple. The ledger entry persist specifies the double-entry data structure, idempotency constraints, and expects record/journal IDs in the result. The reconciliation persist is a simple status write. Both are `persist` with different resource configurations — domain specificity comes from the configuration envelope, not capability naming.

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
  - capability: reshape
    config:
      fields:
        customer_email: "coalesce($.customer_email, validate_payment_eligibility.customer_email)"
        refund_id: "process_gateway_refund.refund_id"
        amount_refunded: "process_gateway_refund.amount_refunded"
        estimated_arrival: "process_gateway_refund.estimated_arrival"
    input_mapping: { type: task_context }

  - capability: emit
    config:
      event_name: "refund.processed"
      event_version: "1.0"
      delivery_mode: durable
      condition: success
      payload:
        refund_id: "$.refund_id"
        amount_refunded: "$.amount_refunded"
        customer_email: "$.customer_email"
        estimated_arrival: "$.estimated_arrival"
      schema:
        type: object
        required: [refund_id, amount_refunded, customer_email]
        properties:
          refund_id: { type: string }
          amount_refunded: { type: number }
          customer_email: { type: string }
          estimated_arrival: { type: string }
    input_mapping: { type: previous }
```

**Observations**: Same Emit pattern as e-commerce confirmation — reshape, emit. The `reshape` uses `coalesce()` to resolve the customer email from multiple sources and projects the refund data. The `emit` fires a `refund.processed` domain event with the relevant data as payload. Downstream consumers (notification service, CRM, audit trail) decide what to do with the event. The composition structure is identical across Emit grammars: reshape gathers the data, emit fires the event. Content construction (email templates, notification formatting) is the downstream consumer's responsibility.

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
    input_mapping: { type: task_context }

  - capability: acquire
    config:
      resource:
        type: api
        endpoint: order_service
        key_field: payment_id
      constraints:
        required: true
      result_shape:
        fields: [original_purchase_date, original_amount]
    input_mapping: { type: previous }

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
  - capability: reshape
    config:
      fields:
        validation_status: "classify_and_enrich_refund.validation_status"
        refund_amount: "classify_and_enrich_refund.refund_amount"
        refund_reason: "classify_and_enrich_refund.refund_reason"
        customer_tier: "classify_and_enrich_refund.customer_tier"
        original_purchase_date: "classify_and_enrich_refund.original_purchase_date"
    input_mapping: { type: task_context }

  - capability: evaluate
    config:
      expressions:
        request_validated: "validation_status == 'passed'"
    input_mapping: { type: previous }

  - capability: assert
    config:
      conditions:
        - name: request_valid
          all: [request_validated]
          error: "Request must be validated before checking refund policy"
      on_failure: fail
    input_mapping: { type: previous }

  - capability: evaluate_rules
    config:
      rules:
        - condition: "refund_amount <= 50"
          result: { approval_path: auto_approved }
        - condition: "refund_reason in auto_approve_reasons AND refund_amount <= 500"
          result: { approval_path: auto_approved }
        - condition: "refund_amount > 500"
          result: { approval_path: manager_review }
        - condition: "true"
          result: { approval_path: standard_review }
      first_match: true
    input_mapping: { type: previous }

result_schema: { refund_amount, refund_reason, customer_tier, original_purchase_date, approval_path }
```

**Step 2: `check_refund_policy`** (domain handler):

Consumes `evaluate_refund_routing.result_schema`. Applies organization-specific tier-based policy rules: refund window duration per tier (premium=90d, gold=60d, standard=30d), maximum refund amounts per tier, date arithmetic against purchase date, and any special-case overrides (seasonal policies, loyalty exceptions, escalation paths). This logic is opaque domain code — policy rules, tier thresholds, and window calculations are organization-specific and change independently of the workflow structure.

**Observations**: The virtual handler handles the composable work — precondition assertion, approval path routing via configurable rules. The domain handler handles what grammars cannot express: organization-specific policy window evaluation with tier-dependent thresholds. The `evaluate_rules` capability appears again in a different context (approval routing vs. insight generation vs. notification selection), confirming it as a core vocabulary capability — configurable rule evaluation with first-match semantics.

### Handler: get_manager_approval / approve_refund

**Internal logic**:
1. Check if approval required
2. If yes: generate manager ID, set approval with manager metadata
3. If no: set auto_approved = true, approver = "system"

**Grammar proposal**:

```yaml
grammar: Acquire
compose:
  - capability: evaluate
    config:
      expressions:
        requires_manager: "requires_approval == true"
    input_mapping: { type: previous }

  - capability: compute
    config:
      operations:
        - select: "$"
          derive:
            approved: true
            approver: "select(requires_manager, generate_id('mgr_'), 'system')"
            approval_type: "select(requires_manager, 'manager', 'auto')"
    input_mapping: { type: previous }
```

**Observations**: Two capabilities — evaluate (boolean determination) and compute (value derivation). This handler is still very simple, and the composition adds overhead over a single function with a branch. Keep as domain handler.

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
  - capability: evaluate
    config:
      expressions:
        task_delegated: "delegation_status == 'completed'"
    input_mapping: { type: previous }

  - capability: assert
    config:
      conditions:
        - name: delegation_confirmed
          all: [task_delegated]
          error: "Task delegation must be completed before updating ticket"
      on_failure: fail
    input_mapping: { type: previous }

  - capability: reshape
    config:
      fields:
        resolution_note: "concat('Refund of ', $.refund_amount, ' processed via ', $.delegated_task_id)"
        refund_id: "$.refund_id"
        correlation_id: "$.correlation_id"
        estimated_arrival: "$.estimated_arrival"
    input_mapping: { type: previous }

  - capability: persist
    config:
      resource:
        type: database
        entity: tickets
      data:
        status: resolved
        customer_notified: true
        resolution_note: "$.resolution_note"
        refund_id: "$.refund_id"
        correlation_id: "$.correlation_id"
      constraints:
        key: ticket_id
        operation: update
      validate_success:
        status: { equals: resolved }
      result_shape:
        fields: [ticket_id, status, resolution_note, updated_at]
    input_mapping: { type: previous }
    checkpoint: true
```

**Observations**: Four capabilities — evaluate, assert, reshape (project fields and construct resolution note), persist. The `evaluate` → `assert` pair gates execution with composable boolean evaluation. The `reshape` uses `concat()` from the expression language to construct the resolution note string. The `persist` expresses the full (action, resource, context) triple: update the tickets entity keyed by ticket_id, write specific data including the reshaped resolution note, validate that status is resolved, and return a specific result shape. The ticket update pattern (evaluate → assert → reshape → persist) is common in case management systems.

---

## Emerging Vocabulary: Capabilities That Appear Repeatedly

| Capability | Occurrences | Grammar(s) | Description |
|-----------|-------------|------------|-------------|
| `reshape` | 12 | All | Selector-path projection/reorganization |
| `evaluate` | 12 | All | Boolean/selection field derivation using shared expression language |
| `compute` | 10 | All | Arithmetic/aggregation/derivation: expressions, numeric casting, content construction |
| `persist` | 8 | Persist | Write data to resource target: (action, resource, context) with typed envelope |
| `validate` | 5 | Validate, Persist | Boundary gate: schema conformance, type coercion, attribute filtering, configurable failure |
| `assert` | 5 | Validate, Persist | Composable execution gate: named conditions with set-logic quantifiers (all/any/none) |
| `acquire` | 5 | Acquire, Validate, Persist | Read data from resource source: (action, resource, context) with typed envelope |
| `evaluate_rules` | 3 | Validate, Transform, Emit | First-match/all-match rule engine (built on evaluate) |
| `emit` | 3 | Emit | Fire domain event: maps to Tasker's `DomainEvent` system (event name, payload, delivery mode, schema) |
| `group_by` | 1 | Transform | Group records by dimension and aggregate |
| `rank` | 1 | Transform | Sort/rank grouped results |

**All capabilities express the (action, resource, context) triple.** The vocabulary divides into two groups:

**Core data operations** (pure, no side effects) share a common expression language infrastructure:

| Capability | Selector-paths | Expressions | Concern |
|-----------|---------------|------------|---------|
| `reshape` | yes | no | Reorganize data shape |
| `evaluate` | yes | boolean/selection | Determine boolean/selection fields |
| `compute` | yes | arithmetic/aggregation | Derive numeric/data fields |
| `validate` | yes (schema) | no | Verify conformance at boundaries |
| `assert` | no (references evaluated fields) | set-logic | Gate execution |
| `evaluate_rules` | yes | boolean (condition matching) | Map conditions to results |
| `group_by` | yes | aggregation | Dimensional aggregation |
| `rank` | yes | comparison | Sorting/ranking |

**Action capabilities** (side-effecting, typed Rust envelope + JSON Schema-flexible config):

| Capability | Action | Config Envelope |
|-----------|--------|----------------|
| `persist` | Write data to target | `resource`, `data`, `constraints`, `validate_success`, `result_shape` |
| `acquire` | Read data from source | `resource`, `constraints`, `validate_success`, `result_shape` |
| `emit` | Fire domain event | `event_name`, `event_version`, `delivery_mode`, `condition`, `payload`, `schema` |

The three action capabilities express all side-effecting operations through the (action, resource, context) triple — domain specificity comes from the configuration envelope, not capability naming. The `emit` capability maps directly onto Tasker's existing `DomainEvent` system — it fires domain events with payloads, not emails or notifications. Content construction and delivery are downstream consumer responsibilities.

**What stays outside grammar scope**: Operations where the "how" is irreducibly domain-specific (`fraud_check`, `payment_gateway_charge`, `gateway_refund`, `inventory_reserve`, `classify_customer`, `generate_credentials`, `check_policy_window`) remain as traditional domain handlers.

---

## Assessment: Which Handlers Benefit from Grammar Composition?

### Strong Candidates (configurable, multi-step internal logic)

| Handler | Why | Pattern |
|---------|-----|---------|
| All extract_* handlers | Same structure, different config (source, fields) | acquire → reshape → compute |
| All transform_* handlers | Same structure, different dimensions/metrics | group_by → rank → compute |
| aggregate_metrics | Cross-source metric derivation | reshape → compute |
| generate_insights | Rule evaluation + scoring — pure configuration | evaluate_rules → evaluate → compute → evaluate |
| validate_cart | Multi-step validation pipeline | validate → compute → evaluate → compute |
| create_user_account | Trust boundary → persist with constraints | validate → persist |
| create_order | Multi-dependency assembly → persist | reshape → evaluate → compute → persist |
| setup_billing | Config lookup → conditional logic → persist | acquire → evaluate → compute → persist |
| initialize_preferences | Config defaults → merge → persist | acquire → reshape → persist |
| update_payment_records | Assert → double-entry persist | evaluate → assert → persist → persist |
| update_ticket_status | Assert → reshape → persist | evaluate → assert → reshape → persist |
| update_user_status | Multi-dependency → conditional persist | reshape → evaluate → persist |
| send_confirmation | Gather → fire domain event | reshape → emit |
| send_welcome_sequence | Merge → evaluate rules → fire domain event | reshape → evaluate_rules → emit |
| notify_customer | Gather → fire domain event | reshape → emit |

### Should Stay as Traditional Domain Handlers

| Handler | Why — (action, resource, context) cannot be deterministically expressed |
|---------|-----|
| process_payment | `payment_gateway_charge` — provider-specific API, PCI compliance, nuanced error handling |
| update_inventory | `inventory_reserve` — warehouse topology, distributed reservation protocols |
| validate_refund_request | Split: virtual handler validates + acquires; domain handler classifies customer (organization-specific) |
| check_refund_policy | Split: virtual handler asserts preconditions + routes approval; domain handler evaluates policy window (organization-specific) |
| execute_refund_workflow | Cross-namespace delegation — orchestration concern |
| get_manager_approval | Single conditional branch — too simple for composition |
| All decision handlers | Runtime step creation — outside grammar scope |
| All batch analyzers | Dynamic step creation — outside grammar scope |
| All batch workers | Cursor/checkpoint lifecycle — different execution model |
| All aggregators | Scenario detection + multi-parent aggregation — tightly coupled to batch lifecycle |

**The dividing line is clear**: handlers whose internal logic can be fully expressed as sequences of (action, resource, context) triples are grammar-composable. Handlers containing opaque domain operations — where the "how" cannot be parameterized through a generic configuration surface — remain as traditional domain handler code.

---

*This case study should be read alongside `actions-traits-and-capabilities.md` for the grammar architecture, `grammar-trait-boundary.md` for the trait design, `composition-validation.md` for how these compositions would be validated, and `virtual-handler-dispatch.md` for how virtual handlers execute within the worker infrastructure.*
