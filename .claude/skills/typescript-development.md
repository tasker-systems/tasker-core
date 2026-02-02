# Skill: TypeScript Development

## When to Use

Use this skill when writing, reviewing, or modifying TypeScript code in `workers/typescript/`, including step handlers, Bun/Node/Deno FFI bindings, vitest tests, or the tsup build pipeline.

## Tooling

| Tool | Purpose | Command |
|------|---------|---------|
| Biome | Linting & formatting | `cargo make check-typescript` |
| tsc | Type checking | Part of `check-typescript` |
| vitest | Testing (via bun test) | `cargo make test-typescript` |
| tsup | TypeScript bundler | `bun run build` |
| Bun | Runtime & package manager | `bun install` |

### Setup

```bash
cd workers/typescript
cargo make install                          # bun install --frozen-lockfile
cargo build -p tasker-worker-ts --release   # Build Rust FFI cdylib
bun run build                               # Build TypeScript
```

### Quality Checks

```bash
cargo make check-typescript  # lint + typecheck + test
cargo make fix-typescript    # format-fix + lint-fix
cargo make test-typescript   # bun test
cargo make test-watch        # Watch mode
cargo make test-coverage     # With coverage
```

## Code Style

### Formatting & Naming

- Biome for linting/formatting; tsc for type checking
- Classes/Types/Interfaces: `PascalCase`
- Functions/methods/variables: `camelCase`
- Constants: `SCREAMING_SNAKE_CASE` or `camelCase`
- Private members: `#private` (ES private fields) or `_underscore`
- File names: `kebab-case.ts`

### Module Organization

```typescript
/**
 * Order processing step handler.
 * @module handlers/order-handler
 */

// 1. External imports
import { z } from 'zod';

// 2. Internal imports (absolute paths preferred)
import { StepHandler } from '@/handler/step-handler';
import { applyAPI } from '@/handler/mixins/api';
import type { StepContext, StepHandlerResult } from '@/types';

// 3. Type definitions
interface OrderData {
  orderId: string;
  items: OrderItem[];
}

// 4. Constants
const DEFAULT_TIMEOUT = 30_000;

// 5. Main exports
export class OrderHandler extends StepHandler { ... }

// 6. Helper functions
function validateOrder(data: unknown): OrderData { ... }
```

## Type Safety

### Strict Types Always

```typescript
// BAD: Using any
function process(data: any): any { return data.result; }

// GOOD: Explicit types
function process(data: StepInput): ProcessResult {
  return { result: data.payload };
}

// GOOD: unknown for truly unknown data
function parseResponse(data: unknown): OrderData {
  return OrderDataSchema.parse(data);
}
```

### Zod for Runtime Validation

```typescript
const OrderInputSchema = z.object({
  orderId: z.string().uuid(),
  items: z.array(z.object({
    sku: z.string(),
    quantity: z.number().positive(),
  })),
});

type OrderInput = z.infer<typeof OrderInputSchema>;

// Validate at boundaries
function call(context: StepContext): StepHandlerResult {
  const input = OrderInputSchema.safeParse(context.inputData);
  if (!input.success) {
    return this.failure(`Invalid input: ${input.error.message}`, 'ValidationError');
  }
  return this.processOrder(input.data);
}
```

### Discriminated Unions for Results

```typescript
type StepHandlerResult =
  | { success: true; result: Record<string, unknown>; metadata?: Record<string, unknown> }
  | { success: false; error: StepError };
```

## Handler Pattern

### The `call(context)` Contract

Handlers extend `StepHandler` and apply mixins in the constructor:

```typescript
export class OrderHandler extends StepHandler {
  constructor() {
    super();
    applyAPI(this);       // HTTP methods
    applyDecision(this);  // Decision routing
  }

  async call(context: StepContext): Promise<StepHandlerResult> {
    try {
      const orderId = context.inputData.orderId as string;
      if (!orderId) return this.failure('Missing order_id', 'ValidationError');

      const response = await this.get(`/api/orders/${orderId}`);
      if (response.ok) {
        return this.success(await response.json(), {
          fetchedAt: new Date().toISOString(),
        });
      }
      return this.failure(
        `API error: ${response.status}`, 'APIError', undefined,
        response.status >= 500,
      );
    } catch (error) {
      return this.failure(
        error instanceof Error ? error.message : 'Unknown error',
        'UnexpectedError', undefined, true,
      );
    }
  }
}
```

### Result Factory Methods

