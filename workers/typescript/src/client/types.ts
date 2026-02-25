/**
 * Client response types matching tasker-shared API response structs.
 *
 * These interfaces mirror the Rust types in `tasker_shared::types::api::orchestration`
 * that are serialized to JSON via `serde_json::to_value()` in the FfiClientBridge.
 * Field names use snake_case because serde's default serialization preserves the
 * Rust field names.
 *
 * Source of truth: tasker-shared/src/types/api/orchestration.rs
 */

// =============================================================================
// Step Readiness (embedded in TaskResponse)
// =============================================================================

/**
 * Step readiness analysis from the orchestration database function.
 *
 * Mirrors: tasker_shared::database::sql_functions::StepReadinessStatus
 */
export interface StepReadinessStatus {
  workflow_step_uuid: string;
  task_uuid: string;
  named_step_uuid: string;
  name: string;
  current_state: string;
  dependencies_satisfied: boolean;
  retry_eligible: boolean;
  ready_for_execution: boolean;
  last_failure_at: string | null;
  next_retry_at: string | null;
  total_parents: number;
  completed_parents: number;
  attempts: number;
  max_attempts: number;
  backoff_request_seconds: number | null;
  last_attempted_at: string | null;
}

// =============================================================================
// Task Responses
// =============================================================================

/**
 * Task details response with execution context and step readiness.
 *
 * Mirrors: tasker_shared::types::api::orchestration::TaskResponse
 */
export interface TaskResponse {
  task_uuid: string;
  name: string;
  namespace: string;
  version: string;
  status: string;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
  context: Record<string, unknown>;
  initiator: string;
  source_system: string;
  reason: string;
  priority: number | null;
  tags: string[] | null;
  correlation_id: string;
  parent_correlation_id: string | null;

  // Execution context
  total_steps: number;
  pending_steps: number;
  in_progress_steps: number;
  completed_steps: number;
  failed_steps: number;
  ready_steps: number;
  execution_status: string;
  recommended_action: string;
  completion_percentage: number;
  health_status: string;

  // Step readiness
  steps: StepReadinessStatus[];
}

/**
 * Pagination metadata for list responses.
 *
 * Mirrors: tasker_shared::models::core::task::PaginationInfo
 */
export interface PaginationInfo {
  total: number;
  limit: number;
  offset: number;
  has_more: boolean;
}

/**
 * Task list response with pagination.
 *
 * Mirrors: tasker_shared::types::api::orchestration::TaskListResponse
 */
export interface TaskListResponse {
  tasks: TaskResponse[];
  pagination: PaginationInfo;
}

/**
 * Cancel task response.
 *
 * Constructed directly in FfiClientBridge::cancel_task as `{ "cancelled": true }`.
 */
export interface CancelTaskResponse {
  cancelled: boolean;
}

// =============================================================================
// Step Responses
// =============================================================================

/**
 * Step details response with readiness information.
 *
 * Mirrors: tasker_shared::types::api::orchestration::StepResponse
 */
export interface StepResponse {
  step_uuid: string;
  task_uuid: string;
  name: string;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
  results: Record<string, unknown> | null;

  // Readiness fields
  current_state: string;
  dependencies_satisfied: boolean;
  retry_eligible: boolean;
  ready_for_execution: boolean;
  total_parents: number;
  completed_parents: number;
  attempts: number;
  max_attempts: number;
  last_failure_at: string | null;
  next_retry_at: string | null;
  last_attempted_at: string | null;
}

/**
 * Step audit record for SOC2-compliant audit trails.
 *
 * Mirrors: tasker_shared::types::api::orchestration::StepAuditResponse
 */
export interface StepAuditResponse {
  audit_uuid: string;
  workflow_step_uuid: string;
  transition_uuid: string;
  task_uuid: string;
  recorded_at: string;
  worker_uuid?: string;
  correlation_id?: string;
  success: boolean;
  execution_time_ms?: number;
  result?: Record<string, unknown>;
  step_name: string;
  from_state?: string;
  to_state: string;
}

// =============================================================================
// Health Response
// =============================================================================

/**
 * Health check response.
 *
 * Constructed directly in FfiClientBridge::health_check as `{ "healthy": true }`.
 */
export interface HealthCheckResponse {
  healthy: boolean;
}
