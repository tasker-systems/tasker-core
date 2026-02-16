/**
 * Structured logging API for TypeScript workers.
 *
 * Provides unified structured logging that integrates with Rust tracing
 * infrastructure via FFI. All log messages are forwarded to the Rust
 * tracing subscriber for consistent formatting and output.
 *
 * Matches Python's logging module and Ruby's tracing module (TAS-92 aligned).
 *
 * To enable FFI logging, call setLoggingRuntime() after loading the FFI layer.
 * If no runtime is installed, logs fall back to console output.
 */

import type { NapiModule } from '../ffi/ffi-layer.js';
import type { LogFields as FfiLogFields } from '../ffi/types.js';

/**
 * Installed module for logging.
 * Set via setLoggingRuntime() for explicit dependency injection.
 */
let installedModule: NapiModule | null = null;

/**
 * Install a napi module for logging to use.
 *
 * Call this after loading the FFI layer to enable Rust tracing integration.
 * If not called, logs fall back to console output.
 *
 * @param module - The napi module to use for logging
 *
 * @example
 * ```typescript
 * const ffiLayer = new FfiLayer();
 * await ffiLayer.load();
 * setLoggingRuntime(ffiLayer.getModule());
 * ```
 */
export function setLoggingRuntime(module: NapiModule): void {
  installedModule = module;
}

/**
 * Clear the installed logging runtime.
 *
 * Primarily for testing.
 */
export function clearLoggingRuntime(): void {
  installedModule = null;
}

/**
 * Get the module for logging.
 * Returns null if no module is installed (falls back to console).
 * @internal
 */
function getLoggingModule(): NapiModule | null {
  return installedModule;
}

/**
 * Structured logging fields.
 *
 * All fields are optional. Common fields include:
 * - component: Component/subsystem identifier (e.g., "handler", "registry")
 * - operation: Operation being performed (e.g., "process_payment")
 * - correlation_id: Distributed tracing correlation ID
 * - task_uuid: Task identifier
 * - step_uuid: Step identifier
 * - namespace: Task namespace
 * - error_message: Error message for error logs
 * - duration_ms: Execution duration for timed operations
 */
export interface LogFields {
  [key: string]: string | number | boolean | null | undefined;
}

/**
 * Convert LogFields to FFI-compatible format.
 * FFI expects all values as string | number | boolean | null.
 */
function toFfiFields(fields?: LogFields): FfiLogFields {
  if (!fields) {
    return {};
  }

  const result: FfiLogFields = {};
  for (const [key, value] of Object.entries(fields)) {
    if (value !== undefined) {
      result[key] = value as string | number | boolean | null;
    }
  }
  return result;
}

/**
 * Fallback console logging when FFI is not available.
 */
function fallbackLog(level: string, message: string, fields?: LogFields): void {
  const timestamp = new Date().toISOString();
  const fieldsStr = fields ? ` ${JSON.stringify(fields)}` : '';
  console.log(`[${timestamp}] ${level.toUpperCase()}: ${message}${fieldsStr}`);
}

/**
 * Log an ERROR level message with structured fields.
 */
export function logError(message: string, fields?: LogFields): void {
  const module = getLoggingModule();
  if (!module) {
    fallbackLog('error', message, fields);
    return;
  }

  try {
    module.logError(message, toFfiFields(fields));
  } catch {
    fallbackLog('error', message, fields);
  }
}

/**
 * Log a WARN level message with structured fields.
 */
export function logWarn(message: string, fields?: LogFields): void {
  const module = getLoggingModule();
  if (!module) {
    fallbackLog('warn', message, fields);
    return;
  }

  try {
    module.logWarn(message, toFfiFields(fields));
  } catch {
    fallbackLog('warn', message, fields);
  }
}

/**
 * Log an INFO level message with structured fields.
 */
export function logInfo(message: string, fields?: LogFields): void {
  const module = getLoggingModule();
  if (!module) {
    fallbackLog('info', message, fields);
    return;
  }

  try {
    module.logInfo(message, toFfiFields(fields));
  } catch {
    fallbackLog('info', message, fields);
  }
}

/**
 * Log a DEBUG level message with structured fields.
 */
export function logDebug(message: string, fields?: LogFields): void {
  const module = getLoggingModule();
  if (!module) {
    fallbackLog('debug', message, fields);
    return;
  }

  try {
    module.logDebug(message, toFfiFields(fields));
  } catch {
    fallbackLog('debug', message, fields);
  }
}

/**
 * Log a TRACE level message with structured fields.
 */
export function logTrace(message: string, fields?: LogFields): void {
  const module = getLoggingModule();
  if (!module) {
    fallbackLog('trace', message, fields);
    return;
  }

  try {
    module.logTrace(message, toFfiFields(fields));
  } catch {
    fallbackLog('trace', message, fields);
  }
}

/**
 * Create a logger with preset fields.
 */
export function createLogger(defaultFields: LogFields) {
  const mergeFields = (fields?: LogFields): LogFields => ({
    ...defaultFields,
    ...fields,
  });

  return {
    error: (message: string, fields?: LogFields) => logError(message, mergeFields(fields)),
    warn: (message: string, fields?: LogFields) => logWarn(message, mergeFields(fields)),
    info: (message: string, fields?: LogFields) => logInfo(message, mergeFields(fields)),
    debug: (message: string, fields?: LogFields) => logDebug(message, mergeFields(fields)),
    trace: (message: string, fields?: LogFields) => logTrace(message, mergeFields(fields)),
  };
}
