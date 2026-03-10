//! Golden-path workflow fixtures for grammar acceptance testing (TAS-335).
//!
//! Provides three real-world [`CompositionSpec`] compositions and their associated
//! test data. These are the "golden path" test suite — they must pass through the
//! `CompositionExecutor` in Phase 1F (TAS-336) and through the validation tooling
//! in Phase 2D (TAS-345).
//!
//! # Workflows
//!
//! | Workflow | Capabilities | Checkpoints | Origin |
//! |----------|-------------|-------------|--------|
//! | E-commerce order processing | validate, transform (x3), persist, emit | persist, emit | workflow-patterns.md §1 |
//! | Payment reconciliation | acquire, validate, transform (x2), assert, persist | persist | workflow-patterns.md — financial patterns |
//! | Customer onboarding | acquire, validate, transform (x2), persist, emit | persist, emit | workflow-patterns.md — onboarding patterns |
//!
//! # Usage
//!
//! ```
//! use tasker_grammar::fixtures;
//!
//! let (spec, input, acquire_data) = fixtures::ecommerce_order_processing();
//! // spec: CompositionSpec with 6 invocations
//! // input: CompositionInput for the executor
//! // acquire_data: HashMap of entity → fixture records (empty for this workflow)
//! ```
//!
//! # Design
//!
//! Each fixture function returns a tuple of:
//! - `CompositionSpec` — the composition definition
//! - `CompositionInput` — realistic test input data
//! - `HashMap<String, Vec<Value>>` — fixture data for `InMemoryOperations` (acquire)
//!
//! The YAML composition specs in `fixtures/workflows/` are the human-readable
//! reference. The Rust fixtures here are the programmatic equivalent that tests
//! consume directly.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::executor::CompositionInput;
use crate::types::{CapabilityInvocation, CompositionSpec, OutcomeDeclaration};

/// Fixture data bundle returned by each workflow fixture function.
#[derive(Debug, Clone)]
pub struct WorkflowFixture {
    /// The composition specification.
    pub spec: CompositionSpec,
    /// Realistic test input data.
    pub input: CompositionInput,
    /// Fixture data for `InMemoryOperations` acquire lookups, keyed by entity name.
    pub acquire_fixtures: HashMap<String, Vec<Value>>,
}

// ---------------------------------------------------------------------------
// Workflow 1: E-commerce Order Processing
// ---------------------------------------------------------------------------

