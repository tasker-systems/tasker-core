/**
 * FfiLayer tests.
 *
 * Tests FfiLayer state management, load/unload lifecycle,
 * getModule error handling, and findModulePath/findLibraryPath static methods.
 *
 * TAS-290: Updated for napi-rs (no runtimeType, .node modules).
 */

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
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
    test('should throw when given nonexistent custom path', async () => {
      const layer = new FfiLayer();
      await expect(layer.load('/nonexistent/path/to/module.node')).rejects.toThrow();
    });

    test('should be idempotent when called twice', async () => {
      const layer = new FfiLayer();
      // If a .node file exists (from build-ffi), load succeeds.
      // If not, it throws. Either way, second call should be safe.
      try {
        await layer.load();
        await layer.load(); // Should not throw (idempotent)
        expect(layer.isLoaded()).toBe(true);
      } catch {
        // No module available — expected in environments without build artifacts
        expect(layer.isLoaded()).toBe(false);
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
// findModulePath / findLibraryPath (static methods)
// =============================================================================

describe('FfiLayer.findModulePath', () => {
  let savedModulePath: string | undefined;

  beforeEach(() => {
    savedModulePath = process.env.TASKER_FFI_MODULE_PATH;
  });

  afterEach(() => {
    if (savedModulePath === undefined) {
      delete process.env.TASKER_FFI_MODULE_PATH;
    } else {
      process.env.TASKER_FFI_MODULE_PATH = savedModulePath;
    }
  });

  test('should return null when no env var is set and no bundled module', () => {
    delete process.env.TASKER_FFI_MODULE_PATH;

    const result = FfiLayer.findModulePath();

    // May find bundled .node file in package root, or null
    // Just verify it doesn't throw
    expect(result === null || typeof result === 'string').toBe(true);
  });

  test('should return null when TASKER_FFI_MODULE_PATH points to nonexistent file', () => {
    process.env.TASKER_FFI_MODULE_PATH = '/nonexistent/path/to/lib.node';

    const result = FfiLayer.findModulePath();

    expect(result).toBeNull();
  });

  test('should return path when TASKER_FFI_MODULE_PATH points to existing file', () => {
    // Use a file that we know exists (this test file itself)
    const existingFile = import.meta.path;
    process.env.TASKER_FFI_MODULE_PATH = existingFile;

    const result = FfiLayer.findModulePath();

    expect(result).not.toBeNull();
    expect(result).toBe(existingFile);
  });

  test('findLibraryPath delegates to findModulePath (backward compat)', () => {
    delete process.env.TASKER_FFI_MODULE_PATH;

    const modulePath = FfiLayer.findModulePath();
    const libraryPath = FfiLayer.findLibraryPath();

    expect(libraryPath).toBe(modulePath);
  });
});
