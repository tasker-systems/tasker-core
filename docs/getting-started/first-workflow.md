# Your First Workflow

This guide walks you through creating a complete workflow with multiple steps.

## What is a Workflow?

A **Workflow** is a directed acyclic graph (DAG) of steps. Steps execute when their dependencies are satisfied, enabling parallel execution where possible.

## Example: Order Processing

Let's build an order processing workflow with these steps:

```
           ┌──────────────┐
           │   validate   │
           └──────┬───────┘
                  │
        ┌─────────┴─────────┐
        ▼                   ▼
┌──────────────┐   ┌──────────────┐
│   reserve    │   │   notify     │
│  inventory   │   │   customer   │
└──────┬───────┘   └──────────────┘
        │
        ▼
┌──────────────┐
│    charge    │
│   payment    │
└──────────────┘
```

## Step 1: Define the Task Template

Create a YAML file defining the workflow structure:

```yaml
# config/tasker/templates/order_processing.yaml
name: order_processing
namespace_name: default
version: 1.0.0
description: Process a customer order

steps:
  - name: validate_order
    description: Validate order data
    handler:
      callable: ValidateOrderHandler
    dependencies: []

  - name: reserve_inventory
    description: Reserve items in warehouse
    handler:
      callable: ReserveInventoryHandler
    dependencies:
      - validate_order

  - name: notify_customer
    description: Send order confirmation email
    handler:
      callable: NotifyCustomerHandler
    dependencies:
      - validate_order

  - name: charge_payment
    description: Charge the customer
    handler:
      callable: ChargePaymentHandler
    dependencies:
      - reserve_inventory
```

## Step 2: Implement Handlers

### Python Implementation

```python
from tasker_core import StepHandler, StepContext, StepHandlerResult
from tasker_core.errors import PermanentError

class ValidateOrderHandler(StepHandler):
    handler_name = "ValidateOrderHandler"

    def call(self, context: StepContext) -> StepHandlerResult:
        order_id = context.get_input("order_id")
        items = context.get_input("items")
        
        if not order_id or not items:
            raise PermanentError(message="Missing order_id or items", error_code="MISSING_INPUT")
        
        total = sum(item["price"] * item["quantity"] for item in items)
        return StepHandlerResult.success({"valid": True, "total": total, "item_count": len(items)})


class ReserveInventoryHandler(StepHandler):
    handler_name = "ReserveInventoryHandler"

    def call(self, context: StepContext) -> StepHandlerResult:
        validation = context.get_dependency_result("validate_order")
        items = context.get_input("items")
        
        # Reserve inventory logic...
        return StepHandlerResult.success({"reserved": True, "reservation_id": "RES-12345"})


class NotifyCustomerHandler(StepHandler):
    handler_name = "NotifyCustomerHandler"

    def call(self, context: StepContext) -> StepHandlerResult:
        email = context.get_input("customer_email")
        validation = context.get_dependency_result("validate_order")
        
        # Send email logic...
        return StepHandlerResult.success({"notified": True, "email": email})


class ChargePaymentHandler(StepHandler):
    handler_name = "ChargePaymentHandler"

    def call(self, context: StepContext) -> StepHandlerResult:
        validation = context.get_dependency_result("validate_order")
        reservation = context.get_dependency_result("reserve_inventory")
        
        total = validation["total"]
        # Charge payment logic...
        return StepHandlerResult.success({"charged": True, "amount": total, "transaction_id": "TXN-67890"})
```

## Step 3: Submit a Task

Use the client SDK to submit tasks:

```python
from tasker_core import TaskerClient

client = TaskerClient()

result = client.create_task(
    "order_processing",
    context={
        "order_id": "ORD-12345",
        "customer_email": "customer@example.com",
        "items": [
            {"sku": "WIDGET-A", "price": 29.99, "quantity": 2},
            {"sku": "GADGET-B", "price": 49.99, "quantity": 1},
        ],
    },
    initiator="api:checkout",
    reason="New order received",
)

print(f"Task created: {result.task_uuid}")
```

## Execution Flow

When this task runs:

1. **validate_order** executes first (no dependencies)
2. **reserve_inventory** and **notify_customer** execute in parallel (both depend only on validate_order)
3. **charge_payment** executes after reserve_inventory completes

The total execution time is determined by the longest path through the DAG.

## Next Steps

- [Language Guides](choosing-your-package.md) — Complete guides for each language
- [Architecture Overview](../architecture/README.md) — Understand Tasker internals
- [Testing Guide](../testing/README.md) — Test your workflows
