/**
 * Functional/factory API for step handlers (TAS-294).
 *
 * This module provides factory-function alternatives to the class-based handler API.
 * It reduces boilerplate for common handler patterns while preserving full access
 * to the underlying StepContext for advanced use cases.
 *
 * Factory functions auto-wrap return values and classify exceptions:
 * - Record return → StepHandlerResult.success(record)
 * - StepHandlerResult return → pass through unchanged
 * - PermanentError thrown → StepHandlerResult.failure(retryable=false)
 * - RetryableError thrown → StepHandlerResult.failure(retryable=true)
 * - Other errors → StepHandlerResult.failure(retryable=true)
 *
 * @example
 * ```typescript
 * import { defineHandler, PermanentError } from 'tasker-core';
 *
 * const ProcessPayment = defineHandler('process_payment', {
 *   depends: { cart: 'validate_cart' },
 *   inputs: { paymentInfo: 'payment_info' },
 * }, async ({ cart, paymentInfo, context }) => {
 *   if (!paymentInfo) throw new PermanentError('Payment info required');
 *   const result = await chargeCard(paymentInfo, cart.total);
 *   return { paymentId: result.id, amount: cart.total };
 * });
 * ```
 *
 * @module handler/functional
 */

import type { BatchWorkerContext } from '../types/batch.js';
import { createBatches as createBatchesOutcome } from '../types/batch.js';
import { ErrorType } from '../types/error-type.js';
import type { StepContext } from '../types/step-context.js';
import { StepHandlerResult } from '../types/step-handler-result.js';
import { StepHandler } from './base.js';
import { BatchableMixin, type RustCursorConfig } from './batchable.js';
import { type APICapable, applyAPI } from './mixins/api.js';
import { DecisionMixin } from './mixins/decision.js';

// ============================================================================
// Error Classes
// ============================================================================

/**
 * Base error for permanent, non-retryable failures.
 *
 * Throw this in functional handlers when the error will not succeed on retry.
 *
 * @example
 * ```typescript
 * throw new PermanentError('Invalid account number');
 * ```
 */
export class PermanentError extends Error {
  readonly retryable = false;
  readonly metadata: Record<string, unknown>;

  constructor(message: string, metadata?: Record<string, unknown>) {
    super(message);
    this.name = 'PermanentError';
    this.metadata = metadata ?? {};
  }
}

/**
 * Base error for transient, retryable failures.
 *
 * Throw this in functional handlers when a retry might succeed.
 *
 * @example
 * ```typescript
 * throw new RetryableError('Service temporarily unavailable');
 * ```
 */
export class RetryableError extends Error {
  readonly retryable = true;
  readonly metadata: Record<string, unknown>;

  constructor(message: string, metadata?: Record<string, unknown>) {
    super(message);
    this.name = 'RetryableError';
    this.metadata = metadata ?? {};
  }
}

// ============================================================================
// Helper Types
// ============================================================================

/**
 * Options for defineHandler and related factory functions.
 */
export interface HandlerOptions {
  /** Mapping of parameter names to dependency step names */
  depends?: Record<string, string>;
  /** Mapping of parameter names to input data keys */
  inputs?: Record<string, string>;
  /** Handler version (default: "1.0.0") */
  version?: string;
}

/**
 * The injected arguments object passed to functional handlers.
 *
 * Contains declared dependencies, inputs, and always includes `context`.
 */
export type HandlerArgs = Record<string, unknown> & {
  context: StepContext;
};

/**
 * Handler function signature.
 */
export type HandlerFn = (
  args: HandlerArgs
) => Promise<Record<string, unknown> | StepHandlerResult | undefined>;

/**
 * Helper for decision handler return values.
 *
 * @example
 * ```typescript
 * const RouteOrder = defineDecisionHandler('route_order', {
 *   depends: { order: 'validate_order' },
 * }, async ({ order }) => {
 *   if (order.tier === 'premium') {
 *     return Decision.route(['process_premium'], { tier: 'premium' });
 *   }
 *   return Decision.route(['process_standard']);
 * });
 * ```
 */
export class Decision {
  private constructor(
    readonly type: 'create_steps' | 'no_branches',
    readonly steps: string[],
    readonly reason?: string,
    readonly routingContext: Record<string, unknown> = {}
  ) {}

  /**
   * Route to the specified steps.
   */
  static route(steps: string[], routingContext?: Record<string, unknown>): Decision {
    return new Decision('create_steps', steps, undefined, routingContext ?? {});
  }

