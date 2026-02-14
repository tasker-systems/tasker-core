# Client Wrapper API

**Last Updated**: 2026-02-13
**Audience**: Developers
**Status**: Active
**Related Docs**: [API Convergence Matrix](api-convergence-matrix.md) | [Workers Overview](README.md)

<- Back to [Workers Overview](README.md)

---

Each FFI worker package (Ruby, Python, TypeScript) includes a high-level client wrapper for the orchestration API. The wrappers provide keyword-argument methods with sensible defaults and return typed response objects, removing the need to construct raw request hashes or parse untyped responses.

## Overview

| | Ruby | Python | TypeScript |
|---|---|---|---|
| **Module** | `TaskerCore::Client` | `TaskerClient` class | `TaskerClient` class |
| **Import** | `require 'tasker_core'` | `from tasker_core import TaskerClient` | `import { TaskerClient } from '@tasker-systems/tasker'` |
| **Response Types** | `Dry::Struct` (e.g., `ClientTypes::TaskResponse`) | Frozen dataclasses (e.g., `TaskResponse`) | Generated DTO types (e.g., `ClientTaskResponse`) |
| **Error Handling** | Falls back to raw `Hash` on schema mismatch | Falls back to raw `dict` on missing fields | Throws `TaskerClientError` on failure |

## Ruby

### Usage

```ruby
require 'tasker_core'

# All methods are module_function — call directly on TaskerCore::Client
response = TaskerCore::Client.create_task(
  name: 'order_processing',
  namespace: 'ecommerce',
  context: { order_id: 123, items: [...] },
  initiator: 'my-service',       # default: 'tasker-core-ruby'
  source_system: 'my-api',       # default: 'tasker-core'
  reason: 'New order received'   # default: 'Task requested'
)

response.task_uuid  # => "550e8400-..."
response.status     # => "pending"
```

### Methods

| Method | Signature | Returns |
|--------|-----------|---------|
| `create_task` | `(name:, namespace: 'default', context: {}, version: '1.0.0', initiator:, source_system:, reason:, **options)` | `ClientTypes::TaskResponse` |
| `get_task` | `(task_uuid)` | `ClientTypes::TaskResponse` |
| `list_tasks` | `(limit: 50, offset: 0, namespace: nil, status: nil)` | `ClientTypes::TaskListResponse` |
| `cancel_task` | `(task_uuid)` | `Hash` |
| `list_task_steps` | `(task_uuid)` | `Array<ClientTypes::StepResponse>` |
| `get_step` | `(task_uuid, step_uuid)` | `ClientTypes::StepResponse` |
| `get_step_audit_history` | `(task_uuid, step_uuid)` | `Array<ClientTypes::StepAuditResponse>` |
| `health_check` | `()` | `ClientTypes::HealthResponse` |

### Response Types

All response types are `Dry::Struct` classes defined in `TaskerCore::Types::ClientTypes`. Access fields as method calls (e.g., `response.task_uuid`, `list.pagination.total_count`). If the API returns fields that don't match the schema, the raw `Hash` is returned instead for forward-compatibility.

### ActionController::Parameters

`create_task` automatically converts Rails `ActionController::Parameters` to plain hashes via `deep_to_hash`, so you can pass `params[:context]` directly from controllers.

---

## Python

### Usage

```python
from tasker_core import TaskerClient

# Create a client with custom defaults
client = TaskerClient(
    initiator="my-service",       # default: "tasker-core-python"
    source_system="my-api",       # default: "tasker-core"
)

response = client.create_task(
    "order_processing",
    namespace="ecommerce",
    context={"order_id": 123, "items": [...]},
    reason="New order received",
)

response.task_uuid  # => "550e8400-..."
response.status     # => "pending"
```

### Methods

| Method | Signature | Returns |
|--------|-----------|---------|
| `create_task` | `(name, *, namespace="default", context=None, version="1.0.0", reason="Task requested", **kwargs)` | `TaskResponse` |
| `get_task` | `(task_uuid: str)` | `TaskResponse` |
| `list_tasks` | `(*, limit=50, offset=0, namespace=None, status=None)` | `TaskListResponse` |
| `cancel_task` | `(task_uuid: str)` | `dict[str, Any]` |
| `list_task_steps` | `(task_uuid: str)` | `list[StepResponse]` |
| `get_step` | `(task_uuid: str, step_uuid: str)` | `StepResponse` |
| `get_step_audit_history` | `(task_uuid: str, step_uuid: str)` | `list[StepAuditResponse]` |
| `health_check` | `()` | `HealthResponse` |

### Response Types

All response types are frozen dataclasses with `from_dict(data)` classmethods:

- `TaskResponse` — `task_uuid`, `name`, `namespace`, `status`, `context`, `steps`, etc.
- `TaskListResponse` — `tasks: list[TaskResponse]`, `pagination: PaginationInfo`
- `StepResponse` — `step_uuid`, `task_uuid`, `name`, `current_state`, `attempts`, etc.
- `StepAuditResponse` — `audit_uuid`, `step_name`, `to_state`, `success`, etc.
- `HealthResponse` — `healthy`, `status`, `timestamp`
- `PaginationInfo` — `page`, `per_page`, `total_count`, `total_pages`, `has_next`, `has_previous`

