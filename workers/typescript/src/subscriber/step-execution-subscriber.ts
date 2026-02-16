/**
 * Step execution subscriber for TypeScript workers.
 *
 * Subscribes to step execution events from the EventPoller and dispatches
 * them to the appropriate handlers via the HandlerRegistry.
 *
 * TAS-290: Uses NapiModule directly instead of TaskerRuntime.
 * All field access uses camelCase (napi-rs auto-converts from Rust snake_case).
 *
 * Matches Python's StepExecutionSubscriber pattern (TAS-92 aligned).
 */

import pino, { type Logger, type LoggerOptions } from 'pino';
import type { StepExecutionReceivedPayload, TaskerEventEmitter } from '../events/event-emitter.js';
import { StepEventNames } from '../events/event-names.js';
import type { NapiModule } from '../ffi/ffi-layer.js';
import type { FfiStepEvent, NapiCheckpointYieldData, NapiStepResult } from '../ffi/types.js';
import type { ExecutableHandler } from '../handler/base.js';
import { logDebug, logError, logInfo, logWarn } from '../logging/index.js';
import { StepContext } from '../types/step-context.js';
import type { StepHandlerResult } from '../types/step-handler-result.js';

// Create a pino logger for the subscriber (for debugging)
const loggerOptions: LoggerOptions = {
  name: 'step-subscriber',
  level: process.env.RUST_LOG ?? 'info',
};

// Add pino-pretty transport in non-production environments
if (process.env.TASKER_ENV !== 'production') {
  loggerOptions.transport = {
    target: 'pino-pretty',
    options: { colorize: true },
  };
}

const pinoLog: Logger = pino(loggerOptions);

/**
 * Interface for handler registry required by StepExecutionSubscriber.
 *
 * TAS-93: Updated to support async resolution via ResolverChain.
 * Returns ExecutableHandler which includes both StepHandler and MethodDispatchWrapper.
 */
export interface HandlerRegistryInterface {
  /** Resolve and instantiate a handler by name (async for resolver chain support) */
  resolve(name: string): Promise<ExecutableHandler | null>;
}

/**
 * Configuration for the step execution subscriber.
 */
export interface StepExecutionSubscriberConfig {
  /** Worker ID for result attribution */
  workerId?: string;

  /** Maximum concurrent handler executions (default: 10) */
  maxConcurrent?: number;

  /** Handler execution timeout in milliseconds (default: 300000 = 5 minutes) */
  handlerTimeoutMs?: number;
}

/**
 * Subscribes to step execution events and dispatches them to handlers.
 *
 * This is the critical component that connects the FFI event stream
 * to TypeScript handler execution. It:
 * 1. Listens for step events from the EventPoller via EventEmitter
 * 2. Resolves the appropriate handler from the HandlerRegistry
 * 3. Creates a StepContext from the FFI event
 * 4. Executes the handler
 * 5. Submits the result back to Rust via FFI
 *
 * @example
 * ```typescript
 * const subscriber = new StepExecutionSubscriber(
 *   eventEmitter,
 *   handlerRegistry,
 *   module,
 *   { workerId: 'worker-1' }
 * );
 *
 * subscriber.start();
 *
 * // Later...
 * subscriber.stop();
 * ```
 */
export class StepExecutionSubscriber {
  private readonly emitter: TaskerEventEmitter;
  private readonly registry: HandlerRegistryInterface;
  private readonly module: NapiModule;
  private readonly workerId: string;
  private readonly maxConcurrent: number;
  private readonly handlerTimeoutMs: number;

  private running = false;
  private activeHandlers = 0;
  private processedCount = 0;
  private errorCount = 0;

  /**
   * Create a new StepExecutionSubscriber.
   *
   * @param emitter - The event emitter to subscribe to (required, no fallback)
   * @param registry - The handler registry for resolving step handlers
   * @param module - The napi-rs module for submitting results (required, no fallback)
   * @param config - Optional configuration for execution behavior
   */
  constructor(
    emitter: TaskerEventEmitter,
    registry: HandlerRegistryInterface,
    module: NapiModule,
    config: StepExecutionSubscriberConfig = {}
  ) {
    this.emitter = emitter;
    this.registry = registry;
    this.module = module;
    this.workerId = config.workerId ?? `typescript-worker-${process.pid}`;
    this.maxConcurrent = config.maxConcurrent ?? 10;
    this.handlerTimeoutMs = config.handlerTimeoutMs ?? 300000;
  }

