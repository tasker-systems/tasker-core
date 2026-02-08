/**
 * Runtime interface for FFI operations.
 *
 * This interface defines the contract that all runtime adapters must implement.
 * It provides strongly-typed access to the Rust FFI functions.
 */

import type {
  BootstrapConfig,
  BootstrapResult,
  CheckpointYieldData,
  ClientResult,
  FfiDispatchMetrics,
  FfiDomainEvent,
  FfiStepEvent,
  LogFields,
  StepExecutionResult,
  StopResult,
  WorkerStatus,
} from './types.js';

/**
 * Interface for runtime-specific FFI implementations.
 *
 * Each runtime (Node.js, Bun, Deno) implements this interface
 * using their native FFI mechanism.
 */
export interface TaskerRuntime {
  /**
   * Get the runtime name
   */
  readonly name: string;

  /**
   * Check if the FFI library is loaded
   */
  readonly isLoaded: boolean;

  /**
   * Load the native library from the given path
   */
  load(libraryPath: string): Promise<void>;

  /**
   * Unload the native library and release resources
   */
  unload(): void;

  // ============================================================================
  // Version and Health
  // ============================================================================

  /**
   * Get the version of the tasker-ts package
   */
  getVersion(): string;

  /**
   * Get detailed Rust library version
   */
  getRustVersion(): string;

  /**
   * Check if the FFI module is functional
   */
  healthCheck(): boolean;

  // ============================================================================
  // Worker Lifecycle
  // ============================================================================

  /**
   * Bootstrap the worker with optional configuration
   */
  bootstrapWorker(config?: BootstrapConfig): BootstrapResult;

  /**
   * Check if the worker is currently running
   */
  isWorkerRunning(): boolean;

  /**
   * Get current worker status
   */
  getWorkerStatus(): WorkerStatus;

  /**
   * Stop the worker gracefully
   */
  stopWorker(): StopResult;

  /**
   * Transition to graceful shutdown mode
   */
  transitionToGracefulShutdown(): StopResult;

  // ============================================================================
  // Event Polling
  // ============================================================================

  /**
   * Poll for pending step events (non-blocking)
   *
   * @returns Step event if available, null otherwise
   */
  pollStepEvents(): FfiStepEvent | null;

  /**
   * Poll for in-process domain events (fast path, non-blocking)
   *
   * Used for real-time notifications that don't require guaranteed delivery
   * (e.g., metrics updates, logging, notifications).
   *
   * @returns Domain event if available, null otherwise
   */
  pollInProcessEvents(): FfiDomainEvent | null;

  /**
   * Complete a step event with the given result
   *
   * @param eventId The event ID to complete
   * @param result The step execution result
   * @returns true if successful, false otherwise
   */
  completeStepEvent(eventId: string, result: StepExecutionResult): boolean;

  /**
   * TAS-125: Submit a checkpoint yield for batch processing
   *
   * Called from batch processing handlers when they want to persist progress
   * and be re-dispatched for continuation. Unlike completeStepEvent, this
   * does NOT complete the step - instead it persists checkpoint data and
   * re-dispatches the step for continued processing.
   *
   * @param eventId The event ID from the step event
   * @param checkpointData The checkpoint data to persist
   * @returns true if checkpoint persisted and step re-dispatched, false otherwise
   */
  checkpointYieldStepEvent(eventId: string, checkpointData: CheckpointYieldData): boolean;

  // ============================================================================
  // Metrics and Monitoring
  // ============================================================================

  /**
   * Get FFI dispatch metrics
   */
  getFfiDispatchMetrics(): FfiDispatchMetrics;

  /**
   * Check for and log starvation warnings
   */
  checkStarvationWarnings(): void;

  /**
   * Cleanup timed-out events
   */
  cleanupTimeouts(): void;

  // ============================================================================
  // Client API Operations (TAS-231)
  // ============================================================================

  /**
   * Create a task via the orchestration API client
   *
   * @param requestJson JSON string of ClientTaskRequest
   * @returns ClientResult containing ClientTaskResponse on success
   */
  clientCreateTask(requestJson: string): ClientResult;

  /**
   * Get a task by UUID
   *
   * @param taskUuid The task UUID
   * @returns ClientResult containing ClientTaskResponse on success
   */
  clientGetTask(taskUuid: string): ClientResult;

