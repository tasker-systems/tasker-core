/**
 * Unit tests for the TaskerClient high-level client wrapper.
 *
 * These tests mock the FfiLayer and runtime to verify that TaskerClient
 * constructs correct requests, applies defaults, unwraps ClientResult
 * envelopes, and throws TaskerClientError on failures.
 */

import { describe, expect, it, mock } from 'bun:test';
import { TaskerClient, TaskerClientError } from '../../../src/client/index.js';
import type { FfiLayer } from '../../../src/ffi/ffi-layer.js';
import type { ClientResult } from '../../../src/ffi/types.js';

/**
 * Create a mock FfiLayer with a mock runtime.
 */
function createMockFfiLayer(runtimeOverrides: Record<string, unknown> = {}) {
  const mockRuntime = {
    clientCreateTask: mock(() => ({}) as ClientResult),
    clientGetTask: mock(() => ({}) as ClientResult),
    clientListTasks: mock(() => ({}) as ClientResult),
    clientCancelTask: mock(() => ({}) as ClientResult),
    clientListTaskSteps: mock(() => ({}) as ClientResult),
    clientGetStep: mock(() => ({}) as ClientResult),
    clientGetStepAuditHistory: mock(() => ({}) as ClientResult),
    clientHealthCheck: mock(() => ({}) as ClientResult),
    ...runtimeOverrides,
  };

  const mockFfiLayer = {
    getRuntime: () => mockRuntime,
  } as unknown as FfiLayer;

  return { mockFfiLayer, mockRuntime };
}

function successResult(data: unknown): ClientResult {
  return { success: true, data, error: null, recoverable: null };
}

function errorResult(error: string, recoverable = false): ClientResult {
  return { success: false, data: null, error, recoverable };
}

const MOCK_TASK_RESPONSE = {
  task_uuid: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
  name: 'test_task',
  namespace: 'test',
  version: '1.0.0',
  status: 'pending',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
  context: { key: 'value' },
  initiator: 'tasker-core-typescript',
  source_system: 'tasker-core',
  reason: 'Task requested',
  correlation_id: 'corr-id-123',
  total_steps: 3,
  pending_steps: 3,
  in_progress_steps: 0,
  completed_steps: 0,
  failed_steps: 0,
  ready_steps: 1,
  execution_status: 'pending',
  recommended_action: 'wait',
  completion_percentage: 0.0,
  health_status: 'healthy',
  steps: [],
};

const MOCK_STEP_RESPONSE = {
  step_uuid: '11111111-2222-3333-4444-555555555555',
  task_uuid: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
  name: 'validate_input',
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
  completed_at: null,
  results: null,
  current_state: 'pending',
  dependencies_satisfied: true,
  retry_eligible: false,
  ready_for_execution: true,
  total_parents: 0,
  completed_parents: 0,
  attempts: 0,
  max_attempts: 3,
};

