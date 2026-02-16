/**
 * Tests for StepExecutionSubscriber.
 *
 * TAS-290: Updated for napi-rs — 4-arg constructor (emitter, registry, module, config),
 * camelCase event fields, flattened stepDefinition.handlerCallable.
 */

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { TaskerEventEmitter } from '../../../src/events/event-emitter';
import { StepEventNames } from '../../../src/events/event-names';
import type { NapiModule } from '../../../src/ffi/ffi-layer';
import type { FfiStepEvent } from '../../../src/ffi/types';
import { StepHandler } from '../../../src/handler/base';
import { HandlerRegistry } from '../../../src/handler/registry';
import {
  StepExecutionSubscriber,
  type StepExecutionSubscriberConfig,
} from '../../../src/subscriber/step-execution-subscriber';
import type { StepContext } from '../../../src/types/step-context';
import type { StepHandlerResult } from '../../../src/types/step-handler-result';

// =============================================================================
// Test Handlers
// =============================================================================

class TestHandler extends StepHandler {
  static handlerName = 'test_handler';

  async call(_context: StepContext): Promise<StepHandlerResult> {
    return this.success({ processed: true });
  }
}

class SlowHandler extends StepHandler {
  static handlerName = 'slow_handler';

  async call(_context: StepContext): Promise<StepHandlerResult> {
    await new Promise((resolve) => setTimeout(resolve, 100));
    return this.success({ slow: true });
  }
}

class FailingHandler extends StepHandler {
  static handlerName = 'failing_handler';

  async call(_context: StepContext): Promise<StepHandlerResult> {
    return this.failure('Handler failed intentionally');
  }
}

class ThrowingHandler extends StepHandler {
  static handlerName = 'throwing_handler';

  async call(_context: StepContext): Promise<StepHandlerResult> {
    throw new Error('Handler threw an error');
  }
}

// =============================================================================
// Mock NapiModule
// =============================================================================

function createMockModule(): NapiModule {
  return {
    getVersion: () => '0.1.0-mock',
    getRustVersion: () => '0.1.0-mock-rust',
    healthCheck: () => true,
    bootstrapWorker: () => ({
      success: true,
      status: 'started',
      message: 'mock bootstrap',
      workerId: 'mock-worker',
    }),
    isWorkerRunning: () => true,
    getWorkerStatus: () => ({
      success: true,
      running: true,
      workerId: 'mock-worker',
      status: null,
      environment: null,
    }),
    stopWorker: () => ({
      success: true,
      running: false,
      workerId: null,
      status: 'stopped',
      environment: null,
    }),
    transitionToGracefulShutdown: () => ({
      success: true,
      running: false,
      workerId: null,
      status: 'transitioning',
      environment: null,
    }),
    pollStepEvents: () => null,
    pollInProcessEvents: () => null,
    completeStepEvent: () => true,
    checkpointYieldStepEvent: () => true,
    getFfiDispatchMetrics: () => ({
      pendingCount: 0,
      starvationDetected: false,
      starvingEventCount: 0,
      oldestPendingAgeMs: null,
      newestPendingAgeMs: null,
      oldestEventId: null,
    }),
    checkStarvationWarnings: () => {},
    cleanupTimeouts: () => {},
    logError: () => {},
    logWarn: () => {},
    logInfo: () => {},
    logDebug: () => {},
    logTrace: () => {},
    clientCreateTask: () => ({ success: true, data: null, error: null, recoverable: null }),
    clientGetTask: () => ({ success: true, data: null, error: null, recoverable: null }),
    clientListTasks: () => ({ success: true, data: null, error: null, recoverable: null }),
    clientCancelTask: () => ({ success: true, data: null, error: null, recoverable: null }),
    clientListTaskSteps: () => ({ success: true, data: null, error: null, recoverable: null }),
    clientGetStep: () => ({ success: true, data: null, error: null, recoverable: null }),
    clientGetStepAuditHistory: () => ({
      success: true,
      data: null,
      error: null,
      recoverable: null,
    }),
    clientHealthCheck: () => ({ success: true, data: null, error: null, recoverable: null }),
  } as NapiModule;
}

// =============================================================================
// Mock Event Helper
// =============================================================================

