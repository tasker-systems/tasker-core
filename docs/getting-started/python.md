# Python Guide

This guide covers using Tasker with Python step handlers via the `tasker-py` package.

## Quick Start

```bash
# Install with pip
pip install tasker-py

# Or with uv (recommended)
uv add tasker-py
```

## Writing a Step Handler

Python step handlers inherit from `StepHandler`:

```python path=null start=null
from tasker_core import StepHandler, StepContext, StepHandlerResult

class MyHandler(StepHandler):
    handler_name = "my_handler"
    handler_version = "1.0.0"

    def call(self, context: StepContext) -> StepHandlerResult:
        return StepHandlerResult.success({"processed": True})
```

### Minimal Handler Example

```python path=null start=null
from tasker_core import StepHandler, StepContext, StepHandlerResult

class LinearStep1Handler(StepHandler):
    handler_name = "linear_step_1"
    handler_version = "1.0.0"

    def call(self, context: StepContext) -> StepHandlerResult:
        # Access task context using get_input()
        even_number = context.get_input("even_number")

        if not even_number or even_number % 2 != 0:
            return self.failure(
                "Task context must contain an even number",
                error_type="validation_error",
                retryable=False
            )

        # Perform business logic
        result = even_number * even_number

        # Return success result
        return self.success(
            {"result": result},
            metadata={
                "operation": "square",
                "step_type": "initial"
            }
        )
```

### Async Handlers

Python handlers can be asynchronous:

```python path=null start=null
import asyncio
from tasker_core import StepHandler, StepContext, StepHandlerResult

class MyAsyncHandler(StepHandler):
    handler_name = "my_async_handler"

    async def call(self, context: StepContext) -> StepHandlerResult:
        # Use async operations like aiohttp, asyncpg, etc.
        await asyncio.sleep(0.1)
        return self.success({"processed": True})
```

### Accessing Task Context

Use `get_input()` for task context access (cross-language standard API):

```python path=null start=null
# Get value from task context
customer_id = context.get_input("customer_id")

# Access nested values
cart_items = context.get_input("cart_items")
```

### Accessing Dependency Results

Access results from upstream steps using `get_dependency_result()`:

```python path=null start=null
# Get result from a specific upstream step
previous_result = context.get_dependency_result("previous_step_name")

# Extract nested values
order_total = context.get_dependency_field("validate_order", "order_total")
```

## Complete Example: Validate Cart Handler

This example shows a real-world e-commerce handler:

```python path=null start=null
from __future__ import annotations

import logging
from datetime import datetime, timezone
from typing import Any

from tasker_core import StepHandler, StepContext, StepHandlerResult
from tasker_core.errors import PermanentError, RetryableError

logger = logging.getLogger(__name__)

# Mock product database
PRODUCTS = {
    1: {"id": 1, "name": "Widget A", "price": 29.99, "stock": 100, "active": True},
    2: {"id": 2, "name": "Widget B", "price": 49.99, "stock": 50, "active": True},
    3: {"id": 3, "name": "Widget C", "price": 19.99, "stock": 25, "active": True},
}


class ValidateCartHandler(StepHandler):
    """Validates cart items, checks availability, and calculates totals."""

    handler_name = "ecommerce.step_handlers.ValidateCartHandler"
    handler_version = "1.0.0"

    def call(self, context: StepContext) -> StepHandlerResult:
        # TAS-137: Use get_input() for task context access
        cart_items = context.get_input("cart_items")

        if not cart_items:
            raise PermanentError(
                message="Cart items are required",
                error_code="MISSING_CART_ITEMS",
            )

        logger.info(
            "ValidateCartHandler: Validating cart - task_uuid=%s, items=%d",
            context.task_uuid,
            len(cart_items),
        )

        # Validate each cart item
        self._validate_cart_item_structure(cart_items)
        validated_items = self._validate_cart_items(cart_items)

        # Calculate totals
        subtotal = sum(item["line_total"] for item in validated_items)
        tax = round(subtotal * 0.08, 2)  # 8% tax
        shipping = self._calculate_shipping(validated_items)
        total = subtotal + tax + shipping

        return StepHandlerResult.success(
            {
                "validated_items": validated_items,
                "subtotal": subtotal,
                "tax": tax,
                "shipping": shipping,
                "total": total,
                "item_count": len(validated_items),
                "validated_at": datetime.now(timezone.utc).isoformat(),
            },
            metadata={
                "operation": "validate_cart",
                "execution_hints": {
                    "items_validated": len(validated_items),
                    "total_amount": total,
                },
            },
        )

    def _validate_cart_item_structure(self, cart_items: list[dict]) -> None:
        for index, item in enumerate(cart_items):
            if not item.get("product_id"):
                raise PermanentError(
                    message=f"Product ID required for item {index + 1}",
                    error_code="MISSING_PRODUCT_ID",
                )

            quantity = item.get("quantity")
            if not quantity or quantity <= 0:
                raise PermanentError(
                    message=f"Valid quantity required for item {index + 1}",
                    error_code="INVALID_QUANTITY",
                )

    def _validate_cart_items(self, cart_items: list[dict]) -> list[dict]:
        validated = []

        for item in cart_items:
            product_id = item["product_id"]
            quantity = item["quantity"]
            product = PRODUCTS.get(product_id)

            if not product:
                raise PermanentError(
                    message=f"Product {product_id} not found",
                    error_code="PRODUCT_NOT_FOUND",
                )

            if not product["active"]:
                raise PermanentError(
                    message=f"Product {product['name']} is not available",
                    error_code="PRODUCT_INACTIVE",
                )

            if product["stock"] < quantity:
                # Temporary failure - retry when stock replenished
                raise RetryableError(
                    message=f"Insufficient stock for {product['name']}",
                    retry_after=30,
                    context={
                        "product_id": product_id,
                        "available": product["stock"],
                        "requested": quantity,
                    },
                )

            validated.append({
                "product_id": product["id"],
                "name": product["name"],
                "price": product["price"],
                "quantity": quantity,
                "line_total": round(product["price"] * quantity, 2),
            })

        return validated

    def _calculate_shipping(self, items: list[dict]) -> float:
        total_weight = sum(item["quantity"] * 0.5 for item in items)
        if total_weight <= 2:
            return 5.99
        elif total_weight <= 10:
            return 9.99
        else:
            return 14.99
```