  /**
   * Start subscribing to step execution events.
   */
  start(): void {
    pinoLog.info(
      { component: 'subscriber', emitterInstanceId: this.emitter.getInstanceId() },
      'StepExecutionSubscriber.start() called'
    );

    if (this.running) {
      logWarn('StepExecutionSubscriber already running', {
        component: 'subscriber',
      });
      return;
    }

    this.running = true;
    this.processedCount = 0;
    this.errorCount = 0;

    // Subscribe to step events
    pinoLog.info(
      {
        component: 'subscriber',
        eventName: StepEventNames.STEP_EXECUTION_RECEIVED,
        emitterInstanceId: this.emitter.getInstanceId(),
      },
      'Registering event listener on emitter'
    );

    this.emitter.on(
      StepEventNames.STEP_EXECUTION_RECEIVED,
      (payload: StepExecutionReceivedPayload) => {
        try {
          pinoLog.info(
            {
              component: 'subscriber',
              eventId: payload.event.eventId,
              stepUuid: payload.event.stepUuid,
            },
            'Received step event in subscriber callback!'
          );
          // Extract the event from the payload wrapper
          pinoLog.info({ component: 'subscriber' }, 'About to call handleEvent from callback');
          this.handleEvent(payload.event);
          pinoLog.info({ component: 'subscriber' }, 'handleEvent returned from callback');
        } catch (error) {
          pinoLog.error(
            {
              component: 'subscriber',
              error: error instanceof Error ? error.message : String(error),
              stack: error instanceof Error ? error.stack : undefined,
            },
            'EXCEPTION in event listener callback!'
          );
        }
      }
    );

    pinoLog.info(
      { component: 'subscriber', workerId: this.workerId },
      'StepExecutionSubscriber started successfully'
    );

    logInfo('StepExecutionSubscriber started', {
      component: 'subscriber',
      operation: 'start',
      worker_id: this.workerId,
    });
  }

  /**
   * Stop subscribing to step execution events.
   *
   * Note: Does not wait for in-flight handlers to complete.
   * Use waitForCompletion() if you need to wait.
   */
  stop(): void {
    if (!this.running) {
      return;
    }

    this.running = false;
    this.emitter.removeAllListeners(StepEventNames.STEP_EXECUTION_RECEIVED);

    logInfo('StepExecutionSubscriber stopped', {
      component: 'subscriber',
      operation: 'stop',
      processed_count: String(this.processedCount),
      error_count: String(this.errorCount),
    });
  }

  /**
   * Check if the subscriber is running.
   */
  isRunning(): boolean {
    return this.running;
  }

  /**
   * Get the count of events processed.
   */
  getProcessedCount(): number {
    return this.processedCount;
  }

  /**
   * Get the count of errors encountered.
   */
  getErrorCount(): number {
    return this.errorCount;
  }

  /**
   * Get the count of currently active handlers.
   */
  getActiveHandlers(): number {
    return this.activeHandlers;
  }

  /**
   * Wait for all active handlers to complete.
   *
   * @param timeoutMs - Maximum time to wait (default: 30000)
   * @returns True if all handlers completed, false if timeout
   */
  async waitForCompletion(timeoutMs = 30000): Promise<boolean> {
    const startTime = Date.now();
    const checkInterval = 100;

    while (this.activeHandlers > 0) {
      if (Date.now() - startTime > timeoutMs) {
        logWarn('Timeout waiting for handlers to complete', {
          component: 'subscriber',
          active_handlers: String(this.activeHandlers),
        });
        return false;
      }
      await new Promise((resolve) => setTimeout(resolve, checkInterval));
    }

    return true;
  }

