/**
 * FFI type coherence tests.
 *
 * TAS-290: Verifies that napi-rs camelCase types are correctly
 * structured and can be used as expected.
 */

import { describe, expect, it } from 'bun:test';
import type {
  BootstrapConfig,
  BootstrapResult,
  FfiDispatchMetrics,
  FfiStepEvent,
  LogFields,
  NapiCheckpointYieldData,
  NapiDependencyResult,
  NapiStepDefinition,
  NapiTaskInfo,
  NapiWorkflowStep,
  StepExecutionResult,
  WorkerStatus,
} from '../../../src/ffi/types.js';

describe('FFI Types (napi-rs camelCase)', () => {
  describe('NapiTaskInfo', () => {
    it('can be created with required fields', () => {
      const task: NapiTaskInfo = createValidTask();
      expect(task.taskUuid).toBeDefined();
      expect(task.name).toBeDefined();
      expect(task.namespace).toBeDefined();
    });

    it('supports optional nullable fields', () => {
      const task = createValidTask();
      expect(task.context).toBeNull();
      expect(task.parentCorrelationId).toBeNull();
      expect(task.initiator).toBeNull();
    });
  });

  describe('NapiWorkflowStep', () => {
    it('can be created with required fields', () => {
      const step: NapiWorkflowStep = createValidWorkflowStep();
      expect(step.workflowStepUuid).toBeDefined();
      expect(step.taskUuid).toBeDefined();
      expect(step.name).toBeDefined();
    });

    it('supports optional nullable fields', () => {
      const step = createValidWorkflowStep();
      expect(step.maxAttempts).toBe(3);
      expect(step.inputs).toBeNull();
      expect(step.results).toBeNull();
    });
  });

  describe('NapiStepDefinition', () => {
    it('can be created with required fields', () => {
      const definition: NapiStepDefinition = createValidStepDefinition();
      expect(definition.name).toBeDefined();
      expect(definition.handlerCallable).toBe('TestHandler');
      expect(definition.retryRetryable).toBe(true);
    });

    it('supports flattened handler fields', () => {
      const definition = createValidStepDefinition();
      expect(definition.handlerCallable).toBe('TestHandler');
      expect(definition.handlerInitialization).toEqual({});
      expect(definition.handlerMethod).toBeNull();
      expect(definition.handlerResolver).toBeNull();
    });

    it('supports flattened retry fields', () => {
      const definition = createValidStepDefinition();
      expect(definition.retryRetryable).toBe(true);
      expect(definition.retryMaxAttempts).toBe(3);
      expect(definition.retryBackoff).toBe('exponential');
      expect(definition.retryBackoffBaseMs).toBe(1000);
      expect(definition.retryMaxBackoffMs).toBe(30000);
    });
  });

  describe('NapiDependencyResult', () => {
    it('can be created for successful dependency', () => {
      const result: NapiDependencyResult = {
        stepUuid: 'step-123',
        success: true,
        result: { data: 'output' },
        status: 'completed',
        errorMessage: null,
        errorType: null,
        errorRetryable: null,
      };

      expect(result.success).toBe(true);
      expect(result.errorMessage).toBeNull();
    });

    it('can be created for failed dependency', () => {
      const result: NapiDependencyResult = {
        stepUuid: 'step-123',
        success: false,
        result: null,
        status: 'failed',
        errorMessage: 'Dependency failed',
        errorType: 'RuntimeError',
        errorRetryable: false,
      };

      expect(result.success).toBe(false);
      expect(result.errorMessage).toBe('Dependency failed');
    });
  });

  describe('FfiStepEvent', () => {
    it('can be created with all required fields', () => {
      const event: FfiStepEvent = createValidFfiStepEvent();

      expect(event.eventId).toBeDefined();
      expect(event.taskUuid).toBeDefined();
      expect(event.stepUuid).toBeDefined();
      expect(event.task).toBeDefined();
      expect(event.workflowStep).toBeDefined();
      expect(event.stepDefinition).toBeDefined();
    });

    it('supports dependency results map', () => {
      const event = createValidFfiStepEvent();
      event.dependencyResults = {
        'dep-step-1': {
          stepUuid: 'dep-step-1',
          success: true,
          result: { value: 42 },
          status: 'completed',
          errorMessage: null,
          errorType: null,
          errorRetryable: null,
        },
      };

      expect(event.dependencyResults['dep-step-1']?.success).toBe(true);
    });
  });

  describe('BootstrapConfig', () => {
    it('can be created with minimal fields', () => {
      const config: BootstrapConfig = {};
      expect(config).toBeDefined();
    });

    it('supports optional fields', () => {
      const config: BootstrapConfig = {
        namespace: 'payments',
        configPath: '/path/to/config.toml',
      };

      expect(config.namespace).toBe('payments');
      expect(config.configPath).toBe('/path/to/config.toml');
    });
  });

  describe('BootstrapResult', () => {
    it('can represent successful start', () => {
      const result: BootstrapResult = {
        success: true,
        status: 'started',
        message: 'Worker started successfully',
        workerId: 'worker-123',
      };

      expect(result.success).toBe(true);
      expect(result.status).toBe('started');
    });
  });

  describe('WorkerStatus', () => {
    it('can represent running worker', () => {
      const status: WorkerStatus = {
        success: true,
        running: true,
        workerId: 'worker-123',
        status: 'active',
        environment: 'production',
      };

      expect(status.running).toBe(true);
      expect(status.workerId).toBe('worker-123');
    });

    it('can represent stopped worker', () => {
      const status: WorkerStatus = {
        success: true,
        running: false,
        status: 'stopped',
        workerId: null,
        environment: null,
      };

      expect(status.running).toBe(false);
    });
  });

  describe('FfiDispatchMetrics', () => {
    it('can represent healthy state', () => {
      const metrics: FfiDispatchMetrics = {
        pendingCount: 5,
        starvationDetected: false,
        starvingEventCount: 0,
        oldestPendingAgeMs: 100,
        newestPendingAgeMs: 10,
        oldestEventId: null,
      };

      expect(metrics.starvationDetected).toBe(false);
      expect(metrics.starvingEventCount).toBe(0);
    });

    it('can represent starvation state', () => {
      const metrics: FfiDispatchMetrics = {
        pendingCount: 50,
        starvationDetected: true,
        starvingEventCount: 10,
        oldestPendingAgeMs: 30000,
        newestPendingAgeMs: 5000,
        oldestEventId: 'event-1',
      };

      expect(metrics.starvationDetected).toBe(true);
      expect(metrics.starvingEventCount).toBe(10);
    });

    it('supports null age fields when empty', () => {
      const metrics: FfiDispatchMetrics = {
        pendingCount: 0,
        starvationDetected: false,
        starvingEventCount: 0,
        oldestPendingAgeMs: null,
        newestPendingAgeMs: null,
        oldestEventId: null,
      };

      expect(metrics.oldestPendingAgeMs).toBeNull();
    });
  });

  describe('NapiStepExecutionResult', () => {
    it('can represent successful completion', () => {
      const result: StepExecutionResult = {
        stepUuid: 'step-123',
        success: true,
        result: { output: 'data' },
        status: 'completed',
        metadata: {
          executionTimeMs: 42,
          workerId: 'worker-1',
          completedAt: new Date().toISOString(),
          retryable: null,
          errorType: null,
          errorCode: null,
          custom: null,
        },
        error: null,
        orchestrationMetadata: null,
      };

      expect(result.success).toBe(true);
      expect(result.status).toBe('completed');
      expect(result.error).toBeNull();
    });

    it('can represent failure with error details', () => {
      const result: StepExecutionResult = {
        stepUuid: 'step-123',
        success: false,
        result: {},
        status: 'failed',
        metadata: {
          executionTimeMs: 100,
          workerId: 'worker-1',
          completedAt: new Date().toISOString(),
          retryable: true,
          errorType: 'RuntimeError',
          errorCode: null,
          custom: null,
        },
        error: {
          message: 'Handler threw exception',
          errorType: 'RuntimeError',
          retryable: true,
          statusCode: null,
          backtrace: null,
          context: null,
        },
        orchestrationMetadata: null,
      };

      expect(result.success).toBe(false);
      expect(result.status).toBe('failed');
      expect(result.error?.message).toBe('Handler threw exception');
    });
  });

  describe('NapiCheckpointYieldData', () => {
    it('can be created with required fields', () => {
      const data: NapiCheckpointYieldData = {
        stepUuid: 'step-123',
        cursor: 500,
        itemsProcessed: 250,
      };

      expect(data.cursor).toBe(500);
      expect(data.itemsProcessed).toBe(250);
    });

    it('supports accumulated results', () => {
      const data: NapiCheckpointYieldData = {
        stepUuid: 'step-123',
        cursor: 1000,
        itemsProcessed: 500,
        accumulatedResults: { sum: 5000, count: 500 },
      };

      expect(data.accumulatedResults?.sum).toBe(5000);
    });
  });

  describe('LogFields', () => {
    it('supports string values', () => {
      const fields: LogFields = {
        request_id: 'req-123',
        handler: 'MyHandler',
      };

      expect(fields.request_id).toBe('req-123');
    });

    it('supports numeric values', () => {
      const fields: LogFields = {
        execution_time_ms: 150,
        attempt: 2,
      };

      expect(fields.execution_time_ms).toBe(150);
    });

    it('supports boolean values', () => {
      const fields: LogFields = {
        success: true,
        retryable: false,
      };

      expect(fields.success).toBe(true);
    });

    it('supports null values', () => {
      const fields: LogFields = {
        optional_field: null,
      };

      expect(fields.optional_field).toBeNull();
    });
  });
});

// Test helpers

function createValidTask(): NapiTaskInfo {
  return {
    taskUuid: 'task-123',
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
  };
}

function createValidWorkflowStep(): NapiWorkflowStep {
  return {
    workflowStepUuid: 'step-123',
    taskUuid: 'task-123',
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
  };
}

function createValidStepDefinition(): NapiStepDefinition {
  return {
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
  };
}

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
    task: createValidTask(),
    workflowStep: createValidWorkflowStep(),
    stepDefinition: createValidStepDefinition(),
    dependencyResults: {},
  };
}