/// E-commerce order processing composition and test data.
///
/// Pipeline: validate → transform (line items) → transform (totals) →
///           transform (routing rules) → persist → emit
///
/// Exercises: validate, transform (x3), persist, emit
/// Checkpoints: persist (index 4), emit (index 5)
pub fn ecommerce_order_processing() -> WorkflowFixture {
    let spec = CompositionSpec {
        name: Some("ecommerce_order_processing".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Process an e-commerce order: validate cart, compute totals with tax \
                          and shipping, evaluate routing rules, persist the order, and emit a \
                          confirmation event."
                .to_owned(),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "order_id": {"type": "string"},
                    "event_id": {"type": "string"},
                    "event_name": {"type": "string"}
                },
                "required": ["order_id", "event_id", "event_name"]
            }),
        },
        invocations: vec![
            // Step 0: Validate incoming order data
            CapabilityInvocation {
                capability: "validate".to_owned(),
                config: json!({
                    "schema": {
                        "type": "object",
                        "required": ["customer_id", "items", "shipping_address"],
                        "properties": {
                            "customer_id": {"type": "string"},
                            "items": {
                                "type": "array",
                                "minItems": 1,
                                "items": {
                                    "type": "object",
                                    "required": ["sku", "name", "quantity", "unit_price"],
                                    "properties": {
                                        "sku": {"type": "string"},
                                        "name": {"type": "string"},
                                        "quantity": {"type": "integer", "minimum": 1},
                                        "unit_price": {"type": "number", "minimum": 0}
                                    }
                                }
                            },
                            "shipping_address": {
                                "type": "object",
                                "required": ["street", "city", "state", "zip"],
                                "properties": {
                                    "street": {"type": "string"},
                                    "city": {"type": "string"},
                                    "state": {"type": "string"},
                                    "zip": {"type": "string"}
                                }
                            }
                        }
                    }
                }),
                checkpoint: false,
            },
            // Step 1: Reshape cart items — compute line totals
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "filter": "{customer_id: .prev.customer_id, shipping_address: .prev.shipping_address, line_items: [.prev.items[] | {sku: .sku, name: .name, quantity: .quantity, unit_price: .unit_price, line_total: (.quantity * .unit_price)}]}",
                    "output": {"type": "object"}
                }),
                checkpoint: false,
            },
            // Step 2: Compute order totals — subtotal, tax, shipping, grand total
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "filter": ".prev as $order | ($order.line_items | map(.line_total) | add) as $subtotal | ($subtotal * 0.08) as $tax | (if $subtotal >= 100 then 0 else 9.99 end) as $shipping | {customer_id: $order.customer_id, shipping_address: $order.shipping_address, line_items: $order.line_items, subtotal: $subtotal, tax: $tax, shipping: $shipping, total: ($subtotal + $tax + $shipping)}",
                    "output": {"type": "object"}
                }),
                checkpoint: false,
            },
            // Step 3: Evaluate business rules for order routing
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "filter": ".prev as $order | {customer_id: $order.customer_id, shipping_address: $order.shipping_address, line_items: $order.line_items, subtotal: $order.subtotal, tax: $order.tax, shipping: $order.shipping, total: $order.total, routing: {priority: (if $order.total > 500 then \"high\" elif $order.total > 100 then \"normal\" else \"low\" end), warehouse: (if $order.shipping_address.state == \"CA\" or $order.shipping_address.state == \"OR\" or $order.shipping_address.state == \"WA\" then \"west\" else \"east\" end), fraud_review: ($order.total > 1000)}}",
                    "output": {"type": "object"}
                }),
                checkpoint: false,
            },
            // Step 4: Persist the order (checkpoint)
            CapabilityInvocation {
                capability: "persist".to_owned(),
                config: json!({
                    "resource": "orders-db",
                    "data": {
                        "expression": "{customer_id: .prev.customer_id, status: \"confirmed\", subtotal: .prev.subtotal, tax: .prev.tax, shipping: .prev.shipping, total: .prev.total, priority: .prev.routing.priority, warehouse: .prev.routing.warehouse, fraud_review: .prev.routing.fraud_review, line_items: .prev.line_items, shipping_address: .prev.shipping_address}"
                    },
                    "validate_success": {
                        "expression": ".affected_rows > 0"
                    },
                    "result_shape": {
                        "expression": "{order_id: .data.customer_id, total: .data.total, priority: .data.priority, warehouse: .data.warehouse, status: .data.status}"
                    }
                }),
                checkpoint: true,
            },
            // Step 5: Emit order.confirmed event (checkpoint)
            CapabilityInvocation {
                capability: "emit".to_owned(),
                config: json!({
                    "event_name": "order.confirmed",
                    "event_version": "1.0",
                    "resource": "event-bus",
                    "payload": {
                        "expression": "{order_id: .prev.order_id, customer_id: .context.customer_id, total: .prev.total, priority: .prev.priority, warehouse: .prev.warehouse}"
                    },
                    "metadata": {
                        "correlation_id": {
                            "expression": ".context.customer_id"
                        }
                    },
                    "result_shape": {
                        "expression": "{order_id: .prev.order_id, event_id: .data.message_id, event_name: .event_name}"
                    }
                }),
                checkpoint: true,
            },
        ],
    };

    let input = CompositionInput {
        context: json!({
            "customer_id": "cust-12345",
            "items": [
                {"sku": "WIDGET-001", "name": "Premium Widget", "quantity": 3, "unit_price": 29.99},
                {"sku": "GADGET-002", "name": "Super Gadget", "quantity": 1, "unit_price": 149.99},
                {"sku": "CABLE-003", "name": "USB-C Cable", "quantity": 5, "unit_price": 12.99}
            ],
            "shipping_address": {
                "street": "123 Main St",
                "city": "Portland",
                "state": "OR",
                "zip": "97201"
            }
        }),
        deps: json!({}),
        step: json!({"name": "process_order"}),
    };

    // No acquire fixtures needed for this workflow
    let acquire_fixtures = HashMap::new();

    WorkflowFixture {
        spec,
        input,
        acquire_fixtures,
    }
}