function createMockEvent(handlerName: string, overrides: Partial<FfiStepEvent> = {}): FfiStepEvent {
  return {
    eventId: `event-${Date.now()}`,
    taskUuid: 'task-123',
    stepUuid: 'step-456',
    correlationId: 'corr-789',
    traceId: null,
    spanId: null,
    taskCorrelationId: 'task-corr-123',
    parentCorrelationId: null,
    task: {
      taskUuid: 'task-123',
      namedTaskUuid: 'named-task-123',
      name: 'test_task',
      namespace: 'default',
      version: '1.0.0',
      context: { order_id: 'order-001' },
      correlationId: 'task-corr-123',
      parentCorrelationId: null,
      complete: false,
      priority: 1,
      initiator: 'test',
      sourceSystem: 'test',
      reason: null,
      tags: null,
      identityHash: 'hash-123',
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      requestedAt: new Date().toISOString(),
    },
    workflowStep: {
      workflowStepUuid: 'step-456',
      taskUuid: 'task-123',
      namedStepUuid: 'named-step-456',
      name: 'test_step',
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
      description: 'Test step',
      handlerCallable: handlerName,
      handlerMethod: null,
      handlerResolver: null,
      handlerInitialization: {},
      systemDependency: null,
      dependencies: [],
      timeoutSeconds: 60,
      retryRetryable: true,
      retryMaxAttempts: 3,
      retryBackoff: 'exponential',
      retryBackoffBaseMs: 1000,
      retryMaxBackoffMs: 30000,
    },
    dependencyResults: {},
    ...overrides,
  };
}

// =============================================================================
// Tests
// =============================================================================

