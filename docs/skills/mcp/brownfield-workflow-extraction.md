# Skill: Brownfield Workflow Extraction with Tasker MCP

Use this skill when a developer has **existing service-layer code** and wants to
orchestrate it as a Tasker workflow. The code already works — functions exist, classes
exist, maybe even a sequential pipeline exists — but it lacks the benefits of DAG-based
orchestration: parallelism, idempotent retry, step-level observability, and distributed
execution.

## When to Apply

- Developer says they want to "add Tasker to existing code", "orchestrate these
  services", or "make this pipeline more resilient"
- There's an existing sequence of function calls that could benefit from parallelism
  or independent retry
- A synchronous process needs to become asynchronous or distributed
- Developer wants observability into a multi-step process that currently runs as a
  single opaque operation

## Agent Posture

You are **analyzing and proposing**. Unlike greenfield design where you co-create from
scratch, here you must understand existing code before suggesting changes. Your
proposals should be conservative — preserve working logic, minimize refactoring, and
clearly explain what changes and why.

**Key principle**: The developer's existing code is the source of truth for business
logic. Tasker adds orchestration around it, not instead of it.

## Workflow Phases

### Phase 1: Understand the Existing Code

Before proposing any workflow, thoroughly read and understand the codebase.

**What to look for:**

1. **Service layer functions/methods**: Look for modules named `services/`, `lib/`,
   `utils/`, or classes with names like `OrderService`, `PaymentProcessor`. These are
   your candidate step implementations
2. **Data flow**: Trace what each function takes as input and returns as output. These
   become your `result_schema` fields
3. **Sequential chains**: Look for code that calls function A, then passes results to
   function B, then to C. This is a linear DAG waiting to be extracted
4. **Independent operations**: Look for operations that don't actually depend on each
   other but are called sequentially by convention. These are parallelism opportunities
5. **Error handling**: Note which operations are retriable vs. permanent failures. Note
   where errors are caught and swallowed vs. propagated
6. **Side effects**: Identify which functions are pure computation vs. which have
   external effects (API calls, database writes, notifications). Side effects define
   natural step boundaries

**Questions to ask the developer:**

1. "Walk me through this process end-to-end — what happens in order?"
2. "Which of these operations are independent of each other?"
3. "If step X fails, should we retry it or fail the whole process?"
4. "Is there existing data validation that should gate the rest of the process?"
5. "Are there any operations here that are slow or unreliable?" (these benefit most
   from orchestration)

### Phase 2: Propose the DAG

Based on your analysis, propose a workflow design. Present it to the developer for
review before generating anything.

**Your proposal should include:**

1. **Step mapping**: Which existing functions map to which workflow steps
2. **Dependency graph**: Which steps depend on which, and why
3. **Parallelism opportunities**: Steps that can run concurrently (highlight the
   performance benefit)
4. **Data contracts**: What each step produces (derived from existing return types)
5. **What changes**: Be explicit about what code moves where

**Proposal format example:**

```
Proposed workflow: order_fulfillment (namespace: ecommerce)

Step 1: validate_order (root)
  Maps to: OrderValidator.validate(order_params)
  Produces: is_valid (bool), validation_errors (string[]), normalized_order (object)

Step 2: check_inventory [depends: validate_order]
  Maps to: InventoryService.check_availability(items)
  Produces: all_available (bool), unavailable_items (string[])

Step 3: calculate_shipping [depends: validate_order]
  Maps to: ShippingCalculator.estimate(address, items)
  Produces: shipping_cost (number), estimated_days (integer), carrier (string)

  ** Steps 2 and 3 run in parallel — no dependency between them **

Step 4: charge_payment [depends: check_inventory, calculate_shipping]
  Maps to: PaymentService.charge(order, total)
  Produces: transaction_id (string), charged_amount (number), status (string)

Step 5: send_confirmation [depends: charge_payment]
  Maps to: NotificationService.send_order_confirmation(order, transaction)
  Produces: notification_id (string), channels (string[])
```

**Wait for developer approval before proceeding.** They know their domain better than
you do — they may identify dependencies you missed or parallelism opportunities that
aren't obvious from the code.

### Phase 3: Generate the Template

Once the developer approves the proposal, use the MCP tools:

```
template_generate  →  template_validate  →  template_inspect
```

Follow the same tool sequence as greenfield (see greenfield-workflow-design.md, Phase 3),
but pay special attention to:

- **Field names should match existing code conventions** — if the service returns
  `transaction_id`, the schema field should be `transaction_id`, not `txn_id`
- **Field types must match existing return types** — if the function returns an integer
  count, use `integer`, not `number`
- **Optional fields** — if the existing code sometimes returns null/None for a field,
  mark it as optional in the schema

### Phase 4: Validate Data Contracts Against Existing Code

This is where brownfield diverges significantly from greenfield. Use `schema_inspect`
and `schema_compare` to verify the proposed contracts match reality:

1. **schema_inspect** each step — verify the fields match what the existing function
   actually returns
2. **schema_compare** between connected steps — verify the data the producer outputs
   is what the consumer expects

**Cross-reference with actual code**: Read the existing function signatures and return
types. If the schema says `plagiarism_score: number` but the function returns an
integer 0-100, there's a mismatch to resolve.

### Phase 5: Generate Handlers and Refactor

Generate handler scaffolding with `handler_generate`, then refactor existing code to
fit the three-layer pattern.

**The refactoring pattern:**

```
BEFORE (monolithic):
  controller/route → calls service functions directly in sequence

AFTER (orchestrated):
  controller/route → submits task via Tasker client
  handler (new)    → thin wrapper that calls existing service
  service (kept)   → existing business logic, unchanged
```

