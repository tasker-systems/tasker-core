/**
 * Tests for bootstrap types and conversion functions.
 *
 * TAS-290: Updated for napi-rs camelCase FFI types.
 */

import { describe, expect, test } from 'bun:test';
import {
  type BootstrapConfig,
  type FfiBootstrapResult,
  type FfiWorkerStatus,
  fromFfiBootstrapResult,
  fromFfiStopResult,
  fromFfiWorkerStatus,
  toFfiBootstrapConfig,
} from '../../../src/bootstrap/types';

describe('Bootstrap Types', () => {
  describe('toFfiBootstrapConfig', () => {
    test('should return empty object for undefined config', () => {
      const result = toFfiBootstrapConfig(undefined);
      expect(result).toEqual({});
    });

    test('should convert TypeScript config to FFI format', () => {
      const config: BootstrapConfig = {
        workerId: 'worker-1',
        namespace: 'payments',
        configPath: '/path/to/config.toml',
        logLevel: 'debug',
        databaseUrl: 'postgresql://localhost/test',
      };

      const result = toFfiBootstrapConfig(config);

      // napi-rs FFI only takes namespace and configPath
      expect(result.namespace).toBe('payments');
      expect(result.configPath).toBe('/path/to/config.toml');
    });

    test('should handle partial config', () => {
      const config: BootstrapConfig = {
        namespace: 'default',
      };

      const result = toFfiBootstrapConfig(config);

      expect(result.namespace).toBe('default');
    });

    test('should handle empty config', () => {
      const config: BootstrapConfig = {};
      const result = toFfiBootstrapConfig(config);
      expect(result).toBeDefined();
    });
  });

  describe('fromFfiBootstrapResult', () => {
    test('should convert successful FFI result', () => {
      const ffiResult: FfiBootstrapResult = {
        success: true,
        status: 'started',
        message: 'Worker started successfully',
        workerId: 'worker-123',
      };

      const result = fromFfiBootstrapResult(ffiResult);

      expect(result.success).toBe(true);
      expect(result.status).toBe('started');
      expect(result.message).toBe('Worker started successfully');
      expect(result.workerId).toBe('worker-123');
      expect(result.error).toBeUndefined();
    });

    test('should convert failed FFI result', () => {
      const ffiResult: FfiBootstrapResult = {
        success: false,
        status: 'error',
        message: 'Database connection failed',
        workerId: null,
      };

      const result = fromFfiBootstrapResult(ffiResult);

      expect(result.success).toBe(false);
      expect(result.status).toBe('error');
      expect(result.message).toBe('Database connection failed');
    });

    test('should convert already_running FFI result', () => {
      const ffiResult: FfiBootstrapResult = {
        success: true,
        status: 'already_running',
        message: 'Worker is already running',
        workerId: 'existing-worker',
      };

      const result = fromFfiBootstrapResult(ffiResult);

      expect(result.success).toBe(true);
      expect(result.status).toBe('already_running');
      expect(result.workerId).toBe('existing-worker');
    });
  });

  describe('fromFfiWorkerStatus', () => {
    test('should convert running worker status', () => {
      const ffiStatus: FfiWorkerStatus = {
        success: true,
        running: true,
        workerId: 'worker-123',
        status: 'healthy',
        environment: 'production',
      };

      const result = fromFfiWorkerStatus(ffiStatus);

      expect(result.success).toBe(true);
      expect(result.running).toBe(true);
      expect(result.workerId).toBe('worker-123');
      expect(result.environment).toBe('production');
    });

    test('should convert stopped worker status', () => {
      const ffiStatus: FfiWorkerStatus = {
        success: true,
        running: false,
        status: 'stopped',
        workerId: null,
        environment: null,
      };

      const result = fromFfiWorkerStatus(ffiStatus);

      expect(result.success).toBe(true);
      expect(result.running).toBe(false);
      expect(result.status).toBe('stopped');
    });

    test('should handle minimal status', () => {
      const ffiStatus: FfiWorkerStatus = {
        success: false,
        running: false,
        workerId: null,
        status: null,
        environment: null,
      };

      const result = fromFfiWorkerStatus(ffiStatus);

      expect(result.success).toBe(false);
      expect(result.running).toBe(false);
      expect(result.workerId).toBeUndefined();
    });
  });

  describe('fromFfiStopResult', () => {
    test('should convert stopped result', () => {
      const ffiResult: FfiWorkerStatus = {
        success: true,
        running: false,
        status: 'Worker stopped successfully',
        workerId: 'worker-123',
        environment: null,
      };

      const result = fromFfiStopResult(ffiResult);

      expect(result.success).toBe(true);
      expect(result.status).toBe('not_running');
      expect(result.workerId).toBe('worker-123');
    });

    test('should convert still-running stop result', () => {
      const ffiResult: FfiWorkerStatus = {
        success: true,
        running: true,
        status: 'stopped',
        workerId: 'worker-123',
        environment: null,
      };

      const result = fromFfiStopResult(ffiResult);

      expect(result.success).toBe(true);
      expect(result.status).toBe('stopped');
    });
  });
});

describe('Bootstrap Config Type', () => {
  test('should accept all valid log levels', () => {
    const levels: Array<'trace' | 'debug' | 'info' | 'warn' | 'error'> = [
      'trace',
      'debug',
      'info',
      'warn',
      'error',
    ];

    for (const level of levels) {
      const config: BootstrapConfig = { logLevel: level };
      expect(config.logLevel).toBe(level);
    }
  });

  test('should allow all fields to be optional', () => {
    const config: BootstrapConfig = {};
    expect(config.workerId).toBeUndefined();
    expect(config.namespace).toBeUndefined();
    expect(config.configPath).toBeUndefined();
    expect(config.logLevel).toBeUndefined();
    expect(config.databaseUrl).toBeUndefined();
  });
});