  /**
   * Handle a step execution event.
   */
  private handleEvent(event: FfiStepEvent): void {
    pinoLog.info(
      {
        component: 'subscriber',
        eventId: event.eventId,
        running: this.running,
        activeHandlers: this.activeHandlers,
        maxConcurrent: this.maxConcurrent,
      },
      'handleEvent() called'
    );

    if (!this.running) {
      pinoLog.warn(
        { component: 'subscriber', eventId: event.eventId },
        'Received event while stopped, ignoring'
      );
      return;
    }

    // Check concurrency limit
    if (this.activeHandlers >= this.maxConcurrent) {
      pinoLog.warn(
        {
          component: 'subscriber',
          activeHandlers: this.activeHandlers,
          maxConcurrent: this.maxConcurrent,
        },
        'Max concurrent handlers reached, event will be re-polled'
      );
      // Don't process - event stays in FFI queue and will be re-polled
      return;
    }

    pinoLog.info(
      { component: 'subscriber', eventId: event.eventId },
      'About to call processEvent()'
    );

    // Process asynchronously
    this.processEvent(event).catch((error) => {
      pinoLog.error(
        {
          component: 'subscriber',
          eventId: event.eventId,
          error: error instanceof Error ? error.message : String(error),
          stack: error instanceof Error ? error.stack : undefined,
        },
        'Unhandled error in processEvent'
      );
    });
  }

  /**
   * Process a step execution event.
   */
  private async processEvent(event: FfiStepEvent): Promise<void> {
    pinoLog.info({ component: 'subscriber', eventId: event.eventId }, 'processEvent() starting');

    this.activeHandlers++;
    const startTime = Date.now();

    try {
      // Extract handler name from step definition
      const handlerName = this.extractHandlerName(event);
      pinoLog.info(
        { component: 'subscriber', eventId: event.eventId, handlerName },
        'Extracted handler name'
      );

      if (!handlerName) {
        pinoLog.error(
          { component: 'subscriber', eventId: event.eventId },
          'No handler name found!'
        );
        await this.submitErrorResult(event, 'No handler name found in step definition', startTime);
        return;
      }

      pinoLog.info(
        {
          component: 'subscriber',
          eventId: event.eventId,
          stepUuid: event.stepUuid,
          handlerName,
        },
        'Processing step event'
      );

      // Emit started event
      this.emitter.emit(StepEventNames.STEP_EXECUTION_STARTED, {
        eventId: event.eventId,
        stepUuid: event.stepUuid,
        handlerName,
        timestamp: new Date(),
      });

      // Resolve handler from registry (TAS-93: async for resolver chain support)
      pinoLog.info({ component: 'subscriber', handlerName }, 'Resolving handler from registry...');
      const handler = await this.registry.resolve(handlerName);
      pinoLog.info(
        { component: 'subscriber', handlerName, handlerFound: !!handler },
        'Handler resolution result'
      );

      if (!handler) {
        pinoLog.error({ component: 'subscriber', handlerName }, 'Handler not found in registry!');
        await this.submitErrorResult(event, `Handler not found: ${handlerName}`, startTime);
        return;
      }

      // Create context from FFI event
      pinoLog.info({ component: 'subscriber', handlerName }, 'Creating StepContext from FFI event');
      const context = StepContext.fromFfiEvent(event, handlerName);
      pinoLog.info(
        { component: 'subscriber', handlerName },
        'StepContext created, executing handler'
      );

      // Execute handler with timeout
      const result = await this.executeWithTimeout(
        () => handler.call(context),
        this.handlerTimeoutMs
      );

      pinoLog.info(
        { component: 'subscriber', handlerName, success: result.success },
        'Handler execution completed'
      );

      const executionTimeMs = Date.now() - startTime;

      // Submit result to Rust
      await this.submitResult(event, result, executionTimeMs);

      // Emit completed/failed event
      if (result.success) {
        this.emitter.emit(StepEventNames.STEP_EXECUTION_COMPLETED, {
          eventId: event.eventId,
          stepUuid: event.stepUuid,
          handlerName,
          executionTimeMs,
          timestamp: new Date(),
        });
      } else {
        this.emitter.emit(StepEventNames.STEP_EXECUTION_FAILED, {
          eventId: event.eventId,
          stepUuid: event.stepUuid,
          handlerName,
          error: result.errorMessage,
          executionTimeMs,
          timestamp: new Date(),
        });
      }

      this.processedCount++;
    } catch (error) {
      this.errorCount++;
      const errorMessage = error instanceof Error ? error.message : String(error);

      logError('Handler execution failed', {
        component: 'subscriber',
        event_id: event.eventId,
        step_uuid: event.stepUuid,
        error_message: errorMessage,
      });

      await this.submitErrorResult(event, errorMessage, startTime);

      this.emitter.emit(StepEventNames.STEP_EXECUTION_FAILED, {
        eventId: event.eventId,
        stepUuid: event.stepUuid,
        error: errorMessage,
        executionTimeMs: Date.now() - startTime,
        timestamp: new Date(),
      });
    } finally {
      this.activeHandlers--;
    }
  }

