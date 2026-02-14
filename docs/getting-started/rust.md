# Rust Guide

This guide covers using Tasker with native Rust step handlers.

## Quick Start

```bash
# Add dependencies to Cargo.toml
[dependencies]
tasker-worker = { git = "https://github.com/tasker-systems/tasker-core" }
tasker-shared = { git = "https://github.com/tasker-systems/tasker-core" }
async-trait = "0.1"
serde_json = "1.0"
```

## Writing a Step Handler

Rust step handlers implement the `RustStepHandler` trait using `async_trait`:

```rust path=null start=null
use async_trait::async_trait;
use anyhow::Result;
use serde_json::json;
use tasker_shared::messaging::StepExecutionResult;
use tasker_shared::types::TaskSequenceStep;

#[async_trait]
pub trait RustStepHandler: Send + Sync {
    async fn call(&self, step_data: &TaskSequenceStep) -> Result<StepExecutionResult>;
    fn name(&self) -> &'static str;
}
```

### Minimal Handler Example

```rust path=null start=null
use async_trait::async_trait;
use anyhow::Result;
use serde_json::json;
use tasker_shared::messaging::StepExecutionResult;
use tasker_shared::types::TaskSequenceStep;

pub struct MyHandler;

#[async_trait]
impl RustStepHandler for MyHandler {
    async fn call(&self, step_data: &TaskSequenceStep) -> Result<StepExecutionResult> {
        // Access task context data
        let input_value: i64 = step_data.get_input("my_field")?;

        // Perform your business logic
        let result = input_value * 2;

        // Return success result
        Ok(StepExecutionResult::success(
            step_data.workflow_step.workflow_step_uuid,
            json!({ "result": result }),
            0, // execution time in ms
            None,
        ))
    }

    fn name(&self) -> &'static str {
        "my_handler"
    }
}
```

### Accessing Task Context

Use `get_input()` for type-safe task context access:

```rust path=null start=null
// Get required value (returns Error if missing)
let customer_id: i64 = step_data.get_input("customer_id")?;

// Get optional value with default
let timeout: i64 = step_data.get_input_or("timeout_ms", 5000);
```

### Accessing Dependency Results

Access results from upstream steps using `get_dependency_result_column_value()`:

```rust path=null start=null
// Get result from a specific upstream step
let previous_result: i64 = step_data
    .get_dependency_result_column_value("previous_step_name")?;

// Handle complex JSON results
let order_data: serde_json::Value = step_data
    .get_dependency_result_column_value("validate_order")?;
let total = order_data["order_total"].as_f64().unwrap_or(0.0);
```

## Complete Example: Order Validation Handler

This example shows a real-world handler with validation and error handling:

```rust path=null start=null
use async_trait::async_trait;
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use tasker_shared::messaging::StepExecutionResult;
use tasker_shared::types::TaskSequenceStep;

pub struct ValidateOrderHandler;

#[async_trait]
impl RustStepHandler for ValidateOrderHandler {
    async fn call(&self, step_data: &TaskSequenceStep) -> Result<StepExecutionResult> {
        let start_time = std::time::Instant::now();
        let step_uuid = step_data.workflow_step.workflow_step_uuid;

        // Extract customer info from task context
        let customer: serde_json::Value = step_data.get_context_field("customer")?;
        let customer_id = customer["id"].as_i64()
            .ok_or_else(|| anyhow::anyhow!("Customer ID is required"))?;

        // Extract and validate order items
        let items: Vec<serde_json::Value> = step_data
            .get_context_field("items")?;

        if items.is_empty() {
            return Ok(StepExecutionResult::failure(
                step_uuid,
                "Order items cannot be empty".to_string(),
                Some("EMPTY_ORDER_ITEMS".to_string()),
                Some("ValidationError".to_string()),
                false, // Not retryable
                start_time.elapsed().as_millis() as i64,
                None,
            ));
        }

        // Calculate order total
        let total: f64 = items.iter()
            .map(|item| {
                let price = item["price"].as_f64().unwrap_or(0.0);
                let qty = item["quantity"].as_i64().unwrap_or(0) as f64;
                price * qty
            })
            .sum();

        // Build metadata for observability
        let mut metadata = HashMap::new();
        metadata.insert("operation".to_string(), json!("validate_order"));
        metadata.insert("item_count".to_string(), json!(items.len()));

        Ok(StepExecutionResult::success(
            step_uuid,
            json!({
                "customer_id": customer_id,
                "validated_items": items,
                "order_total": total,
                "validation_status": "complete",
                "validated_at": Utc::now().to_rfc3339()
            }),
            start_time.elapsed().as_millis() as i64,
            Some(metadata),
        ))
    }

    fn name(&self) -> &'static str {
        "validate_order"
    }
}
```

## Handler Registry

Register handlers so the worker can discover them:

```rust path=null start=null
use std::collections::HashMap;
use std::sync::Arc;

pub struct RustStepHandlerRegistry {
    handlers: HashMap<String, Box<dyn Fn() -> Box<dyn RustStepHandler>>>,
}

impl RustStepHandlerRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };

        // Register handlers
        registry.register("validate_order", || Box::new(ValidateOrderHandler));
        registry.register("process_payment", || Box::new(ProcessPaymentHandler));

        registry
    }

    pub fn register<F>(&mut self, name: &str, factory: F)
    where
        F: Fn() -> Box<dyn RustStepHandler> + 'static,
    {
        self.handlers.insert(name.to_string(), Box::new(factory));
    }

    pub fn get_handler(&self, name: &str) -> Option<Box<dyn RustStepHandler>> {
        self.handlers.get(name).map(|f| f())
    }
}
```

