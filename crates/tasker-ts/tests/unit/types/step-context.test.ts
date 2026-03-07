/**
 * StepContext class tests.
 *
 * TAS-290: Updated for napi-rs camelCase FFI types.
 * Verifies context creation, field access, and factory method.
 */

import { describe, expect, it } from 'bun:test';
import type { FfiStepEvent } from '../../../src/ffi/types.js';
import { StepContext } from '../../../src/types/step-context.js';

describe('StepContext', () => {
  describe('constructor', () => {
    it('initializes all properties from params', () => {
      const event = createValidFfiStepEvent();
      const context = new StepContext({
        event,
        taskUuid: 'task-123',
        stepUuid: 'step-456',
        correlationId: 'corr-789',
        handlerName: 'test_handler',
        inputData: { key: 'value' },
        dependencyResults: { dep1: { result: 'data' } },
        stepConfig: { setting: true },
        stepInputs: { cursor: null },
        retryCount: 1,
        maxRetries: 3,
      });

      expect(context.event).toBe(event);
      expect(context.taskUuid).toBe('task-123');
      expect(context.stepUuid).toBe('step-456');
      expect(context.correlationId).toBe('corr-789');
      expect(context.handlerName).toBe('test_handler');
      expect(context.inputData).toEqual({ key: 'value' });
      expect(context.dependencyResults).toEqual({ dep1: { result: 'data' } });
      expect(context.stepConfig).toEqual({ setting: true });
      expect(context.stepInputs).toEqual({ cursor: null });
      expect(context.retryCount).toBe(1);
      expect(context.maxRetries).toBe(3);
    });

    it('creates readonly properties', () => {
      const context = createValidStepContext();

      expect(context.taskUuid).toBeDefined();
      expect(context.stepUuid).toBeDefined();
    });
  });

  describe('fromFfiEvent', () => {
    it('extracts task UUID and step UUID from event', () => {
      const event = createValidFfiStepEvent();
      const context = StepContext.fromFfiEvent(event, 'my_handler');

      expect(context.taskUuid).toBe(event.taskUuid);
      expect(context.stepUuid).toBe(event.stepUuid);
    });

    it('extracts correlation ID from event', () => {
      const event = createValidFfiStepEvent();
      const context = StepContext.fromFfiEvent(event, 'my_handler');

      expect(context.correlationId).toBe(event.correlationId);
    });

    it('sets handler name from parameter', () => {
      const event = createValidFfiStepEvent();
      const context = StepContext.fromFfiEvent(event, 'custom_handler');

      expect(context.handlerName).toBe('custom_handler');
    });

    it('extracts input data from task context', () => {
      const event = createValidFfiStepEvent();
      event.task.context = { order_id: 'ORD-123', amount: 99.99 };
      const context = StepContext.fromFfiEvent(event, 'my_handler');

      expect(context.inputData).toEqual({ order_id: 'ORD-123', amount: 99.99 });
    });

    it('defaults inputData to empty object when task context is null', () => {
      const event = createValidFfiStepEvent();
      event.task.context = null;
      const context = StepContext.fromFfiEvent(event, 'my_handler');

      expect(context.inputData).toEqual({});
    });

    it('extracts dependency results from event', () => {
      const event = createValidFfiStepEvent();
      event.dependencyResults = {
        step_1: {
          stepUuid: 's1',
          success: true,
          result: { value: 42 },
          status: 'completed',
          errorMessage: null,
          errorType: null,
          errorRetryable: null,
        },
      };
      const context = StepContext.fromFfiEvent(event, 'my_handler');

      expect(context.dependencyResults).toEqual(event.dependencyResults);
    });

    it('extracts step config from handler initialization', () => {
      const event = createValidFfiStepEvent();
      event.stepDefinition.handlerInitialization = {
        api_key: 'secret',
        timeout: 30,
      };
      const context = StepContext.fromFfiEvent(event, 'my_handler');

      expect(context.stepConfig).toEqual({ api_key: 'secret', timeout: 30 });
    });

    it('extracts retry count and max retries from workflow step', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.attempts = 2;
      event.workflowStep.maxAttempts = 5;
      const context = StepContext.fromFfiEvent(event, 'my_handler');

      expect(context.retryCount).toBe(2);
      expect(context.maxRetries).toBe(5);
    });

    it('extracts step inputs from workflow step', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.inputs = { cursor: { offset: 100 } };
      const context = StepContext.fromFfiEvent(event, 'my_handler');

      expect(context.stepInputs).toEqual({ cursor: { offset: 100 } });
    });

    it('stores original event', () => {
      const event = createValidFfiStepEvent();
      const context = StepContext.fromFfiEvent(event, 'my_handler');

      expect(context.event).toBe(event);
    });
  });

  describe('getDependencyResult', () => {
    it('extracts result value from nested structure', () => {
      const context = createValidStepContext({
        dependencyResults: {
          step_1: { result: { computed: 'value' } },
        },
      });

      const result = context.getDependencyResult('step_1');
      expect(result).toEqual({ computed: 'value' });
    });

    it('returns primitive result value', () => {
      const context = createValidStepContext({
        dependencyResults: {
          step_1: { result: 42 },
        },
      });

      const result = context.getDependencyResult('step_1');
      expect(result).toBe(42);
    });

    it('returns null for missing dependency', () => {
      const context = createValidStepContext({
        dependencyResults: {},
      });

      const result = context.getDependencyResult('nonexistent');
      expect(result).toBeNull();
    });

    it('returns null when dependency value is null', () => {
      const context = createValidStepContext({
        dependencyResults: {
          step_1: null,
        },
      });

      const result = context.getDependencyResult('step_1');
      expect(result).toBeNull();
    });

    it('returns whole value when no result key exists', () => {
      const context = createValidStepContext({
        dependencyResults: {
          step_1: { other_key: 'value' },
        },
      });

      const result = context.getDependencyResult('step_1');
      expect(result).toEqual({ other_key: 'value' });
    });
  });

  describe('getInput', () => {
    it('returns value for existing key', () => {
      const context = createValidStepContext({
        inputData: { order_id: 'ORD-123', amount: 99.99 },
      });

      expect(context.getInput('order_id')).toBe('ORD-123');
      expect(context.getInput('amount')).toBe(99.99);
    });

    it('returns undefined for missing key', () => {
      const context = createValidStepContext({
        inputData: { order_id: 'ORD-123' },
      });

      expect(context.getInput('nonexistent')).toBeUndefined();
    });

    it('supports generic type parameter', () => {
      const context = createValidStepContext({
        inputData: { count: 42 },
      });

      const count = context.getInput<number>('count');
      expect(count).toBe(42);
    });
  });

  describe('getConfig', () => {
    it('returns value for existing key', () => {
      const context = createValidStepContext({
        stepConfig: { api_endpoint: 'https://api.example.com', timeout: 30 },
      });

      expect(context.getConfig('api_endpoint')).toBe('https://api.example.com');
      expect(context.getConfig('timeout')).toBe(30);
    });

    it('returns undefined for missing key', () => {
      const context = createValidStepContext({
        stepConfig: { api_endpoint: 'https://api.example.com' },
      });

      expect(context.getConfig('nonexistent')).toBeUndefined();
    });

    it('supports generic type parameter', () => {
      const context = createValidStepContext({
        stepConfig: { enabled: true },
      });

      const enabled = context.getConfig<boolean>('enabled');
      expect(enabled).toBe(true);
    });
  });

  describe('isRetry', () => {
    it('returns false when retryCount is 0', () => {
      const context = createValidStepContext({ retryCount: 0 });
      expect(context.isRetry()).toBe(false);
    });

    it('returns true when retryCount is 1', () => {
      const context = createValidStepContext({ retryCount: 1 });
      expect(context.isRetry()).toBe(true);
    });

    it('returns true when retryCount is greater than 1', () => {
      const context = createValidStepContext({ retryCount: 5 });
      expect(context.isRetry()).toBe(true);
    });
  });

  describe('isLastRetry', () => {
    it('returns false when retry count is less than max - 1', () => {
      const context = createValidStepContext({ retryCount: 0, maxRetries: 3 });
      expect(context.isLastRetry()).toBe(false);
    });

    it('returns true when retry count equals max - 1', () => {
      const context = createValidStepContext({ retryCount: 2, maxRetries: 3 });
      expect(context.isLastRetry()).toBe(true);
    });

    it('returns true when retry count exceeds max - 1', () => {
      const context = createValidStepContext({ retryCount: 5, maxRetries: 3 });
      expect(context.isLastRetry()).toBe(true);
    });
  });

  describe('getInputOr', () => {
    it('returns value when key exists', () => {
      const context = createValidStepContext({
        inputData: { batch_size: 200 },
      });

      expect(context.getInputOr('batch_size', 100)).toBe(200);
    });

    it('returns default when key is missing', () => {
      const context = createValidStepContext({ inputData: {} });

      expect(context.getInputOr('batch_size', 100)).toBe(100);
    });

    it('returns default when value is undefined', () => {
      const context = createValidStepContext({
        inputData: { batch_size: undefined },
      });

      expect(context.getInputOr('batch_size', 100)).toBe(100);
    });

    it('returns falsy value when present (not default)', () => {
      const context = createValidStepContext({
        inputData: { count: 0, flag: false, name: '' },
      });

      expect(context.getInputOr('count', 99)).toBe(0);
      expect(context.getInputOr('flag', true)).toBe(false);
      expect(context.getInputOr('name', 'default')).toBe('');
    });
  });

  describe('getDependencyField', () => {
    it('extracts a single-level field from dependency result', () => {
      const context = createValidStepContext({
        dependencyResults: {
          analyze_csv: { result: { csv_file_path: '/tmp/data.csv', row_count: 500 } },
        },
      });

      expect(context.getDependencyField('analyze_csv', 'csv_file_path')).toBe('/tmp/data.csv');
    });

    it('extracts a multi-level path from dependency result', () => {
      const context = createValidStepContext({
        dependencyResults: {
          step_1: { result: { data: { items: [1, 2, 3] } } },
        },
      });

      expect(context.getDependencyField('step_1', 'data', 'items')).toEqual([1, 2, 3]);
    });

    it('returns null when dependency is missing', () => {
      const context = createValidStepContext({ dependencyResults: {} });

      expect(context.getDependencyField('nonexistent', 'field')).toBeNull();
    });

    it('returns null when dependency result is null', () => {
      const context = createValidStepContext({
        dependencyResults: { step_1: null },
      });

      expect(context.getDependencyField('step_1', 'field')).toBeNull();
    });

    it('returns null when intermediate path is not an object', () => {
      const context = createValidStepContext({
        dependencyResults: {
          step_1: { result: { data: 'not_an_object' } },
        },
      });

      expect(context.getDependencyField('step_1', 'data', 'nested')).toBeNull();
    });

    it('returns undefined for missing key at end of valid path', () => {
      const context = createValidStepContext({
        dependencyResults: {
          step_1: { result: { data: { present: true } } },
        },
      });

      expect(context.getDependencyField('step_1', 'data', 'missing')).toBeUndefined();
    });
  });

  describe('checkpoint', () => {
    it('returns checkpoint data when present', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.checkpoint = { cursor: 500, items_processed: 500 };
      const context = StepContext.fromFfiEvent(event, 'handler');

      expect(context.checkpoint).toEqual({ cursor: 500, items_processed: 500 });
    });

    it('returns null when no checkpoint exists', () => {
      const event = createValidFfiStepEvent();
      const context = StepContext.fromFfiEvent(event, 'handler');

      expect(context.checkpoint).toBeNull();
    });
  });

  describe('checkpointCursor', () => {
    it('returns cursor value when present', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.checkpoint = { cursor: 1000 };
      const context = StepContext.fromFfiEvent(event, 'handler');

      expect(context.checkpointCursor).toBe(1000);
    });

    it('returns null when no checkpoint', () => {
      const context = createValidStepContext();

      expect(context.checkpointCursor).toBeNull();
    });

    it('returns string cursor', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.checkpoint = { cursor: 'page_token_abc' };
      const context = StepContext.fromFfiEvent(event, 'handler');

      expect(context.checkpointCursor).toBe('page_token_abc');
    });
  });

  describe('checkpointItemsProcessed', () => {
    it('returns items_processed when present', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.checkpoint = { cursor: 100, items_processed: 250 };
      const context = StepContext.fromFfiEvent(event, 'handler');

      expect(context.checkpointItemsProcessed).toBe(250);
    });

    it('returns 0 when no checkpoint', () => {
      const context = createValidStepContext();

      expect(context.checkpointItemsProcessed).toBe(0);
    });

    it('returns 0 when checkpoint has no items_processed', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.checkpoint = { cursor: 100 };
      const context = StepContext.fromFfiEvent(event, 'handler');

      expect(context.checkpointItemsProcessed).toBe(0);
    });
  });

  describe('accumulatedResults', () => {
    it('returns accumulated_results when present', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.checkpoint = {
        cursor: 100,
        accumulated_results: { sum: 5000, count: 100 },
      };
      const context = StepContext.fromFfiEvent(event, 'handler');

      expect(context.accumulatedResults).toEqual({ sum: 5000, count: 100 });
    });

    it('returns null when no checkpoint', () => {
      const context = createValidStepContext();

      expect(context.accumulatedResults).toBeNull();
    });

    it('returns null when checkpoint has no accumulated_results', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.checkpoint = { cursor: 100 };
      const context = StepContext.fromFfiEvent(event, 'handler');

      expect(context.accumulatedResults).toBeNull();
    });
  });

  describe('hasCheckpoint', () => {
    it('returns true when cursor exists', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.checkpoint = { cursor: 500 };
      const context = StepContext.fromFfiEvent(event, 'handler');

      expect(context.hasCheckpoint()).toBe(true);
    });

    it('returns false when no checkpoint', () => {
      const context = createValidStepContext();

      expect(context.hasCheckpoint()).toBe(false);
    });

    it('returns false when checkpoint has no cursor', () => {
      const event = createValidFfiStepEvent();
      event.workflowStep.checkpoint = { items_processed: 100 };
      const context = StepContext.fromFfiEvent(event, 'handler');

      expect(context.hasCheckpoint()).toBe(false);
    });
  });

  describe('getDependencyResultKeys', () => {
    it('returns array of step names', () => {
      const context = createValidStepContext({
        dependencyResults: {
          step_1: { result: 'a' },
          step_2: { result: 'b' },
          step_3: { result: 'c' },
        },
      });

      const keys = context.getDependencyResultKeys();

      expect(keys).toEqual(['step_1', 'step_2', 'step_3']);
    });

    it('returns empty array when no dependencies', () => {
      const context = createValidStepContext({ dependencyResults: {} });

      expect(context.getDependencyResultKeys()).toEqual([]);
    });
  });

  describe('getAllDependencyResults', () => {
    it('returns results matching prefix', () => {
      const context = createValidStepContext({
        dependencyResults: {
          process_batch_001: { result: { count: 10 } },
          process_batch_002: { result: { count: 20 } },
          analyze_csv: { result: { total: 30 } },
        },
      });

      const results = context.getAllDependencyResults('process_batch_');

      expect(results).toHaveLength(2);
      expect(results).toContainEqual({ count: 10 });
      expect(results).toContainEqual({ count: 20 });
    });

    it('returns empty array when no prefix matches', () => {
      const context = createValidStepContext({
        dependencyResults: {
          step_1: { result: 'data' },
        },
      });

      expect(context.getAllDependencyResults('nonexistent_')).toEqual([]);
    });

    it('skips null dependency results', () => {
      const context = createValidStepContext({
        dependencyResults: {
          batch_001: null,
          batch_002: { result: { count: 5 } },
        },
      });

      const results = context.getAllDependencyResults('batch_');

      expect(results).toHaveLength(1);
      expect(results[0]).toEqual({ count: 5 });
    });

    it('returns all matching results for broad prefix', () => {
      const context = createValidStepContext({
        dependencyResults: {
          step_1: { result: 'a' },
          step_2: { result: 'b' },
        },
      });

      const results = context.getAllDependencyResults('step_');

      expect(results).toHaveLength(2);
    });
  });
});

