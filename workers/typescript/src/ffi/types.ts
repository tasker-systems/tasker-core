/**
 * FFI type definitions for TypeScript/JavaScript workers.
 *
 * TAS-290: These types match the napi-rs `#[napi(object)]` structs defined in
 * the Rust layer. napi-rs automatically converts snake_case field names to
 * camelCase in JavaScript.
 *
 * Unlike the previous koffi approach, these types represent native JavaScript
 * objects that cross the FFI boundary directly — no JSON serialization.
 */

// =============================================================================
// Step Event Types (from bridge.rs)
// =============================================================================

/**
 * A step event dispatched to the TypeScript handler.
 *
 * This is the primary data structure received when polling for work.
 * All fields are camelCase (napi-rs auto-converts from Rust snake_case).
 */
export interface NapiStepEvent {
  eventId: string;
  taskUuid: string;
  stepUuid: string;
  correlationId: string;
  traceId: string | null;
  spanId: string | null;
  taskCorrelationId: string;
  parentCorrelationId: string | null;
  task: NapiTaskInfo;
  workflowStep: NapiWorkflowStep;
  stepDefinition: NapiStepDefinition;
  dependencyResults: Record<string, NapiDependencyResult>;
}

export interface NapiTaskInfo {
  taskUuid: string;
  namedTaskUuid: string;
  name: string;
  namespace: string;
  version: string;
  context: unknown | null;
  correlationId: string;
  parentCorrelationId: string | null;
  complete: boolean;
  priority: number;
  initiator: string | null;
  sourceSystem: string | null;
  reason: string | null;
  tags: unknown | null;
  identityHash: string;
  createdAt: string;
  updatedAt: string;
  requestedAt: string;
}

export interface NapiWorkflowStep {
  workflowStepUuid: string;
  taskUuid: string;
  namedStepUuid: string;
  name: string;
  templateStepName: string;
  retryable: boolean;
  maxAttempts: number;
  attempts: number;
  inProcess: boolean;
  processed: boolean;
  inputs: unknown | null;
  results: unknown | null;
  backoffRequestSeconds: number | null;
  processedAt: string | null;
  lastAttemptedAt: string | null;
  createdAt: string;
  updatedAt: string;
  checkpoint: unknown | null;
}

export interface NapiStepDefinition {
  name: string;
  description: string | null;
  handlerCallable: string;
  handlerMethod: string | null;
  handlerResolver: string | null;
  handlerInitialization: unknown;
  systemDependency: string | null;
  dependencies: string[];
  timeoutSeconds: number | null;
  retryRetryable: boolean;
  retryMaxAttempts: number;
  retryBackoff: string;
  retryBackoffBaseMs: number | null;
  retryMaxBackoffMs: number | null;
}

export interface NapiDependencyResult {
  stepUuid: string;
  success: boolean;
  result: unknown;
  status: string;
  errorMessage: string | null;
  errorType: string | null;
  errorRetryable: boolean | null;
}

// =============================================================================
// Bootstrap Types (from bridge.rs)
// =============================================================================

export interface BootstrapConfig {
  namespace?: string;
  configPath?: string;
}

export interface BootstrapResult {
  success: boolean;
  status: string;
  message: string;
  workerId: string | null;
}

export interface WorkerStatus {
  success: boolean;
  running: boolean;
  workerId: string | null;
  status: string | null;
  environment: string | null;
}

// =============================================================================
// Dispatch Metrics Types (from bridge.rs)
// =============================================================================

export interface NapiDispatchMetrics {
  pendingCount: number;
  starvationDetected: boolean;
  starvingEventCount: number;
  oldestPendingAgeMs: number | null;
  newestPendingAgeMs: number | null;
  oldestEventId: string | null;
}

// =============================================================================
// Domain Event Types (from bridge.rs)
// =============================================================================

export interface NapiDomainEvent {
  eventId: string;
  eventName: string;
  eventVersion: string;
  metadata: NapiDomainEventMetadata;
  payload: Record<string, unknown>;
}

export interface NapiDomainEventMetadata {
  taskUuid: string;
  stepUuid: string | null;
  stepName: string | null;
  namespace: string;
  correlationId: string;
  firedAt: string;
  firedBy: string | null;
}

// =============================================================================
// Checkpoint Types (TAS-125, from bridge.rs)
// =============================================================================

export interface NapiCheckpointYieldData {
  stepUuid: string;
  cursor: unknown;
  itemsProcessed: number;
  accumulatedResults?: Record<string, unknown>;
}

// =============================================================================
// Client Types (from client_ffi.rs)
// =============================================================================

export interface NapiTaskRequest {
  name: string;
  namespace: string;
  version: string;
  context: unknown;
  initiator: string;
  sourceSystem: string;
  reason: string;
  tags?: string[];
  priority?: number;
  correlationId?: string;
  parentCorrelationId?: string;
  idempotencyKey?: string;
}

export interface NapiClientResult {
  success: boolean;
  data: unknown | null;
  error: string | null;
  recoverable: boolean | null;
}

export interface NapiListTasksParams {
  limit?: number;
  offset?: number;
  namespace?: string;
  status?: string;
}

// =============================================================================
// Compatibility Aliases
// =============================================================================

/** @deprecated Use NapiStepEvent directly */
export type FfiStepEvent = NapiStepEvent;

/** @deprecated Use NapiDispatchMetrics directly */
export type FfiDispatchMetrics = NapiDispatchMetrics;

/** @deprecated Use NapiDomainEvent directly */
export type FfiDomainEvent = NapiDomainEvent;

/** @deprecated Use NapiDomainEventMetadata directly */
export type FfiDomainEventMetadata = NapiDomainEventMetadata;

// =============================================================================
// Runtime Types (not from napi-rs)
// =============================================================================

/**
 * Log fields for structured logging
 */
export interface LogFields {
  [key: string]: string | number | boolean | null;
}

/**
 * Step execution result to send back to Rust (serialized via serde_json)
 */
export interface StepExecutionResult {
  stepUuid: string;
  success: boolean;
  result: Record<string, unknown>;
  status: 'completed' | 'failed' | 'error';
  errorMessage?: string;
  errorType?: string;
  errorRetryable?: boolean;
  errorStatusCode?: number;
  metadata?: StepExecutionMetadata;
}

export interface StepExecutionMetadata {
  executionTimeMs: number;
  workerId?: string;
  handlerName?: string;
  attemptNumber?: number;
  [key: string]: unknown;
}

/**
 * Orchestration metadata for routing
 */
export interface OrchestrationMetadata {
  routingContext?: Record<string, unknown>;
  nextSteps?: string[];
  [key: string]: unknown;
}

/**
 * Checkpoint yield data for batch processing handlers (TAS-125)
 */
export interface CheckpointYieldData {
  stepUuid: string;
  cursor: unknown;
  itemsProcessed: number;
  accumulatedResults?: Record<string, unknown>;
}

/**
 * Stop result from stop_worker
 */
export type StopResult = WorkerStatus;
