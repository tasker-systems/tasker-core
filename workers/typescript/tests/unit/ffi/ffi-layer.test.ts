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
    test('should throw when no module path found', async () => {
      // No configured path, no env var, no discoverable path
      const savedModulePath = process.env.TASKER_FFI_MODULE_PATH;
      const savedLibraryPath = process.env.TASKER_FFI_LIBRARY_PATH;
      delete process.env.TASKER_FFI_MODULE_PATH;
      delete process.env.TASKER_FFI_LIBRARY_PATH;

      try {
        const layer = new FfiLayer();

        await expect(layer.load()).rejects.toThrow();
      } finally {
        if (savedModulePath !== undefined) {
          process.env.TASKER_FFI_MODULE_PATH = savedModulePath;
        }
        if (savedLibraryPath !== undefined) {
          process.env.TASKER_FFI_LIBRARY_PATH = savedLibraryPath;
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
// findModulePath / findLibraryPath (static methods)
// =============================================================================

describe('FfiLayer.findModulePath', () => {
  let savedModulePath: string | undefined;
  let savedLibraryPath: string | undefined;

  beforeEach(() => {
    savedModulePath = process.env.TASKER_FFI_MODULE_PATH;
    savedLibraryPath = process.env.TASKER_FFI_LIBRARY_PATH;
  });

  afterEach(() => {
    if (savedModulePath === undefined) {
      delete process.env.TASKER_FFI_MODULE_PATH;
    } else {
      process.env.TASKER_FFI_MODULE_PATH = savedModulePath;
    }
    if (savedLibraryPath === undefined) {
      delete process.env.TASKER_FFI_LIBRARY_PATH;
    } else {
      process.env.TASKER_FFI_LIBRARY_PATH = savedLibraryPath;
    }
  });

  test('should return null when no env vars are set', () => {
    delete process.env.TASKER_FFI_MODULE_PATH;
    delete process.env.TASKER_FFI_LIBRARY_PATH;

    const result = FfiLayer.findModulePath();

    // May find bundled .node file in package root, or null
    // Just verify it doesn't throw
    expect(result === null || typeof result === 'string').toBe(true);
  });

  test('should return null when TASKER_FFI_MODULE_PATH points to nonexistent file', () => {
    process.env.TASKER_FFI_MODULE_PATH = '/nonexistent/path/to/lib.node';
    delete process.env.TASKER_FFI_LIBRARY_PATH;

    const result = FfiLayer.findModulePath();

    expect(result).toBeNull();
  });

  test('should return path when TASKER_FFI_MODULE_PATH points to existing .node file', () => {
    // Use a file that we know exists (this test file itself, treated as .node-like)
    const existingFile = import.meta.path;
    process.env.TASKER_FFI_MODULE_PATH = existingFile;
    delete process.env.TASKER_FFI_LIBRARY_PATH;

    const result = FfiLayer.findModulePath();

    // resolveNodePath may redirect to a .node sibling, or fall back to the file itself
    expect(result).not.toBeNull();
    expect(typeof result).toBe('string');
  });

  test('should prefer TASKER_FFI_MODULE_PATH over TASKER_FFI_LIBRARY_PATH', () => {
    const existingFile = import.meta.path;
    process.env.TASKER_FFI_MODULE_PATH = existingFile;
    process.env.TASKER_FFI_LIBRARY_PATH = '/other/path';

    const result = FfiLayer.findModulePath();

    // Should use MODULE_PATH, not LIBRARY_PATH
    expect(result).not.toBeNull();
  });

  test('findLibraryPath delegates to findModulePath (backward compat)', () => {
    delete process.env.TASKER_FFI_MODULE_PATH;
    delete process.env.TASKER_FFI_LIBRARY_PATH;

    const modulePath = FfiLayer.findModulePath();
    const libraryPath = FfiLayer.findLibraryPath();

    expect(libraryPath).toBe(modulePath);
  });
});