// Test helpers

function createValidFfiStepEvent(): FfiStepEvent {
  return {
    eventId: 'event-123',
    taskUuid: 'task-456',
    stepUuid: 'step-789',
    correlationId: 'corr-001',
    traceId: null,
    spanId: null,
    taskCorrelationId: 'task-corr-001',
    parentCorrelationId: null,
    task: {
      taskUuid: 'task-456',
      namedTaskUuid: 'named-task-001',
      name: 'TestTask',
      namespace: 'test',
      version: '1.0.0',
      context: null,
      correlationId: 'corr-001',
      parentCorrelationId: null,
      complete: false,
      priority: 0,
      initiator: null,
      sourceSystem: null,
      reason: null,
      tags: null,
      identityHash: 'hash-123',
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      requestedAt: new Date().toISOString(),
    },
    workflowStep: {
      workflowStepUuid: 'step-789',
      taskUuid: 'task-456',
      namedStepUuid: 'named-step-001',
      name: 'TestStep',
      templateStepName: 'test_step',
      retryable: true,
      maxAttempts: 3,
      attempts: 0,
      inProcess: false,
      processed: false,
      inputs: null,
      results: null,
      backoffRequestSeconds: null,
      processedAt: null,
      lastAttemptedAt: null,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      checkpoint: null,
    },
    stepDefinition: {
      name: 'test_step',
      description: 'A test step',
      handlerCallable: 'TestHandler',
      handlerMethod: null,
      handlerResolver: null,
      handlerInitialization: {},
      systemDependency: null,
      dependencies: [],
      timeoutSeconds: 30,
      retryRetryable: true,
      retryMaxAttempts: 3,
      retryBackoff: 'exponential',
      retryBackoffBaseMs: 1000,
      retryMaxBackoffMs: 30000,
    },
    dependencyResults: {},
  };
}

function createValidStepContext(
  overrides: Partial<{
    inputData: Record<string, unknown>;
    dependencyResults: Record<string, unknown>;
    stepConfig: Record<string, unknown>;
    stepInputs: Record<string, unknown>;
    retryCount: number;
    maxRetries: number;
  }> = {}
): StepContext {
  const event = createValidFfiStepEvent();
  return new StepContext({
    event,
    taskUuid: 'task-123',
    stepUuid: 'step-456',
    correlationId: 'corr-789',
    handlerName: 'test_handler',
    inputData: overrides.inputData ?? {},
    dependencyResults: overrides.dependencyResults ?? {},
    stepConfig: overrides.stepConfig ?? {},
    stepInputs: overrides.stepInputs ?? {},
    retryCount: overrides.retryCount ?? 0,
    maxRetries: overrides.maxRetries ?? 3,
  });
}
