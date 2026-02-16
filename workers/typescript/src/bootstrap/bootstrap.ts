/**
 * Bootstrap API for TypeScript workers.
 *
 * High-level TypeScript API for worker lifecycle management.
 * Wraps FFI calls with type-safe interfaces and error handling.
 *
 * Matches Python's bootstrap.py and Ruby's bootstrap.rb (TAS-92 aligned).
 *
 * TAS-290: Uses NapiModule directly instead of TaskerRuntime abstraction.
 * All functions require an explicit module parameter. Use FfiLayer to load
 * the module before calling these functions.
 */

import type { NapiModule } from '../ffi/ffi-layer.js';
import type { BootstrapConfig, BootstrapResult, StopResult, WorkerStatus } from './types.js';
import {
  fromFfiBootstrapResult,
  fromFfiStopResult,
  fromFfiWorkerStatus,
  toFfiBootstrapConfig,
} from './types.js';

/**
 * Initialize the worker system.
 *
 * This function bootstraps the full tasker-worker system, including:
 * - Creating a Tokio runtime for async operations
 * - Connecting to the database
 * - Setting up the FFI dispatch channel for step events
 * - Subscribing to domain events
 *
 * @param config - Optional bootstrap configuration
 * @param module - The loaded napi-rs module (required)
 * @returns BootstrapResult with worker details and status
 * @throws Error if bootstrap fails critically
 *
 * @example
 * ```typescript
 * const ffiLayer = new FfiLayer();
 * await ffiLayer.load();
 * const result = await bootstrapWorker({ namespace: 'payments' }, ffiLayer.getModule());
 * console.log(`Worker ${result.workerId} started`);
 * ```
 */
export async function bootstrapWorker(
  config: BootstrapConfig | undefined,
  module: NapiModule
): Promise<BootstrapResult> {
  try {
    if (!module) {
      return {
        success: false,
        status: 'error',
        message: 'Module not loaded. Ensure the FFI library is available.',
        error: 'Module not loaded',
      };
    }

    const ffiConfig = toFfiBootstrapConfig(config);
    const ffiResult = module.bootstrapWorker(ffiConfig);
    return fromFfiBootstrapResult(ffiResult);
  } catch (error) {
    return {
      success: false,
      status: 'error',
      message: `Bootstrap failed: ${error instanceof Error ? error.message : String(error)}`,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Stop the worker system gracefully.
 *
 * This function stops the worker system and releases all resources.
 * Safe to call even if the worker is not running.
 *
 * @param module - The loaded napi-rs module (optional - returns success if not loaded)
 * @returns StopResult indicating the outcome
 *
 * @example
 * ```typescript
 * const result = stopWorker(module);
 * if (result.success) {
 *   console.log('Worker stopped successfully');
 * }
 * ```
 */
export function stopWorker(module?: NapiModule): StopResult {
  if (!module) {
    return {
      success: true,
      status: 'not_running',
      message: 'Module not loaded',
    };
  }

  try {
    const ffiResult = module.stopWorker();
    return fromFfiStopResult(ffiResult);
  } catch (error) {
    return {
      success: false,
      status: 'error',
      message: `Stop failed: ${error instanceof Error ? error.message : String(error)}`,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Get the current worker system status.
 *
 * Returns detailed information about the worker's current state,
 * including resource usage and operational status.
 *
 * @param module - The loaded napi-rs module (optional - returns stopped if not loaded)
 * @returns WorkerStatus with current state and metrics
 *
 * @example
 * ```typescript
 * const status = getWorkerStatus(module);
 * if (status.running) {
 *   console.log(`Pool size: ${status.databasePoolSize}`);
 * } else {
 *   console.log(`Worker not running`);
 * }
 * ```
 */
export function getWorkerStatus(module?: NapiModule): WorkerStatus {
  if (!module) {
    return {
      success: false,
      running: false,
      status: 'stopped',
    };
  }

  try {
    const ffiStatus = module.getWorkerStatus();
    return fromFfiWorkerStatus(ffiStatus);
  } catch (_error) {
    return {
      success: false,
      running: false,
      status: 'stopped',
    };
  }
}

/**
 * Initiate graceful shutdown of the worker system.
 *
 * This function begins the graceful shutdown process, allowing
 * in-flight operations to complete before fully stopping.
 * Call stopWorker() after this to fully stop the worker.
 *
 * @param module - The loaded napi-rs module (optional - returns success if not loaded)
 * @returns StopResult indicating the transition status
 *
 * @example
 * ```typescript
 * // Start graceful shutdown
 * transitionToGracefulShutdown(module);
 *
 * // Wait for in-flight operations...
 * await new Promise(resolve => setTimeout(resolve, 5000));
 *
 * // Fully stop
 * stopWorker(module);
 * ```
 */
export function transitionToGracefulShutdown(module?: NapiModule): StopResult {
  if (!module) {
    return {
      success: true,
      status: 'not_running',
      message: 'Module not loaded',
    };
  }

  try {
    const ffiResult = module.transitionToGracefulShutdown();
    return fromFfiStopResult(ffiResult);
  } catch (error) {
    return {
      success: false,
      status: 'error',
      message: `Graceful shutdown failed: ${error instanceof Error ? error.message : String(error)}`,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/**
 * Check if the worker system is currently running.
 *
 * Lightweight check that doesn't query the full status.
 *
 * @param module - The loaded napi-rs module (optional - returns false if not loaded)
 * @returns True if the worker is running
 *
 * @example
 * ```typescript
 * if (!isWorkerRunning(module)) {
 *   await bootstrapWorker(config, module);
 * }
 * ```
 */
export function isWorkerRunning(module?: NapiModule): boolean {
  if (!module) {
    return false;
  }

  try {
    return module.isWorkerRunning();
  } catch {
    return false;
  }
}

/**
 * Get version information for the worker system.
 *
 * @param module - The loaded napi-rs module (optional)
 * @returns Version string from the Rust library
 */
export function getVersion(module?: NapiModule): string {
  if (!module) {
    return 'unknown (module not loaded)';
  }

  try {
    return module.getVersion();
  } catch {
    return 'unknown';
  }
}

/**
 * Get detailed Rust library version.
 *
 * @param module - The loaded napi-rs module (optional)
 * @returns Detailed version information
 */
export function getRustVersion(module?: NapiModule): string {
  if (!module) {
    return 'unknown (module not loaded)';
  }

  try {
    return module.getRustVersion();
  } catch {
    return 'unknown';
  }
}

/**
 * Perform a health check on the FFI module.
 *
 * @param module - The loaded napi-rs module (optional - returns false if not loaded)
 * @returns True if the FFI module is functional
 */
export function healthCheck(module?: NapiModule): boolean {
  if (!module) {
    return false;
  }

  try {
    return module.healthCheck();
  } catch {
    return false;
  }
}
