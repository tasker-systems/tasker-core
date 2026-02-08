/**
 * Client API FFI Integration Tests (TAS-231)
 *
 * Tests the client FFI functions against a running orchestration server.
 * Verifies full round-trip: TypeScript -> C FFI -> Rust -> REST API -> PostgreSQL -> response.
 *
 * Prerequisites:
 * - FFI library built: cargo build -p tasker-worker-ts
 * - DATABASE_URL set and database accessible
 * - Orchestration server running (default: http://localhost:8080)
 * - FFI_CLIENT_TESTS=true environment variable
 * - FFI_BOOTSTRAP_TESTS=true environment variable
 *
 * Run: FFI_CLIENT_TESTS=true FFI_BOOTSTRAP_TESTS=true bun test tests/integration/ffi/client-api.test.ts
 */

import { afterAll, beforeAll, describe, expect, it } from 'bun:test';
import { BunRuntime } from '../../../src/ffi/bun-runtime.js';
import type { ClientResult } from '../../../src/ffi/types.js';
import {
  findLibraryPath,
  SKIP_CLIENT_MESSAGE,
  SKIP_LIBRARY_MESSAGE,
  shouldRunClientTests,
} from './common.js';

describe('Client API FFI Integration', () => {
  let runtime: BunRuntime;
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

    runtime = new BunRuntime();
    await runtime.load(libraryPath);

    // Bootstrap the worker (required for client to be initialized)
    const bootstrapResult = runtime.bootstrapWorker({});
    if (!bootstrapResult.success) {
      console.warn(`Skipping: Bootstrap failed: ${bootstrapResult.error}`);
      skipAll = true;
      return;
    }
  });

  afterAll(() => {
    if (runtime?.isWorkerRunning()) {
      runtime.stopWorker();
    }
    if (runtime?.isLoaded) {
      runtime.unload();
    }
  });

  describe('health check', () => {
    it('returns healthy response from orchestration API', () => {
      if (skipAll) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: ClientResult = runtime.clientHealthCheck();
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
      expect(result.data.healthy).toBeDefined();
    });
  });

  describe('task lifecycle', () => {
    it('creates a task via client API', () => {
      if (skipAll) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const request = JSON.stringify({
        name: 'success_only_ts',
        namespace: 'test_scenarios_ts',
        version: '1.0.0',
        context: { test_run: 'client_api_integration', run_id: crypto.randomUUID() },
        initiator: 'typescript-client-test',
        source_system: 'integration-test',
        reason: 'TAS-231 client API integration test',
      });

      const result: ClientResult = runtime.clientCreateTask(request);
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
      expect(result.data.task_uuid).toBeDefined();
      expect(typeof result.data.task_uuid).toBe('string');
      expect(result.data.name).toBe('success_only_ts');
      expect(result.data.namespace).toBe('test_scenarios_ts');

      // Save for subsequent tests
      taskUuid = result.data.task_uuid;
    });

    it('gets the created task by UUID', () => {
      if (skipAll || !taskUuid) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: ClientResult = runtime.clientGetTask(taskUuid);
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
      expect(result.data.task_uuid).toBe(taskUuid);
      expect(result.data.name).toBe('success_only_ts');
      expect(result.data.namespace).toBe('test_scenarios_ts');
      expect(result.data.version).toBe('1.0.0');
      expect(result.data.created_at).toBeDefined();
      expect(result.data.updated_at).toBeDefined();
      expect(result.data.correlation_id).toBeDefined();
      expect(typeof result.data.total_steps).toBe('number');
    });

    it('lists tasks with pagination', () => {
      if (skipAll) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const params = JSON.stringify({ limit: 50, offset: 0 });
      const result: ClientResult = runtime.clientListTasks(params);
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
      expect(result.data.tasks).toBeDefined();
      expect(Array.isArray(result.data.tasks)).toBe(true);
      expect(result.data.pagination).toBeDefined();
      expect(typeof result.data.pagination.total_count).toBe('number');
      expect(result.data.pagination.total_count).toBeGreaterThanOrEqual(1);
    });

    it('lists task steps', () => {
      if (skipAll || !taskUuid) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: ClientResult = runtime.clientListTaskSteps(taskUuid);
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
      // The response is an array of steps
      expect(Array.isArray(result.data)).toBe(true);

      if (result.data.length > 0) {
        const step = result.data[0];
        expect(step.step_uuid).toBeDefined();
        expect(step.task_uuid).toBe(taskUuid);
        expect(step.name).toBeDefined();
        // Save for subsequent tests
        stepUuid = step.step_uuid;
      }
    });

    it('gets a specific step', () => {
      if (skipAll || !taskUuid || !stepUuid) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: ClientResult = runtime.clientGetStep(taskUuid, stepUuid);
      expect(result.success).toBe(true);
      expect(result.data).toBeDefined();
      expect(result.data.step_uuid).toBe(stepUuid);
      expect(result.data.task_uuid).toBe(taskUuid);
      expect(result.data.name).toBeDefined();
      expect(result.data.current_state).toBeDefined();
      expect(typeof result.data.attempts).toBe('number');
      expect(typeof result.data.max_attempts).toBe('number');
    });

    it('gets step audit history', () => {
      if (skipAll || !taskUuid || !stepUuid) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      const result: ClientResult = runtime.clientGetStepAuditHistory(taskUuid, stepUuid);
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

      const result: ClientResult = runtime.clientCancelTask(taskUuid);
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
        const result: ClientResult = runtime.clientHealthCheck();
        expect(result.success).toBe(true);
      }
    });

    it('handles error cases gracefully', () => {
      if (skipAll) {
        console.warn(SKIP_CLIENT_MESSAGE);
        return;
      }

      // Get a non-existent task - should return error, not crash
      const result: ClientResult = runtime.clientGetTask('00000000-0000-0000-0000-000000000000');
      // May succeed with 404-mapped error or fail gracefully
      expect(result).toBeDefined();
      expect(typeof result.success).toBe('boolean');
    });
  });
});
