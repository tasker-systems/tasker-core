/**
 * Event poller coherence tests.
 *
 * TAS-290: Updated to use NapiModule mock instead of TaskerRuntime.
 */

import { beforeEach, describe, expect, it, mock } from 'bun:test';
import { TaskerEventEmitter } from '../../../src/events/event-emitter.js';
import {
  createEventPoller,
  EventPoller,
  type EventPollerConfig,
} from '../../../src/events/event-poller.js';
import type { NapiModule } from '../../../src/ffi/ffi-layer.js';
import type { FfiStepEvent } from '../../../src/ffi/types.js';

describe('EventPoller', () => {
  let mockModule: NapiModule;
  let emitter: TaskerEventEmitter;

  beforeEach(() => {
    mockModule = createMockModule();
    emitter = new TaskerEventEmitter();
  });

  describe('construction', () => {
    it('creates a poller with default configuration', () => {
      const poller = new EventPoller(mockModule, emitter);
      expect(poller).toBeInstanceOf(EventPoller);
      expect(poller.getState()).toBe('stopped');
    });

    it('creates a poller with custom configuration', () => {
      const config: EventPollerConfig = {
        pollIntervalMs: 20,
        starvationCheckInterval: 50,
        cleanupInterval: 500,
        metricsInterval: 50,
        maxEventsPerCycle: 50,
      };
      const poller = new EventPoller(mockModule, emitter, config);
      expect(poller).toBeInstanceOf(EventPoller);
    });

    it('uses the provided event emitter', () => {
      const poller = new EventPoller(mockModule, emitter);
      const startedHandler = mock(() => {});
      emitter.on('poller.started', startedHandler);

      poller.start();
      poller.stop();

      expect(startedHandler).toHaveBeenCalled();
    });
  });

  describe('lifecycle', () => {
    it('starts in stopped state', () => {
      const poller = new EventPoller(mockModule, emitter);
      expect(poller.getState()).toBe('stopped');
      expect(poller.isRunning()).toBe(false);
    });

    it('transitions to running state on start', () => {
      const poller = new EventPoller(mockModule, emitter);
      poller.start();

      expect(poller.getState()).toBe('running');
      expect(poller.isRunning()).toBe(true);

      poller.stop();
    });

    it('transitions back to stopped state on stop', async () => {
      const poller = new EventPoller(mockModule, emitter);
      poller.start();
      await poller.stop();

      expect(poller.getState()).toBe('stopped');
      expect(poller.isRunning()).toBe(false);
    });

    it('is idempotent when starting multiple times', () => {
      const poller = new EventPoller(mockModule, emitter);
      poller.start();
      poller.start(); // Should not throw

      expect(poller.getState()).toBe('running');
      poller.stop();
    });

    it('is idempotent when stopping multiple times', async () => {
      const poller = new EventPoller(mockModule, emitter);
      poller.start();
      await poller.stop();
      await poller.stop(); // Should not throw

      expect(poller.getState()).toBe('stopped');
    });
  });

  describe('event emission', () => {
    it('emits poller.started when starting', () => {
      const handler = mock(() => {});
      emitter.on('poller.started', handler);

      const poller = new EventPoller(mockModule, emitter);
      poller.start();
      poller.stop();

      expect(handler).toHaveBeenCalledTimes(1);
    });

    it('emits poller.stopped when stopping', async () => {
      const handler = mock(() => {});
      emitter.on('poller.stopped', handler);

      const poller = new EventPoller(mockModule, emitter);
      poller.start();
      await poller.stop();

      expect(handler).toHaveBeenCalledTimes(1);
    });
  });

  describe('callbacks', () => {
    it('supports onStepEvent callback registration', () => {
      const poller = new EventPoller(mockModule, emitter);
      const callback = mock(async () => {});

      const result = poller.onStepEvent(callback);
      expect(result).toBe(poller); // Fluent API
    });

    it('supports onError callback registration', () => {
      const poller = new EventPoller(mockModule, emitter);
      const callback = mock(() => {});

      const result = poller.onError(callback);
      expect(result).toBe(poller); // Fluent API
    });

    it('supports onMetrics callback registration', () => {
      const poller = new EventPoller(mockModule, emitter);
      const callback = mock(() => {});

      const result = poller.onMetrics(callback);
      expect(result).toBe(poller); // Fluent API
    });

    it('supports fluent callback chaining', () => {
      const poller = new EventPoller(mockModule, emitter);

      const result = poller
        .onStepEvent(async () => {})
        .onError(() => {})
        .onMetrics(() => {});

      expect(result).toBe(poller);
    });
  });

  describe('polling counters', () => {
    it('initializes poll count to 0', () => {
      const poller = new EventPoller(mockModule, emitter);
      expect(poller.getPollCount()).toBe(0);
    });

    it('initializes cycle count to 0', () => {
      const poller = new EventPoller(mockModule, emitter);
      expect(poller.getCycleCount()).toBe(0);
    });

    it('resets counters on start', async () => {
      const poller = new EventPoller(mockModule, emitter, {
        pollIntervalMs: 5,
      });

      poller.start();
      // Wait for a few poll cycles
      await sleep(25);
      await poller.stop();

      const countAfterFirstRun = poller.getPollCount();
      expect(countAfterFirstRun).toBeGreaterThan(0);

      // Start again - counters should reset
      poller.start();
      // Poll count resets immediately on start
      expect(poller.getPollCount()).toBe(0);
      await poller.stop();
    });
  });

  describe('createEventPoller factory', () => {
    it('creates an EventPoller instance', () => {
      const poller = createEventPoller(mockModule, emitter);
      expect(poller).toBeInstanceOf(EventPoller);
    });

    it('passes configuration to the poller', () => {
      const config: EventPollerConfig = {
        pollIntervalMs: 50,
      };
      const poller = createEventPoller(mockModule, emitter, config);
      expect(poller).toBeInstanceOf(EventPoller);
    });
  });
});

// Mock NapiModule for testing

function createMockModule(): NapiModule {
  return {
    getVersion: () => '1.0.0-mock',
    getRustVersion: () => '1.0.0-mock-rust',
    healthCheck: () => true,
    bootstrapWorker: () => ({
      success: true,
      status: 'started',
      message: 'Mock worker started',
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
      workerId: 'mock-worker',
      status: 'stopped',
      environment: null,
    }),
    transitionToGracefulShutdown: () => ({
      success: true,
      running: false,
      workerId: 'mock-worker',
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

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
