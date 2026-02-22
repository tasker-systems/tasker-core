# Worker Crates: Common Patterns and Practices

**Last Updated**: 2026-01-06
**Audience**: Developers, Architects
**Status**: Active
**Related Docs**: [Worker Event Systems](../architecture/worker-event-systems.md) | [Worker Actors](../architecture/worker-actors.md)

<- Back to [Worker Crates Overview](index.md)

---

This document describes the common patterns and practices shared across all four worker implementations (Rust, Ruby, Python, TypeScript). Understanding these patterns helps developers write consistent handlers regardless of the language.

## Table of Contents

- [Architectural Patterns](#architectural-patterns)
- [Handler Lifecycle](#handler-lifecycle)
- [Error Handling](#error-handling)
- [Polling Architecture](#polling-architecture)
- [Event Bridge Pattern](#event-bridge-pattern)
- [Singleton Pattern](#singleton-pattern)
- [Observability](#observability)
- [Checkpoint Yielding](#checkpoint-yielding)

---

## Architectural Patterns

### Dual-Channel Architecture

All workers implement a dual-channel architecture for non-blocking step execution:

```
┌─────────────────────────────────────────────────────────────────┐
│                    DUAL-CHANNEL PATTERN                         │
└─────────────────────────────────────────────────────────────────┘

    PostgreSQL PGMQ
          │
          ▼
  ┌───────────────────┐
  │  Dispatch Channel │  ──→  Step events flow TO handlers
  └───────────────────┘
          │
          ▼
  ┌───────────────────┐
  │  Handler Execution │  ──→  Business logic runs here
  └───────────────────┘
          │
          ▼
  ┌───────────────────┐
  │ Completion Channel │  ──→  Results flow BACK to orchestration
  └───────────────────┘
          │
          ▼
    Orchestration
```

**Benefits**:

- Fire-and-forget dispatch (non-blocking)
- Bounded concurrency via semaphores
- Results processed independently from dispatch
- Consistent pattern across all languages

### Language-Specific Implementations

| Component | Rust | Ruby | Python |
|-----------|------|------|--------|
| Dispatch Channel | `mpsc::channel` | `poll_step_events` FFI | `poll_step_events` FFI |
| Completion Channel | `mpsc::channel` | `complete_step_event` FFI | `complete_step_event` FFI |
| Concurrency Model | Tokio async tasks | Ruby threads + FFI polling | Python threads + FFI polling |
| GIL Handling | N/A | Pull-based polling | Pull-based polling |

---

## Handler Lifecycle

### Handler Registration

All implementations follow the same registration pattern:

```
1. Define handler (DSL declaration or class/struct)
2. Set handler name identifier
3. Register with HandlerRegistry (automatic for DSL and class-based handlers)
4. Handler ready for resolution
```

> Both **DSL handlers** and **class-based handlers** are discovered automatically at runtime. DSL handlers auto-register their callable name when the handler module is loaded. Class-based handlers are found by the auto-resolver, which scans for any class derived from the base step handler classes. Explicit registration is only needed for edge cases. See [Handler Resolution](../guides/handler-resolution.md) for the full resolver chain and [Class-Based Handlers](../reference/class-based-handlers.md) for the class-based pattern.

**Python (DSL)**:

```python
from tasker_core.step_handler.functional import step_handler, inputs
from app.services.types import OrderInput
from app.services import orders as svc

@step_handler("process_order")
@inputs(OrderInput)
def process_order(inputs: OrderInput, context):
    return svc.process_order(order_id=inputs.order_id, amount=inputs.amount)
```

**Ruby (DSL)**:

```ruby
module Orders
  module StepHandlers
    extend TaskerCore::StepHandler::Functional

    ProcessOrderHandler = step_handler(
      'Orders::StepHandlers::ProcessOrderHandler',
      inputs: Types::Orders::OrderInput
    ) do |inputs:, context:|
      Orders::Service.process_order(order_id: inputs.order_id, amount: inputs.amount)
    end
  end
end
```

**TypeScript (DSL)**:

```typescript
import { defineHandler } from '@tasker-systems/tasker';
import * as svc from '../services/orders';

export const ProcessOrderHandler = defineHandler(
  'Orders.StepHandlers.ProcessOrderHandler',
  { inputs: { orderId: 'order_id', amount: 'amount' } },
  async ({ orderId, amount }) =>
    svc.processOrder(orderId as string, amount as number),
);
```

**Rust** (explicit registration required — no DSL):

```rust
use tasker_worker::worker::handlers::StepHandlerRegistry;

let registry = StepHandlerRegistry::new();
registry.register_fn("process_order",
    Box::new(|ctx, _deps| handlers::orders::process_order(ctx)));
```

### Handler Resolution Flow

```
1. Step event received with handler name
2. Registry.resolve(handler_name) called
3. Handler class instantiated
4. handler.call(context) invoked
5. Result returned to completion channel
```

### Handler Context

**DSL handlers** receive their inputs and dependency results as **typed function parameters** — the DSL extracts and validates these from the raw context automatically. Handlers also receive a `context` parameter for accessing additional task metadata.

**Class-based handlers** receive a context object containing:

| Field | Description |
|-------|-------------|
| `task_uuid` | Unique identifier for the task |
| `step_uuid` | Unique identifier for the step |
| `input_data` | Task context data passed to the step |
| `dependency_results` | Results from parent/dependency steps |
| `step_config` | Configuration from step definition |
| `step_inputs` | Runtime inputs from workflow_step.inputs |
| `retry_count` | Current retry attempt number |
| `max_retries` | Maximum retry attempts allowed |

### Handler Results

All handlers return a structured result indicating success or failure. However, **the APIs differ between Ruby and Python** - this is a known design inconsistency that may be addressed in a future ticket.

**Ruby** - Uses keyword arguments and separate Success/Error types:

```ruby
# Via base handler shortcuts
success(result: { key: "value" }, metadata: { duration_ms: 150 })

failure(
  message: "Something went wrong",
  error_type: "PermanentError",
  error_code: "VALIDATION_ERROR",  # Ruby has error_code field
  retryable: false,
  metadata: { field: "email" }
)

# Or via type factory methods
TaskerCore::Types::StepHandlerCallResult.success(result: { key: "value" })
TaskerCore::Types::StepHandlerCallResult.error(
  error_type: "PermanentError",
  message: "Error message",
  error_code: "ERR_001"
)
```

**Python** - Uses keyword arguments and a single result type:

```python
# Via base handler shortcuts
self.success(result={"key": "value"}, metadata={"duration_ms": 150})

self.failure(
    message="Something went wrong",
    error_type="ValidationError",
    error_code="VALIDATION_ERROR",
    retryable=False,
    metadata={"field": "email"}
)

# Or via class factory methods
StepHandlerResult.success(
    result={"key": "value"},
    metadata={"duration_ms": 150}
)
StepHandlerResult.failure(
    message="Something went wrong",
    error_type="ValidationError",
    error_code="VALIDATION_ERROR",
    retryable=False,
    metadata={"field": "email"}
)
```

**Key Differences**:

| Aspect | Ruby | Python |
|--------|------|--------|
| Factory method names | `.success()`, `.error()` | `.success()`, `.failure()` |
| Result type | `Success` / `Error` structs | Single `StepHandlerResult` class |
| Error code field | `error_code` (freeform) | `error_code` (optional) |
| Argument style | Keyword required (`result:`) | Keyword arguments |

---

## Error Handling

### Error Classification

All workers classify errors into two categories:

| Type | Description | Behavior |
|------|-------------|----------|
| **Retryable** | Transient errors that may succeed on retry | Step re-enqueued up to max_retries |
| **Permanent** | Unrecoverable errors | Step marked as failed immediately |

### HTTP Status Code Classification (ApiHandler)

```
400, 401, 403, 404, 422  →  Permanent Error (client errors)
429                       →  Retryable Error (rate limiting)
500-599                   →  Retryable Error (server errors)
```

### Exception Hierarchy

**Ruby**:

```ruby
TaskerCore::Error                  # Base class
├── TaskerCore::RetryableError     # Transient failures
├── TaskerCore::PermanentError     # Unrecoverable failures
├── TaskerCore::FFIError           # FFI bridge errors
└── TaskerCore::ConfigurationError # Configuration issues
```

**Python** (two modules — FFI/bootstrap errors and execution errors):

```python
# tasker_core.exceptions (FFI / bootstrap)
TaskerError                        # Base class
├── WorkerNotInitializedError      # Worker not bootstrapped
├── WorkerBootstrapError           # Bootstrap failed
├── WorkerAlreadyRunningError      # Double initialization
├── FFIError                       # FFI bridge errors
└── ConversionError                # Type conversion errors

# tasker_core.errors (execution — used in handlers)
TaskerError                        # Base class
├── RetryableError                 # Transient failures (retry with backoff)
│   ├── TimeoutError               # Request/connection timeouts
│   ├── NetworkError               # Network connectivity issues
│   ├── RateLimitError             # Rate limiting (429)
│   ├── ServiceUnavailableError    # Service unavailable (503)
│   └── ResourceContentionError    # Lock/resource conflicts
├── PermanentError                 # Unrecoverable failures
│   ├── ValidationError            # Input validation failures
│   ├── NotFoundError              # Resource not found
│   ├── AuthenticationError        # Authentication failures
│   ├── AuthorizationError         # Permission denied
│   └── BusinessLogicError         # Business rule violations
└── ConfigurationError             # Configuration issues
```

### Error Context Propagation

All errors should include context for debugging:

```python
StepHandlerResult.failure(
    message="Payment gateway timeout",
    error_type="gateway_timeout",
    retryable=True,
    metadata={
        "gateway": "stripe",
        "request_id": "req_xyz",
        "response_time_ms": 30000
    }
)
```

---

## Polling Architecture

### Why Polling?

Ruby and Python workers use a pull-based polling model due to language runtime constraints:

**Ruby**: The Global VM Lock (GVL) prevents Rust from safely calling Ruby methods from Rust threads. Polling allows Ruby to control thread context.

**Python**: The Global Interpreter Lock (GIL) has the same limitation. Python must initiate all cross-language calls.

### Polling Characteristics

| Parameter | Default Value | Description |
|-----------|---------------|-------------|
| Poll Interval | 10ms | Time between polls when no events |
| Max Latency | ~10ms | Time from event generation to processing start |
| Starvation Check | Every 100 polls (1 second) | Detect processing bottlenecks |
| Cleanup Interval | Every 1000 polls (10 seconds) | Clean up timed-out events |

### Poll Loop Structure

```python
while running:
    # 1. Poll for event
    event = poll_step_events()

    if event:
        # 2. Process event through handler
        process_event(event)
    else:
        # 3. Sleep when no events
        time.sleep(0.01)  # 10ms

    # 4. Periodic maintenance
    if poll_count % 100 == 0:
        check_starvation_warnings()

    if poll_count % 1000 == 0:
        cleanup_timeouts()
```

### FFI Contract

Ruby and Python share the same FFI contract:

| Function | Description |
|----------|-------------|
| `poll_step_events()` | Get next pending event (returns None if empty) |
| `complete_step_event(event_id, result)` | Submit handler result |
| `get_ffi_dispatch_metrics()` | Get dispatch channel metrics |
| `check_starvation_warnings()` | Trigger starvation logging |
| `cleanup_timeouts()` | Clean up timed-out events |

---

## Event Bridge Pattern

### Overview

All workers implement an EventBridge (pub/sub) pattern for internal coordination:

```
┌─────────────────────────────────────────────────────────────────┐
│                      EVENT BRIDGE PATTERN                        │
└─────────────────────────────────────────────────────────────────┘

  Publishers                    EventBridge                 Subscribers
  ─────────                    ───────────                 ───────────
  HandlerRegistry  ──publish──→            ──notify──→  StepExecutionSubscriber
  EventPoller      ──publish──→  [Events]  ──notify──→  MetricsCollector
  Worker           ──publish──→            ──notify──→  Custom Subscribers
```

### Standard Event Names

| Event | Description | Payload |
|-------|-------------|---------|
| `handler_registered` | Handler added to registry | `(name, handler_class)` |
| `step_execution_received` | Step event received | `FfiStepEvent` |
| `step_execution_completed` | Handler finished | `StepHandlerResult` |
| `worker_started` | Worker bootstrap complete | `worker_id` |
| `worker_stopped` | Worker shutdown | `worker_id` |

### Implementation Libraries

| Language | Library | Pattern |
|----------|---------|---------|
| Ruby | `dry-events` | Publisher/Subscriber |
| Python | `pyee` | EventEmitter |
| Rust | Native channels | mpsc |

### Usage Example (Python)

```python
from tasker_core import EventBridge, EventNames

bridge = EventBridge.instance()

# Subscribe to events
def on_step_received(event):
    print(f"Processing step {event.step_uuid}")

bridge.subscribe(EventNames.STEP_EXECUTION_RECEIVED, on_step_received)

# Publish events
bridge.publish(EventNames.HANDLER_REGISTERED, "my_handler", MyHandler)
```

---

## Singleton Pattern

### Worker State Management

All workers store global state in a thread-safe singleton:

```
┌─────────────────────────────────────────────────────────────────┐
│                    SINGLETON WORKER STATE                        │
└─────────────────────────────────────────────────────────────────┘

    Thread-Safe Global
           │
           ▼
    ┌──────────────────┐
    │   WorkerSystem   │
    │  ┌────────────┐  │
    │  │ Mutex/Lock │  │
    │  │  Inner     │  │
    │  │  State     │  │
    │  └────────────┘  │
    └──────────────────┘
           │
           ├──→ HandlerRegistry
           ├──→ EventBridge
           ├──→ EventPoller
           └──→ Configuration
```

### Singleton Classes

| Language | Singleton Implementation |
|----------|------------------------|
| Rust | `OnceLock<Mutex<WorkerSystem>>` |
| Ruby | `Singleton` module |
| Python | Class-level `_instance` with `instance()` classmethod |

### Reset for Testing

All singletons provide reset methods for test isolation:

```python
# Python
HandlerRegistry.reset_instance()
EventBridge.reset_instance()
```

```ruby
# Ruby
TaskerCore::Registry::HandlerRegistry.reset_instance!
```

---

## Observability

### Health Checks

All workers expose health information via FFI:

```python
from tasker_core import get_health_check

health = get_health_check()
# Returns: HealthCheck with component statuses
```

### Metrics

Standard metrics available from all workers:

| Metric | Description |
|--------|-------------|
| `pending_count` | Events awaiting processing |
| `in_flight_count` | Events currently being processed |
| `completed_count` | Successfully completed events |
| `failed_count` | Failed events |
| `starvation_detected` | Whether events are timing out |
| `starving_event_count` | Events exceeding timeout threshold |

### Structured Logging

All workers use structured logging with consistent fields:

```python
from tasker_core import log_info, LogContext

context = LogContext(
    correlation_id="abc-123",
    task_uuid="task-456",
    operation="process_order"
)
log_info("Processing order", context)
```

---

## Specialized Handlers

All handler types — including API, Decision, and Batchable — support both DSL and class-based patterns. The DSL approach is recommended for new projects. See [Example Handlers](example-handlers.md) for full cross-language examples and [Class-Based Handlers](../reference/class-based-handlers.md) for the class-based alternative.

### Handler Type Hierarchy

**Ruby** (class hierarchy / DSL factories):

```
TaskerCore::StepHandler::Base
├── TaskerCore::StepHandler::Api        # HTTP/REST API integration
├── TaskerCore::StepHandler::Decision   # Dynamic workflow decisions
└── TaskerCore::StepHandler::Batchable  # Batch processing support

TaskerCore::StepHandler::Functional     # DSL module
├── step_handler()                      # Standard step
├── decision_handler()                  # Decision routing
├── api_handler()                       # HTTP API integration
├── batch_analyzer()                    # Batch analysis
└── batch_worker()                      # Batch processing
```

**Python** (class hierarchy / DSL decorators):

```
StepHandler (ABC)
├── ApiHandler         # HTTP/REST API integration
├── DecisionHandler    # Dynamic workflow decisions
└── + Batchable        # Batch processing (mixin)

Decorators (tasker_core.step_handler.functional)
├── @step_handler      # Standard step
├── @decision_handler  # Decision routing
├── @api_handler       # HTTP API integration
├── @batch_analyzer    # Batch analysis
└── @batch_worker      # Batch processing
```

**TypeScript** (factory functions):

```
defineHandler()          # Standard step
defineDecisionHandler()  # Decision routing
defineApiHandler()       # HTTP API integration
defineBatchAnalyzer()    # Batch analysis
defineBatchWorker()      # Batch processing
```

**Rust** (trait composition — no DSL):

```
StepHandler (trait)
+ APICapable           # HTTP client methods
+ DecisionCapable      # Workflow routing
+ BatchableCapable     # Cursor-based batch processing
```

### Quick DSL Examples

**Decision** — returns `Decision.route(steps)` or `Decision.skip(reason)`:

```python
@decision_handler("routing_decision")
@inputs('amount')
def routing_decision(amount, context):
    if float(amount or 0) < 1000:
        return Decision.route(['auto_approve'], route_type='automatic')
    return Decision.route(['manager_approval', 'finance_review'], route_type='dual')
```

**API** — receives `api` with HTTP methods and automatic error classification:

```python
@api_handler("fetch_user", base_url="https://api.example.com")
@inputs('user_id')
def fetch_user(user_id, api, context):
    response = api.get(f"/users/{user_id}")
    return api.api_success(result={"user_id": user_id, "email": response["email"]})
```

**Batch** — analyzer returns `BatchConfig`, workers receive `batch_context`:

```python
@batch_analyzer("analyze_csv", worker_template="process_csv_batch")
@inputs('csv_file_path')
def analyze_csv(csv_file_path, context):
    return BatchConfig(total_items=count_csv_rows(csv_file_path), batch_size=100)

@batch_worker("process_csv_batch")
def process_csv_batch(batch_context, context):
    records = read_csv_range(batch_context.start_cursor, batch_context.batch_size)
    return {"items_processed": len(records), "items_succeeded": len(records)}
```

---

## Checkpoint Yielding

Checkpoint yielding enables batch workers to persist progress and yield control back to the orchestrator for re-dispatch. This is essential for long-running batch operations.

### When to Use

- Processing takes longer than visibility timeout
- You need resumable processing after failures
- Long-running operations need progress visibility

### Cross-Language API

All Batchable handlers provide `checkpoint_yield()` (or `checkpointYield()` in TypeScript):

**Ruby**:

```ruby
class MyBatchWorker < TaskerCore::StepHandler::Batchable
  def call(context)
    batch_ctx = get_batch_context(context)

    # Resume from checkpoint if present
    start = batch_ctx.has_checkpoint? ? batch_ctx.checkpoint_cursor : 0

    items.each_with_index do |item, idx|
      process_item(item)

      # Checkpoint every 1000 items
      if (idx + 1) % 1000 == 0
        checkpoint_yield(
          cursor: start + idx + 1,
          items_processed: idx + 1,
          accumulated_results: { partial: "data" }
        )
      end
    end

    batch_worker_success(items_processed: items.size, items_succeeded: items.size)
  end
end
```

**Python**:

```python
class MyBatchWorker(StepHandler, Batchable):
    def call(self, context):
        batch_ctx = self.get_batch_context(context)

        # Resume from checkpoint if present
        start = batch_ctx.checkpoint_cursor if batch_ctx.has_checkpoint() else 0

        for idx, item in enumerate(items):
            self.process_item(item)

            # Checkpoint every 1000 items
            if (idx + 1) % 1000 == 0:
                self.checkpoint_yield(
                    cursor=start + idx + 1,
                    items_processed=idx + 1,
                    accumulated_results={"partial": "data"}
                )

        return self.batch_worker_success(items_processed=len(items), items_succeeded=len(items))
```

**TypeScript**:

```typescript
class MyBatchWorker extends BatchableHandler {
  async call(context: StepContext): Promise<StepHandlerResult> {
    const batchCtx = this.getBatchContext(context);

    // Resume from checkpoint if present
    const start = batchCtx.hasCheckpoint() ? batchCtx.checkpointCursor : 0;

    for (let idx = 0; idx < items.length; idx++) {
      await this.processItem(items[idx]);

      // Checkpoint every 1000 items
      if ((idx + 1) % 1000 === 0) {
        await this.checkpointYield({
          cursor: start + idx + 1,
          itemsProcessed: idx + 1,
          accumulatedResults: { partial: "data" }
        });
      }
    }

    return this.batchWorkerSuccess({
      itemsProcessed: items.length,
      itemsSucceeded: items.length,
      itemsFailed: 0,
      itemsSkipped: 0,
      results: [],
      errors: [],
      lastCursor: null,
    });
  }
}
```

### BatchWorkerContext Checkpoint Accessors

All languages provide consistent accessors for checkpoint data:

| Accessor | Ruby | Python | TypeScript |
|----------|------|--------|------------|
| Cursor position | `checkpoint_cursor` | `checkpoint_cursor` | `checkpointCursor` |
| Accumulated data | `accumulated_results` | `accumulated_results` | `accumulatedResults` |
| Has checkpoint? | `has_checkpoint?` | `has_checkpoint()` | `hasCheckpoint()` |
| Items processed | `checkpoint_items_processed` | `checkpoint_items_processed` | `checkpointItemsProcessed` |

### FFI Contract

| Function | Description |
|----------|-------------|
| `checkpoint_yield_step_event(event_id, data)` | Persist checkpoint and re-dispatch step |

### Key Invariants

1. **Checkpoint-Persist-Then-Redispatch**: Progress saved before re-dispatch
2. **Step Stays InProgress**: No state machine transitions during yield
3. **Handler-Driven**: Handlers decide when to checkpoint

See [Batch Processing Guide - Checkpoint Yielding](../guides/batch-processing.md#checkpoint-yielding-tas-125) for comprehensive documentation.

---

## Best Practices

### 1. Keep Handlers Focused

Each handler should do one thing well:

- Validate input
- Perform single operation
- Return clear result

### 2. Use Error Classification

Always specify whether errors are retryable:

```python
# Good - clear error classification
return self.failure("API rate limit", retryable=True)

# Bad - ambiguous error handling
raise Exception("API error")
```

### 3. Include Context in Errors

```python
return StepHandlerResult.failure(
    message="Database connection failed",
    error_type="database_error",
    retryable=True,
    metadata={
        "host": "db.example.com",
        "port": 5432,
        "connection_timeout_ms": 5000
    }
)
```

### 4. Use Structured Logging

```python
log_info("Order processed", {
    "order_id": order_id,
    "total": total,
    "items_count": len(items)
})
```

### 5. Test Handler Isolation

Reset singletons between tests:

```python
def setup_method(self):
    HandlerRegistry.reset_instance()
    EventBridge.reset_instance()
```

---

## See Also

- [Worker Crates Overview](index.md) - High-level introduction
- [Rust Worker](rust.md) - Native Rust implementation
- [Ruby Worker](ruby.md) - Ruby gem documentation
- [Python Worker](python.md) - Python package documentation
- [Worker Event Systems](../architecture/worker-event-systems.md) - Detailed architecture
- [Worker Actors](../architecture/worker-actors.md) - Actor pattern documentation