  /**
   * Skip all branches.
   */
  static skip(reason: string, routingContext?: Record<string, unknown>): Decision {
    return new Decision('no_branches', [], reason, routingContext ?? {});
  }
}

/**
 * Configuration returned by batch analyzer handlers.
 */
export interface BatchConfig {
  totalItems: number;
  batchSize: number;
  metadata?: Record<string, unknown>;
}

// ============================================================================
// Internal: Auto-wrapping
// ============================================================================

function wrapResult(result: unknown): StepHandlerResult {
  if (result instanceof StepHandlerResult) {
    return result;
  }
  if (result != null && typeof result === 'object' && !Array.isArray(result)) {
    return StepHandlerResult.success(result as Record<string, unknown>);
  }
  if (result == null) {
    return StepHandlerResult.success({});
  }
  return StepHandlerResult.success({ result });
}

function wrapError(error: unknown): StepHandlerResult {
  if (error instanceof PermanentError) {
    return StepHandlerResult.failure(
      error.message,
      ErrorType.PERMANENT_ERROR,
      false,
      error.metadata
    );
  }
  if (error instanceof RetryableError) {
    return StepHandlerResult.failure(
      error.message,
      ErrorType.RETRYABLE_ERROR,
      true,
      error.metadata
    );
  }
  const message = error instanceof Error ? error.message : String(error);
  return StepHandlerResult.failure(message, ErrorType.HANDLER_ERROR, true, {
    errorType: error instanceof Error ? error.constructor.name : typeof error,
  });
}

function injectArgs(
  context: StepContext,
  depends: Record<string, string>,
  inputs: Record<string, string>
): HandlerArgs {
  const args: HandlerArgs = { context };

  for (const [paramName, stepName] of Object.entries(depends)) {
    args[paramName] = context.getDependencyResult(stepName);
  }

  for (const [paramName, inputKey] of Object.entries(inputs)) {
    args[paramName] = context.getInput(inputKey);
  }

  return args;
}

// ============================================================================
// Public Factory Functions
// ============================================================================

/**
 * Define a step handler from a function.
 *
 * Returns a StepHandler subclass that can be registered with HandlerRegistry.
 * The function receives injected dependencies, inputs, and context.
 * Return values are auto-wrapped as success results, and exceptions are
 * auto-classified as failure results.
 *
 * @param name - Handler name (must match step definition)
 * @param options - Dependencies, inputs, and version
 * @param fn - Handler function
 * @returns StepHandler subclass
 *
 * @example
 * ```typescript
 * const ProcessPayment = defineHandler('process_payment', {
 *   depends: { cart: 'validate_cart' },
 *   inputs: { paymentInfo: 'payment_info' },
 * }, async ({ cart, paymentInfo, context }) => {
 *   if (!paymentInfo) throw new PermanentError('Payment info required');
 *   const result = await chargeCard(paymentInfo, cart.total);
 *   return { paymentId: result.id, amount: cart.total };
 * });
 *
 * // Register:
 * registry.register(ProcessPayment);
 * ```
 */
export function defineHandler(
  name: string,
  options: HandlerOptions,
  fn: HandlerFn
): typeof StepHandler & { new (): StepHandler } {
  const depends = options.depends ?? {};
  const inputs = options.inputs ?? {};
  const version = options.version ?? '1.0.0';

  const HandlerClass = class extends StepHandler {
    static override handlerName = name;
    static override handlerVersion = version;

    async call(context: StepContext): Promise<StepHandlerResult> {
      try {
        const args = injectArgs(context, depends, inputs);
        const rawResult = await fn(args);
        return wrapResult(rawResult);
      } catch (error: unknown) {
        return wrapError(error);
      }
    }
  };

  Object.defineProperty(HandlerClass, 'name', { value: name });
  return HandlerClass as typeof StepHandler & { new (): StepHandler };
}

/**
 * Define a decision handler from a function.
 *
 * The function should return a `Decision.route(...)` or `Decision.skip(...)`.
 *
 * @param name - Handler name
 * @param options - Dependencies, inputs, and version
 * @param fn - Handler function returning a Decision
 * @returns StepHandler subclass
 *
 * @example
 * ```typescript
 * const RouteOrder = defineDecisionHandler('route_order', {
 *   depends: { order: 'validate_order' },
 * }, async ({ order }) => {
 *   if (order.tier === 'premium') {
 *     return Decision.route(['process_premium'], { tier: 'premium' });
 *   }
 *   return Decision.route(['process_standard']);
 * });
 * ```
 */