## Error Handling

Use typed errors to control retry behavior:

```python path=null start=null
from tasker_core.errors import PermanentError, RetryableError

# Permanent error - will NOT be retried
raise PermanentError(
    message="Invalid order data",
    error_code="VALIDATION_ERROR",
)

# Retryable error - will be retried after delay
raise RetryableError(
    message="Payment gateway timeout",
    retry_after=30,  # seconds
    context={"gateway": "stripe"},
)
```

Or return failure results directly:

```python path=null start=null
# Non-retryable failure
return self.failure(
    message="Validation failed",
    error_type="validation_error",
    retryable=False,
    error_code="INVALID_DATA"
)

# Retryable failure
return self.failure(
    message="External service unavailable",
    error_type="network_error",
    retryable=True
)
```

## Task Template Configuration

Define workflows in YAML:

```yaml path=null start=null
name: checkout_workflow
namespace_name: ecommerce
version: 1.0.0
description: "E-commerce checkout workflow"

steps:
  - name: validate_cart
    handler:
      callable: ecommerce.step_handlers.ValidateCartHandler
    dependencies: []

  - name: process_payment
    handler:
      callable: ecommerce.step_handlers.ProcessPaymentHandler
    dependencies:
      - validate_cart

  - name: update_inventory
    handler:
      callable: ecommerce.step_handlers.UpdateInventoryHandler
    dependencies:
      - validate_cart

  - name: send_confirmation
    handler:
      callable: ecommerce.step_handlers.SendConfirmationHandler
    dependencies:
      - process_payment
      - update_inventory
```

## Handler Registration

Register handlers using the registry:

```python path=null start=null
from tasker_core import HandlerRegistry

registry = HandlerRegistry()
registry.register(ValidateCartHandler)
registry.register(ProcessPaymentHandler)
```

## Testing

Write pytest tests for your handlers:

```python path=null start=null
import pytest
from tasker_core import StepHandlerResult
from your_app.handlers import ValidateCartHandler


def build_test_context(task_context: dict):
    """Helper to create test contexts."""
    # Implementation depends on your test setup
    pass


class TestValidateCartHandler:
    def test_validates_cart_successfully(self):
        handler = ValidateCartHandler()
        context = build_test_context({
            "cart_items": [
                {"product_id": 1, "quantity": 2}
            ]
        })

        result = handler.call(context)

        assert result.is_success
        assert result.result["total"] == 59.98 + 4.80 + 5.99

    def test_rejects_empty_cart(self):
        handler = ValidateCartHandler()
        context = build_test_context({"cart_items": []})

        with pytest.raises(PermanentError) as exc_info:
            handler.call(context)

        assert exc_info.value.error_code == "MISSING_CART_ITEMS"

    def test_handles_out_of_stock(self):
        handler = ValidateCartHandler()
        context = build_test_context({
            "cart_items": [
                {"product_id": 1, "quantity": 1000}  # More than stock
            ]
        })

        with pytest.raises(RetryableError):
            handler.call(context)
```

Run tests:

```bash
pytest tests/
```

## Common Patterns

### Type-Safe Context Access

