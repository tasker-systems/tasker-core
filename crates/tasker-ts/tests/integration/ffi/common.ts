/**
 * Common utilities for FFI integration tests.
 *
 * Provides shared helper functions for testing TaskerRuntime implementations
 * against the actual FFI library.
 */

import { FfiLayer } from '../../../src/ffi/ffi-layer.ts';

/**
 * Find the FFI module path for testing.
 *
 * Re-exports FfiLayer.findModulePath() for test convenience.
 * See FfiLayer for full search order documentation.
 */
export function findModulePath(): string | null {
  return FfiLayer.findModulePath();
}

/**
 * @deprecated Use findModulePath() instead
 */
export function findLibraryPath(): string | null {
  return FfiLayer.findModulePath();
}

/**
 * Check if DATABASE_URL is set.
 *
 * Note: This only checks if the environment variable is set, not if
 * the database is actually accessible.
 */
export function isDatabaseAvailable(): boolean {
  return typeof process.env.DATABASE_URL === 'string' && process.env.DATABASE_URL.length > 0;
}

/**
 * Check if bootstrap tests should run.
 *
 * Bootstrap tests require database connectivity and can timeout if the database
 * is not accessible. To avoid slow test failures, bootstrap tests only run when
 * explicitly enabled via FFI_BOOTSTRAP_TESTS=true environment variable.
 *
 * This is useful because DATABASE_URL may be set in the environment but the
 * database may not be running or accessible.
 */
export function shouldRunBootstrapTests(): boolean {
  return process.env.FFI_BOOTSTRAP_TESTS === 'true' && isDatabaseAvailable();
}

/**
 * Skip message for tests requiring bootstrap (database connectivity).
 */
export const SKIP_BOOTSTRAP_MESSAGE =
  'Skipping: Set FFI_BOOTSTRAP_TESTS=true and DATABASE_URL to run bootstrap tests';

/**
 * Skip message for tests requiring database connectivity.
 */
export const SKIP_DATABASE_MESSAGE = 'Skipping: DATABASE_URL not set';

/**
 * Skip message for tests requiring FFI library.
 */
export const SKIP_LIBRARY_MESSAGE =
  'Skipping: No .node module found. Build with: cargo make build-ffi (in workers/typescript/)';

/**
 * Check if client API integration tests should run.
 *
 * Client tests require:
 * 1. FFI_CLIENT_TESTS=true environment variable
 * 2. All bootstrap test prerequisites (DATABASE_URL, FFI library)
 * 3. Orchestration server running at TASKER_ORCHESTRATION_URL
 */
export function shouldRunClientTests(): boolean {
  return process.env.FFI_CLIENT_TESTS === 'true' && shouldRunBootstrapTests();
}

/**
 * Get the orchestration server URL for client tests.
 */
export function getOrchestrationUrl(): string {
  return process.env.TASKER_ORCHESTRATION_URL ?? 'http://localhost:8080';
}

/**
 * Skip message for tests requiring client API (orchestration server).
 */
export const SKIP_CLIENT_MESSAGE =
  'Skipping: Set FFI_CLIENT_TESTS=true with orchestration server running';

/**
 * Assert that a value is a non-empty string.
 */
export function assertNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

/**
 * Assert that a value looks like a semantic version string.
 */
export function assertVersionString(value: unknown): value is string {
  if (typeof value !== 'string') return false;
  // Basic semver pattern: x.y.z with optional suffix
  return /^\d+\.\d+\.\d+/.test(value);
}

/**
 * Test configuration for FFI integration tests.
 */
export interface TestConfig {
  libraryPath: string;
  skipDatabase: boolean;
}

/**
 * Initialize test configuration.
 *
 * @throws Error if FFI library is not found
 */
export function initTestConfig(): TestConfig {
  const libraryPath = findModulePath();
  if (!libraryPath) {
    throw new Error(SKIP_LIBRARY_MESSAGE);
  }

  return {
    libraryPath,
    skipDatabase: !isDatabaseAvailable(),
  };
}