  /**
   * Execute a function with a timeout.
   */
  private async executeWithTimeout<T>(fn: () => Promise<T>, timeoutMs: number): Promise<T> {
    return Promise.race([
      fn(),
      new Promise<never>((_, reject) =>
        setTimeout(
          () => reject(new Error(`Handler execution timed out after ${timeoutMs}ms`)),
          timeoutMs
        )
      ),
    ]);
  }

  /**
   * Extract handler name from FFI event.
   *
   * TAS-290: With napi-rs, handler callable is flattened to stepDefinition.handlerCallable
   */
  private extractHandlerName(event: FfiStepEvent): string | null {
    return event.stepDefinition?.handlerCallable || null;
  }

  /**
   * Submit a handler result via FFI.
   *
   * TAS-125: Detects checkpoint yields and routes them to checkpointYieldStepEvent
   * instead of the normal completion path.
   */
  private async submitResult(
    event: FfiStepEvent,
    result: StepHandlerResult,
    _executionTimeMs: number
  ): Promise<void> {
    pinoLog.info(
      { component: 'subscriber', eventId: event.eventId },
      'submitResult() called'
    );

    // TAS-125: Check for checkpoint yield in metadata
    if (result.metadata?.checkpoint_yield === true) {
      await this.submitCheckpointYield(event, result);
      return;
    }

    const napiResult = this.buildNapiStepResult(event, result);
    await this.sendCompletionViaFfi(event, napiResult, result.success);
  }