**Critical rule**: The existing service functions should require **zero or minimal
changes**. The handler is a thin adapter between Tasker's orchestration and the
existing logic.

**Handler implementation pattern by language:**

#### Python (FastAPI)
```python
# services/inventory.py — EXISTING, unchanged
class InventoryService:
    def check_availability(self, items: list[str]) -> dict:
        # existing logic
        return {"all_available": True, "unavailable_items": []}

# handlers/ecommerce.py — NEW, wraps existing service
from .models import CheckInventoryResult

@step_handler("ecommerce.check_inventory")
@depends_on(validate_order_result="validate_order")
def check_inventory(validate_order_result, context) -> CheckInventoryResult:
    service = InventoryService()
    result = service.check_availability(validate_order_result.items)
    return CheckInventoryResult(**result)
```

#### TypeScript (Bun/Hono)
```typescript
// services/inventory.ts — EXISTING, unchanged
export function checkAvailability(items: string[]) {
  // existing logic
  return { allAvailable: true, unavailableItems: [] };
}

// handlers/ecommerce.ts — NEW, wraps existing service
export const CheckInventoryHandler = defineHandler(
  'ecommerce.check_inventory',
  { depends: { validateOrderResult: 'validate_order' } },
  async ({ validateOrderResult, context }) => {
    return checkAvailability(validateOrderResult.items);
  }
);
```

#### Ruby (Rails)
```ruby
# app/services/inventory_service.rb — EXISTING, unchanged
class InventoryService
  def check_availability(items)
    # existing logic
    { all_available: true, unavailable_items: [] }
  end
end

# app/handlers/ecommerce/check_inventory_handler.rb — NEW
CheckInventoryHandler = step_handler(
  'ecommerce.check_inventory',
  depends_on: { validate_order_result: 'validate_order' }
) do |validate_order_result:, context:|
  result = InventoryService.new.check_availability(validate_order_result.items)
  TaskerCore::Types::StepHandlerCallResult.success(result: result)
end
```

#### Rust (Axum)
```rust
// services/inventory.rs — EXISTING, unchanged
pub fn check_availability(items: &[String]) -> CheckInventoryResult {
    // existing logic
    CheckInventoryResult { all_available: true, unavailable_items: vec![] }
}

// handlers/ecommerce.rs — NEW, wraps existing service
pub fn check_inventory(_ctx: &Value, deps: &HashMap<String, Value>) -> Result<Value, String> {
    let order: ValidateOrderResult = get_dependency(deps, "validate_order")?;
    let result = services::inventory::check_availability(&order.items);
    serde_json::to_value(result).map_err(|e| e.to_string())
}
```

### Phase 6: Wire Up the Route/Controller

Replace the direct sequential calls with a Tasker task submission:

**Before:**
```
POST /orders → validate → check inventory → calculate shipping → charge → notify
(all synchronous, all in one request)
```

**After:**
```
POST /orders → submit task to Tasker → return 202 Accepted with task_id
(Tasker orchestrates steps asynchronously with parallelism and retry)
```

The developer may want some workflows to remain synchronous (simple, fast operations)
and only orchestrate the complex ones. This is a valid choice — not everything needs
to be a DAG.

## Refactoring Decision Framework

Help the developer decide what to orchestrate:

| Signal | Orchestrate? | Why |
|--------|-------------|-----|
| Steps are independent but called sequentially | Yes | Free parallelism |
| A step is slow or calls external APIs | Yes | Retry + timeout isolation |
| A step can fail without failing everything | Yes | Partial completion |
| The whole process must be atomic | Maybe not | Sagas add complexity |
| The process is fast and simple (< 100ms) | Probably not | Orchestration overhead exceeds benefit |
| You need visibility into step-level progress | Yes | Built-in observability |
| Multiple services/teams own different steps | Yes | Decoupled ownership |

## Common Extraction Patterns

### Pattern: Sequential-to-Parallel

**Before**: A → B → C → D (all sequential)

**Analysis**: B and C don't actually depend on each other, only on A.

**After**: A → [B, C] → D (B and C run in parallel)

**Benefit**: Wall-clock time reduced by max(B, C) instead of B + C.

### Pattern: Gate Extraction

**Before**: validate → process → notify (validation is buried in process)

**Analysis**: Validation is a natural gate — if it fails, nothing else should run.

**After**: validate → process → notify (validate is now an explicit step with typed
output that gates downstream work)

**Benefit**: Failed validations don't waste resources on downstream steps.

### Pattern: Observation Extraction

**Before**: process → (log metrics inline)

**Analysis**: Metrics/analytics are a side effect that doesn't affect the main flow.

**After**: process → [deliver, record_analytics] (analytics runs in parallel)

**Benefit**: Analytics failures don't block delivery; analytics gets its own retry.

### Pattern: Saga Decomposition

**Before**: One giant transaction that reserves inventory, charges payment, and ships.

**Analysis**: Each operation has different failure modes and recovery strategies.

**After**: reserve → charge → ship (each step independently retriable with compensating
actions on failure)

**Benefit**: Partial completion is visible; failed charges don't leave phantom
reservations.

## Anti-Patterns to Avoid

| Anti-Pattern | Why It's Bad | Instead |
|-------------|-------------|---------|
| Rewriting services during extraction | Risk of introducing bugs | Wrap existing code, refactor later |
| Making every function a step | Overhead exceeds benefit | Only orchestrate at natural boundaries |
| Changing function signatures to match Tasker | Couples services to orchestration | Keep services framework-agnostic |
| Ignoring existing error handling | Dual error paths cause confusion | Understand and preserve existing patterns |
| Big-bang migration | Too risky, hard to debug | Extract one workflow at a time |
| Skipping the proposal step | Incorrect DAG is worse than no DAG | Always propose and get approval first |
