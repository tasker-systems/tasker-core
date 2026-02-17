/**
 * Tasker TypeScript Worker
 *
 * FFI-based worker for tasker-core supporting Bun, Node.js, and Deno runtimes.
 *
 * @packageDocumentation
 */

// =============================================================================
// Bootstrap module (TAS-104) - Worker lifecycle management
// =============================================================================
export {
  bootstrapWorker,
  getRustVersion,
  getVersion,
  getWorkerStatus,
  healthCheck,
  isWorkerRunning,
  stopWorker,
  transitionToGracefulShutdown,
} from './bootstrap/index.js';

// Export bootstrap types (user-friendly camelCase versions)
export type {
  BootstrapConfig,
  BootstrapResult,
  StopResult,
  WorkerStatus,
} from './bootstrap/types.js';
// =============================================================================
// Client module (TAS-231) - High-level orchestration API client
// =============================================================================
export {
  type CreateTaskOptions,
  type ListTasksOptions,
  TaskerClient,
  TaskerClientError,
} from './client/index.js';
// =============================================================================
// Events module
// =============================================================================
export * from './events/index.js';
// =============================================================================
// FFI module - napi-rs native module
// =============================================================================
export { FfiLayer, type FfiLayerConfig, type NapiModule } from './ffi/index.js';

// Export FFI types under Ffi prefix to avoid conflicts with bootstrap types
export type {
  BootstrapConfig as FfiBootstrapConfig,
  BootstrapResult as FfiBootstrapResult,
  FfiDispatchMetrics,
  FfiDomainEvent,
  FfiStepEvent,
  LogFields as FfiLogFields,
  NapiCheckpointYieldData,
  NapiClientResult,
  NapiDependencyResult,
  NapiDispatchMetrics,
  NapiDomainEvent,
  NapiDomainEventMetadata,
  NapiListTasksParams,
  NapiStepDefinition,
  NapiStepEvent,
  NapiTaskInfo,
  NapiTaskRequest,
  NapiWorkflowStep,
  OrchestrationMetadata,
  StepExecutionMetadata,
  StepExecutionResult,
  StopResult as FfiStopResult,
  WorkerStatus as FfiWorkerStatus,
} from './ffi/types.js';
// =============================================================================
// Handler module (TAS-102/103)
// =============================================================================
export * from './handler/index.js';
// =============================================================================
// Logging module (TAS-104)
// =============================================================================
export {
  createLogger,
  type LogFields,
  logDebug,
  logError,
  logInfo,
  logTrace,
  logWarn,
} from './logging/index.js';
// =============================================================================
// Registry module (TAS-93) - Step handler resolver chain infrastructure
// =============================================================================
export * from './registry/index.js';
// =============================================================================
// Server module (TAS-104)
// =============================================================================
export {
  ShutdownController,
  type ShutdownHandler,
  WorkerServer,
} from './server/index.js';
// Export server types under Server prefix to avoid conflicts with bootstrap
export type {
  HealthCheckResult as ServerHealthCheckResult,
  ServerComponents,
  ServerState,
  ServerStatus,
  WorkerServerConfig,
} from './server/types.js';
// =============================================================================
// Subscriber module (TAS-104)
// =============================================================================
export * from './subscriber/index.js';

// =============================================================================
// Types module (TAS-102)
// =============================================================================
export * from './types/index.js';
