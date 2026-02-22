# Example Handlers - Cross-Language Reference

**Last Updated**: 2026-02-21
**Status**: Active
<- Back to [Worker Crates Overview](index.md)

> **DSL syntax**: All examples below use the functional DSL pattern (recommended for new projects). For the class-based alternative, see [Class-Based Handlers](../reference/class-based-handlers.md).

---

## Overview

This document provides side-by-side handler examples across Python, Ruby, TypeScript, and Rust. These examples demonstrate the aligned patterns that enable consistent handler authoring across all worker implementations.

---

## Simple Step Handler

### Python

```python
from tasker_core.step_handler.functional import step_handler, inputs
from app.services.types import EcommerceOrderInput
from app.services import ecommerce as svc

@step_handler("ecommerce_validate_cart")
@inputs(EcommerceOrderInput)
def validate_cart(inputs: EcommerceOrderInput, context):
    return svc.validate_cart_items(
        cart_items=inputs.cart_items,
        customer_email=inputs.customer_email,
    )
```

### Ruby

```ruby
module Ecommerce
  module StepHandlers
    extend TaskerCore::StepHandler::Functional

    ValidateCartHandler = step_handler(
      'Ecommerce::StepHandlers::ValidateCartHandler',
      inputs: Types::Ecommerce::OrderInput
    ) do |inputs:, context:|
      Ecommerce::Service.validate_cart_items(
        cart_items: inputs.cart_items,
        customer_email: inputs.customer_email,
      )
    end
  end
end
```

### TypeScript

```typescript
import { defineHandler } from '@tasker-systems/tasker';
import type { CartItem } from '../services/types';
import * as svc from '../services/ecommerce';

export const ValidateCartHandler = defineHandler(
  'Ecommerce.StepHandlers.ValidateCartHandler',
  { inputs: { cartItems: 'cart_items' } },
  async ({ cartItems }) =>
    svc.validateCartItems(cartItems as CartItem[] | undefined),
);
```

### Rust

```rust
use serde_json::{json, Value};

pub fn validate_cart(context: &Value) -> Result<Value, String> {
    let cart_items = context.get("cart_items")
        .and_then(|v| v.as_array())
        .ok_or("Missing cart_items in context")?;

    if cart_items.is_empty() {
        return Err("Cart cannot be empty".to_string());
    }

    // Business logic: validate items, calculate pricing...
    Ok(json!({
        "validated_items": cart_items,
        "subtotal": 59.97,
        "tax": 4.80,
        "total": 64.77
    }))
}
```

---

## Handler with Dependencies

### Python

```python
from tasker_core.step_handler.functional import step_handler, depends_on, inputs
from app.services.types import (
    EcommerceOrderInput,
    EcommerceValidateCartResult,
    EcommerceProcessPaymentResult,
    EcommerceUpdateInventoryResult,
)
from app.services import ecommerce as svc

@step_handler("ecommerce_create_order")
@depends_on(
    cart_result=("validate_cart", EcommerceValidateCartResult),
    payment_result=("process_payment", EcommerceProcessPaymentResult),
    inventory_result=("update_inventory", EcommerceUpdateInventoryResult),
)
@inputs(EcommerceOrderInput)
def create_order(
    cart_result: EcommerceValidateCartResult,
    payment_result: EcommerceProcessPaymentResult,
    inventory_result: EcommerceUpdateInventoryResult,
    inputs: EcommerceOrderInput,
    context,
):
    return svc.create_order(
        cart_result=cart_result,
        payment_result=payment_result,
        inventory_result=inventory_result,
        customer_email=inputs.customer_email,
    )
```

### Ruby

```ruby
module Microservices
  module StepHandlers
    extend TaskerCore::StepHandler::Functional

    SendWelcomeSequenceHandler = step_handler(
      'Microservices::StepHandlers::SendWelcomeSequenceHandler',
      depends_on: {
        account_data: ['create_user_account', Types::Microservices::CreateUserResult],
        billing_data: ['setup_billing_profile', Types::Microservices::SetupBillingResult],
        preferences_data: ['initialize_preferences', Types::Microservices::InitPreferencesResult]
      }
    ) do |account_data:, billing_data:, preferences_data:, context:|
      Microservices::Service.send_welcome_sequence(
        account_data: account_data,
        billing_data: billing_data,
        preferences_data: preferences_data,
      )
    end
  end
end
```

