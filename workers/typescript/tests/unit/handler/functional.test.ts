/**
 * Tests for TAS-294 functional/factory handler API.
 *
 * Tests:
 * 1. Basic defineHandler with auto-wrapping
 * 2. Dependency injection via depends
 * 3. Input injection via inputs
 * 4. Error auto-classification (PermanentError, RetryableError, generic)
 * 5. Decision handler helpers (Decision.route, Decision.skip)
 * 6. Batch analyzer helpers
 * 7. Batch worker helpers
 * 8. Passthrough when returning StepHandlerResult directly
 * 9. Missing dependency returns null
 * 10. Async handler support
 */

import { describe, expect, it } from 'bun:test';
import type { FfiStepEvent } from '../../../src/ffi/types.js';
import { StepHandler } from '../../../src/handler/base.js';
import {
  type BatchConfig,
  Decision,
  PermanentError,
  RetryableError,
  defineBatchAnalyzer,
  defineBatchWorker,
  defineDecisionHandler,
  defineHandler,
} from '../../../src/handler/functional.js';
import { StepContext } from '../../../src/types/step-context.js';
import { StepHandlerResult } from '../../../src/types/step-handler-result.js';

// ============================================================================
// Test Helpers
// ============================================================================

function createFfiEvent(): FfiStepEvent {
  return {
    event_id: 'event-123',
    task_uuid: 'task-456',
    step_uuid: 'step-789',
    correlation_id: 'corr-001',
    trace_id: null,
    span_id: null,
    task_correlation_id: 'task-corr-001',
    parent_correlation_id: null,
    task: {
      task_uuid: 'task-456',
      named_task_uuid: 'named-task-001',
      name: 'TestTask',
      namespace: 'test',
      version: '1.0.0',
      context: null,
      correlation_id: 'corr-001',
      parent_correlation_id: null,
      complete: false,
      priority: 0,
      initiator: null,
      source_system: null,
      reason: null,
      tags: null,
      identity_hash: 'hash-123',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      requested_at: new Date().toISOString(),
    },
    workflow_step: {
      workflow_step_uuid: 'ws-001',
      task_uuid: 'task-456',
      named_step_uuid: 'ns-001',
      retries: 0,
      attempts: 0,
      max_attempts: 3,
      in_process: false,
      processed: false,
      inputs: null,
      results: null,
      checkpoint: null,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    },
    step_definition: {
      named_step_uuid: 'ns-001',
      named_task_uuid: 'nt-001',
      name: 'test_step',
      handler_class: 'TestHandler',
      handler_callable: 'test_handler',
      handler_initialization: null,
      default_retries: 3,
      skippable: false,
      is_decision_step: false,
      is_batchable_step: false,
      depends_on_step_names: null,
    },
    dependency_results: null,
  } as unknown as FfiStepEvent;
}

function makeContext(overrides: {
  inputData?: Record<string, unknown>;
  dependencyResults?: Record<string, unknown>;
  stepConfig?: Record<string, unknown>;
} = {}): StepContext {
  const event = createFfiEvent();
  return new StepContext({
    event,
    taskUuid: 'task-123',
    stepUuid: 'step-456',
    correlationId: 'corr-789',
    handlerName: 'test_handler',
    inputData: overrides.inputData ?? {},
    dependencyResults: overrides.dependencyResults ?? {},
    stepConfig: overrides.stepConfig ?? {},
    stepInputs: {},
    retryCount: 0,
    maxRetries: 3,
  });
}

// ============================================================================
// Tests: Basic defineHandler
// ============================================================================

describe('defineHandler', () => {
  it('wraps dict return as success', async () => {
    const Handler = defineHandler('my_handler', {}, async () => {
      return { processed: true };
    });

    const handler = new Handler();
    expect(handler.name).toBe('my_handler');
    expect(handler.version).toBe('1.0.0');

    const result = await handler.call(makeContext());
    expect(result.success).toBe(true);
    expect(result.result).toEqual({ processed: true });
  });

  it('sets custom version', async () => {
    const Handler = defineHandler(
      'versioned',
      { version: '2.0.0' },
      async () => ({})
    );

    const handler = new Handler();
    expect(handler.version).toBe('2.0.0');
  });

  it('wraps void return as empty success', async () => {
    const Handler = defineHandler('void_handler', {}, async () => {
      // no return
    });

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(true);
    expect(result.result).toEqual({});
  });

  it('passes through StepHandlerResult without double-wrapping', async () => {
    const Handler = defineHandler('passthrough', {}, async () => {
      return StepHandlerResult.success({ direct: true });
    });

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(true);
    expect(result.result).toEqual({ direct: true });
  });

  it('is a StepHandler subclass', () => {
    const Handler = defineHandler('compat', {}, async () => ({}));
    expect(new Handler()).toBeInstanceOf(StepHandler);
  });
});

