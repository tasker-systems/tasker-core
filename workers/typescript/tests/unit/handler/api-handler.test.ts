/**
 * Tests for defineApiHandler functional factory.
 *
 * Tests:
 * 1. Handler class composition (applyAPI + StepHandler)
 * 2. HTTP methods via mocked fetch (get, post, delete)
 * 3. apiSuccess / apiFailure result helpers
 * 4. Error classification (retryable vs permanent by status code)
 * 5. Configuration passthrough (baseUrl, timeout, headers)
 * 6. api parameter identity (api === this via applyAPI)
 */

import { afterEach, describe, expect, it, mock } from 'bun:test';
import type { FfiStepEvent } from '../../../src/ffi/types.js';
import { StepHandler } from '../../../src/handler/base.js';
import { defineApiHandler } from '../../../src/handler/functional.js';
import type { APICapable } from '../../../src/handler/mixins/api.js';
import { StepContext } from '../../../src/types/step-context.js';

/** Static properties set by defineApiHandler on the generated class. */
interface ApiHandlerStatics {
  baseUrl: string;
  defaultTimeout: number;
  defaultHeaders: Record<string, string>;
}

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

function makeContext(overrides: { inputData?: Record<string, unknown> } = {}): StepContext {
  const event = createFfiEvent();
  return new StepContext({
    event,
    taskUuid: 'task-123',
    stepUuid: 'step-456',
    correlationId: 'corr-789',
    handlerName: 'test_api',
    inputData: overrides.inputData ?? {},
    dependencyResults: {},
    stepConfig: {},
    stepInputs: {},
    retryCount: 0,
    maxRetries: 3,
  });
}

function mockFetchResponse(
  status: number,
  body: unknown = {},
  headers: Record<string, string> = {}
): Response {
  const responseHeaders = new Headers(headers);
  if (!responseHeaders.has('content-type')) {
    responseHeaders.set('content-type', 'application/json');
  }
  return new Response(JSON.stringify(body), {
    status,
    headers: responseHeaders,
  });
}

afterEach(() => {
  mock.restore();
});

// ============================================================================
// Tests: Handler Composition
// ============================================================================

