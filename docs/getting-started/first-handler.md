# Your First Handler

This guide walks you through writing your first step handler.

## What is a Handler?

A **Step Handler** is a class that executes business logic for a workflow step. Handlers:

- Receive task context (input data)
- Perform operations (API calls, database queries, calculations)
- Return results for downstream steps

## Minimal Handler

Here's a minimal handler in each supported language:

### Python

```python
from tasker_core import StepHandler, StepContext, StepHandlerResult

class GreetingHandler(StepHandler):
    handler_name = "greeting"

    def call(self, context: StepContext) -> StepHandlerResult:
        name = context.get_input("name") or "World"
        return StepHandlerResult.success({"greeting": f"Hello, {name}!"})
```

### Ruby

```ruby
require 'tasker_core'

class GreetingHandler < TaskerCore::StepHandler::Base
  def call(context)
    name = context.get_input("name") || "World"
    TaskerCore::Types::StepHandlerCallResult.success(
      result: { greeting: "Hello, #{name}!" }
    )
  end
end
```

### TypeScript

```typescript
import { StepHandler, StepHandlerResult } from '@tasker-systems/tasker';
import type { StepContext } from '@tasker-systems/tasker';

export class GreetingHandler extends StepHandler {
  static handlerName = 'greeting';

  async call(context: StepContext): Promise<StepHandlerResult> {
    const name = context.getInput<string>('name') ?? 'World';
    return this.success({ greeting: `Hello, ${name}!` });
  }
}
```

### Rust

```rust
use async_trait::async_trait;
use anyhow::Result;
use serde_json::json;
use tasker_shared::messaging::StepExecutionResult;
use tasker_shared::types::TaskSequenceStep;

pub struct GreetingHandler;

#[async_trait]
impl RustStepHandler for GreetingHandler {
    async fn call(&self, step_data: &TaskSequenceStep) -> Result<StepExecutionResult> {
        let name: String = step_data.get_input_or("name", "World".to_string());
        Ok(StepExecutionResult::success(
            step_data.workflow_step.workflow_step_uuid,
            json!({ "greeting": format!("Hello, {}!", name) }),
            0,
            None,
        ))
    }

    fn name(&self) -> &'static str {
        "greeting"
    }
}
```

## Key Handler Methods

| Method | Purpose |
|--------|----------|
| `call(context)` | Main entry point; implement your logic here |
| `context.get_input(key)` | Get a value from task context |
| `context.get_dependency_result(step)` | Get result from an upstream step |

## Registering Handlers

Handlers are resolved by matching the `handler.callable` field in task templates:

```python
# Python - handler_name attribute maps to task template
class GreetingHandler(StepHandler):
    handler_name = "greeting"  # matches handler.callable in YAML
```

The `handler.callable` in your task template YAML must match either the registered handler name or the class path (e.g., `"GreetingHandler"`).

## Error Handling

Use typed errors to control retry behavior:

```python
from tasker_core import StepHandler, StepContext, StepHandlerResult
from tasker_core.errors import PermanentError, RetryableError

class ValidatingHandler(StepHandler):
    handler_name = "validating"

    def call(self, context: StepContext) -> StepHandlerResult:
        data = context.get_input("data")
        
        if not data:
            raise PermanentError(message="Missing required input 'data'", error_code="MISSING_DATA")
        
        try:
            result = external_api_call(data)
        except ConnectionError:
            raise RetryableError(message="API temporarily unavailable", error_code="API_TIMEOUT")
        
        return StepHandlerResult.success({"processed": result})
```

## Next Steps

- [Your First Workflow](first-workflow.md) — Connect handlers into a workflow
- [Language Guides](choosing-your-package.md) — Deep dives for each language