// ============================================================================
// Tests: Dependency Injection
// ============================================================================

describe('dependency injection', () => {
  it('injects dependencies from context', async () => {
    const Handler = defineHandler(
      'with_deps',
      { depends: { cart: 'validate_cart' } },
      async ({ cart }) => {
        return { total: (cart as Record<string, number>).total };
      }
    );

    const handler = new Handler();
    const ctx = makeContext({
      dependencyResults: { validate_cart: { result: { total: 99.99 } } },
    });
    const result = await handler.call(ctx);
    expect(result.success).toBe(true);
    expect(result.result).toEqual({ total: 99.99 });
  });

  it('injects null for missing dependencies', async () => {
    const Handler = defineHandler(
      'missing_dep',
      { depends: { cart: 'validate_cart' } },
      async ({ cart }) => {
        return { cartIsNull: cart === null };
      }
    );

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(true);
    expect(result.result).toEqual({ cartIsNull: true });
  });

  it('injects multiple dependencies', async () => {
    const Handler = defineHandler(
      'multi_deps',
      { depends: { cart: 'validate_cart', user: 'fetch_user' } },
      async ({ cart, user }) => {
        return { cart, user };
      }
    );

    const handler = new Handler();
    const ctx = makeContext({
      dependencyResults: {
        validate_cart: { result: { total: 50 } },
        fetch_user: { result: { name: 'Alice' } },
      },
    });
    const result = await handler.call(ctx);
    expect(result.success).toBe(true);
    expect(result.result!.cart).toEqual({ total: 50 });
    expect(result.result!.user).toEqual({ name: 'Alice' });
  });
});

// ============================================================================
// Tests: Input Injection
// ============================================================================

describe('input injection', () => {
  it('injects inputs from task context', async () => {
    const Handler = defineHandler(
      'with_inputs',
      { inputs: { paymentInfo: 'payment_info' } },
      async ({ paymentInfo }) => {
        return { payment: paymentInfo };
      }
    );

    const handler = new Handler();
    const ctx = makeContext({
      inputData: { payment_info: { card: '1234' } },
    });
    const result = await handler.call(ctx);
    expect(result.success).toBe(true);
    expect(result.result).toEqual({ payment: { card: '1234' } });
  });

  it('injects undefined for missing inputs', async () => {
    const Handler = defineHandler(
      'missing_input',
      { inputs: { val: 'nonexistent' } },
      async ({ val }) => {
        return { isUndefined: val === undefined };
      }
    );

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(true);
    expect(result.result).toEqual({ isUndefined: true });
  });
});

// ============================================================================
// Tests: Error Classification
// ============================================================================

describe('error classification', () => {
  it('PermanentError → failure(retryable=false)', async () => {
    const Handler = defineHandler('perm_err', {}, async () => {
      throw new PermanentError('Invalid input');
    });

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(false);
    expect(result.retryable).toBe(false);
    expect(result.errorMessage).toContain('Invalid input');
  });

  it('RetryableError → failure(retryable=true)', async () => {
    const Handler = defineHandler('retry_err', {}, async () => {
      throw new RetryableError('Service unavailable');
    });

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(false);
    expect(result.retryable).toBe(true);
    expect(result.errorMessage).toContain('Service unavailable');
  });

  it('generic error → failure(retryable=true)', async () => {
    const Handler = defineHandler('generic_err', {}, async () => {
      throw new TypeError('Something went wrong');
    });

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(false);
    expect(result.retryable).toBe(true);
    expect(result.errorMessage).toContain('Something went wrong');
  });
});

// ============================================================================
// Tests: Decision Handler
// ============================================================================

