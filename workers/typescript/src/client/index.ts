/**
 * High-level client wrapper for orchestration API operations.
 *
 * The raw FFI exposes `runtime.clientCreateTask(json)` and similar methods
 * that require callers to construct complete JSON request strings with all
 * required fields and return untyped `ClientResult` envelopes.
 *
 * This module provides a `TaskerClient` class with typed methods, sensible
 * defaults, and proper error handling.
 *
 * @example
 * ```typescript
 * import { FfiLayer, TaskerClient } from '@tasker-systems/tasker';
 *
 * const ffiLayer = new FfiLayer();
 * await ffiLayer.load();
 * const client = new TaskerClient(ffiLayer);
 *
 * const task = client.createTask({ name: 'process_order', namespace: 'ecommerce' });
 * console.log(task.task_uuid);
 * ```
 *
 * @packageDocumentation
 */

import type { FfiLayer } from '../ffi/ffi-layer.js';
import type {
  ClientHealthResponse,
  ClientResult,
  ClientStepAuditResponse,
  ClientStepResponse,
  ClientTaskListResponse,
  ClientTaskResponse,
} from '../ffi/types.js';

/**
 * Options for creating a task.
 *
 * Only `name` is required; all other fields have sensible defaults.
 */
export interface CreateTaskOptions {
  /** Named task template name */
  name: string;
  /** Task namespace (default: 'default') */
  namespace?: string;
  /** Workflow context passed to step handlers (default: {}) */
  context?: Record<string, unknown>;
  /** Template version (default: '1.0.0') */
  version?: string;
  /** Who initiated the request (default: 'tasker-core-typescript') */
  initiator?: string;
  /** Originating system (default: 'tasker-core') */
  sourceSystem?: string;
  /** Reason for creating the task (default: 'Task requested') */
  reason?: string;
  /** Optional tags */
  tags?: string[];
  /** Optional priority */
  priority?: number | null;
  /** Optional correlation ID (auto-generated if not provided) */
  correlationId?: string;
  /** Optional parent correlation ID */
  parentCorrelationId?: string | null;
  /** Optional idempotency key */
  idempotencyKey?: string | null;
}

/**
 * Options for listing tasks.
 */
export interface ListTasksOptions {
  /** Maximum number of results (default: 50) */
  limit?: number;
  /** Pagination offset (default: 0) */
  offset?: number;
  /** Filter by namespace */
  namespace?: string;
  /** Filter by status */
  status?: string;
}

/**
 * Error thrown when a client operation fails.
 */
export class TaskerClientError extends Error {
  /** Whether the error is potentially recoverable */
  readonly recoverable: boolean;

  constructor(message: string, recoverable = false) {
    super(message);
    this.name = 'TaskerClientError';
    this.recoverable = recoverable;
  }
}

/**
 * High-level client for orchestration API operations.
 *
 * Wraps the raw FFI methods with typed interfaces, sensible defaults,
 * and proper error handling via `TaskerClientError`.
 */
export class TaskerClient {
  private readonly ffiLayer: FfiLayer;

  constructor(ffiLayer: FfiLayer) {
    this.ffiLayer = ffiLayer;
  }

  /**
   * Create a task via the orchestration API.
   *
   * @param options - Task creation options (only `name` is required)
   * @returns Typed task response
   * @throws TaskerClientError if the operation fails
   */
  createTask(options: CreateTaskOptions): ClientTaskResponse {
    const request = {
      name: options.name,
      namespace: options.namespace ?? 'default',
      version: options.version ?? '1.0.0',
      context: options.context ?? {},
      initiator: options.initiator ?? 'tasker-core-typescript',
      source_system: options.sourceSystem ?? 'tasker-core',
      reason: options.reason ?? 'Task requested',
      tags: options.tags ?? [],
      requested_at: new Date().toISOString(),
      options: null,
      priority: options.priority ?? null,
      correlation_id: options.correlationId ?? crypto.randomUUID(),
      parent_correlation_id: options.parentCorrelationId ?? null,
      idempotency_key: options.idempotencyKey ?? null,
    };

    const result = this.getRuntime().clientCreateTask(JSON.stringify(request));
    return this.unwrap<ClientTaskResponse>(result);
  }

  /**
   * Get a task by UUID.
   *
   * @param taskUuid - The task UUID
   * @returns Typed task response
   * @throws TaskerClientError if the operation fails
   */
  getTask(taskUuid: string): ClientTaskResponse {
    const result = this.getRuntime().clientGetTask(taskUuid);
    return this.unwrap<ClientTaskResponse>(result);
  }

  /**
   * List tasks with optional filtering and pagination.
   *
   * @param options - Filtering and pagination options
   * @returns Typed task list response with pagination
   * @throws TaskerClientError if the operation fails
   */
  listTasks(options: ListTasksOptions = {}): ClientTaskListResponse {
    const params = {
      limit: options.limit ?? 50,
      offset: options.offset ?? 0,
      namespace: options.namespace ?? null,
      status: options.status ?? null,
    };

    const result = this.getRuntime().clientListTasks(JSON.stringify(params));
    return this.unwrap<ClientTaskListResponse>(result);
  }

  /**
   * Cancel a task by UUID.
   *
   * @param taskUuid - The task UUID
   * @throws TaskerClientError if the operation fails
   */
  cancelTask(taskUuid: string): void {
    const result = this.getRuntime().clientCancelTask(taskUuid);
    this.unwrap(result);
  }

  /**
   * List workflow steps for a task.
   *
   * @param taskUuid - The task UUID
   * @returns Array of typed step responses
   * @throws TaskerClientError if the operation fails
   */
  listTaskSteps(taskUuid: string): ClientStepResponse[] {
    const result = this.getRuntime().clientListTaskSteps(taskUuid);
    return this.unwrap<ClientStepResponse[]>(result);
  }

  /**
   * Get a specific workflow step.
   *
   * @param taskUuid - The task UUID
   * @param stepUuid - The step UUID
   * @returns Typed step response
   * @throws TaskerClientError if the operation fails
   */
  getStep(taskUuid: string, stepUuid: string): ClientStepResponse {
    const result = this.getRuntime().clientGetStep(taskUuid, stepUuid);
    return this.unwrap<ClientStepResponse>(result);
  }

  /**
   * Get audit history for a workflow step.
   *
   * @param taskUuid - The task UUID
   * @param stepUuid - The step UUID
   * @returns Array of typed audit history entries
   * @throws TaskerClientError if the operation fails
   */
  getStepAuditHistory(taskUuid: string, stepUuid: string): ClientStepAuditResponse[] {
    const result = this.getRuntime().clientGetStepAuditHistory(taskUuid, stepUuid);
    return this.unwrap<ClientStepAuditResponse[]>(result);
  }

  /**
   * Check orchestration API health.
   *
   * @returns Typed health response
   * @throws TaskerClientError if the operation fails
   */
  healthCheck(): ClientHealthResponse {
    const result = this.getRuntime().clientHealthCheck();
    return this.unwrap<ClientHealthResponse>(result);
  }

  /**
   * Unwrap a ClientResult envelope, throwing on error.
   */
  private unwrap<T>(result: ClientResult): T {
    if (!result.success) {
      throw new TaskerClientError(
        result.error ?? 'Unknown client error',
        result.recoverable ?? false
      );
    }
    return result.data as T;
  }

  /**
   * Get the FFI runtime from the layer.
   */
  private getRuntime() {
    return this.ffiLayer.getRuntime();
  }
}