  /**
   * TAS-125: Submit a checkpoint yield via FFI.
   *
   * Called when a handler returns a checkpoint_yield result.
   * This persists the checkpoint and re-dispatches the step.
   */
  private async submitCheckpointYield(
    event: FfiStepEvent,
    result: StepHandlerResult
  ): Promise<void> {
    pinoLog.info(
      { component: 'subscriber', eventId: event.eventId },
      'submitCheckpointYield() called - handler yielded checkpoint'
    );

    // Extract checkpoint data from the result
    const resultData = result.result ?? {};
    const checkpointData: NapiCheckpointYieldData = {
      stepUuid: event.stepUuid,
      cursor: resultData.cursor ?? 0,
      itemsProcessed: (resultData.items_processed as number) ?? 0,
    };

    // Only set accumulatedResults if it exists
    const accumulatedResults = resultData.accumulated_results as
      | Record<string, unknown>
      | undefined;
    if (accumulatedResults !== undefined) {
      checkpointData.accumulatedResults = accumulatedResults;
    }

    try {
      const success = this.module.checkpointYieldStepEvent(event.eventId, checkpointData);

      if (success) {
        pinoLog.info(
          {
            component: 'subscriber',
            eventId: event.eventId,
            cursor: checkpointData.cursor,
            itemsProcessed: checkpointData.itemsProcessed,
          },
          'Checkpoint yield submitted successfully - step will be re-dispatched'
        );

        this.emitter.emit(StepEventNames.STEP_CHECKPOINT_YIELD_SENT, {
          eventId: event.eventId,
          stepUuid: event.stepUuid,
          cursor: checkpointData.cursor,
          itemsProcessed: checkpointData.itemsProcessed,
          timestamp: new Date(),
        });

        logInfo('Checkpoint yield submitted', {
          component: 'subscriber',
          event_id: event.eventId,
          step_uuid: event.stepUuid,
          cursor: String(checkpointData.cursor),
          items_processed: String(checkpointData.itemsProcessed),
        });
      } else {
        pinoLog.error(
          { component: 'subscriber', eventId: event.eventId },
          'Checkpoint yield rejected by Rust - event may not be in pending map'
        );
        logError('Checkpoint yield rejected', {
          component: 'subscriber',
          event_id: event.eventId,
          step_uuid: event.stepUuid,
        });
      }
    } catch (error) {
      pinoLog.error(
        {
          component: 'subscriber',
          eventId: event.eventId,
          error: error instanceof Error ? error.message : String(error),
        },
        'Checkpoint yield failed with error'
      );
      logError('Failed to submit checkpoint yield', {
        component: 'subscriber',
        event_id: event.eventId,
        error_message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  /**
   * Submit an error result via FFI (for handler resolution/execution failures).
   */
  private async submitErrorResult(
    event: FfiStepEvent,
    errorMessage: string,
    _startTime: number
  ): Promise<void> {
    const napiResult = this.buildErrorNapiStepResult(event, errorMessage);
    const accepted = await this.sendCompletionViaFfi(event, napiResult, false);
    if (accepted) {
      this.errorCount++;
    }
  }

  /**
   * Build a NapiStepResult from a handler result.
   *
   * TAS-290: Flat structure matching #[napi(object)] NapiStepResult.
   * Metadata (executionTimeMs, workerId, etc.) is not passed — Rust side uses defaults.
   */
  private buildNapiStepResult(
    event: FfiStepEvent,
    result: StepHandlerResult
  ): NapiStepResult {
    return {
      stepUuid: event.stepUuid,
      success: result.success,
      result: result.result ?? {},
      status: result.success ? 'completed' : 'failed',
      errorMessage: result.success ? null : (result.errorMessage ?? 'Unknown error'),
      errorType: result.success ? null : (result.errorType ?? 'handler_error'),
      errorRetryable: result.success ? null : (result.retryable ?? false),
      errorStatusCode: null,
    };
  }

  /**
   * Build an error NapiStepResult for handler resolution/execution failures.
   */
  private buildErrorNapiStepResult(
    event: FfiStepEvent,
    errorMessage: string
  ): NapiStepResult {
    return {
      stepUuid: event.stepUuid,
      success: false,
      result: {},
      status: 'error',
      errorMessage,
      errorType: 'handler_error',
      errorRetryable: true,
      errorStatusCode: null,
    };
  }

  /**
   * Send a completion result to Rust via FFI and handle the response.
   *
   * @returns true if the completion was accepted by Rust, false otherwise
   */
  private async sendCompletionViaFfi(
    event: FfiStepEvent,
    napiResult: NapiStepResult,
    isSuccess: boolean
  ): Promise<boolean> {
    pinoLog.info(
      {
        component: 'subscriber',
        eventId: event.eventId,
        stepUuid: event.stepUuid,
        success: napiResult.success,
        status: napiResult.status,
      },
      'About to call module.completeStepEvent()'
    );

    try {
      const ffiResult = this.module.completeStepEvent(event.eventId, napiResult);

      if (ffiResult) {
        this.handleFfiSuccess(event, isSuccess);
        return true;
      }
      this.handleFfiRejection(event);
      return false;
    } catch (error) {
      this.handleFfiError(event, error);
      return false;
    }
  }

  /**
   * Handle successful FFI completion submission.
   */
  private handleFfiSuccess(
    event: FfiStepEvent,
    isSuccess: boolean
  ): void {
    pinoLog.info(
      { component: 'subscriber', eventId: event.eventId, success: isSuccess },
      'completeStepEvent() returned TRUE - completion accepted by Rust'
    );

    this.emitter.emit(StepEventNames.STEP_COMPLETION_SENT, {
      eventId: event.eventId,
      stepUuid: event.stepUuid,
      success: isSuccess,
      timestamp: new Date(),
    });

    logDebug('Step result submitted', {
      component: 'subscriber',
      event_id: event.eventId,
      step_uuid: event.stepUuid,
      success: String(isSuccess),
    });
  }

  /**
   * Handle FFI completion rejection (event not in pending map).
   */
  private handleFfiRejection(event: FfiStepEvent): void {
    pinoLog.error(
      {
        component: 'subscriber',
        eventId: event.eventId,
        stepUuid: event.stepUuid,
      },
      'completeStepEvent() returned FALSE - completion REJECTED by Rust! Event may not be in pending map.'
    );
    logError('FFI completion rejected', {
      component: 'subscriber',
      event_id: event.eventId,
      step_uuid: event.stepUuid,
    });
  }

  /**
   * Handle FFI completion error.
   */
  private handleFfiError(event: FfiStepEvent, error: unknown): void {
    pinoLog.error(
      {
        component: 'subscriber',
        eventId: event.eventId,
        error: error instanceof Error ? error.message : String(error),
        stack: error instanceof Error ? error.stack : undefined,
      },
      'completeStepEvent() THREW AN ERROR!'
    );
    logError('Failed to submit step result', {
      component: 'subscriber',
      event_id: event.eventId,
      error_message: error instanceof Error ? error.message : String(error),
    });
  }
}