### TypeScript

```typescript
import { defineHandler } from '@tasker-systems/tasker';
import * as svc from '../services/ecommerce';

export const CreateOrderHandler = defineHandler(
  'Ecommerce.StepHandlers.CreateOrderHandler',
  {
    depends: {
      cartResult: 'validate_cart',
      paymentResult: 'process_payment',
      inventoryResult: 'update_inventory',
    },
    inputs: { customerEmail: 'customer_email' },
  },
  async ({ cartResult, paymentResult, inventoryResult, customerEmail }) =>
    svc.createOrder(
      cartResult as Record<string, unknown>,
      paymentResult as Record<string, unknown>,
      inventoryResult as Record<string, unknown>,
      customerEmail as string | undefined,
    ),
);
```

### Rust

```rust
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn create_order(
    context: &Value,
    dependency_results: &HashMap<String, Value>,
) -> Result<Value, String> {
    let cart_result = dependency_results
        .get("validate_cart")
        .ok_or("Missing validate_cart dependency")?;
    let payment_result = dependency_results
        .get("process_payment")
        .ok_or("Missing process_payment dependency")?;
    let inventory_result = dependency_results
        .get("update_inventory")
        .ok_or("Missing update_inventory dependency")?;

    let customer_email = context
        .get("customer_email")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown@example.com");

    Ok(json!({
        "order_id": format!("ord_{}", uuid::Uuid::new_v4()),
        "customer_email": customer_email,
        "total": cart_result.get("total").and_then(|v| v.as_f64()).unwrap_or(0.0),
        "payment_id": payment_result.get("payment_id"),
        "inventory_log_id": inventory_result.get("inventory_log_id"),
        "status": "confirmed"
    }))
}
```

---

## Decision Handler

Decision handlers route workflows dynamically by activating different step sets based on business logic. The DSL returns a `Decision` object — `Decision.route(steps)` to activate branches or `Decision.skip(reason)` to skip. For full details, see [Conditional Workflows](../guides/conditional-workflows.md).

### Python

```python
from tasker_core.step_handler.functional import decision_handler, inputs, Decision

@decision_handler("routing_decision")
@inputs('amount')
def routing_decision(amount, context):
    amount = float(amount or 0)

    if amount < 1000:
        return Decision.route(
            ['auto_approve'],
            route_type='automatic', amount=amount,
        )
    elif amount < 5000:
        return Decision.route(
            ['manager_approval'],
            route_type='manager', amount=amount,
        )
    else:
        return Decision.route(
            ['manager_approval', 'finance_review'],
            route_type='dual_approval', amount=amount,
        )
```

### Ruby

```ruby
module Orders
  module StepHandlers
    extend TaskerCore::StepHandler::Functional

    RoutingDecisionHandler = decision_handler(
      'Orders::StepHandlers::RoutingDecisionHandler',
      inputs: [:amount]
    ) do |amount:, context:|
      amount = amount.to_f

      if amount < 1000
        Decision.route(['auto_approve'], route_type: 'automatic', amount: amount)
      elsif amount < 5000
        Decision.route(['manager_approval'], route_type: 'manager', amount: amount)
      else
        Decision.route(
          ['manager_approval', 'finance_review'],
          route_type: 'dual_approval', amount: amount
        )
      end
    end
  end
end
```

### TypeScript

```typescript
import { defineDecisionHandler, Decision } from '@tasker-systems/tasker';

export const RoutingDecisionHandler = defineDecisionHandler(
  'Orders.StepHandlers.RoutingDecisionHandler',
  { inputs: { amount: 'amount' } },
  async ({ amount }) => {
    const amt = (amount as number) || 0;

    if (amt < 1000) {
      return Decision.route(['auto_approve'], { routeType: 'automatic', amount: amt });
    } else if (amt < 5000) {
      return Decision.route(['manager_approval'], { routeType: 'manager', amount: amt });
    } else {
      return Decision.route(
        ['manager_approval', 'finance_review'],
        { routeType: 'dual_approval', amount: amt },
      );
    }
  },
);
```

### Rust

