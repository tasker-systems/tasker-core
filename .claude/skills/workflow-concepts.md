# Skill: Workflow Concepts

## When to Use

Use this skill when implementing workflows, understanding task/step relationships, working with handler patterns (API, Decision, Batch), conditional workflows, or troubleshooting stuck tasks.

## Core Concepts

### Tasks and Steps

A **Task** is a workflow definition with a set of **Steps** that execute in dependency order.

- Tasks are created from **task templates** that define the steps, their handlers, and dependencies
- Steps execute when their dependencies are satisfied
- Steps are processed by workers via handler dispatch
- Results flow back to orchestration for evaluation and next-step triggering

### Task Lifecycle

```
Create Task -> Pending -> Initializing -> EnqueuingSteps -> StepsInProcess
    -> EvaluatingResults -> [more steps?] -> Complete
```

### Step Lifecycle

```
Pending -> Enqueued -> InProgress -> EnqueuedForOrchestration -> Complete
```

Error path: `InProgress -> EnqueuedAsErrorForOrchestration -> WaitingForRetry -> Pending`

## Handler Patterns

### Base Handler

All handlers follow the `call(context)` pattern and return results:

```ruby
class MyHandler < TaskerCore::StepHandler::Base
  def call(context)
    # Business logic
    success(result: { data: "value" })
  end
end
```

### API Handler (HTTPCapable)

For HTTP operations. Provides `get`, `post`, `put`, `delete` methods:

```ruby
class MyAPIHandler < TaskerCore::StepHandler::Base
  include TaskerCore::StepHandler::Mixins::API

  def call(context)
    response = get("/api/resource", params: { id: context.input_data["id"] })
    success(result: response.body)
  end
end
```

### Decision Handler (Conditional Workflows)

For branching logic. Returns which steps to activate:

```ruby
class MyDecisionHandler < TaskerCore::StepHandler::Base
  include TaskerCore::StepHandler::Mixins::Decision

  def call(context)
    if context.input_data["amount"] > 1000
      decision_success(steps: ["high_value_review", "compliance_check"], result_data: {})
    else
      decision_success(steps: ["standard_processing"], result_data: {})
    end
  end
end
```

Use `decision_no_branches(result_data: {})` when no branches should activate.

### Batchable Handler (Batch Processing)

For cursor-based batch processing:

```ruby
class MyBatchHandler < TaskerCore::StepHandler::Base
  include TaskerCore::StepHandler::Mixins::Batchable

  def call(context)
    batch_ctx = get_batch_context(context)
    items = fetch_items(batch_ctx.cursor_position, batch_ctx.batch_size)

    if items.empty?
      handle_no_op_worker(batch_ctx)
    else
      process(items)
      batch_worker_complete(processed_count: items.size, result_data: { processed: items.size })
    end
  end
end
```

## Composition Over Inheritance

Handlers gain capabilities via mixins, NOT class hierarchies:

```
Not: class Handler < APIHandler
But: class Handler < Base; include API, include Decision
```

This pattern enables selective capability inclusion and avoids diamond inheritance problems.

## Step Context

The `StepContext` provides:

| Field | Type | Description |
|-------|------|-------------|
| `task_uuid` | String | Unique task identifier |
| `step_uuid` | String | Unique step identifier |
| `input_data` | Dict/Hash | Input data for the step |
| `step_config` | Dict/Hash | Handler configuration |
| `dependency_results` | Wrapper | Results from parent steps |
| `retry_count` | Integer | Current retry attempt |
| `max_retries` | Integer | Maximum retry attempts |

Methods: `get_task_field(name)`, `get_dependency_result(step_name)`

## Result Factories

| Operation | Pattern |
|-----------|---------|
| Success | `success(result_data, metadata?)` |
| Failure | `failure(message, error_type, error_code?, retryable?, metadata?)` |

## Retry Semantics

- Steps can be retryable or permanent failures
- Retryable failures enter `WaitingForRetry` state with backoff
- Max retries configured per step in template
- Backoff configuration in TOML: base delay, max delay, multiplier
- After max retries exhausted: permanent `Error` state

## Dead Letter Queue (DLQ)

- Permanently failed steps can be inspected and manually resolved
- `ResolvedManually` terminal state for operator intervention
- Task-level `BlockedByFailures` state when failures prevent progress

## Identity Strategy

Tasks use identity hashing for deduplication:
- One active task per (namespace, external_id) combination
- Unique constraints at database level prevent duplicates
- Identity hash strategy configurable per task template

## Troubleshooting Stuck Tasks

1. Check `docs/architecture/states-and-lifecycles.md` for valid state transitions
2. Check `docs/guides/dlq-system.md` if task appears stuck
3. Check `docs/guides/retry-semantics.md` for error handling behavior
4. Query task/step transitions for audit trail

## References

- Batch processing: `docs/guides/batch-processing.md`
- Conditional workflows: `docs/guides/conditional-workflows.md`
- Retry semantics: `docs/guides/retry-semantics.md`
- DLQ system: `docs/guides/dlq-system.md`
- Identity strategy: `docs/guides/identity-strategy.md`
- Handler patterns: `docs/workers/patterns-and-practices.md`
- Cross-language: `docs/principles/cross-language-consistency.md`