export function defineDecisionHandler(
  name: string,
  options: HandlerOptions,
  fn: (args: HandlerArgs) => Promise<Decision | StepHandlerResult>
): typeof StepHandler & { new (): StepHandler } {
  const depends = options.depends ?? {};
  const inputs = options.inputs ?? {};
  const version = options.version ?? '1.0.0';

  const HandlerClass = class extends StepHandler {
    static override handlerName = name;
    static override handlerVersion = version;

    get capabilities(): string[] {
      return ['process', 'decision', 'routing'];
    }

    async call(context: StepContext): Promise<StepHandlerResult> {
      try {
        const args = injectArgs(context, depends, inputs);
        const rawResult = await fn(args);

        if (rawResult instanceof StepHandlerResult) {
          return rawResult;
        }

        if (rawResult instanceof Decision) {
          // Delegate to DecisionMixin for correct serialization format
          const mixin = new DecisionMixin();
          // Bind name/version for metadata generation
          Object.defineProperty(mixin, 'name', { get: () => name });
          Object.defineProperty(mixin, 'version', { get: () => version });

          if (rawResult.type === 'create_steps') {
            return mixin.decisionSuccess(rawResult.steps, rawResult.routingContext);
          }
          return mixin.skipBranches(rawResult.reason ?? '', rawResult.routingContext);
        }

        return wrapResult(rawResult);
      } catch (error: unknown) {
        return wrapError(error);
      }
    }
  };

  Object.defineProperty(HandlerClass, 'name', { value: name });
  return HandlerClass as typeof StepHandler & { new (): StepHandler };
}

/**
 * Define a batch analyzer handler.
 *
 * The function should return a `BatchConfig` with `totalItems` and `batchSize`.
 * Cursor configs are generated automatically.
 *
 * @param name - Handler name
 * @param options - Handler options plus `workerTemplate`
 * @param fn - Handler function returning a BatchConfig
 * @returns StepHandler subclass
 */
export function defineBatchAnalyzer(
  name: string,
  options: HandlerOptions & { workerTemplate: string },
  fn: (args: HandlerArgs) => Promise<BatchConfig | StepHandlerResult>
): typeof StepHandler & { new (): StepHandler } {
  const depends = options.depends ?? {};
  const inputs = options.inputs ?? {};
  const version = options.version ?? '1.0.0';
  const workerTemplate = options.workerTemplate;

  const HandlerClass = class extends StepHandler {
    static override handlerName = name;
    static override handlerVersion = version;

    async call(context: StepContext): Promise<StepHandlerResult> {
      try {
        const args = injectArgs(context, depends, inputs);
        const rawResult = await fn(args);

        if (rawResult instanceof StepHandlerResult) {
          return rawResult;
        }

        const config = rawResult as BatchConfig;
        const { totalItems, batchSize } = config;

        // Delegate cursor generation to BatchableMixin
        const batchMixin = new BatchableMixin();
        const cursorRanges = batchMixin.createCursorRanges(totalItems, batchSize);

        // Convert CursorConfig[] to RustCursorConfig[] for FFI boundary
        const rustConfigs: RustCursorConfig[] = cursorRanges.map((c, idx) => ({
          batch_id: `batch_${String(idx).padStart(3, '0')}`,
          start_cursor: c.startCursor,
          end_cursor: c.endCursor,
          batch_size: c.endCursor - c.startCursor,
        }));

        // Use typed FFI outcome factory
        const batchProcessingOutcome = createBatchesOutcome(
          workerTemplate,
          rustConfigs.length,
          rustConfigs,
          totalItems
        );

        return StepHandlerResult.success(
          { batch_processing_outcome: batchProcessingOutcome },
          {
            batch_count: rustConfigs.length,
            total_items: totalItems,
            ...(config.metadata ?? {}),
          }
        );
      } catch (error: unknown) {
        return wrapError(error);
      }
    }
  };

  Object.defineProperty(HandlerClass, 'name', { value: name });
  return HandlerClass as typeof StepHandler & { new (): StepHandler };
}

/**
 * Define a batch worker handler.
 *
 * The function receives a `batchContext` parameter extracted from the step
 * context, containing cursor configuration for this worker's partition.
 *
 * @param name - Handler name
 * @param options - Dependencies, inputs, and version
 * @param fn - Handler function receiving batch context
 * @returns StepHandler subclass
 */