describe('TaskerClient', () => {
  describe('createTask', () => {
    it('creates a task with defaults and returns typed response', () => {
      const { mockFfiLayer, mockRuntime } = createMockFfiLayer({
        clientCreateTask: mock(() => successResult(MOCK_TASK_RESPONSE)),
      });
      const client = new TaskerClient(mockFfiLayer);

      const result = client.createTask({ name: 'test_task', namespace: 'test' });

      expect(mockRuntime.clientCreateTask).toHaveBeenCalledTimes(1);
      const requestJson = (mockRuntime.clientCreateTask as ReturnType<typeof mock>).mock
        .calls[0][0] as string;
      const request = JSON.parse(requestJson);

      expect(request.name).toBe('test_task');
      expect(request.namespace).toBe('test');
      expect(request.version).toBe('1.0.0');
      expect(request.initiator).toBe('tasker-core-typescript');
      expect(request.source_system).toBe('tasker-core');
      expect(request.reason).toBe('Task requested');
      expect(request.context).toEqual({});
      expect(request.tags).toEqual([]);

      expect(result.task_uuid).toBe('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee');
      expect(result.name).toBe('test_task');
    });

    it('allows overriding all defaults', () => {
      const { mockFfiLayer, mockRuntime } = createMockFfiLayer({
        clientCreateTask: mock(() => successResult(MOCK_TASK_RESPONSE)),
      });
      const client = new TaskerClient(mockFfiLayer);

      client.createTask({
        name: 'test_task',
        namespace: 'custom',
        version: '2.0.0',
        context: { order_id: 123 },
        initiator: 'my-app',
        sourceSystem: 'my-system',
        reason: 'Custom reason',
        tags: ['tag1', 'tag2'],
      });

      const requestJson = (mockRuntime.clientCreateTask as ReturnType<typeof mock>).mock
        .calls[0][0] as string;
      const request = JSON.parse(requestJson);

      expect(request.namespace).toBe('custom');
      expect(request.version).toBe('2.0.0');
      expect(request.context).toEqual({ order_id: 123 });
      expect(request.initiator).toBe('my-app');
      expect(request.source_system).toBe('my-system');
      expect(request.reason).toBe('Custom reason');
      expect(request.tags).toEqual(['tag1', 'tag2']);
    });

    it('throws TaskerClientError on failure', () => {
      const { mockFfiLayer } = createMockFfiLayer({
        clientCreateTask: mock(() => errorResult('Task template not found')),
      });
      const client = new TaskerClient(mockFfiLayer);

      expect(() => client.createTask({ name: 'nonexistent' })).toThrow(TaskerClientError);

      try {
        client.createTask({ name: 'nonexistent' });
      } catch (e) {
        expect(e).toBeInstanceOf(TaskerClientError);
        expect((e as TaskerClientError).message).toBe('Task template not found');
        expect((e as TaskerClientError).recoverable).toBe(false);
      }
    });

    it('sets recoverable flag from error result', () => {
      const { mockFfiLayer } = createMockFfiLayer({
        clientCreateTask: mock(() => errorResult('Service unavailable', true)),
      });
      const client = new TaskerClient(mockFfiLayer);

      try {
        client.createTask({ name: 'test' });
      } catch (e) {
        expect((e as TaskerClientError).recoverable).toBe(true);
      }
    });
  });

  describe('getTask', () => {
    it('gets a task by UUID and returns typed response', () => {
      const { mockFfiLayer, mockRuntime } = createMockFfiLayer({
        clientGetTask: mock(() => successResult(MOCK_TASK_RESPONSE)),
      });
      const client = new TaskerClient(mockFfiLayer);

      const result = client.getTask('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee');

      expect(mockRuntime.clientGetTask).toHaveBeenCalledWith(
        'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'
      );
      expect(result.task_uuid).toBe('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee');
      expect(result.namespace).toBe('test');
    });

    it('throws TaskerClientError on failure', () => {
      const { mockFfiLayer } = createMockFfiLayer({
        clientGetTask: mock(() => errorResult('Task not found')),
      });
      const client = new TaskerClient(mockFfiLayer);

      expect(() => client.getTask('00000000-0000-0000-0000-000000000000')).toThrow(
        TaskerClientError
      );
    });
  });

  describe('listTasks', () => {
    it('lists tasks with default pagination', () => {
      const mockListResponse = {
        tasks: [MOCK_TASK_RESPONSE],
        pagination: { page: 1, per_page: 50, total_count: 1, total_pages: 1 },
      };
      const { mockFfiLayer, mockRuntime } = createMockFfiLayer({
        clientListTasks: mock(() => successResult(mockListResponse)),
      });
      const client = new TaskerClient(mockFfiLayer);

      const result = client.listTasks();

      const paramsJson = (mockRuntime.clientListTasks as ReturnType<typeof mock>).mock
        .calls[0][0] as string;
      const params = JSON.parse(paramsJson);

      expect(params.limit).toBe(50);
      expect(params.offset).toBe(0);
      expect(params.namespace).toBeNull();
      expect(params.status).toBeNull();

      expect(result.tasks).toHaveLength(1);
      expect(result.pagination.total_count).toBe(1);
    });

    it('passes filter arguments', () => {
      const { mockFfiLayer, mockRuntime } = createMockFfiLayer({
        clientListTasks: mock(() => successResult({ tasks: [], pagination: {} })),
      });
      const client = new TaskerClient(mockFfiLayer);

      client.listTasks({ limit: 10, offset: 5, namespace: 'test', status: 'pending' });

      const paramsJson = (mockRuntime.clientListTasks as ReturnType<typeof mock>).mock
        .calls[0][0] as string;
      const params = JSON.parse(paramsJson);

      expect(params.limit).toBe(10);
      expect(params.offset).toBe(5);
      expect(params.namespace).toBe('test');
      expect(params.status).toBe('pending');
    });
  });

  describe('cancelTask', () => {
    it('cancels a task without throwing', () => {
      const { mockFfiLayer, mockRuntime } = createMockFfiLayer({
        clientCancelTask: mock(() => successResult({ cancelled: true })),
      });
      const client = new TaskerClient(mockFfiLayer);

      client.cancelTask('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee');

      expect(mockRuntime.clientCancelTask).toHaveBeenCalledWith(
        'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'
      );
    });
  });

  describe('listTaskSteps', () => {
    it('lists steps and returns typed array', () => {
      const { mockFfiLayer } = createMockFfiLayer({
        clientListTaskSteps: mock(() => successResult([MOCK_STEP_RESPONSE])),
      });
      const client = new TaskerClient(mockFfiLayer);

      const result = client.listTaskSteps('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee');

      expect(result).toHaveLength(1);
      expect(result[0].step_uuid).toBe('11111111-2222-3333-4444-555555555555');
      expect(result[0].name).toBe('validate_input');
    });

    it('returns empty array when no steps', () => {
      const { mockFfiLayer } = createMockFfiLayer({
        clientListTaskSteps: mock(() => successResult([])),
      });
      const client = new TaskerClient(mockFfiLayer);

      const result = client.listTaskSteps('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee');

      expect(result).toHaveLength(0);
    });
  });

  describe('getStep', () => {
    it('gets a step and returns typed response', () => {
      const { mockFfiLayer, mockRuntime } = createMockFfiLayer({
        clientGetStep: mock(() => successResult(MOCK_STEP_RESPONSE)),
      });
      const client = new TaskerClient(mockFfiLayer);

      const result = client.getStep('task-uuid', 'step-uuid');

      expect(mockRuntime.clientGetStep).toHaveBeenCalledWith('task-uuid', 'step-uuid');
      expect(result.current_state).toBe('pending');
    });
  });

  describe('getStepAuditHistory', () => {
    it('gets audit history and returns typed array', () => {
      const mockAudit = {
        audit_uuid: 'audit-1',
        workflow_step_uuid: 'step-1',
        transition_uuid: 'trans-1',
        task_uuid: 'task-1',
        recorded_at: '2026-01-01T00:00:00Z',
        success: true,
        step_name: 'validate_input',
        to_state: 'complete',
      };
      const { mockFfiLayer, mockRuntime } = createMockFfiLayer({
        clientGetStepAuditHistory: mock(() => successResult([mockAudit])),
      });
      const client = new TaskerClient(mockFfiLayer);

      const result = client.getStepAuditHistory('task-uuid', 'step-uuid');

      expect(mockRuntime.clientGetStepAuditHistory).toHaveBeenCalledWith('task-uuid', 'step-uuid');
      expect(result).toHaveLength(1);
      expect(result[0].step_name).toBe('validate_input');
    });

    it('returns empty array when no audit entries', () => {
      const { mockFfiLayer } = createMockFfiLayer({
        clientGetStepAuditHistory: mock(() => successResult([])),
      });
      const client = new TaskerClient(mockFfiLayer);

      const result = client.getStepAuditHistory('task-uuid', 'step-uuid');

      expect(result).toHaveLength(0);
    });
  });

  describe('healthCheck', () => {
    it('checks health and returns typed response', () => {
      const { mockFfiLayer } = createMockFfiLayer({
        clientHealthCheck: mock(() =>
          successResult({ status: 'ok', timestamp: '2026-01-01T00:00:00Z' })
        ),
      });
      const client = new TaskerClient(mockFfiLayer);

      const result = client.healthCheck();

      expect(result.status).toBe('ok');
      expect(result.timestamp).toBe('2026-01-01T00:00:00Z');
    });
  });

  describe('error handling', () => {
    it('throws TaskerClientError with message from result.error', () => {
      const { mockFfiLayer } = createMockFfiLayer({
        clientGetTask: mock(() => errorResult('Not found')),
      });
      const client = new TaskerClient(mockFfiLayer);

      try {
        client.getTask('nonexistent');
        expect(true).toBe(false); // Should not reach here
      } catch (e) {
        expect(e).toBeInstanceOf(TaskerClientError);
        expect((e as TaskerClientError).message).toBe('Not found');
        expect((e as TaskerClientError).name).toBe('TaskerClientError');
      }
    });

    it('handles null error message gracefully', () => {
      const { mockFfiLayer } = createMockFfiLayer({
        clientGetTask: mock(() => ({
          success: false,
          data: null,
          error: null,
          recoverable: null,
        })),
      });
      const client = new TaskerClient(mockFfiLayer);

      try {
        client.getTask('nonexistent');
        expect(true).toBe(false);
      } catch (e) {
        expect(e).toBeInstanceOf(TaskerClientError);
        expect((e as TaskerClientError).message).toBe('Unknown client error');
      }
    });
  });
});