/// E-commerce order with invalid data (empty items array) for failure testing.
pub fn ecommerce_order_processing_invalid_empty_items() -> CompositionInput {
    CompositionInput {
        context: json!({
            "customer_id": "cust-99999",
            "items": [],
            "shipping_address": {
                "street": "456 Elm St",
                "city": "Seattle",
                "state": "WA",
                "zip": "98101"
            }
        }),
        deps: json!({}),
        step: json!({"name": "process_order"}),
    }
}

/// E-commerce order missing required shipping_address for failure testing.
pub fn ecommerce_order_processing_invalid_missing_address() -> CompositionInput {
    CompositionInput {
        context: json!({
            "customer_id": "cust-99999",
            "items": [
                {"sku": "WIDGET-001", "name": "Widget", "quantity": 1, "unit_price": 10.00}
            ]
        }),
        deps: json!({}),
        step: json!({"name": "process_order"}),
    }
}

// ---------------------------------------------------------------------------
// Workflow 2: Payment Reconciliation
// ---------------------------------------------------------------------------

/// Payment reconciliation composition and test data.
///
/// Pipeline: acquire (external txns) → validate (schema) → transform (matching) →
///           transform (discrepancies) → assert (balance) → persist (report)
///
/// Exercises: acquire, validate, transform (x2), assert, persist
/// Checkpoints: persist (index 5)
pub fn payment_reconciliation() -> WorkflowFixture {
    let spec = CompositionSpec {
        name: Some("payment_reconciliation".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Reconcile external payment gateway transactions against internal \
                          records: acquire transactions, validate schema, compute matching, \
                          evaluate discrepancies, assert balance within threshold, and persist \
                          the reconciliation report."
                .to_owned(),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "reconciliation_id": {"type": "string"},
                    "matched_count": {"type": "integer"},
                    "unmatched_count": {"type": "integer"},
                    "total_variance": {"type": "number"}
                },
                "required": ["matched_count", "unmatched_count", "total_variance"]
            }),
        },
        invocations: vec![
            // Step 0: Acquire external transactions
            CapabilityInvocation {
                capability: "acquire".to_owned(),
                config: json!({
                    "resource": {
                        "ref": "payments-gateway",
                        "entity": "transactions"
                    },
                    "params": {
                        "expression": "{date_from: .context.reconciliation_date, date_to: .context.reconciliation_date, status: \"settled\"}"
                    },
                    "constraints": {
                        "limit": 1000,
                        "timeout_ms": 30000
                    },
                    "result_shape": {
                        "expression": "{external_transactions: .data, external_count: .record_count}"
                    }
                }),
                checkpoint: false,
            },
            // Step 1: Validate acquired data schema
            CapabilityInvocation {
                capability: "validate".to_owned(),
                config: json!({
                    "schema": {
                        "type": "object",
                        "required": ["external_transactions", "external_count"],
                        "properties": {
                            "external_transactions": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "required": ["transaction_id", "amount", "currency", "reference"],
                                    "properties": {
                                        "transaction_id": {"type": "string"},
                                        "amount": {"type": "number"},
                                        "currency": {"type": "string"},
                                        "reference": {"type": "string"},
                                        "settled_at": {"type": "string"}
                                    }
                                }
                            },
                            "external_count": {"type": "integer"}
                        }
                    }
                }),
                checkpoint: false,
            },
            // Step 2: Compute matching
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "filter": ".context.internal_records as $internal | .prev.external_transactions as $external | [$external[] | . as $ext | {external_id: $ext.transaction_id, external_amount: $ext.amount, reference: $ext.reference, internal_match: ([$internal[] | select(.reference == $ext.reference)] | first), matched: ([$internal[] | select(.reference == $ext.reference)] | length > 0)}] as $pairs | {pairs: $pairs, matched: [$pairs[] | select(.matched)], unmatched: [$pairs[] | select(.matched | not)]}",
                    "output": {"type": "object"}
                }),
                checkpoint: false,
            },
            // Step 3: Evaluate discrepancies
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "filter": ".prev as $result | [$result.matched[] | {reference: .reference, external_amount: .external_amount, internal_amount: .internal_match.amount, variance: (.external_amount - .internal_match.amount), variance_pct: (if .internal_match.amount == 0 then 100 else (((.external_amount - .internal_match.amount) / .internal_match.amount) * 100) end)}] as $discrepancies | ($discrepancies | map(.variance) | add // 0) as $total_variance | {matched_count: ($result.matched | length), unmatched_count: ($result.unmatched | length), discrepancies: $discrepancies, total_variance: $total_variance, unmatched_external: [$result.unmatched[] | .external_id]}",
                    "output": {"type": "object"}
                }),
                checkpoint: false,
            },
            // Step 4: Assert balance within threshold
            CapabilityInvocation {
                capability: "assert".to_owned(),
                config: json!({
                    "filter": "((.prev.total_variance | fabs) <= .context.variance_threshold) and (.prev.unmatched_count <= .context.max_unmatched)",
                    "error": "Reconciliation failed: total variance or unmatched count exceeds threshold"
                }),
                checkpoint: false,
            },
            // Step 5: Persist reconciliation report (checkpoint)
            CapabilityInvocation {
                capability: "persist".to_owned(),
                config: json!({
                    "resource": "reconciliation-db",
                    "data": {
                        "expression": "{reconciliation_date: .context.reconciliation_date, matched_count: .prev.matched_count, unmatched_count: .prev.unmatched_count, total_variance: .prev.total_variance, discrepancies: .prev.discrepancies, unmatched_external: .prev.unmatched_external, status: \"completed\"}"
                    },
                    "validate_success": {
                        "expression": ".affected_rows > 0"
                    },
                    "result_shape": {
                        "expression": "{reconciliation_id: .data.reconciliation_date, matched_count: .data.matched_count, unmatched_count: .data.unmatched_count, total_variance: .data.total_variance, status: .data.status}"
                    }
                }),
                checkpoint: true,
            },
        ],
    };

    let input = CompositionInput {
        context: json!({
            "reconciliation_date": "2026-03-10",
            "variance_threshold": 5.0,
            "max_unmatched": 2,
            "internal_records": [
                {"reference": "INV-2026-001", "amount": 100.00, "status": "paid"},
                {"reference": "INV-2026-002", "amount": 250.00, "status": "paid"},
                {"reference": "INV-2026-003", "amount": 75.25, "status": "paid"}
            ]
        }),
        deps: json!({}),
        step: json!({"name": "reconcile_payments"}),
    };

    let mut acquire_fixtures = HashMap::new();
    acquire_fixtures.insert(
        "transactions".to_owned(),
        vec![
            json!({"transaction_id": "txn-001", "amount": 100.00, "currency": "USD", "reference": "INV-2026-001", "settled_at": "2026-03-10T10:00:00Z"}),
            json!({"transaction_id": "txn-002", "amount": 250.50, "currency": "USD", "reference": "INV-2026-002", "settled_at": "2026-03-10T11:30:00Z"}),
            json!({"transaction_id": "txn-003", "amount": 75.25, "currency": "USD", "reference": "INV-2026-003", "settled_at": "2026-03-10T14:15:00Z"}),
            json!({"transaction_id": "txn-004", "amount": 500.00, "currency": "USD", "reference": "INV-2026-099", "settled_at": "2026-03-10T16:45:00Z"}),
        ],
    );

    WorkflowFixture {
        spec,
        input,
        acquire_fixtures,
    }
}