### Exports

All types are re-exported from `tasker_core` with `Client` prefix to avoid collisions:

```python
from tasker_core import (
    TaskerClient,
    ClientTaskResponse,
    ClientTaskListResponse,
    ClientStepResponse,
    ClientStepAuditResponse,
    ClientHealthResponse,
    ClientPaginationInfo,
)
```

---

## TypeScript

### Usage

```typescript
import { FfiLayer, TaskerClient } from '@tasker-systems/tasker';

const ffiLayer = new FfiLayer();
await ffiLayer.load();
const client = new TaskerClient(ffiLayer);

const response = client.createTask({
  name: 'order_processing',
  namespace: 'ecommerce',          // default: 'default'
  context: { orderId: 123 },       // default: {}
  initiator: 'my-service',         // default: 'tasker-core-typescript'
  sourceSystem: 'my-api',          // default: 'tasker-core'
  reason: 'New order received',    // default: 'Task requested'
});

response.task_uuid;  // "550e8400-..."
response.status;     // "pending"
```

### Methods

| Method | Signature | Returns |
|--------|-----------|---------|
| `createTask` | `(options: CreateTaskOptions)` | `ClientTaskResponse` |
| `getTask` | `(taskUuid: string)` | `ClientTaskResponse` |
| `listTasks` | `(options?: ListTasksOptions)` | `ClientTaskListResponse` |
| `cancelTask` | `(taskUuid: string)` | `void` |
| `listTaskSteps` | `(taskUuid: string)` | `ClientStepResponse[]` |
| `getStep` | `(taskUuid: string, stepUuid: string)` | `ClientStepResponse` |
| `getStepAuditHistory` | `(taskUuid: string, stepUuid: string)` | `ClientStepAuditResponse[]` |
| `healthCheck` | `()` | `ClientHealthResponse` |

### Interfaces

```typescript
interface CreateTaskOptions {
  name: string;
  namespace?: string;                    // default: 'default'
  context?: Record<string, unknown>;     // default: {}
  version?: string;                      // default: '1.0.0'
  initiator?: string;                    // default: 'tasker-core-typescript'
  sourceSystem?: string;                 // default: 'tasker-core'
  reason?: string;                       // default: 'Task requested'
  tags?: string[];
  correlationId?: string;                // default: crypto.randomUUID()
  parentCorrelationId?: string;
  idempotencyKey?: string;
  priority?: number;
}

interface ListTasksOptions {
  limit?: number;    // default: 50
  offset?: number;   // default: 0
  namespace?: string;
  status?: string;
}
```

### Error Handling

All methods unwrap the raw FFI `ClientResult` envelope. On failure, a `TaskerClientError` is thrown:

```typescript
import { TaskerClientError } from '@tasker-systems/tasker';

try {
  const task = client.getTask('nonexistent-uuid');
} catch (error) {
  if (error instanceof TaskerClientError) {
    console.error(error.message);
    console.error(error.recoverable);  // boolean — whether retry is appropriate
  }
}
```

### Exports

```typescript
export {
  TaskerClient,
  TaskerClientError,
  type CreateTaskOptions,
  type ListTasksOptions,
} from '@tasker-systems/tasker';
```

---

## Raw FFI (Advanced)

The client wrappers call through to raw FFI functions. For advanced use cases or when the wrapper doesn't expose a needed field, the raw FFI is still available:

| Operation | Ruby | Python | TypeScript |
|-----------|------|--------|------------|
| Create task | `TaskerCore::FFI.client_create_task(hash)` | `from tasker_core._tasker_core import client_create_task` | `runtime.clientCreateTask(json)` |
| Get task | `TaskerCore::FFI.client_get_task(uuid)` | `client_get_task(uuid)` | `runtime.clientGetTask(uuid)` |
| List tasks | `TaskerCore::FFI.client_list_tasks(limit, offset, ns, status)` | `client_list_tasks(limit, offset, ns, status)` | `runtime.clientListTasks(json)` |
| Cancel task | `TaskerCore::FFI.client_cancel_task(uuid)` | `client_cancel_task(uuid)` | `runtime.clientCancelTask(uuid)` |
| List steps | `TaskerCore::FFI.client_list_task_steps(uuid)` | `client_list_task_steps(uuid)` | `runtime.clientListTaskSteps(uuid)` |
| Get step | `TaskerCore::FFI.client_get_step(task, step)` | `client_get_step(task, step)` | `runtime.clientGetStep(task, step)` |
| Audit history | `TaskerCore::FFI.client_get_step_audit_history(task, step)` | `client_get_step_audit_history(task, step)` | `runtime.clientGetStepAuditHistory(task, step)` |
| Health check | `TaskerCore::FFI.client_health_check` | `client_health_check()` | `runtime.clientHealthCheck()` |

Raw FFI returns plain `Hash`/`dict`/`ClientResult` — no type wrapping. TypeScript raw FFI returns a `ClientResult` envelope (`{ success, data, error, recoverable }`).