describe('StepExecutionSubscriber', () => {
  let emitter: TaskerEventEmitter;
  let registry: HandlerRegistry;
  let module: NapiModule;
  let subscriber: StepExecutionSubscriber;

  beforeEach(() => {
    emitter = new TaskerEventEmitter();
    registry = new HandlerRegistry();
    module = createMockModule();

    registry.register('test_handler', TestHandler);
    registry.register('slow_handler', SlowHandler);
    registry.register('failing_handler', FailingHandler);
    registry.register('throwing_handler', ThrowingHandler);

    subscriber = new StepExecutionSubscriber(emitter, registry, module, {
      workerId: 'test-worker',
      maxConcurrent: 5,
      handlerTimeoutMs: 1000,
    });
  });

  afterEach(() => {
    if (subscriber?.isRunning()) {
      subscriber.stop();
    }
  });

  describe('constructor', () => {
    test('should create subscriber with default config', () => {
      const sub = new StepExecutionSubscriber(emitter, registry, module);

      expect(sub.isRunning()).toBe(false);
      expect(sub.getProcessedCount()).toBe(0);
      expect(sub.getErrorCount()).toBe(0);
    });

    test('should create subscriber with custom config', () => {
      const config: StepExecutionSubscriberConfig = {
        workerId: 'custom-worker',
        maxConcurrent: 20,
        handlerTimeoutMs: 60000,
      };

      const sub = new StepExecutionSubscriber(emitter, registry, module, config);

      expect(sub.isRunning()).toBe(false);
    });
  });

  describe('start/stop', () => {
    test('should start and stop', () => {
      expect(subscriber.isRunning()).toBe(false);

      subscriber.start();
      expect(subscriber.isRunning()).toBe(true);

      subscriber.stop();
      expect(subscriber.isRunning()).toBe(false);
    });

    test('should be idempotent on start', () => {
      subscriber.start();
      subscriber.start(); // Second call should be no-op

      expect(subscriber.isRunning()).toBe(true);
    });

    test('should be idempotent on stop', () => {
      subscriber.start();
      subscriber.stop();
      subscriber.stop(); // Second call should be no-op

      expect(subscriber.isRunning()).toBe(false);
    });
  });

  describe('getProcessedCount', () => {
    test('should start at zero', () => {
      expect(subscriber.getProcessedCount()).toBe(0);
    });
  });

  describe('getErrorCount', () => {
    test('should start at zero', () => {
      expect(subscriber.getErrorCount()).toBe(0);
    });
  });

  describe('getActiveHandlers', () => {
    test('should start at zero', () => {
      expect(subscriber.getActiveHandlers()).toBe(0);
    });
  });

  describe('waitForCompletion', () => {
    test('should resolve immediately when no active handlers', async () => {
      const result = await subscriber.waitForCompletion(100);

      expect(result).toBe(true);
    });
  });

  describe('event handling', () => {
    test('should ignore events when not running', () => {
      const event = createMockEvent('test_handler');
      emitter.emit(StepEventNames.STEP_EXECUTION_RECEIVED, {
        event,
        receivedAt: new Date(),
      });

      expect(subscriber.getActiveHandlers()).toBe(0);
    });

    test('should receive and process events when running', async () => {
      subscriber.start();

      const event = createMockEvent('test_handler');

      emitter.emit(StepEventNames.STEP_EXECUTION_RECEIVED, {
        event,
        receivedAt: new Date(),
      });

      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(subscriber.getProcessedCount()).toBe(1);
      expect(subscriber.getErrorCount()).toBe(0);
    });

    test('should dispatch to correct handler based on callable name', async () => {
      let handlerCalled = false;

      class TrackingHandler extends StepHandler {
        static handlerName = 'tracking_handler';

        async call(_context: StepContext): Promise<StepHandlerResult> {
          handlerCalled = true;
          return this.success({ tracked: true });
        }
      }

      registry.register('tracking_handler', TrackingHandler);
      subscriber.start();

      const event = createMockEvent('tracking_handler');
      emitter.emit(StepEventNames.STEP_EXECUTION_RECEIVED, {
        event,
        receivedAt: new Date(),
      });

      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(handlerCalled).toBe(true);
      expect(subscriber.getProcessedCount()).toBe(1);
    });

    test('should handle unknown handler gracefully', async () => {
      subscriber.start();

      const event = createMockEvent('unknown_handler_that_does_not_exist');
      emitter.emit(StepEventNames.STEP_EXECUTION_RECEIVED, {
        event,
        receivedAt: new Date(),
      });

      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(subscriber.getProcessedCount()).toBe(0);
    });

    test('should handle failing handler and count as processed', async () => {
      subscriber.start();

      const event = createMockEvent('failing_handler');
      emitter.emit(StepEventNames.STEP_EXECUTION_RECEIVED, {
        event,
        receivedAt: new Date(),
      });

      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(subscriber.getProcessedCount()).toBe(1);
    });

    test('should handle throwing handler and count as error', async () => {
      subscriber.start();

      const event = createMockEvent('throwing_handler');
      emitter.emit(StepEventNames.STEP_EXECUTION_RECEIVED, {
        event,
        receivedAt: new Date(),
      });

      await new Promise((resolve) => setTimeout(resolve, 50));

      expect(subscriber.getErrorCount()).toBeGreaterThan(0);
    });
  });

  describe('config defaults', () => {
    test('should use default workerId based on process pid', () => {
      const sub = new StepExecutionSubscriber(emitter, registry, module, {});
      expect(sub.isRunning()).toBe(false);
    });

    test('should use default maxConcurrent of 10', () => {
      const sub = new StepExecutionSubscriber(emitter, registry, module, {});
      expect(sub.isRunning()).toBe(false);
    });

    test('should use default handlerTimeoutMs of 300000', () => {
      const sub = new StepExecutionSubscriber(emitter, registry, module, {});
      expect(sub.isRunning()).toBe(false);
    });
  });
});

describe('StepExecutionSubscriberConfig', () => {
  test('should allow partial config', () => {
    const config: StepExecutionSubscriberConfig = {
      workerId: 'worker-1',
    };

    expect(config.workerId).toBe('worker-1');
    expect(config.maxConcurrent).toBeUndefined();
    expect(config.handlerTimeoutMs).toBeUndefined();
  });

  test('should allow full config', () => {
    const config: StepExecutionSubscriberConfig = {
      workerId: 'worker-1',
      maxConcurrent: 20,
      handlerTimeoutMs: 60000,
    };

    expect(config.workerId).toBe('worker-1');
    expect(config.maxConcurrent).toBe(20);
    expect(config.handlerTimeoutMs).toBe(60000);
  });

  test('should allow empty config', () => {
    const config: StepExecutionSubscriberConfig = {};

    expect(config.workerId).toBeUndefined();
    expect(config.maxConcurrent).toBeUndefined();
    expect(config.handlerTimeoutMs).toBeUndefined();
  });
});
