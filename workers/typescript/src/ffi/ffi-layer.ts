/**
 * FfiLayer - Owns napi-rs module loading and lifecycle.
 *
 * TAS-290: Simplified from the multi-runtime koffi approach.
 * The napi-rs `.node` file IS the runtime — no runtime detection,
 * no NodeRuntime/DenoRuntime adapters, no JSON serialization.
 *
 * Design principles:
 * - Explicit construction: No singleton pattern
 * - Clear ownership: Owns the napi module instance
 * - Explicit lifecycle: load() and unload() methods
 */

import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type {
  BootstrapConfig,
  BootstrapResult,
  NapiCheckpointYieldData,
  NapiClientResult,
  NapiDispatchMetrics,
  NapiDomainEvent,
  NapiListTasksParams,
  NapiStepEvent,
  NapiStepResult,
  NapiTaskRequest,
  WorkerStatus,
} from './types.js';

/**
 * Interface for the napi-rs native module.
 *
 * These are the functions exported by the Rust `#[napi]` bindings.
 * Function names are auto-camelCased by napi-rs from Rust snake_case.
 */
export interface NapiModule {
  // Lifecycle
  getVersion(): string;
  getRustVersion(): string;
  healthCheck(): boolean;
  bootstrapWorker(config: BootstrapConfig): BootstrapResult;
  isWorkerRunning(): boolean;
  getWorkerStatus(): WorkerStatus;
  stopWorker(): WorkerStatus;
  transitionToGracefulShutdown(): WorkerStatus;

  // Step dispatch
  pollStepEvents(): NapiStepEvent | null;
  completeStepEvent(eventId: string, result: NapiStepResult): boolean;
  pollInProcessEvents(): NapiDomainEvent | null;
  checkpointYieldStepEvent(eventId: string, checkpoint: NapiCheckpointYieldData): boolean;

  // Metrics & maintenance
  getFfiDispatchMetrics(): NapiDispatchMetrics;
  checkStarvationWarnings(): void;
  cleanupTimeouts(): void;

  // Client API
  clientCreateTask(request: NapiTaskRequest): NapiClientResult;
  clientGetTask(taskUuid: string): NapiClientResult;
  clientListTasks(params: NapiListTasksParams): NapiClientResult;
  clientCancelTask(taskUuid: string): NapiClientResult;
  clientListTaskSteps(taskUuid: string): NapiClientResult;
  clientGetStep(taskUuid: string, stepUuid: string): NapiClientResult;
  clientGetStepAuditHistory(taskUuid: string, stepUuid: string): NapiClientResult;
  clientHealthCheck(): NapiClientResult;

  // Logging
  logError(message: string, fields?: Record<string, unknown>): void;
  logWarn(message: string, fields?: Record<string, unknown>): void;
  logInfo(message: string, fields?: Record<string, unknown>): void;
  logDebug(message: string, fields?: Record<string, unknown>): void;
  logTrace(message: string, fields?: Record<string, unknown>): void;
}

/**
 * Configuration for FfiLayer.
 */
export interface FfiLayerConfig {
  /** Custom module path (overrides discovery) */
  modulePath?: string;
}

/**
 * Owns napi-rs module loading and lifecycle.
 *
 * @example
 * ```typescript
 * const ffiLayer = new FfiLayer();
 * await ffiLayer.load();
 * const module = ffiLayer.getModule();
 * const result = module.bootstrapWorker({ namespace: 'default' });
 * await ffiLayer.unload();
 * ```
 */
export class FfiLayer {
  private module: NapiModule | null = null;
  private modulePath: string | null = null;
  private readonly configuredModulePath: string | undefined;

  constructor(config: FfiLayerConfig = {}) {
    this.configuredModulePath = config.modulePath;
  }

  /**
   * Load the napi-rs native module.
   *
   * @param customPath - Optional override for module path
   * @throws Error if module not found or failed to load
   */
  async load(customPath?: string): Promise<void> {
    if (this.module) {
      return; // Already loaded
    }

    const path = customPath ?? this.configuredModulePath ?? this.discoverModulePath();

    if (!path) {
      throw new Error(
        'napi-rs native module not found. No bundled .node file matches this platform, ' +
          'and TASKER_FFI_MODULE_PATH is not set.\n' +
          `Current platform: ${process.platform}-${process.arch}\n` +
          'Supported: linux-x64, darwin-arm64\n' +
          'Override: export TASKER_FFI_MODULE_PATH=/path/to/tasker_ts.linux-x64-gnu.node'
      );
    }

    // Load the .node file — this is a native Node-API module
    const nativeModule = require(path) as NapiModule;
    this.module = nativeModule;
    this.modulePath = path;
  }

