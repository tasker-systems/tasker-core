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
  StopResult,
  WorkerStatus,
} from './types.js';
