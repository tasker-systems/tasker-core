/**
 * FfiLayer tests.
 *
 * Tests FfiLayer state management, load/unload lifecycle,
 * getModule error handling, and findLibraryPath static method.
 *
 * TAS-290: Updated for napi-rs (no runtimeType, .node modules).
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { FfiLayer } from '../../../src/ffi/ffi-layer.js';

// =============================================================================
// Tests
// =============================================================================

describe('FfiLayer', () => {
  describe('constructor', () => {
    test('should create with defaults', () => {
      const layer = new FfiLayer();
      expect(layer.isLoaded()).toBe(false);
    });

    test('should accept modulePath override', () => {
      const layer = new FfiLayer({ modulePath: '/custom/path.node' });
      expect(layer.isLoaded()).toBe(false);
    });
  });

  describe('isLoaded', () => {
    test('should be false before load', () => {
      const layer = new FfiLayer();

      expect(layer.isLoaded()).toBe(false);
    });
  });

  describe('getModule', () => {
    test('should throw when not loaded', () => {
      const layer = new FfiLayer();

      expect(() => layer.getModule()).toThrow('FFI not loaded');
    });
  });

  describe('getModulePath', () => {
    test('should return null before load', () => {
      const layer = new FfiLayer();

      expect(layer.getModulePath()).toBeNull();
    });
  });

  describe('load', () => {
    test('should throw when no module path found', async () => {
      // No configured path, no env var, no discoverable path
      const originalEnv = process.env.TASKER_FFI_LIBRARY_PATH;
      delete process.env.TASKER_FFI_LIBRARY_PATH;

      try {
        const layer = new FfiLayer();

        await expect(layer.load()).rejects.toThrow();
      } finally {
        if (originalEnv !== undefined) {
          process.env.TASKER_FFI_LIBRARY_PATH = originalEnv;
        }
      }
    });
  });

  describe('unload', () => {
    test('should be safe to call when not loaded', async () => {
      const layer = new FfiLayer();

      // Should not throw
      await layer.unload();

      expect(layer.isLoaded()).toBe(false);
      expect(layer.getModulePath()).toBeNull();
    });

    test('should clear state after unload', async () => {
      const layer = new FfiLayer();

      await layer.unload();

      expect(layer.isLoaded()).toBe(false);
      expect(layer.getModulePath()).toBeNull();
    });
  });
});

// =============================================================================
// findLibraryPath (static method)
// =============================================================================

describe('FfiLayer.findLibraryPath', () => {
  const originalEnv = process.env.TASKER_FFI_LIBRARY_PATH;

  afterEach(() => {
    if (originalEnv === undefined) {
      delete process.env.TASKER_FFI_LIBRARY_PATH;
    } else {
      process.env.TASKER_FFI_LIBRARY_PATH = originalEnv;
    }
  });

  test('should return null when TASKER_FFI_LIBRARY_PATH is not set', () => {
    delete process.env.TASKER_FFI_LIBRARY_PATH;

    const result = FfiLayer.findLibraryPath();

    expect(result).toBeNull();
  });

  test('should return null when path does not exist', () => {
    process.env.TASKER_FFI_LIBRARY_PATH = '/nonexistent/path/to/lib.node';

    const result = FfiLayer.findLibraryPath();

    expect(result).toBeNull();
  });

  test('should return path when env var is set and file exists', () => {
    // Use a file that we know exists (this test file itself)
    const existingFile = import.meta.path;
    process.env.TASKER_FFI_LIBRARY_PATH = existingFile;

    const result = FfiLayer.findLibraryPath();

    expect(result).toBe(existingFile);
  });

  test('should accept deprecated callerDir parameter', () => {
    delete process.env.TASKER_FFI_LIBRARY_PATH;

    // callerDir is deprecated and ignored, should still return null
    const result = FfiLayer.findLibraryPath('/some/dir');

    expect(result).toBeNull();
  });
});