/// Payment reconciliation with large variance for assert failure testing.
pub fn payment_reconciliation_large_variance() -> (CompositionInput, HashMap<String, Vec<Value>>) {
    let input = CompositionInput {
        context: json!({
            "reconciliation_date": "2026-03-10",
            "variance_threshold": 5.0,
            "max_unmatched": 2,
            "internal_records": [
                {"reference": "INV-2026-001", "amount": 100.00, "status": "paid"}
            ]
        }),
        deps: json!({}),
        step: json!({"name": "reconcile_payments"}),
    };

    let mut acquire_fixtures = HashMap::new();
    acquire_fixtures.insert(
        "transactions".to_owned(),
        vec![json!({"transaction_id": "txn-001", "amount": 200.00, "currency": "USD", "reference": "INV-2026-001", "settled_at": "2026-03-10T10:00:00Z"})],
    );

    (input, acquire_fixtures)
}

/// Payment reconciliation with too many unmatched for assert failure testing.
pub fn payment_reconciliation_too_many_unmatched() -> (CompositionInput, HashMap<String, Vec<Value>>)
{
    let input = CompositionInput {
        context: json!({
            "reconciliation_date": "2026-03-10",
            "variance_threshold": 5.0,
            "max_unmatched": 2,
            "internal_records": []
        }),
        deps: json!({}),
        step: json!({"name": "reconcile_payments"}),
    };

    let mut acquire_fixtures = HashMap::new();
    acquire_fixtures.insert(
        "transactions".to_owned(),
        vec![
            json!({"transaction_id": "txn-001", "amount": 100.00, "currency": "USD", "reference": "NO-MATCH-1", "settled_at": "2026-03-10T10:00:00Z"}),
            json!({"transaction_id": "txn-002", "amount": 200.00, "currency": "USD", "reference": "NO-MATCH-2", "settled_at": "2026-03-10T11:00:00Z"}),
            json!({"transaction_id": "txn-003", "amount": 300.00, "currency": "USD", "reference": "NO-MATCH-3", "settled_at": "2026-03-10T12:00:00Z"}),
        ],
    );

    (input, acquire_fixtures)
}

