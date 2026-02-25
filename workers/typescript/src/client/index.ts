/**
 * High-level client wrapper for orchestration API operations.
 *
 * TAS-290: With napi-rs, requests are passed as typed objects directly —
 * no JSON.stringify() at the boundary. This eliminates TAS-283 trailing
 * input bugs.
 *
 * @example
 * ```typescript
 * import { getTaskerClient } from '@tasker-systems/tasker';
 *
 * const client = await getTaskerClient();
 * const task = client.createTask({ name: 'process_order', namespace: 'ecommerce' });
 * console.log(task.task_uuid);
 * ```
 *
 * @packageDocumentation
 */

import { FfiLayer } from '../ffi/ffi-layer.js';
import type { NapiClientResult, NapiListTasksParams, NapiTaskRequest } from '../ffi/types.js';
import type {
  HealthCheckResponse,
  StepAuditResponse,
  StepResponse,
  TaskListResponse,
  TaskResponse,
} from './types.js';

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
  priority?: number;
  /** Optional correlation ID (auto-generated if not provided) */
  correlationId?: string;
  /** Optional parent correlation ID */
  parentCorrelationId?: string;
  /** Optional idempotency key */
  idempotencyKey?: string;
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
   * @returns Task response with execution context and step readiness
   * @throws TaskerClientError if the operation fails
   */
  createTask(options: CreateTaskOptions): TaskResponse {
    const request: NapiTaskRequest = {
      name: options.name,
      namespace: options.namespace ?? 'default',
      version: options.version ?? '1.0.0',
      context: options.context ?? {},
      initiator: options.initiator ?? 'tasker-core-typescript',
      sourceSystem: options.sourceSystem ?? 'tasker-core',
      reason: options.reason ?? 'Task requested',
      tags: options.tags ?? [],
    };
    if (options.priority !== undefined) request.priority = options.priority;
    if (options.correlationId !== undefined) request.correlationId = options.correlationId;
    else request.correlationId = crypto.randomUUID();
    if (options.parentCorrelationId !== undefined)
      request.parentCorrelationId = options.parentCorrelationId;
    if (options.idempotencyKey !== undefined) request.idempotencyKey = options.idempotencyKey;

    const result = this.getModule().clientCreateTask(request);
    return this.unwrap(result);
  }

  /**
   * Get a task by UUID.
   *
   * @param taskUuid - The task UUID
   * @returns Task response with execution context and step readiness
   * @throws TaskerClientError if the operation fails
   */
  getTask(taskUuid: string): TaskResponse {
    const result = this.getModule().clientGetTask(taskUuid);
    return this.unwrap(result);
  }

  /**
   * List tasks with optional filtering and pagination.
   *
   * @param options - Filtering and pagination options
   * @returns Task list with pagination metadata
   * @throws TaskerClientError if the operation fails
   */
  listTasks(options: ListTasksOptions = {}): TaskListResponse {
    const params: NapiListTasksParams = {
      limit: options.limit ?? 50,
      offset: options.offset ?? 0,
    };
    if (options.namespace !== undefined) params.namespace = options.namespace;
    if (options.status !== undefined) params.status = options.status;

    const result = this.getModule().clientListTasks(params);
    return this.unwrap(result);
  }

  /**
   * Cancel a task by UUID.
   *
   * @param taskUuid - The task UUID
   * @throws TaskerClientError if the operation fails
   */
  cancelTask(taskUuid: string): void {
    const result = this.getModule().clientCancelTask(taskUuid);
    this.unwrap(result);
  }

  /**
   * List workflow steps for a task.
   *
   * @param taskUuid - The task UUID
   * @returns Array of step responses with readiness information
   * @throws TaskerClientError if the operation fails
   */
  listTaskSteps(taskUuid: string): StepResponse[] {
    const result = this.getModule().clientListTaskSteps(taskUuid);
    return this.unwrap(result);
  }

  /**
   * Get a specific workflow step.
   *
   * @param taskUuid - The task UUID
   * @param stepUuid - The step UUID
   * @returns Step response with readiness information
   * @throws TaskerClientError if the operation fails
   */
  getStep(taskUuid: string, stepUuid: string): StepResponse {
    const result = this.getModule().clientGetStep(taskUuid, stepUuid);
    return this.unwrap(result);
  }

  /**
   * Get audit history for a workflow step.
   *
   * @param taskUuid - The task UUID
   * @param stepUuid - The step UUID
   * @returns Array of audit history entries with attribution context
   * @throws TaskerClientError if the operation fails
   */
  getStepAuditHistory(taskUuid: string, stepUuid: string): StepAuditResponse[] {
    const result = this.getModule().clientGetStepAuditHistory(taskUuid, stepUuid);
    return this.unwrap(result);
  }

  /**
   * Check orchestration API health.
   *
   * @returns Health status from the orchestration API
   * @throws TaskerClientError if the operation fails
   */
  healthCheck(): HealthCheckResponse {
    const result = this.getModule().clientHealthCheck();
    return this.unwrap(result);
  }

  /**
   * Unwrap a NapiClientResult envelope, throwing on error.
   *
   * The type parameter allows callers to assert the expected response shape.
   * The actual data comes from Rust via serde_json serialization, so the
   * runtime shape is guaranteed by the Rust type system — the cast here
   * is safe at the FFI boundary.
   */
  private unwrap<T>(result: NapiClientResult): T {
    if (!result.success) {
      throw new TaskerClientError(
        result.error ?? 'Unknown client error',
        result.recoverable ?? false
      );
    }
    return result.data as T;
  }

  /**
   * Get the napi-rs module from the layer.
   */
  private getModule() {
    return this.ffiLayer.getModule();
  }
}

// =============================================================================
// Singleton convenience functions
//
// Most consumers need exactly one FfiLayer and one TaskerClient for the
// lifetime of their process. These functions provide a lazy-initialized
// singleton so consumers don't have to manage FfiLayer lifecycle themselves.
//
// For advanced use cases (multiple workers, custom config), construct
// FfiLayer and TaskerClient directly.
// =============================================================================

let sharedFfiLayer: FfiLayer | null = null;
let loadPromise: Promise<FfiLayer> | null = null;

/**
 * Returns a loaded FfiLayer singleton, initializing it on first call.
 *
 * The native library is loaded once and reused for all subsequent calls.
 * Safe to call concurrently — concurrent callers await the same load promise.
 *
 * @example
 * ```typescript
 * import { getFfiLayer } from '@tasker-systems/tasker';
 *
 * const ffiLayer = await getFfiLayer();
 * // Use for custom TaskerClient construction or direct FFI access
 * ```
 */
export async function getFfiLayer(): Promise<FfiLayer> {
  if (sharedFfiLayer) {
    return sharedFfiLayer;
  }

  if (!loadPromise) {
    loadPromise = (async () => {
      const layer = new FfiLayer();
      await layer.load();
      sharedFfiLayer = layer;
      return layer;
    })();
  }

  return loadPromise;
}

/**
 * Returns a TaskerClient backed by the shared FfiLayer singleton.
 *
 * This is the recommended entry point for most consumers. The FfiLayer
 * is loaded lazily on first call and reused thereafter.
 *
 * @example
 * ```typescript
 * import { getTaskerClient } from '@tasker-systems/tasker';
 *
 * const client = await getTaskerClient();
 * const task = client.createTask({ name: 'process_order', namespace: 'ecommerce' });
 * console.log(task.task_uuid);
 * ```
 */
export async function getTaskerClient(): Promise<TaskerClient> {
  const ffiLayer = await getFfiLayer();
  return new TaskerClient(ffiLayer);
}