describe('defineApiHandler', () => {
  describe('handler composition', () => {
    it('produces a StepHandler subclass', () => {
      const Handler = defineApiHandler(
        'fetch_data',
        { baseUrl: 'https://api.example.com' },
        async () => ({})
      );

      const handler = new Handler();
      expect(handler).toBeInstanceOf(StepHandler);
    });

    it('sets handler name and version', () => {
      const Handler = defineApiHandler(
        'fetch_data',
        { baseUrl: 'https://api.example.com', version: '2.0.0' },
        async () => ({})
      );

      const handler = new Handler();
      expect(handler.name).toBe('fetch_data');
      expect(handler.version).toBe('2.0.0');
    });

    it('sets static baseUrl', () => {
      const Handler = defineApiHandler(
        'fetch_data',
        { baseUrl: 'https://api.example.com' },
        async () => ({})
      );

      expect((Handler as unknown as ApiHandlerStatics).baseUrl).toBe('https://api.example.com');
    });

    it('sets static defaultTimeout', () => {
      const Handler = defineApiHandler(
        'fetch_data',
        { baseUrl: 'https://api.example.com', defaultTimeout: 60000 },
        async () => ({})
      );

      expect((Handler as unknown as ApiHandlerStatics).defaultTimeout).toBe(60000);
    });

    it('sets static defaultHeaders', () => {
      const Handler = defineApiHandler(
        'fetch_data',
        {
          baseUrl: 'https://api.example.com',
          defaultHeaders: { Authorization: 'Bearer token123' },
        },
        async () => ({})
      );

      expect((Handler as unknown as ApiHandlerStatics).defaultHeaders).toEqual({
        Authorization: 'Bearer token123',
      });
    });
  });

  // ============================================================================
  // Tests: HTTP Methods
  // ============================================================================

  describe('HTTP methods via api parameter', () => {
    it('GET returns success via api.get', async () => {
      const fetchMock = mock(() =>
        Promise.resolve(mockFetchResponse(200, { id: 1, name: 'Alice' }))
      );
      globalThis.fetch = fetchMock;

      const Handler = defineApiHandler(
        'fetch_user',
        { baseUrl: 'https://api.example.com' },
        async ({ api }) => {
          const response = await api.get('/users/1');
          if (response.ok) {
            return api.apiSuccess(response);
          }
          return api.apiFailure(response);
        }
      );

      const handler = new Handler();
      const result = await handler.call(makeContext());

      expect(result.success).toBe(true);
      expect(result.result?.id).toBe(1);
      expect(result.result?.name).toBe('Alice');
      expect(fetchMock).toHaveBeenCalledTimes(1);
    });

    it('POST returns success via api.post', async () => {
      const fetchMock = mock(() =>
        Promise.resolve(mockFetchResponse(201, { id: 42, created: true }))
      );
      globalThis.fetch = fetchMock;

      const Handler = defineApiHandler(
        'create_user',
        { baseUrl: 'https://api.example.com' },
        async ({ api }) => {
          const response = await api.post('/users', {
            body: { name: 'Bob' },
          });
          if (response.ok) {
            return api.apiSuccess(response);
          }
          return api.apiFailure(response);
        }
      );

      const handler = new Handler();
      const result = await handler.call(makeContext());

      expect(result.success).toBe(true);
      expect(result.result?.id).toBe(42);
    });

    it('DELETE returns success', async () => {
      const fetchMock = mock(() => Promise.resolve(mockFetchResponse(204, '')));
      globalThis.fetch = fetchMock;

      const Handler = defineApiHandler(
        'remove_user',
        { baseUrl: 'https://api.example.com' },
        async ({ api }) => {
          const response = await api.delete('/users/1');
          if (response.ok) {
            return { deleted: true };
          }
          return api.apiFailure(response);
        }
      );

      const handler = new Handler();
      const result = await handler.call(makeContext());

      expect(result.success).toBe(true);
      expect(result.result?.deleted).toBe(true);
    });
  });

  // ============================================================================
  // Tests: Error Classification
  // ============================================================================

  describe('error classification via apiFailure', () => {
    it('404 is not retryable', async () => {
      globalThis.fetch = mock(() =>
        Promise.resolve(mockFetchResponse(404, { error: 'not found' }))
      );

      const Handler = defineApiHandler(
        'fetch_missing',
        { baseUrl: 'https://api.example.com' },
        async ({ api }) => {
          const response = await api.get('/users/999');
          return api.apiFailure(response);
        }
      );

      const handler = new Handler();
      const result = await handler.call(makeContext());

      expect(result.success).toBe(false);
      expect(result.retryable).toBe(false);
    });

    it('503 is retryable', async () => {
      globalThis.fetch = mock(() =>
        Promise.resolve(mockFetchResponse(503, { error: 'service down' }))
      );

      const Handler = defineApiHandler(
        'fetch_unavailable',
        { baseUrl: 'https://api.example.com' },
        async ({ api }) => {
          const response = await api.get('/health');
          return api.apiFailure(response);
        }
      );

      const handler = new Handler();
      const result = await handler.call(makeContext());

      expect(result.success).toBe(false);
      expect(result.retryable).toBe(true);
    });

    it('429 is retryable', async () => {
      globalThis.fetch = mock(() =>
        Promise.resolve(mockFetchResponse(429, { error: 'rate limited' }, { 'retry-after': '30' }))
      );

      const Handler = defineApiHandler(
        'fetch_throttled',
        { baseUrl: 'https://api.example.com' },
        async ({ api }) => {
          const response = await api.get('/data');
          return api.apiFailure(response);
        }
      );

      const handler = new Handler();
      const result = await handler.call(makeContext());

      expect(result.success).toBe(false);
      expect(result.retryable).toBe(true);
    });
  });

  // ============================================================================
  // Tests: api parameter identity
  // ============================================================================

  describe('api parameter identity', () => {
    it('api has HTTP methods from applyAPI', async () => {
      const Handler = defineApiHandler(
        'check_methods',
        { baseUrl: 'https://api.example.com' },
        async () => ({})
      );

      const handler = new Handler() as unknown as APICapable;
      // applyAPI binds these directly onto the instance
      expect(typeof handler.get).toBe('function');
      expect(typeof handler.post).toBe('function');
      expect(typeof handler.put).toBe('function');
      expect(typeof handler.patch).toBe('function');
      expect(typeof handler.delete).toBe('function');
    });

    it('api has result helpers from applyAPI', async () => {
      const Handler = defineApiHandler(
        'check_helpers',
        { baseUrl: 'https://api.example.com' },
        async () => ({})
      );

      const handler = new Handler() as unknown as APICapable;
      expect(typeof handler.apiSuccess).toBe('function');
      expect(typeof handler.apiFailure).toBe('function');
      expect(typeof handler.connectionError).toBe('function');
      expect(typeof handler.timeoutError).toBe('function');
    });

    it('api parameter is the handler instance', async () => {
      let capturedApi: unknown = null;

      const Handler = defineApiHandler(
        'check_self',
        { baseUrl: 'https://api.example.com' },
        async ({ api }) => {
          capturedApi = api;
          return {};
        }
      );

      const handler = new Handler();
      await handler.call(makeContext());

      expect(capturedApi).toBe(handler);
    });
  });
});