export function defineBatchWorker(
  name: string,
  options: HandlerOptions,
  fn: (
    args: HandlerArgs & { batchContext: BatchWorkerContext | null }
  ) => Promise<Record<string, unknown> | StepHandlerResult | undefined>
): typeof StepHandler & { new (): StepHandler } {
  const depends = options.depends ?? {};
  const inputs = options.inputs ?? {};
  const version = options.version ?? '1.0.0';

  const HandlerClass = class extends StepHandler {
    static override handlerName = name;
    static override handlerVersion = version;

    async call(context: StepContext): Promise<StepHandlerResult> {
      try {
        const args = injectArgs(context, depends, inputs) as HandlerArgs & {
          batchContext: BatchWorkerContext | null;
        };

        // Delegate batch context extraction to BatchableMixin
        const batchMixin = new BatchableMixin();
        args.batchContext = batchMixin.getBatchContext(context);

        const rawResult = await fn(args);
        return wrapResult(rawResult);
      } catch (error: unknown) {
        return wrapError(error);
      }
    }
  };

  Object.defineProperty(HandlerClass, 'name', { value: name });
  return HandlerClass as typeof StepHandler & { new (): StepHandler };
}

// ============================================================================
// API Handler Options
// ============================================================================

/**
 * Options for defineApiHandler.
 */
export interface ApiHandlerOptions extends HandlerOptions {
  /** Base URL for API calls */
  baseUrl: string;
  /** Default request timeout in milliseconds (default: 30000) */
  defaultTimeout?: number;
  /** Default headers to include in all requests */
  defaultHeaders?: Record<string, string>;
}

/**
 * The injected arguments for API handlers, including the `api` object
 * with pre-configured HTTP methods and result helpers.
 *
 * `api` is the handler instance itself with API methods applied (matching
 * the Python/Ruby pattern where `api=self`).
 */
export type ApiHandlerArgs = HandlerArgs & {
  api: APICapable;
};

// ============================================================================
// API Handler Factory
// ============================================================================

/**
 * Define an API handler with HTTP client functionality.
 *
 * The function receives an `api` object providing pre-configured HTTP methods
 * (get, post, put, patch, delete, request) and result helpers (apiSuccess,
 * apiFailure, connectionError, timeoutError) from the APIMixin.
 *
 * @param name - Handler name (must match step definition)
 * @param options - Dependencies, inputs, version, and API configuration
 * @param fn - Handler function receiving api and other injected args
 * @returns StepHandler subclass
 *
 * @example
 * ```typescript
 * const FetchUser = defineApiHandler('fetch_user', {
 *   baseUrl: 'https://api.example.com',
 *   depends: { userId: 'validate_user' },
 * }, async ({ userId, api }) => {
 *   const response = await api.get(`/users/${userId}`);
 *   if (response.ok) {
 *     return api.apiSuccess(response);
 *   }
 *   return api.apiFailure(response);
 * });
 * ```
 */
export function defineApiHandler(
  name: string,
  options: ApiHandlerOptions,
  fn: (args: ApiHandlerArgs) => Promise<Record<string, unknown> | StepHandlerResult | undefined>
): typeof StepHandler & { new (): StepHandler } {
  const depends = options.depends ?? {};
  const inputs = options.inputs ?? {};
  const version = options.version ?? '1.0.0';
  const apiBaseUrl = options.baseUrl;
  const apiTimeout = options.defaultTimeout ?? 30000;
  const apiHeaders = options.defaultHeaders ?? {};

  const HandlerClass = class extends StepHandler {
    static override handlerName = name;
    static override handlerVersion = version;
    static baseUrl = apiBaseUrl;
    static defaultTimeout = apiTimeout;
    static defaultHeaders = apiHeaders;

    constructor() {
      super();
      // Apply HTTP methods (get, post, put, patch, delete, request) and
      // result helpers (apiSuccess, apiFailure, connectionError, timeoutError)
      // directly onto this instance — matching the Python/Ruby pattern
      // where api=self.
      applyAPI(this);
    }

    async call(context: StepContext): Promise<StepHandlerResult> {
      try {
        const args = injectArgs(context, depends, inputs) as ApiHandlerArgs;
        args.api = this as unknown as APICapable;
        const rawResult = await fn(args);
        return wrapResult(rawResult);
      } catch (error: unknown) {
        return wrapError(error);
      }
    }
  };

  Object.defineProperty(HandlerClass, 'name', { value: name });
  return HandlerClass as typeof StepHandler & { new (): StepHandler };
}