  /**
   * List tasks with optional filters
   *
   * @param paramsJson JSON string with limit, offset, namespace, status
   * @returns ClientResult containing ClientTaskListResponse on success
   */
  clientListTasks(paramsJson: string): ClientResult;

  /**
   * Cancel a task
   *
   * @param taskUuid The task UUID to cancel
   * @returns ClientResult with cancellation status
   */
  clientCancelTask(taskUuid: string): ClientResult;

  /**
   * List workflow steps for a task
   *
   * @param taskUuid The task UUID
   * @returns ClientResult containing step list on success
   */
  clientListTaskSteps(taskUuid: string): ClientResult;

  /**
   * Get a specific workflow step
   *
   * @param taskUuid The task UUID
   * @param stepUuid The step UUID
   * @returns ClientResult containing ClientStepResponse on success
   */
  clientGetStep(taskUuid: string, stepUuid: string): ClientResult;

  /**
   * Get audit history for a workflow step (SOC2 compliance)
   *
   * @param taskUuid The task UUID
   * @param stepUuid The step UUID
   * @returns ClientResult containing audit history on success
   */
  clientGetStepAuditHistory(taskUuid: string, stepUuid: string): ClientResult;

  /**
   * Health check against the orchestration API
   *
   * @returns ClientResult containing ClientHealthResponse on success
   */
  clientHealthCheck(): ClientResult;

  // ============================================================================
  // Logging
  // ============================================================================

  /**
   * Log an error message
   */
  logError(message: string, fields?: LogFields): void;

  /**
   * Log a warning message
   */
  logWarn(message: string, fields?: LogFields): void;

  /**
   * Log an info message
   */
  logInfo(message: string, fields?: LogFields): void;

  /**
   * Log a debug message
   */
  logDebug(message: string, fields?: LogFields): void;

  /**
   * Log a trace message
   */
  logTrace(message: string, fields?: LogFields): void;
}

/**
 * Base class with common functionality for all runtime implementations.
 *
 * Runtime-specific implementations extend this class and implement
 * the abstract methods using their native FFI mechanism.
 */
export abstract class BaseTaskerRuntime implements TaskerRuntime {
  abstract readonly name: string;
  abstract readonly isLoaded: boolean;

  abstract load(libraryPath: string): Promise<void>;
  abstract unload(): void;

  abstract getVersion(): string;
  abstract getRustVersion(): string;
  abstract healthCheck(): boolean;

  abstract bootstrapWorker(config?: BootstrapConfig): BootstrapResult;
  abstract isWorkerRunning(): boolean;
  abstract getWorkerStatus(): WorkerStatus;
  abstract stopWorker(): StopResult;
  abstract transitionToGracefulShutdown(): StopResult;

  abstract pollStepEvents(): FfiStepEvent | null;
  abstract pollInProcessEvents(): FfiDomainEvent | null;
  abstract completeStepEvent(eventId: string, result: StepExecutionResult): boolean;
  abstract checkpointYieldStepEvent(eventId: string, checkpointData: CheckpointYieldData): boolean;

  abstract getFfiDispatchMetrics(): FfiDispatchMetrics;
  abstract checkStarvationWarnings(): void;
  abstract cleanupTimeouts(): void;

  abstract clientCreateTask(requestJson: string): ClientResult;
  abstract clientGetTask(taskUuid: string): ClientResult;
  abstract clientListTasks(paramsJson: string): ClientResult;
  abstract clientCancelTask(taskUuid: string): ClientResult;
  abstract clientListTaskSteps(taskUuid: string): ClientResult;
  abstract clientGetStep(taskUuid: string, stepUuid: string): ClientResult;
  abstract clientGetStepAuditHistory(taskUuid: string, stepUuid: string): ClientResult;
  abstract clientHealthCheck(): ClientResult;

  abstract logError(message: string, fields?: LogFields): void;
  abstract logWarn(message: string, fields?: LogFields): void;
  abstract logInfo(message: string, fields?: LogFields): void;
  abstract logDebug(message: string, fields?: LogFields): void;
  abstract logTrace(message: string, fields?: LogFields): void;

  /**
   * Helper to parse JSON string from FFI
   */
  protected parseJson<T>(jsonStr: string | null): T | null {
    if (jsonStr === null || jsonStr === '') {
      return null;
    }
    try {
      return JSON.parse(jsonStr) as T;
    } catch {
      return null;
    }
  }

  /**
   * Helper to stringify JSON for FFI
   */
  protected toJson(value: unknown): string {
    return JSON.stringify(value);
  }
}
