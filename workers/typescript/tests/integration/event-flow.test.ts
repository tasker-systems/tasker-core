/**
 * Integration tests for the complete event flow.
 *
 * These tests verify that the EventPoller → EventEmitter → StepExecutionSubscriber
 * flow works correctly using real (non-mock) components.
 *
 * TAS-290: Updated for napi-rs — uses NapiModule mock, camelCase event fields,
 * flattened stepDefinition.handlerCallable.
 */

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { TaskerEventEmitter } from '../../src/events/event-emitter';
import { StepEventNames } from '../../src/events/event-names';
import type { NapiModule } from '../../src/ffi/ffi-layer';
import type { FfiStepEvent } from '../../src/ffi/types';
import { StepHandler } from '../../src/handler/base';
import { HandlerRegistry } from '../../src/handler/registry';
import { StepExecutionSubscriber } from '../../src/subscriber/step-execution-subscriber';
import type { StepContext } from '../../src/types/step-context';
import type { StepHandlerResult } from '../../src/types/step-handler-result';

/**
 * Create a mock NapiModule for testing.
 */
function createMockModule(): NapiModule {
  return {
    getVersion: () => '0.0.0-mock',
    getRustVersion: () => '0.0.0-mock',
    healthCheck: () => true,
    bootstrapWorker: () => ({
      success: true,
      status: 'started',
      message: 'Mock bootstrap',
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

// Create a mock FFI step event with camelCase fields (napi-rs format)
function createMockEvent(handlerName: string): FfiStepEvent {
  return {
    eventId: `event-${Date.now()}-${Math.random()}`,
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
  };
}

describe('Event Flow Integration', () => {
  let emitter: TaskerEventEmitter;
  let registry: HandlerRegistry;
  let module: NapiModule;
  let subscriber: StepExecutionSubscriber;

  beforeEach(() => {
    // Create fresh instances for each test (explicit construction)
    emitter = new TaskerEventEmitter();
    registry = new HandlerRegistry();
    module = createMockModule();
  });

  afterEach(() => {
    if (subscriber?.isRunning()) {
      subscriber.stop();
    }
  });

  test('TaskerEventEmitter instances have unique IDs', () => {
    const emitter1 = new TaskerEventEmitter();
    const emitter2 = new TaskerEventEmitter();

    expect(emitter1.getInstanceId()).not.toBe(emitter2.getInstanceId());
  });

  test('emitStepReceived should emit correct payload format', () => {
    let receivedPayload: unknown = null;

    emitter.on(StepEventNames.STEP_EXECUTION_RECEIVED, (payload) => {
      receivedPayload = payload;
    });

    const event = createMockEvent('test_handler');
    emitter.emitStepReceived(event);

    expect(receivedPayload).not.toBeNull();
    expect((receivedPayload as { event: FfiStepEvent }).event).toBe(event);
    expect((receivedPayload as { receivedAt: Date }).receivedAt).toBeInstanceOf(Date);
  });

  test('StepExecutionSubscriber should receive events via emitStepReceived', async () => {
    let handlerCalled = false;
    let receivedContext: StepContext | null = null;

    // Create tracking handler
    class IntegrationTestHandler extends StepHandler {
      static handlerName = 'integration_test_handler';

      async call(context: StepContext): Promise<StepHandlerResult> {
        handlerCalled = true;
        receivedContext = context;
        return this.success({ integration_test: true });
      }
    }

    registry.register('integration_test_handler', IntegrationTestHandler);

    // Create subscriber with the SAME emitter and mock module (explicit injection)
    subscriber = new StepExecutionSubscriber(emitter, registry, module, {
      workerId: 'integration-test-worker',
    });
    subscriber.start();

    // Use the emitter's helper method (same as EventPoller does)
    const event = createMockEvent('integration_test_handler');
    emitter.emitStepReceived(event);

    // Wait for async processing
    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(handlerCalled).toBe(true);
    expect(receivedContext).not.toBeNull();
    expect(subscriber.getProcessedCount()).toBe(1);
  });

  test('complete flow: emitter → subscriber → handler with correct context', async () => {
    let capturedStepUuid: string | null = null;
    let capturedTaskUuid: string | null = null;
    let capturedHandlerName: string | null = null;

    class ContextCapturingHandler extends StepHandler {
      static handlerName = 'context_capturing_handler';

      async call(context: StepContext): Promise<StepHandlerResult> {
        capturedStepUuid = context.stepUuid;
        capturedTaskUuid = context.taskUuid;
        capturedHandlerName = context.handlerName;
        return this.success({ captured: true });
      }
    }

    registry.register('context_capturing_handler', ContextCapturingHandler);

    subscriber = new StepExecutionSubscriber(emitter, registry, module, {
      workerId: 'context-test-worker',
    });
    subscriber.start();

    const event = createMockEvent('context_capturing_handler');
    event.stepUuid = 'test-step-uuid-12345';
    event.taskUuid = 'test-task-uuid-67890';

    emitter.emitStepReceived(event);

    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(capturedStepUuid).toBe('test-step-uuid-12345');
    expect(capturedTaskUuid).toBe('test-task-uuid-67890');
    expect(capturedHandlerName).toBe('context_capturing_handler');
  });

  test('multiple events should all be processed', async () => {
    let processCount = 0;

    class CountingHandler extends StepHandler {
      static handlerName = 'counting_handler';

      async call(_context: StepContext): Promise<StepHandlerResult> {
        processCount++;
        return this.success({ count: processCount });
      }
    }

    registry.register('counting_handler', CountingHandler);

    subscriber = new StepExecutionSubscriber(emitter, registry, module, {
      workerId: 'counting-test-worker',
    });
    subscriber.start();

    // Emit 5 events
    for (let i = 0; i < 5; i++) {
      const event = createMockEvent('counting_handler');
      emitter.emitStepReceived(event);
    }

    await new Promise((resolve) => setTimeout(resolve, 200));

    expect(processCount).toBe(5);
    expect(subscriber.getProcessedCount()).toBe(5);
  });
});
