/**
 * FFI module for TypeScript/JavaScript workers.
 *
 * TAS-290: Simplified to napi-rs native module loading.
 * No more multi-runtime abstraction (koffi/DenoRuntime/NodeRuntime).
 */

// FfiLayer - primary API for FFI management
export { FfiLayer, type FfiLayerConfig, type NapiModule } from './ffi-layer.js';

// FFI types
export type {
  BootstrapConfig,
  BootstrapResult,
  CheckpointYieldData,
  FfiDispatchMetrics,
  FfiDomainEvent,
  FfiDomainEventMetadata,
  FfiStepEvent,
  LogFields,
  NapiBackoffHint,
  NapiCheckpointYieldData,
  NapiClientResult,
  NapiDependencyResult,
  NapiDispatchMetrics,
  NapiDomainEvent,
  NapiDomainEventMetadata,
  NapiListTasksParams,
  NapiOrchestrationMetadata,
  NapiStepDefinition,
  NapiStepEvent,
  NapiStepExecutionError,
  NapiStepExecutionMetadata,
  NapiStepExecutionResult,
  NapiTaskInfo,
  NapiTaskRequest,
  NapiWorkflowStep,
  StepExecutionResult,
  StopResult,
  WorkerStatus,
} from './types.js';