```python path=null start=null
# TAS-137: Cross-language standard API
value = context.get_input("field_name")

# Get with default value
batch_size = context.get_input_or("batch_size", 100)
```

### Dependency Result Access

```python path=null start=null
# Get computed result from upstream step
result = context.get_dependency_result("step_name")

# Extract nested field
value = context.get_dependency_field("step_name", "nested", "field")
```

### Metadata for Observability

```python path=null start=null
return StepHandlerResult.success(
    {"data": processed_data},
    metadata={
        "operation": "my_operation",
        "input_refs": {
            "field": 'context.get_input("field")'
        },
        "execution_hints": {
            "items_processed": 100,
            "duration_ms": 250
        }
    }
)
```

### Handler with Dependencies

```python path=null start=null
class ProcessPaymentHandler(StepHandler):
    handler_name = "process_payment"

    def call(self, context: StepContext) -> StepHandlerResult:
        # Get results from upstream steps
        cart_result = context.get_dependency_result("validate_cart")
        total = cart_result["total"]

        # Get payment info from task context
        payment_method = context.get_input("payment_method")
        payment_token = context.get_input("payment_token")

        # Process payment
        payment_id = self._charge_payment(total, payment_method, payment_token)

        return self.success({
            "payment_id": payment_id,
            "amount_charged": total,
            "status": "completed"
        })
```

### Batch Processing with Checkpoints

```python path=null start=null
class BatchProcessHandler(StepHandler):
    handler_name = "batch_process"

    def call(self, context: StepContext) -> StepHandlerResult:
        # Check for existing checkpoint
        if context.has_checkpoint():
            cursor = context.checkpoint_cursor
            accumulated = context.accumulated_results or {}
        else:
            cursor = 0
            accumulated = {"total": 0, "processed": 0}

        # Process batch
        batch_size = context.get_input_or("batch_size", 100)
        items = self._fetch_items(cursor, batch_size)

        for item in items:
            accumulated["total"] += item["value"]
            accumulated["processed"] += 1

        if len(items) < batch_size:
            # All done
            return self.success(accumulated)
        else:
            # More to process - yield checkpoint
            return StepHandlerResult.checkpoint(
                cursor=cursor + batch_size,
                items_processed=accumulated["processed"],
                accumulated_results=accumulated
            )
```

## Submitting Tasks via Client SDK

The `tasker-py` package includes a `TaskerClient` wrapper that provides keyword-argument methods with sensible defaults and returns typed dataclass responses:

```python path=null start=null
from tasker_core import TaskerClient

# Create a client (defaults: initiator="tasker-core-python", source_system="tasker-core")
client = TaskerClient(initiator="my-service", source_system="my-api")

# Create a task
response = client.create_task(
    "order_fulfillment",
    namespace="ecommerce",
    context={
        "customer": {"id": 123, "email": "customer@example.com"},
        "items": [
            {"product_id": 1, "quantity": 2, "price": 29.99}
        ],
    },
    reason="New order received",
)
print(f"Task created: {response.task_uuid}")
print(f"Status: {response.status}")

# Get task status
task = client.get_task(response.task_uuid)
print(f"Task status: {task.status}")

# List tasks with filters
task_list = client.list_tasks(namespace="ecommerce", limit=10)
for t in task_list.tasks:
    print(f"  {t.task_uuid}: {t.status}")
print(f"Total: {task_list.pagination.total_count}")

# List task steps
steps = client.list_task_steps(response.task_uuid)
for step in steps:
    print(f"Step {step.name}: {step.current_state}")

# Check health
health = client.health_check()
print(f"API healthy: {health.healthy}")

# Cancel a task
client.cancel_task(response.task_uuid)
```

### Available Client Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `create_task(name, *, namespace, context, version, reason, **kwargs)` | `TaskResponse` | Create a new task |
| `get_task(task_uuid)` | `TaskResponse` | Get task by UUID |
| `list_tasks(*, limit, offset, namespace, status)` | `TaskListResponse` | List tasks with filters |
| `cancel_task(task_uuid)` | `dict` | Cancel a task |
| `list_task_steps(task_uuid)` | `list[StepResponse]` | List workflow steps |
| `get_step(task_uuid, step_uuid)` | `StepResponse` | Get specific step |
| `get_step_audit_history(task_uuid, step_uuid)` | `list[StepAuditResponse]` | Get step audit trail |
| `health_check()` | `HealthResponse` | Check API health |

Response types are frozen dataclasses with typed fields (e.g., `TaskResponse.task_uuid`, `TaskResponse.status`, `TaskListResponse.pagination.total_count`).

## Next Steps

- See [Architecture](../architecture/README.md) for system design
- See [Workers Reference](../workers/README.md) for advanced patterns
- See the [tasker-core workers/python](https://github.com/tasker-systems/tasker-core/tree/main/workers/python) for complete examples
