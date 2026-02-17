/**
 * Client API FFI Integration Tests (TAS-231)
 *
 * Tests the client FFI functions against a running orchestration server.
 * Verifies full round-trip: TypeScript -> napi-rs -> Rust -> REST API -> PostgreSQL -> response.
 *
 * TAS-290: Updated to use FfiLayer + NapiModule instead of NodeRuntime.
 * Client calls now pass typed objects directly (no JSON serialization).
 *
 * Prerequisites:
 * - FFI library built: cargo build -p tasker-ts
 * - DATABASE_URL set and database accessible
 * - Orchestration server running (default: http://localhost:8080)
 * - FFI_CLIENT_TESTS=true environment variable
 * - FFI_BOOTSTRAP_TESTS=true environment variable
 *
 * Run: FFI_CLIENT_TESTS=true FFI_BOOTSTRAP_TESTS=true bun test tests/integration/ffi/client-api.test.ts
 */

import { afterAll, beforeAll, describe, expect, it } from 'bun:test';
import { FfiLayer, type NapiModule } from '../../../src/ffi/ffi-layer.js';
import type { NapiClientResult } from '../../../src/ffi/types.js';
import {
  findLibraryPath,
  SKIP_CLIENT_MESSAGE,
  SKIP_LIBRARY_MESSAGE,
  shouldRunClientTests,
} from './common.js';

describe('Client API FFI Integration', () => {
  let ffiLayer: FfiLayer;
  let module: NapiModule;
  let libraryPath: string | null;
  let skipAll = false;

  // Shared state across ordered tests
  let taskUuid: string;
  let stepUuid: string;

  beforeAll(async () => {
    libraryPath = findLibraryPath();
    if (!libraryPath) {
      console.warn(SKIP_LIBRARY_MESSAGE);
      skipAll = true;
      return;
    }

    if (!shouldRunClientTests()) {
      console.warn(SKIP_CLIENT_MESSAGE);
      skipAll = true;
      return;
    }

    ffiLayer = new FfiLayer();
    await ffiLayer.load(libraryPath);
    module = ffiLayer.getModule();

    // Bootstrap the worker (required for client to be initialized)
    const bootstrapResult = module.bootstrapWorker({});
    if (!bootstrapResult.success) {
      console.warn(`Skipping: Bootstrap failed: ${bootstrapResult.message}`);
      skipAll = true;
      return;
    }
  });

  afterAll(async () => {
    if (module && ffiLayer.isLoaded()) {
      try {
        if (module.isWorkerRunning()) {
          module.stopWorker();
        }
      } catch {
        // Ignore cleanup errors
      }
      await ffiLayer.unload();
    }
  });

  describe('health check', () => {
    it('returns healthy response from orchestration API', () => {
      if (skipAll) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: NapiClientResult = module.clientHealthCheck();
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
    });
  });

  describe('task lifecycle', () => {
    it('creates a task via client API', () => {
      if (skipAll) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      // TAS-290: Pass typed object directly (no JSON.stringify)
      const result: NapiClientResult = module.clientCreateTask({
        name: 'success_only_ts',
        namespace: 'test_scenarios_ts',
        version: '1.0.0',
        context: { test_run: 'client_api_integration', run_id: crypto.randomUUID() },
        initiator: 'typescript-client-test',
        sourceSystem: 'integration-test',
        reason: 'TAS-231 client API integration test',
      });
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();

      const data = result.data as Record<string, unknown>;
      expect(data.task_uuid).toBeDefined();
      expect(typeof data.task_uuid).toBe('string');
      expect(data.name).toBe('success_only_ts');
      expect(data.namespace).toBe('test_scenarios_ts');

      // Save for subsequent tests
      taskUuid = data.task_uuid as string;
    });

    it('gets the created task by UUID', () => {
      if (skipAll || !taskUuid) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: NapiClientResult = module.clientGetTask(taskUuid);
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();

      const data = result.data as Record<string, unknown>;
      expect(data.task_uuid).toBe(taskUuid);
      expect(data.name).toBe('success_only_ts');
      expect(data.namespace).toBe('test_scenarios_ts');
      expect(data.version).toBe('1.0.0');
      expect(data.created_at).toBeDefined();
      expect(data.updated_at).toBeDefined();
      expect(data.correlation_id).toBeDefined();
      expect(typeof data.total_steps).toBe('number');
    });

    it('lists tasks with pagination', () => {
      if (skipAll) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      // TAS-290: Pass typed object directly
      const result: NapiClientResult = module.clientListTasks({ limit: 50, offset: 0 });
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();

      const data = result.data as Record<string, unknown>;
      expect(data.tasks).toBeDefined();
      expect(Array.isArray(data.tasks)).toBe(true);
      expect(data.pagination).toBeDefined();
      const pagination = data.pagination as Record<string, unknown>;
      expect(typeof pagination.total_count).toBe('number');
      expect(pagination.total_count as number).toBeGreaterThanOrEqual(1);
    });

    it('lists task steps', () => {
      if (skipAll || !taskUuid) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: NapiClientResult = module.clientListTaskSteps(taskUuid);
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
      // The response is an array of steps
      const data = result.data as Record<string, unknown>[];
      expect(Array.isArray(data)).toBe(true);

      if (data.length > 0) {
        const step = data[0];
        expect(step.step_uuid).toBeDefined();
        expect(step.task_uuid).toBe(taskUuid);
        expect(step.name).toBeDefined();
        // Save for subsequent tests
        stepUuid = step.step_uuid as string;
      }
    });

    it('gets a specific step', () => {
      if (skipAll || !taskUuid || !stepUuid) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: NapiClientResult = module.clientGetStep(taskUuid, stepUuid);
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();

      const data = result.data as Record<string, unknown>;
      expect(data.step_uuid).toBe(stepUuid);
      expect(data.task_uuid).toBe(taskUuid);
      expect(data.name).toBeDefined();
      expect(data.current_state).toBeDefined();
      expect(typeof data.attempts).toBe('number');
      expect(typeof data.max_attempts).toBe('number');
    });

    it('gets step audit history', () => {
      if (skipAll || !taskUuid || !stepUuid) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: NapiClientResult = module.clientGetStepAuditHistory(taskUuid, stepUuid);
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
      // Audit history is an array (may be empty for newly created tasks)
      expect(Array.isArray(result.data)).toBe(true);
    });

    it('cancels the task', () => {
      if (skipAll || !taskUuid) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: NapiClientResult = module.clientCancelTask(taskUuid);
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
    });
  });

  describe('memory safety', () => {
    it('handles rapid successive health checks without crash', () => {
      if (skipAll) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      // Rapid-fire 100 calls to verify no memory leaks or use-after-free
      for (let i = 0; i < 100; i++) {
        const result: NapiClientResult = module.clientHealthCheck();
        expect(result.success).toBe(true);
      }
    });

    it('handles error cases gracefully', () => {
      if (skipAll) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      // Get a non-existent task - should return error, not crash
      const result: NapiClientResult = module.clientGetTask('00000000-0000-0000-0000-000000000000');
      // May succeed with 404-mapped error or fail gracefully
      expect(result).toBeDefined();
      expect(typeof result.success).toBe('boolean');
    });
  });
});