```rust
use tasker_shared::messaging::DecisionPointOutcome;
use serde_json::{json, Value};

pub fn routing_decision(context: &Value) -> Result<Value, String> {
    let amount = context.get("amount")
        .and_then(|v| v.as_f64())
        .ok_or("Amount is required for routing decision")?;

    let (route_type, steps) = if amount < 1000.0 {
        ("automatic", vec!["auto_approve"])
    } else if amount < 5000.0 {
        ("manager", vec!["manager_approval"])
    } else {
        ("dual_approval", vec!["manager_approval", "finance_review"])
    };

    let outcome = DecisionPointOutcome::create_steps(
        steps.iter().map(|s| s.to_string()).collect()
    );

    Ok(json!({
        "route_type": route_type,
        "amount": amount,
        "decision_point_outcome": outcome.to_value()
    }))
}
```

---

## Batch Processing Handler

Batch handlers use the Analyzer/Worker pattern. The analyzer returns a `BatchConfig` specifying total items and batch size; the orchestrator automatically generates cursor ranges and spawns workers. For full details, see [Batch Processing](../guides/batch-processing.md).

### Python (Analyzer + Worker)

```python
from tasker_core.step_handler.functional import (
    batch_analyzer, batch_worker, inputs, depends_on, BatchConfig,
)

@batch_analyzer("analyze_csv", worker_template="process_csv_batch")
@inputs('csv_file_path')
def analyze_csv(csv_file_path, context):
    total_rows = count_csv_rows(csv_file_path)
    return BatchConfig(total_items=total_rows, batch_size=100)

@batch_worker("process_csv_batch")
@depends_on(analyzer_result="analyze_csv")
def process_csv_batch(analyzer_result, batch_context, context):
    records = read_csv_range(
        analyzer_result['csv_file_path'],
        batch_context.start_cursor,
        batch_context.batch_size,
    )
    processed = [transform_row(row) for row in records]
    return {"items_processed": len(processed), "items_succeeded": len(processed)}
```

### Ruby (Analyzer + Worker)

```ruby
module Csv
  module StepHandlers
    extend TaskerCore::StepHandler::Functional

    AnalyzeCsvHandler = batch_analyzer(
      'Csv::StepHandlers::AnalyzeCsvHandler',
      worker_template: 'process_csv_batch',
      inputs: [:csv_file_path]
    ) do |csv_file_path:, context:|
      total_rows = count_csv_rows(csv_file_path)
      BatchConfig.new(total_items: total_rows, batch_size: 100)
    end

    ProcessCsvBatchHandler = batch_worker(
      'Csv::StepHandlers::ProcessCsvBatchHandler',
      depends_on: { analyzer_result: ['analyze_csv'] }
    ) do |analyzer_result:, batch_context:, context:|
      records = read_csv_range(
        analyzer_result['csv_file_path'],
        batch_context.start_cursor,
        batch_context.batch_size
      )
      processed = records.map { |row| transform_row(row) }
      { items_processed: processed.size, items_succeeded: processed.size }
    end
  end
end
```

### TypeScript (Analyzer + Worker)

```typescript
import { defineBatchAnalyzer, defineBatchWorker } from '@tasker-systems/tasker';

export const AnalyzeCsvHandler = defineBatchAnalyzer(
  'Csv.StepHandlers.AnalyzeCsvHandler',
  { workerTemplate: 'process_csv_batch', inputs: { csvFilePath: 'csv_file_path' } },
  async ({ csvFilePath }) => ({
    totalItems: await countCsvRows(csvFilePath as string),
    batchSize: 100,
  }),
);

export const ProcessCsvBatchHandler = defineBatchWorker(
  'Csv.StepHandlers.ProcessCsvBatchHandler',
  { depends: { analyzerResult: 'analyze_csv' } },
  async ({ analyzerResult, batchContext }) => {
    const records = await readCsvRange(
      (analyzerResult as Record<string, unknown>).csvFilePath as string,
      batchContext?.startCursor ?? 0,
      batchContext?.batchSize ?? 100,
    );
    return { itemsProcessed: records.length, itemsSucceeded: records.length };
  },
);
```

### Rust

```rust
use serde_json::{json, Value};

// Batch analyzers in Rust return batch configuration via the result
pub fn analyze_csv(context: &Value) -> Result<Value, String> {
    let file_path = context.get("csv_file_path")
        .and_then(|v| v.as_str())
        .ok_or("Missing csv_file_path")?;

    let total_rows = count_csv_rows(file_path)?;

    Ok(json!({
        "total_items": total_rows,
        "batch_size": 100,
        "csv_file_path": file_path
    }))
}
```