```typescript
// Success
this.success({ orderId: '123', status: 'processed' }, { durationMs: 150 });

// Failure
this.failure('Validation failed', 'ValidationError', 'INVALID_QUANTITY', false);

// Decision
this.decisionSuccess(['shipOrder', 'sendConfirmation'], { decision: 'standardFlow' });

// Skip branches
this.skipBranches('No items require processing', { skipReason: 'emptyCart' });
```

## Composition Over Inheritance (TAS-112)

TypeScript uses mixin functions applied in constructor:

```typescript
// WRONG: Specialized base class
class MyHandler extends APIHandler { ... }

// RIGHT: Mixin functions
class MyHandler extends StepHandler implements APICapable, DecisionCapable {
  constructor() {
    super();
    applyAPI(this);
    applyDecision(this);
  }
}
```

| Mixin | Provides |
|-------|----------|
| `applyAPI` | `get`, `post`, `put`, `delete` |
| `applyDecision` | `decisionSuccess`, `skipBranches`, `decisionFailure` |
| `applyBatchable` | `getBatchContext`, `batchWorkerComplete`, `handleNoOpWorker` |

## Error Handling

### Custom Error Classes

```typescript
export class HandlerError extends Error {
  constructor(
    message: string,
    public readonly errorType: string,
    public readonly retryable: boolean = false,
  ) {
    super(message);
    this.name = 'HandlerError';
  }
}

export class ValidationError extends HandlerError {
  constructor(message: string) {
    super(message, 'ValidationError', false);
  }
}

export class APIError extends HandlerError {
  constructor(message: string, public readonly statusCode: number) {
    super(message, 'APIError', statusCode >= 500);
  }
}
```

### Catch Specific Errors

```typescript
async call(context: StepContext): Promise<StepHandlerResult> {
  try {
    return await this.processOrder(context);
  } catch (error) {
    if (error instanceof ValidationError) {
      return this.failure(error.message, 'ValidationError', undefined, false);
    }
    if (error instanceof APIError) {
      return this.failure(error.message, 'APIError', String(error.statusCode), error.retryable);
    }
    const message = error instanceof Error ? error.message : 'Unknown error';
    return this.failure(message, 'UnexpectedError', undefined, true);
  }
}
```

## FFI Considerations

```typescript
// Bun can handle concurrent callbacks (no GIL like Ruby/Python)
// But be mindful of async boundaries with FFI calls

// Multi-runtime support: Bun, Node.js, Deno
// Use runtime-agnostic APIs where possible
const runtime = detectRuntime();
```

## Testing (vitest)

```typescript
describe('OrderHandler', () => {
  let handler: OrderHandler;

  beforeEach(() => { handler = new OrderHandler(); });

  it('returns success with valid order', async () => {
    const context = createTestContext({ inputData: { orderId: '12345' } });
    vi.spyOn(handler, 'get').mockResolvedValue(
      new Response(JSON.stringify({ id: '12345' }), { status: 200 }),
    );
    const result = await handler.call(context);
    expect(result.success).toBe(true);
  });

  it.each([
    [400, false], [500, true], [502, true],
  ])('handles %i status with retryable=%s', async (status, retryable) => {
    const context = createTestContext({ inputData: { orderId: '123' } });
    vi.spyOn(handler, 'get').mockResolvedValue(new Response(null, { status }));
    const result = await handler.call(context);
    expect(result.success).toBe(false);
    expect(result.error?.retryable).toBe(retryable);
  });
});
```

### Test Helpers

```typescript
export function createTestContext(overrides: Partial<StepContext> = {}): StepContext {
  return {
    taskUuid: 'test-task-uuid', stepUuid: 'test-step-uuid',
    inputData: {}, stepConfig: {}, dependencyResults: {},
    retryCount: 0, maxRetries: 3, ...overrides,
  };
}
```

## Documentation (JSDoc)

```typescript
/**
 * Handles order processing operations.
 *
 * @example
 * ```typescript
 * const handler = new OrderHandler();
 * const result = await handler.call(context);
 * ```
 */
export class OrderHandler extends StepHandler {
  /**
   * Process an order step.
   * @param context - Execution context containing order data
   * @returns Promise resolving to success or failure result
   * @throws {ValidationError} If order_id is missing
   */
  async call(context: StepContext): Promise<StepHandlerResult> { ... }
}
```

## References

- Best practices: `docs/development/best-practices-typescript.md`
- Composition: `docs/principles/composition-over-inheritance.md`
- Cross-language: `docs/principles/cross-language-consistency.md`
- Biome: https://biomejs.dev/
