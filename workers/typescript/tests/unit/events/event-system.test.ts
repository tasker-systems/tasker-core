/**
 * EventSystem lifecycle tests.
 *
 * TAS-290: Updated to use NapiModule mock instead of TaskerRuntime.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { EventSystem } from '../../../src/events/event-system.js';
import type { NapiModule } from '../../../src/ffi/ffi-layer.js';
import type { HandlerRegistryInterface } from '../../../src/subscriber/step-execution-subscriber.js';

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
// Mock Registry
// =============================================================================

class MockHandlerRegistry implements HandlerRegistryInterface {
  async resolve(_name: string) {
    return null;
  }
}

// =============================================================================
// Tests
// =============================================================================

describe('EventSystem', () => {
  let module: NapiModule;
  let registry: MockHandlerRegistry;
  let eventSystem: EventSystem;

  afterEach(async () => {
    // Ensure cleanup even if test fails
    if (eventSystem?.isRunning()) {
      await eventSystem.stop();
    }
  });

  function createEventSystem(config = {}): EventSystem {
    module = createMockModule();
    registry = new MockHandlerRegistry();
    eventSystem = new EventSystem(module, registry, config);
    return eventSystem;
  }

  describe('constructor', () => {
    test('should create in stopped state', () => {
      const system = createEventSystem();

      expect(system.isRunning()).toBe(false);
    });

    test('should accept configuration', () => {
      const system = createEventSystem({
        poller: { pollIntervalMs: 50 },
        subscriber: { workerId: 'test-worker', maxConcurrent: 5 },
      });

      expect(system.isRunning()).toBe(false);
    });
  });

  describe('getEmitter', () => {
    test('should return the emitter instance', () => {
      const system = createEventSystem();
      const emitter = system.getEmitter();

      expect(emitter).toBeDefined();
      expect(typeof emitter.emit).toBe('function');
      expect(typeof emitter.on).toBe('function');
    });

    test('should return same emitter on multiple calls', () => {
      const system = createEventSystem();
      const emitter1 = system.getEmitter();
      const emitter2 = system.getEmitter();

      expect(emitter1).toBe(emitter2);
    });
  });

  describe('isRunning', () => {
    test('should be false before start', () => {
      const system = createEventSystem();
      expect(system.isRunning()).toBe(false);
    });

    test('should be true after start', () => {
      const system = createEventSystem();
      system.start();
      expect(system.isRunning()).toBe(true);
    });

    test('should be false after stop', async () => {
      const system = createEventSystem();
      system.start();
      await system.stop();
      expect(system.isRunning()).toBe(false);
    });
  });

  describe('start', () => {
    test('should set running to true', () => {
      const system = createEventSystem();
      system.start();

      expect(system.isRunning()).toBe(true);
    });

    test('should be idempotent (calling start when running)', () => {
      const system = createEventSystem();
      system.start();
      system.start(); // Should not throw

      expect(system.isRunning()).toBe(true);
    });
  });

  describe('stop', () => {
    test('should set running to false', async () => {
      const system = createEventSystem();
      system.start();
      expect(system.isRunning()).toBe(true);

      await system.stop();

      expect(system.isRunning()).toBe(false);
    });

    test('should be safe to call when not running', async () => {
      const system = createEventSystem();

      // Should not throw
      await system.stop();

      expect(system.isRunning()).toBe(false);
    });

    test('should be idempotent', async () => {
      const system = createEventSystem();
      system.start();

      await system.stop();
      await system.stop(); // Second stop should be safe

      expect(system.isRunning()).toBe(false);
    });
  });

  describe('getStats', () => {
    test('should return stats with running=false before start', () => {
      const system = createEventSystem();
      const stats = system.getStats();

      expect(stats.running).toBe(false);
      expect(stats.processedCount).toBe(0);
      expect(stats.errorCount).toBe(0);
      expect(stats.activeHandlers).toBe(0);
      expect(stats.pollCount).toBe(0);
    });

    test('should return stats with running=true after start', () => {
      const system = createEventSystem();
      system.start();
      const stats = system.getStats();

      expect(stats.running).toBe(true);
    });

    test('should return stats with running=false after stop', async () => {
      const system = createEventSystem();
      system.start();
      await system.stop();
      const stats = system.getStats();

      expect(stats.running).toBe(false);
    });
  });
});