---

## API Handler

API handlers add HTTP client methods with automatic error classification (429 -> retryable, 4xx -> permanent, 5xx -> retryable). The DSL provides an `api` object with `get`, `post`, `put`, `delete` methods and result helpers like `api_success` / `api_failure`.

### Python

```python
from tasker_core.step_handler.functional import api_handler, inputs

@api_handler("fetch_user", base_url="https://api.example.com")
@inputs('user_id')
def fetch_user(user_id, api, context):
    response = api.get(f"/users/{user_id}")
    return api.api_success(result={
        "user_id": user_id,
        "email": response["email"],
        "name": response["name"],
    })
```

### Ruby

```ruby
module Users
  module StepHandlers
    extend TaskerCore::StepHandler::Functional

    FetchUserHandler = api_handler(
      'Users::StepHandlers::FetchUserHandler',
      base_url: 'https://api.example.com',
      inputs: [:user_id]
    ) do |user_id:, api:, context:|
      response = api.get("/users/#{user_id}")
      api.api_success(result: {
        user_id: user_id,
        email: response.body['email'],
        name: response.body['name']
      })
    end
  end
end
```

### TypeScript

```typescript
import { defineApiHandler } from '@tasker-systems/tasker';

export const FetchUserHandler = defineApiHandler(
  'Users.StepHandlers.FetchUserHandler',
  {
    baseUrl: 'https://api.example.com',
    inputs: { userId: 'user_id' },
  },
  async ({ userId, api }) => {
    const response = await api.get(`/users/${userId}`);
    return api.apiSuccess({
      userId,
      email: response.email,
      name: response.name,
    });
  },
);
```

---

## Error Handling Patterns

### Python (DSL — exceptions)

```python
from tasker_core import PermanentError, RetryableError

@step_handler("validate_order")
@inputs(OrderInput)
def validate_order(inputs: OrderInput, context):
    if not inputs.amount or inputs.amount <= 0:
        raise PermanentError(
            "Order amount must be positive",
            error_code="INVALID_AMOUNT",
        )

    if external_service_unavailable():
        raise RetryableError("External service temporarily unavailable")

    return {"valid": True, "amount": inputs.amount}
```

### Ruby (DSL — exceptions)

```ruby
ValidateOrderHandler = step_handler(
  'Orders::StepHandlers::ValidateOrderHandler',
  inputs: Types::Orders::OrderInput
) do |inputs:, context:|
  raise TaskerCore::Errors::PermanentError.new(
    'Order amount must be positive',
    error_code: 'INVALID_AMOUNT'
  ) if inputs.amount.to_f <= 0

  raise TaskerCore::Errors::RetryableError.new(
    'External service temporarily unavailable'
  ) if external_service_unavailable?

  { valid: true, amount: inputs.amount }
end
```

### TypeScript (DSL — exceptions)

```typescript
import { defineHandler, PermanentError, RetryableError } from '@tasker-systems/tasker';

export const ValidateOrderHandler = defineHandler(
  'Orders.StepHandlers.ValidateOrderHandler',
  { inputs: { amount: 'amount' } },
  async ({ amount }) => {
    if (!amount || (amount as number) <= 0) {
      throw new PermanentError('Order amount must be positive', 'INVALID_AMOUNT');
    }
    return { valid: true, amount };
  },
);
```

### Rust (Result type)

```rust
pub fn validate_order(context: &Value) -> Result<Value, String> {
    let amount = context.get("amount")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    if amount <= 0.0 {
        return Err("Order amount must be positive".to_string());
    }

    Ok(json!({ "valid": true, "amount": amount }))
}
```

For finer control over retryability in Rust, use `StepExecutionResult` directly in a `StepHandler` implementation. See [Building with Rust](../building/rust.md#error-handling).

---

## See Also

- [Class-Based Handlers](../reference/class-based-handlers.md) - Class-based alternative for all languages
- [API Convergence Matrix](api-convergence-matrix.md) - Quick reference tables
- [Patterns and Practices](patterns-and-practices.md) - Common patterns
- [Building with Python](../building/python.md) - Python handler guide
- [Building with Ruby](../building/ruby.md) - Ruby handler guide
- [Building with TypeScript](../building/typescript.md) - TypeScript handler guide
- [Building with Rust](../building/rust.md) - Rust handler guide
