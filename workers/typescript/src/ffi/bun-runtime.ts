/**
 * Bun FFI runtime adapter using koffi (via Node-API).
 *
 * Bun supports Node-API modules natively, so we use koffi (the same FFI
 * library as NodeRuntime) rather than bun:ffi. This gives us:
 * - Stable, well-tested string/pointer handling
 * - Identical behavior across Node.js and Bun
 * - No manual Buffer→pointer conversion bugs
 *
 * See: https://bun.sh/docs/runtime/node-api
 */

import { NodeRuntime } from './node-runtime.js';

/**
 * Bun FFI runtime implementation using koffi (Node-API).
 *
 * Extends NodeRuntime since both use koffi for FFI. The only difference
 * is the runtime name identifier used for logging and diagnostics.
 */
export class BunRuntime extends NodeRuntime {
  override readonly name = 'bun';
}