## Task Template Configuration

Define workflows in YAML:

```yaml path=null start=null
name: order_fulfillment
namespace_name: ecommerce
version: 1.0.0
description: "E-commerce order processing workflow"

steps:
  - name: validate_order
    handler:
      callable: order_fulfillment::ValidateOrderHandler
    dependencies: []

  - name: reserve_inventory
    handler:
      callable: order_fulfillment::ReserveInventoryHandler
    dependencies:
      - validate_order

  - name: process_payment
    handler:
      callable: order_fulfillment::ProcessPaymentHandler
    dependencies:
      - validate_order
      - reserve_inventory

  - name: ship_order
    handler:
      callable: order_fulfillment::ShipOrderHandler
    dependencies:
      - validate_order
      - reserve_inventory
      - process_payment
```

## Running the Worker

Bootstrap and run a native Rust worker:

```rust path=null start=null
use tasker_worker::WorkerBootstrap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize handler registry
    let registry = Arc::new(RustStepHandlerRegistry::new());

    // Create event handler
    let event_system = get_global_event_system();
    let event_handler = RustEventHandler::new(
        registry,
        event_system.clone(),
        "rust-worker-1".to_string(),
    );

    // Start event handler
    event_handler.start().await?;

    // Bootstrap worker
    let config = WorkerBootstrapConfig {
        namespace: "order_fulfillment".to_string(),
        ..Default::default()
    };

    let worker_handle = WorkerBootstrap::bootstrap_with_event_system(
        config,
        Some(event_system),
    ).await?;

    // Wait for shutdown signal
    worker_handle.wait_for_shutdown().await?;

    Ok(())
}
```

## Testing

Write integration tests with real database interactions:

```rust path=null start=null
#[tokio::test]
async fn test_validate_order_handler() {
    let handler = ValidateOrderHandler;

    // Create mock step data
    let step_data = create_test_step_data(json!({
        "customer": {"id": 123, "email": "test@example.com"},
        "items": [
            {"product_id": 1, "quantity": 2, "price": 29.99}
        ]
    }));

    let result = handler.call(&step_data).await.unwrap();

    assert!(result.success);
    assert_eq!(result.result["order_total"], 59.98);
}
```

Run tests:

```bash
cargo test --test integration
```

## Error Handling

Return structured errors with error codes and retryability:

```rust path=null start=null
// Non-retryable validation error
Ok(StepExecutionResult::failure(
    step_uuid,
    "Invalid order data".to_string(),
    Some("VALIDATION_ERROR".to_string()),   // error_code
    Some("ValidationError".to_string()),     // error_type
    false,                                    // retryable
    execution_time_ms,
    None,
))

// Retryable transient error
Ok(StepExecutionResult::failure(
    step_uuid,
    "Payment gateway timeout".to_string(),
    Some("GATEWAY_TIMEOUT".to_string()),
    Some("NetworkError".to_string()),
    true,                                     // retryable
    execution_time_ms,
    Some(metadata),
))
```

## Common Patterns

### Type-Safe Context Access

```rust path=null start=null
// TAS-137: Cross-language standard API
let value: String = step_data.get_input("field_name")?;
let optional: i64 = step_data.get_input_or("timeout", 5000);
```

### Dependency Result Access

```rust path=null start=null
// Get computed result from upstream step
let result: serde_json::Value = step_data
    .get_dependency_result_column_value("step_name")?;
```

### Metadata for Observability

```rust path=null start=null
let mut metadata = HashMap::new();
metadata.insert("operation".to_string(), json!("my_operation"));
metadata.insert("input_refs".to_string(), json!({
    "field": "step_data.get_input(\"field\")"
}));
```

## Submitting Tasks via Client SDK

Rust applications can submit tasks directly using `tasker-client`:

```rust path=null start=null
use tasker_client::{OrchestrationApiClient, OrchestrationApiConfig};
use tasker_shared::models::core::task_request::TaskRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let config = OrchestrationApiConfig::default();
    let client = OrchestrationApiClient::new(config)?;

    // Create a task
    let task_request = TaskRequest {
        name: "order_fulfillment".to_string(),
        namespace: "ecommerce".to_string(),
        version: "1.0.0".to_string(),
        context: serde_json::json!({
            "customer": {"id": 123, "email": "customer@example.com"},
            "items": [
                {"product_id": 1, "quantity": 2, "price": 29.99}
            ]
        }),
        initiator: "my-service".to_string(),
        source_system: "api".to_string(),
        reason: "New order received".to_string(),
        ..Default::default()
    };

    let response = client.create_task(task_request).await?;
    println!("Task created: {}", response.task_uuid);

    // Get task status
    let task = client.get_task(response.task_uuid).await?;
    println!("Task status: {}", task.status);

    // List task steps
    let steps = client.list_task_steps(response.task_uuid).await?;
    for step in steps {
        println!("Step {}: {}", step.name, step.current_state);
    }

    Ok(())
}
```

### Configuration

Configure via environment variables or TOML config:

```bash
export ORCHESTRATION_URL=http://localhost:8080
export ORCHESTRATION_API_KEY=your-api-key
```

Or create `.config/tasker-client.toml`:

```toml path=null start=null
[profiles.local]
transport = "rest"
orchestration_url = "http://localhost:8080"

[profiles.production]
transport = "grpc"
orchestration_url = "https://tasker.example.com:9190"
api_key = "your-production-key"
```

## Next Steps

- See [Architecture](../architecture/README.md) for system design
- See [Workers Reference](../workers/README.md) for advanced patterns
- See the [tasker-core workers/rust](https://github.com/tasker-systems/tasker-core/tree/main/workers/rust) for complete examples