describe('defineDecisionHandler', () => {
  it('Decision.route() creates create_steps outcome', async () => {
    const Handler = defineDecisionHandler('route_order', {}, async () => {
      return Decision.route(['process_premium'], { tier: 'premium' });
    });

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(true);
    const outcome = result.result!.decision_point_outcome as Record<string, unknown>;
    expect(outcome.type).toBe('create_steps');
    expect(outcome.step_names).toEqual(['process_premium']);
    // routing_context is at result level (mixin format)
    expect((result.result!.routing_context as Record<string, unknown>).tier).toBe('premium');
  });

  it('Decision.skip() creates no_branches outcome', async () => {
    const Handler = defineDecisionHandler('skip_handler', {}, async () => {
      return Decision.skip('No items to process');
    });

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(true);
    const outcome = result.result!.decision_point_outcome as Record<string, unknown>;
    expect(outcome.type).toBe('no_branches');
    // reason is at result level (mixin format)
    expect(result.result!.reason).toBe('No items to process');
  });

  it('has decision capabilities', () => {
    const Handler = defineDecisionHandler('test_decision', {}, async () => {
      return Decision.route(['step_a']);
    });

    const handler = new Handler();
    expect(handler.capabilities).toContain('decision');
    expect(handler.capabilities).toContain('routing');
  });

  it('injects dependencies in decision handler', async () => {
    const Handler = defineDecisionHandler(
      'route_with_deps',
      { depends: { order: 'validate_order' } },
      async ({ order }) => {
        const o = order as Record<string, string>;
        if (o?.tier === 'premium') {
          return Decision.route(['process_premium']);
        }
        return Decision.route(['process_standard']);
      }
    );

    const handler = new Handler();
    const ctx = makeContext({
      dependencyResults: { validate_order: { result: { tier: 'premium' } } },
    });
    const result = await handler.call(ctx);
    expect(result.success).toBe(true);
    const outcome = result.result!.decision_point_outcome as Record<string, unknown>;
    expect(outcome.step_names).toEqual(['process_premium']);
  });
});

// ============================================================================
// Tests: Batch Analyzer
// ============================================================================

describe('defineBatchAnalyzer', () => {
  it('auto-generates cursor configs from BatchConfig', async () => {
    const Handler = defineBatchAnalyzer(
      'analyze',
      { workerTemplate: 'process_batch' },
      async () => {
        return { totalItems: 250, batchSize: 100 } satisfies BatchConfig;
      }
    );

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(true);

    const outcome = result.result!.batch_processing_outcome as Record<string, unknown>;
    expect(outcome.type).toBe('create_batches');
    expect(outcome.worker_template_name).toBe('process_batch');
    expect(outcome.total_items).toBe(250);
    expect(outcome.worker_count).toBe(3);

    const configs = outcome.cursor_configs as Record<string, unknown>[];
    expect(configs).toHaveLength(3);
    expect(configs[0]!.start_cursor).toBe(0);
    expect(configs[0]!.end_cursor).toBe(100);
    expect(configs[1]!.start_cursor).toBe(100);
    expect(configs[1]!.end_cursor).toBe(200);
    expect(configs[2]!.start_cursor).toBe(200);
    expect(configs[2]!.end_cursor).toBe(250);
  });
});

// ============================================================================
// Tests: Batch Worker
// ============================================================================

describe('defineBatchWorker', () => {
  it('receives null batch context when no batch data', async () => {
    const Handler = defineBatchWorker('process_batch', {}, async ({ batchContext }) => {
      return { noBatch: batchContext === null };
    });

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(true);
    expect(result.result).toEqual({ noBatch: true });
  });

  it('extracts batch context from step config', async () => {
    const Handler = defineBatchWorker('process_batch', {}, async ({ batchContext }) => {
      const bc = batchContext!;
      return {
        start: bc.startCursor,
        end: bc.endCursor,
        batchId: bc.batchId,
      };
    });

    const handler = new Handler();
    const ctx = makeContext({
      stepConfig: {
        batch_context: {
          batch_id: 'batch_001',
          cursor_config: { start_cursor: 100, end_cursor: 200, step_size: 1 },
          batch_index: 1,
          total_batches: 3,
        },
      },
    });
    const result = await handler.call(ctx);
    expect(result.success).toBe(true);
    expect(result.result).toEqual({
      start: 100,
      end: 200,
      batchId: 'batch_001',
    });
  });
});

// ============================================================================
// Tests: Combined Dependencies and Inputs
// ============================================================================

describe('combined dependencies and inputs', () => {
  it('works together', async () => {
    const Handler = defineHandler(
      'combined',
      {
        depends: { prev: 'step_1' },
        inputs: { configKey: 'config_key' },
      },
      async ({ prev, configKey }) => {
        return { prev, config: configKey };
      }
    );

    const handler = new Handler();
    const ctx = makeContext({
      inputData: { config_key: 'abc' },
      dependencyResults: { step_1: { result: { count: 5 } } },
    });
    const result = await handler.call(ctx);
    expect(result.success).toBe(true);
    expect(result.result!.prev).toEqual({ count: 5 });
    expect(result.result!.config).toBe('abc');
  });
});

// ============================================================================
// Tests: Context Always Available
// ============================================================================

describe('context always available', () => {
  it('context is always passed to handler', async () => {
    const Handler = defineHandler('ctx_check', {}, async ({ context }) => {
      return {
        hasContext: context != null,
        taskUuid: context.taskUuid,
      };
    });

    const handler = new Handler();
    const result = await handler.call(makeContext());
    expect(result.success).toBe(true);
    expect(result.result).toEqual({ hasContext: true, taskUuid: 'task-123' });
  });
});