  /**
   * Unload the native module and release resources.
   */
  async unload(): Promise<void> {
    this.module = null;
    this.modulePath = null;
  }

  /**
   * Check if the native module is loaded.
   */
  isLoaded(): boolean {
    return this.module !== null;
  }

  /**
   * Get the loaded napi-rs module.
   *
   * @throws Error if module is not loaded
   */
  getModule(): NapiModule {
    if (!this.module) {
      throw new Error('FFI not loaded. Call load() first.');
    }
    return this.module;
  }

  /**
   * Backward-compatible alias for getModule().
   *
   * @deprecated Use getModule() instead
   */
  getRuntime(): NapiModule {
    return this.getModule();
  }

  /**
   * Get the path to the loaded module.
   */
  getModulePath(): string | null {
    return this.modulePath;
  }

  /**
   * Find the napi-rs module path.
   *
   * Resolution order:
   * 1. TASKER_FFI_MODULE_PATH environment variable (explicit override)
   * 2. TASKER_FFI_LIBRARY_PATH environment variable (backward compat)
   * 3. Bundled .node file in package directory
   * 4. .node symlink in cargo target directory (local dev)
   *
   * Note: require() only recognizes .node extension as native modules.
   * If a path points to a .dylib/.so, we look for a .node sibling symlink.
   */
  static findModulePath(): string | null {
    // 1. Check explicit environment variable
    const envPath = process.env.TASKER_FFI_MODULE_PATH;
    if (envPath) {
      const resolved = resolveNodePath(envPath);
      if (resolved) return resolved;
      console.warn(`TASKER_FFI_MODULE_PATH is set to "${envPath}" but the file does not exist`);
      return null;
    }

    // 2. Backward compat: check old env var
    const legacyPath = process.env.TASKER_FFI_LIBRARY_PATH;
    if (legacyPath) {
      const resolved = resolveNodePath(legacyPath);
      if (resolved) return resolved;
      console.warn(`TASKER_FFI_LIBRARY_PATH is set to "${legacyPath}" but the file does not exist`);
      return null;
    }

    // 3. Try bundled .node file
    const bundledPath = findBundledNodeModule();
    if (bundledPath && existsSync(bundledPath)) {
      return bundledPath;
    }

    return null;
  }

  /**
   * Backward-compatible alias for findModulePath().
   *
   * @deprecated Use findModulePath() instead
   */
  static findLibraryPath(_callerDir?: string): string | null {
    return FfiLayer.findModulePath();
  }

  private discoverModulePath(): string | null {
    return FfiLayer.findModulePath();
  }
}

/**
 * Resolve a path to a .node file that require() can load.
 *
 * require() only recognizes the .node extension as a native addon.
 * If the given path points to a .dylib/.so, look for a .node sibling
 * (created by `cargo make build-ffi` as a symlink).
 */
function resolveNodePath(path: string): string | null {
  if (!existsSync(path)) return null;

  // If it already has .node extension, use it directly
  if (path.endsWith('.node')) return path;

  // For .dylib/.so paths, check for a .node symlink in the same directory
  const dir = dirname(path);
  const nodeFile = join(dir, 'tasker_ts.node');
  if (existsSync(nodeFile)) return nodeFile;

  // Fall back to the original path (may fail at require() time)
  return path;
}

/**
 * Bundled .node module filenames by platform/arch.
 *
 * napi-rs generates platform-specific .node files with this naming convention.
 */
const BUNDLED_NODE_MODULES: Record<string, string> = {
  'linux-x64': 'tasker_ts.linux-x64-gnu.node',
  'darwin-arm64': 'tasker_ts.darwin-arm64.node',
  'darwin-x64': 'tasker_ts.darwin-x64.node',
};

/**
 * Find the bundled .node module for the current platform.
 */
function findBundledNodeModule(): string | null {
  const key = `${process.platform}-${process.arch}`;
  const filename = BUNDLED_NODE_MODULES[key];
  if (!filename) {
    return null;
  }

  // Walk up from current file to find package root
  let dir = dirname(fileURLToPath(import.meta.url));
  for (let i = 0; i < 5; i++) {
    // Check in package root directory
    const candidate = join(dir, filename);
    if (existsSync(candidate)) return candidate;
    // Check in native/ subdirectory (backward compat layout)
    const nativeCandidate = join(dir, 'native', filename);
    if (existsSync(nativeCandidate)) return nativeCandidate;
    const parent = dirname(dir);
    if (parent === dir) break;
    dir = parent;
  }
  return null;
}
