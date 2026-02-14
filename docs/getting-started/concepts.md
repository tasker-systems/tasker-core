# Core Concepts

This page explains the fundamental building blocks of Tasker.

## Tasks

A **Task** is a unit of work submitted to Tasker for execution. Tasks have:

- A **task template** that defines the workflow structure
- An **initiator** identifying the source (e.g., `user:123`, `system:scheduler`)
- A **context** containing input data and metadata
- A **state** managed by a 12-state machine (see below)

```json
{
  "name": "order_fulfillment",
  "initiator": "api:checkout",
  "context": {
    "order_id": "ORD-12345",
    "customer_email": "customer@example.com"
  }
}
```

## Task Templates

A **Task Template** is a YAML definition of a workflow. It specifies:

- **Steps** to execute
- **Dependencies** between steps (creating a DAG)
- **Handler mappings** connecting steps to your code

```yaml
name: order_fulfillment
namespace_name: ecommerce
version: 1.0.0
steps:
  - name: validate_order
    handler:
      callable: OrderValidationHandler
    dependencies: []

  - name: reserve_inventory
    handler:
      callable: InventoryHandler
    dependencies:
      - validate_order

  - name: charge_payment
    handler:
      callable: PaymentHandler
    dependencies:
      - validate_order
```

## Steps

A **Step** is a single operation within a workflow. Steps:

- Execute independently once dependencies are satisfied
- Can run in parallel when they have no mutual dependencies
- Return results that downstream steps can access
- Can be retried on failure

### Task Lifecycle

Tasks progress through a multi-phase lifecycle managed by the orchestration actors:

```
Pending → Initializing → EnqueuingSteps → StepsInProcess → EvaluatingResults → Complete
```

The evaluating phase may loop back to enqueue more steps as dependencies are satisfied, wait for retries, or transition to terminal states (`Complete`, `Error`, `Cancelled`, `ResolvedManually`). Tasks support cancellation from any non-terminal state and manual resolution from `BlockedByFailures`.

### Step Lifecycle

Steps follow a worker-to-orchestration handoff pattern through 10 states:

```
Pending → Enqueued → InProgress → EnqueuedForOrchestration → Complete
```

After a worker executes a step, the result is enqueued back to orchestration for processing. Steps can also transition through `WaitingForRetry` for automatic retry with backoff, or be cancelled, failed, or manually resolved.

For the full state machine diagrams and transition tables, see [States and Lifecycles](../architecture/states-and-lifecycles.md).

## Step Handlers

A **Step Handler** is your code that executes a step's business logic. Handlers:

- Extend a base class (`StepHandler` in Ruby/Python/TS, `RustStepHandler` trait in Rust)
- Implement a `call()` method
- Access task context via `get_input()`
- Access upstream results via `get_dependency_result()` (or `sequence.get()` in Ruby)

```python
from tasker_core import StepHandler, StepContext, StepHandlerResult

class OrderValidationHandler(StepHandler):
    handler_name = "order_validation"

    def call(self, context: StepContext) -> StepHandlerResult:
        order_id = context.get_input("order_id")
        # Validate the order...
        return StepHandlerResult.success({"valid": True, "order_total": 99.99})
```

## Dependency Results

Steps can access results from their dependencies:

```python
class PaymentHandler(StepHandler):
    handler_name = "payment"

    def call(self, context: StepContext) -> StepHandlerResult:
        # Get result from upstream step
        validation = context.get_dependency_result("validate_order")
        order_total = validation["order_total"]
        # Process payment...
        return StepHandlerResult.success({"charged": True})
```

## Workflow Steps

A **Workflow Step** is a special step that starts another task as a sub-workflow:

```yaml
steps:
  - name: process_line_items
    handler:
      callable: WorkflowHandler
      initialization:
        task_template: line_item_processing
```

This enables composing complex workflows from simpler building blocks.

## Error Handling

Tasker distinguishes between error types:

| Error Type | Behavior |
|------------|----------|
| `PermanentError` | No retry; step fails immediately |
| `RetryableError` | Automatically retried with backoff |

```python
from tasker_core.errors import PermanentError, RetryableError

def call(self, context):
    if invalid_input:
        raise PermanentError(message="Invalid order ID format", error_code="INVALID_ID")
    if service_unavailable:
        raise RetryableError(message="Payment gateway timeout", error_code="GATEWAY_TIMEOUT")
```

## Next Steps

- [Installation](install.md) — Set up Tasker
- [Your First Handler](first-handler.md) — Write your first step handler
- [Your First Workflow](first-workflow.md) — Create a complete workflow
