/**
 * Bootstrap configuration and result types.
 *
 * TAS-290: With napi-rs, FFI types are already camelCase, so conversion
 * functions are simplified (mostly pass-through).
 */

import type {
  BootstrapConfig as FfiBootstrapConfig,
  BootstrapResult as FfiBootstrapResult,
  WorkerStatus as FfiWorkerStatus,
} from '../ffi/types.js';

// Re-export FFI types for convenience
export type { FfiBootstrapConfig, FfiBootstrapResult, FfiWorkerStatus };

/**
 * Configuration for worker bootstrap.
 *
 * Matches Python's BootstrapConfig and Ruby's bootstrap options.
 */
export interface BootstrapConfig {
  /** Optional worker ID. Auto-generated if not provided. */
  workerId?: string;

  /** Task namespace this worker handles (default: "default"). */
  namespace?: string;

  /** Path to custom configuration file (TOML). */
  configPath?: string;

  /** Log level: trace, debug, info, warn, error (default: "info"). */
  logLevel?: 'trace' | 'debug' | 'info' | 'warn' | 'error';

  /** Database URL override. */
  databaseUrl?: string;
}

/**
 * Result from worker bootstrap.
 */
export interface BootstrapResult {
  /** Whether bootstrap was successful. */
  success: boolean;

  /** Current status (started, already_running, error). */
  status: 'started' | 'already_running' | 'error';

  /** Human-readable status message. */
  message: string;

  /** Unique identifier for this worker instance. */
  workerId?: string;

  /** Error message if bootstrap failed. */
  error?: string;
}

/**
 * Current worker status.
 */
export interface WorkerStatus {
  /** Whether the status query succeeded. */
  success: boolean;

  /** Whether the worker is currently running. */
  running: boolean;

  /** Current status string. */
  status?: string;

  /** Worker ID if running. */
  workerId?: string;

  /** Current environment (test, development, production). */
  environment?: string;

  /** Internal worker core status. */
  workerCoreStatus?: string;

  /** Whether the web API is enabled. */
  webApiEnabled?: boolean;

  /** List of task namespaces this worker handles. */
  supportedNamespaces?: string[];

  /** Total database connection pool size. */
  databasePoolSize?: number;

  /** Number of idle database connections. */
  databasePoolIdle?: number;
}

/**
 * Result from stopping the worker.
 */
export interface StopResult {
  /** Whether the stop was successful. */
  success: boolean;

  /** Current status (stopped, not_running, error). */
  status: 'stopped' | 'not_running' | 'error';

  /** Human-readable status message. */
  message: string;

  /** Worker ID that was stopped. */
  workerId?: string;

  /** Error message if stop failed. */
  error?: string;
}

/**
 * Convert TypeScript BootstrapConfig to FFI format.
 *
 * TAS-290: With napi-rs, the FFI BootstrapConfig is already camelCase.
 * Only namespace and configPath are passed to the Rust layer.
 */
export function toFfiBootstrapConfig(config?: BootstrapConfig): FfiBootstrapConfig {
  if (!config) {
    return {};
  }

  const result: FfiBootstrapConfig = {};
  if (config.namespace !== undefined) result.namespace = config.namespace;
  if (config.configPath !== undefined) result.configPath = config.configPath;
  return result;
}

/**
 * Convert FFI BootstrapResult to TypeScript format.
 *
 * TAS-290: FFI result is already camelCase, minimal conversion needed.
 */
export function fromFfiBootstrapResult(result: FfiBootstrapResult): BootstrapResult {
  const out: BootstrapResult = {
    success: result.success,
    status: result.status as BootstrapResult['status'],
    message: result.message,
  };
  if (result.workerId != null) out.workerId = result.workerId;
  return out;
}

/**
 * Convert FFI WorkerStatus to TypeScript format.
 */
export function fromFfiWorkerStatus(status: FfiWorkerStatus): WorkerStatus {
  const out: WorkerStatus = {
    success: status.success,
    running: status.running,
  };
  if (status.status != null) out.status = status.status;
  if (status.workerId != null) out.workerId = status.workerId;
  if (status.environment != null) out.environment = status.environment;
  return out;
}

/**
 * Convert FFI WorkerStatus to StopResult format.
 */
export function fromFfiStopResult(result: FfiWorkerStatus): StopResult {
  const out: StopResult = {
    success: result.success,
    status: result.running ? 'stopped' : 'not_running',
    message: result.status ?? 'Worker stopped',
  };
  if (result.workerId != null) out.workerId = result.workerId;
  return out;
}