// ---------------------------------------------------------------------------
// Workflow 3: Customer Onboarding
// ---------------------------------------------------------------------------

/// Customer onboarding composition and test data.
///
/// Pipeline: acquire (CRM profile) → validate (completeness) →
///           transform (tier classification) → transform (reshape/enrich) →
///           persist (upsert) → emit (welcome event)
///
/// Exercises: acquire, validate, transform (x2), persist, emit
/// Checkpoints: persist (index 4), emit (index 5)
pub fn customer_onboarding() -> WorkflowFixture {
    let spec = CompositionSpec {
        name: Some("customer_onboarding".to_owned()),
        outcome: OutcomeDeclaration {
            description: "Onboard a new customer: acquire profile from CRM, validate \
                          completeness, classify loyalty tier, reshape for downstream, \
                          persist enriched profile, and emit a welcome event."
                .to_owned(),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "customer_id": {"type": "string"},
                    "tier": {"type": "string"},
                    "event_id": {"type": "string"}
                },
                "required": ["customer_id", "tier", "event_id"]
            }),
        },
        invocations: vec![
            // Step 0: Acquire customer profile from CRM
            CapabilityInvocation {
                capability: "acquire".to_owned(),
                config: json!({
                    "resource": {
                        "ref": "crm-api",
                        "entity": "customers"
                    },
                    "params": {
                        "expression": "{customer_id: .context.customer_id, include: \"contact,preferences,history\"}"
                    },
                    "constraints": {
                        "timeout_ms": 10000
                    },
                    "result_shape": {
                        "expression": ".data[0] // {error: \"customer not found\"}"
                    }
                }),
                checkpoint: false,
            },
            // Step 1: Validate profile completeness
            CapabilityInvocation {
                capability: "validate".to_owned(),
                config: json!({
                    "schema": {
                        "type": "object",
                        "required": ["id", "email", "first_name", "last_name"],
                        "properties": {
                            "id": {"type": "string"},
                            "email": {"type": "string"},
                            "first_name": {"type": "string"},
                            "last_name": {"type": "string"},
                            "phone": {"type": "string"},
                            "company": {"type": "string"},
                            "signup_date": {"type": "string"},
                            "total_purchases": {"type": "number"},
                            "purchase_count": {"type": "integer"},
                            "preferences": {"type": "object"}
                        }
                    }
                }),
                checkpoint: false,
            },
            // Step 2: Evaluate tier classification
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "filter": ".prev as $profile | ($profile.total_purchases // 0) as $total | ($profile.purchase_count // 0) as $count | (if $total >= 10000 and $count >= 50 then \"platinum\" elif $total >= 5000 and $count >= 20 then \"gold\" elif $total >= 1000 and $count >= 5 then \"silver\" else \"bronze\" end) as $tier | (if $tier == \"platinum\" then 20 elif $tier == \"gold\" then 15 elif $tier == \"silver\" then 10 else 5 end) as $discount_pct | {profile: $profile, tier: $tier, discount_pct: $discount_pct, tier_benefits: {free_shipping: ($tier == \"platinum\" or $tier == \"gold\"), priority_support: ($tier == \"platinum\"), loyalty_multiplier: (if $tier == \"platinum\" then 3 elif $tier == \"gold\" then 2 elif $tier == \"silver\" then 1.5 else 1 end)}}",
                    "output": {"type": "object"}
                }),
                checkpoint: false,
            },
            // Step 3: Reshape for downstream
            CapabilityInvocation {
                capability: "transform".to_owned(),
                config: json!({
                    "filter": ".prev as $classified | $classified.profile as $p | {customer_id: $p.id, email: $p.email, display_name: (($p.first_name // \"\") + \" \" + ($p.last_name // \"\")), first_name: $p.first_name, last_name: $p.last_name, phone: ($p.phone // null), company: ($p.company // null), signup_date: $p.signup_date, loyalty: {tier: $classified.tier, discount_pct: $classified.discount_pct, benefits: $classified.tier_benefits}, preferences: ($p.preferences // {}), onboarding_status: \"completed\", onboarded_at: .context.onboarding_timestamp}",
                    "output": {"type": "object"}
                }),
                checkpoint: false,
            },
            // Step 4: Persist enriched profile (upsert, checkpoint)
            CapabilityInvocation {
                capability: "persist".to_owned(),
                config: json!({
                    "resource": "customers-db",
                    "mode": "upsert",
                    "data": {
                        "expression": ".prev"
                    },
                    "identity": {
                        "primary_key": ["customer_id"]
                    },
                    "constraints": {
                        "on_conflict": "Update"
                    },
                    "validate_success": {
                        "expression": ".affected_rows > 0"
                    },
                    "result_shape": {
                        "expression": "{customer_id: .data.customer_id, tier: .data.loyalty.tier, display_name: .data.display_name, onboarding_status: .data.onboarding_status}"
                    }
                }),
                checkpoint: true,
            },
            // Step 5: Emit customer.onboarded event (checkpoint)
            CapabilityInvocation {
                capability: "emit".to_owned(),
                config: json!({
                    "event_name": "customer.onboarded",
                    "event_version": "1.0",
                    "resource": "event-bus",
                    "payload": {
                        "expression": "{customer_id: .prev.customer_id, tier: .prev.tier, display_name: .prev.display_name, onboarding_status: .prev.onboarding_status}"
                    },
                    "metadata": {
                        "correlation_id": {
                            "expression": ".context.customer_id"
                        },
                        "idempotency_key": {
                            "expression": "(\"onboard-\" + .prev.customer_id)"
                        }
                    },
                    "result_shape": {
                        "expression": "{customer_id: .prev.customer_id, tier: .prev.tier, event_id: .data.message_id, event_name: .event_name}"
                    }
                }),
                checkpoint: true,
            },
        ],
    };

    let input = CompositionInput {
        context: json!({
            "customer_id": "cust-67890",
            "onboarding_timestamp": "2026-03-10T08:30:00Z"
        }),
        deps: json!({}),
        step: json!({"name": "onboard_customer"}),
    };

    let mut acquire_fixtures = HashMap::new();
    acquire_fixtures.insert(
        "customers".to_owned(),
        vec![json!({
            "id": "cust-67890",
            "email": "jane.doe@example.com",
            "first_name": "Jane",
            "last_name": "Doe",
            "phone": "+1-555-0123",
            "company": "Acme Corp",
            "signup_date": "2025-06-15",
            "total_purchases": 7500.00,
            "purchase_count": 25,
            "preferences": {
                "newsletter": true,
                "sms_notifications": false,
                "preferred_language": "en"
            }
        })],
    );

    WorkflowFixture {
        spec,
        input,
        acquire_fixtures,
    }
}

/// Customer onboarding with incomplete profile for validation failure testing.
pub fn customer_onboarding_incomplete_profile() -> (CompositionInput, HashMap<String, Vec<Value>>) {
    let input = CompositionInput {
        context: json!({
            "customer_id": "cust-incomplete",
            "onboarding_timestamp": "2026-03-10T09:00:00Z"
        }),
        deps: json!({}),
        step: json!({"name": "onboard_customer"}),
    };

    let mut acquire_fixtures = HashMap::new();
    acquire_fixtures.insert(
        "customers".to_owned(),
        vec![json!({
            "id": "cust-incomplete",
            "first_name": "Bob",
            "last_name": "Smith",
            "total_purchases": 0,
            "purchase_count": 0
        })],
    );

    (input, acquire_fixtures)
}

// ---------------------------------------------------------------------------
// Summary helpers
// ---------------------------------------------------------------------------

/// Returns all three workflow fixtures for bulk testing.
pub fn all_workflow_fixtures() -> Vec<(&'static str, WorkflowFixture)> {
    vec![
        ("ecommerce_order_processing", ecommerce_order_processing()),
        ("payment_reconciliation", payment_reconciliation()),
        ("customer_onboarding", customer_onboarding()),
    ]
}
