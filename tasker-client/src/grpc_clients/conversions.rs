//! Type conversions from Protocol Buffer types to domain types.
//!
//! This module provides conversion functions for transforming gRPC proto response
//! types back to the domain types used by the REST API. This is the inverse of
//! the conversions in `tasker_shared::proto::conversions`.
//!
//! # Design Philosophy
//!
//! The gRPC clients should return the exact same domain types as the REST clients.
//! This ensures consistent behavior regardless of transport protocol and allows
//! the client library to be transport-agnostic from the caller's perspective.
//!
//! We use helper functions rather than trait implementations due to Rust's orphan
//! rules - we cannot implement `From` for types defined in other crates.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use tasker_shared::proto::v1 as proto;

use crate::error::ClientError;

// ============================================================================
// Timestamp Conversions
// ============================================================================

/// Convert an optional protobuf Timestamp to DateTime<Utc>.
pub fn proto_timestamp_to_datetime(ts: Option<prost_types::Timestamp>) -> DateTime<Utc> {
    ts.and_then(|t| DateTime::from_timestamp(t.seconds, t.nanos as u32))
        .unwrap_or_default()
}

/// Convert an optional protobuf Timestamp to an optional DateTime<Utc>.
pub fn proto_timestamp_to_datetime_opt(
    ts: Option<prost_types::Timestamp>,
) -> Option<DateTime<Utc>> {
    ts.and_then(|t| DateTime::from_timestamp(t.seconds, t.nanos as u32))
}

/// Convert an optional protobuf Timestamp to an RFC3339 string.
pub fn proto_timestamp_to_string(ts: Option<prost_types::Timestamp>) -> String {
    proto_timestamp_to_datetime(ts).to_rfc3339()
}

/// Convert an optional protobuf Timestamp to an optional RFC3339 string.
pub fn proto_timestamp_to_string_opt(ts: Option<prost_types::Timestamp>) -> Option<String> {
    proto_timestamp_to_datetime_opt(ts).map(|dt| dt.to_rfc3339())
}

// ============================================================================
// JSON/Struct Conversions
// ============================================================================

/// Convert optional prost_types::Struct to serde_json::Value.
pub fn proto_struct_to_json_opt(s: Option<prost_types::Struct>) -> serde_json::Value {
    s.map(proto_struct_to_json)
        .unwrap_or(serde_json::Value::Null)
}

/// Convert prost_types::Struct to serde_json::Value.
pub fn proto_struct_to_json(s: prost_types::Struct) -> serde_json::Value {
    serde_json::Value::Object(
        s.fields
            .into_iter()
            .map(|(k, v)| (k, prost_value_to_json(v)))
            .collect(),
    )
}

fn prost_value_to_json(value: prost_types::Value) -> serde_json::Value {
    use prost_types::value::Kind;
    match value.kind {
        Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(b),
        Some(Kind::NumberValue(n)) => serde_json::Number::from_f64(n)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s),
        Some(Kind::ListValue(l)) => {
            serde_json::Value::Array(l.values.into_iter().map(prost_value_to_json).collect())
        }
        Some(Kind::StructValue(s)) => proto_struct_to_json(s),
        None => serde_json::Value::Null,
    }
}

// ============================================================================
// State String Conversions
// ============================================================================

/// Convert proto TaskState enum to domain string representation.
pub fn proto_task_state_to_string(state: i32) -> String {
    use tasker_shared::state_machine::states::TaskState;

    match proto::TaskState::try_from(state) {
        Ok(proto::TaskState::Pending) => TaskState::Pending.as_str().to_string(),
        Ok(proto::TaskState::Initializing) => TaskState::Initializing.as_str().to_string(),
        Ok(proto::TaskState::EnqueuingSteps) => TaskState::EnqueuingSteps.as_str().to_string(),
        Ok(proto::TaskState::StepsInProcess) => TaskState::StepsInProcess.as_str().to_string(),
        Ok(proto::TaskState::EvaluatingResults) => {
            TaskState::EvaluatingResults.as_str().to_string()
        }
        Ok(proto::TaskState::WaitingForDependencies) => {
            TaskState::WaitingForDependencies.as_str().to_string()
        }
        Ok(proto::TaskState::WaitingForRetry) => TaskState::WaitingForRetry.as_str().to_string(),
        Ok(proto::TaskState::BlockedByFailures) => {
            TaskState::BlockedByFailures.as_str().to_string()
        }
        Ok(proto::TaskState::Complete) => TaskState::Complete.as_str().to_string(),
        Ok(proto::TaskState::Error) => TaskState::Error.as_str().to_string(),
        Ok(proto::TaskState::Cancelled) => TaskState::Cancelled.as_str().to_string(),
        Ok(proto::TaskState::ResolvedManually) => TaskState::ResolvedManually.as_str().to_string(),
        Ok(proto::TaskState::Unspecified) | Err(_) => "unspecified".to_string(),
    }
}

/// Convert proto StepState enum to domain string representation.
pub fn proto_step_state_to_string(state: i32) -> String {
    use tasker_shared::state_machine::states::WorkflowStepState;

    match proto::StepState::try_from(state) {
        Ok(proto::StepState::Pending) => WorkflowStepState::Pending.as_str().to_string(),
        Ok(proto::StepState::Enqueued) => WorkflowStepState::Enqueued.as_str().to_string(),
        Ok(proto::StepState::InProgress) => WorkflowStepState::InProgress.as_str().to_string(),
        Ok(proto::StepState::EnqueuedForOrchestration) => {
            WorkflowStepState::EnqueuedForOrchestration
                .as_str()
                .to_string()
        }
        Ok(proto::StepState::EnqueuedAsErrorForOrchestration) => {
            WorkflowStepState::EnqueuedAsErrorForOrchestration
                .as_str()
                .to_string()
        }
        Ok(proto::StepState::WaitingForRetry) => {
            WorkflowStepState::WaitingForRetry.as_str().to_string()
        }
        Ok(proto::StepState::Complete) => WorkflowStepState::Complete.as_str().to_string(),
        Ok(proto::StepState::Error) => WorkflowStepState::Error.as_str().to_string(),
        Ok(proto::StepState::Cancelled) => WorkflowStepState::Cancelled.as_str().to_string(),
        Ok(proto::StepState::ResolvedManually) => {
            WorkflowStepState::ResolvedManually.as_str().to_string()
        }
        Ok(proto::StepState::Unspecified) | Err(_) => "unspecified".to_string(),
    }
}

// ============================================================================
// Task Response Conversions
// ============================================================================

use tasker_shared::models::core::task::PaginationInfo;
use tasker_shared::types::api::orchestration::{
    StepAuditResponse, StepResponse, TaskListResponse, TaskResponse,
};

/// Convert proto Task to domain TaskResponse.
pub fn proto_task_to_domain(task: proto::Task) -> Result<TaskResponse, ClientError> {
    let correlation_id = uuid::Uuid::parse_str(&task.correlation_id)
        .map_err(|e| ClientError::Internal(format!("Invalid correlation_id: {e}")))?;

    let parent_correlation_id = task
        .parent_correlation_id
        .as_ref()
        .filter(|s| !s.is_empty())
        .map(|s| uuid::Uuid::parse_str(s))
        .transpose()
        .map_err(|e| ClientError::Internal(format!("Invalid parent_correlation_id: {e}")))?;

    Ok(TaskResponse {
        task_uuid: task.task_uuid,
        name: task.name,
        namespace: task.namespace,
        version: task.version,
        status: proto_task_state_to_string(task.state),
        created_at: proto_timestamp_to_datetime(task.created_at),
        updated_at: proto_timestamp_to_datetime(task.updated_at),
        completed_at: proto_timestamp_to_datetime_opt(task.completed_at),
        context: proto_struct_to_json_opt(task.context),
        initiator: task.initiator,
        source_system: task.source_system,
        reason: task.reason,
        priority: task.priority,
        tags: if task.tags.is_empty() {
            None
        } else {
            Some(task.tags)
        },
        correlation_id,
        parent_correlation_id,
        total_steps: task.total_steps,
        pending_steps: task.pending_steps,
        in_progress_steps: task.in_progress_steps,
        completed_steps: task.completed_steps,
        failed_steps: task.failed_steps,
        ready_steps: task.ready_steps,
        execution_status: task.execution_status,
        recommended_action: task.recommended_action,
        completion_percentage: task.completion_percentage,
        health_status: task.health_status,
        steps: vec![], // Steps are fetched separately via GetSteps
    })
}

/// Convert proto GetTaskResponse to domain TaskResponse.
pub fn proto_get_task_response_to_domain(
    response: proto::GetTaskResponse,
) -> Result<TaskResponse, ClientError> {
    response
        .task
        .ok_or_else(|| ClientError::Internal("Server returned empty task".to_string()))
        .and_then(proto_task_to_domain)
}

/// Convert proto CreateTaskResponse to domain TaskResponse.
pub fn proto_create_task_response_to_domain(
    response: proto::CreateTaskResponse,
) -> Result<TaskResponse, ClientError> {
    response
        .task
        .ok_or_else(|| {
            ClientError::Internal("Server returned empty task in create response".to_string())
        })
        .and_then(proto_task_to_domain)
}

/// Convert proto ListTasksResponse to domain TaskListResponse.
pub fn proto_list_tasks_response_to_domain(
    response: proto::ListTasksResponse,
) -> Result<TaskListResponse, ClientError> {
    let tasks: Result<Vec<TaskResponse>, ClientError> = response
        .tasks
        .into_iter()
        .map(proto_task_to_domain)
        .collect();

    let pagination = response.pagination.unwrap_or_default();
    let total = pagination.total as u64;
    let per_page = pagination.count.max(1) as u32;
    let offset = pagination.offset as u64;
    let page = if per_page > 0 {
        (offset / per_page as u64) as u32 + 1
    } else {
        1
    };
    let total_pages = if per_page > 0 {
        total.div_ceil(per_page as u64) as u32
    } else {
        1
    };

    Ok(TaskListResponse {
        tasks: tasks?,
        pagination: PaginationInfo {
            page,
            per_page,
            total_count: total,
            total_pages,
            has_next: pagination.has_more,
            has_previous: page > 1,
        },
    })
}

// ============================================================================
// Step Response Conversions
// ============================================================================

/// Convert proto Step to domain StepResponse.
pub fn proto_step_to_domain(step: proto::Step) -> Result<StepResponse, ClientError> {
    Ok(StepResponse {
        step_uuid: step.step_uuid,
        task_uuid: step.task_uuid,
        name: step.name,
        created_at: proto_timestamp_to_string(step.created_at),
        updated_at: proto_timestamp_to_string(step.updated_at),
        completed_at: proto_timestamp_to_string_opt(step.completed_at),
        results: step.results.map(proto_struct_to_json),
        current_state: proto_step_state_to_string(step.state),
        dependencies_satisfied: step.dependencies_satisfied,
        retry_eligible: step.retry_eligible,
        ready_for_execution: step.ready_for_execution,
        total_parents: step.total_parents,
        completed_parents: step.completed_parents,
        attempts: step.attempts,
        max_attempts: step.max_attempts,
        last_failure_at: proto_timestamp_to_string_opt(step.last_failure_at),
        next_retry_at: proto_timestamp_to_string_opt(step.next_retry_at),
        last_attempted_at: proto_timestamp_to_string_opt(step.last_attempted_at),
    })
}

/// Convert proto GetStepResponse to domain StepResponse.
pub fn proto_get_step_response_to_domain(
    response: proto::GetStepResponse,
) -> Result<StepResponse, ClientError> {
    response
        .step
        .ok_or_else(|| ClientError::Internal("Server returned empty step".to_string()))
        .and_then(proto_step_to_domain)
}

/// Convert proto StepAuditRecord to domain StepAuditResponse.
pub fn proto_audit_to_domain(
    record: proto::StepAuditRecord,
) -> Result<StepAuditResponse, ClientError> {
    Ok(StepAuditResponse {
        audit_uuid: record.audit_uuid,
        workflow_step_uuid: record.step_uuid,
        transition_uuid: record.transition_uuid,
        task_uuid: record.task_uuid,
        recorded_at: proto_timestamp_to_string(record.recorded_at),
        worker_uuid: record.worker_uuid,
        correlation_id: record.correlation_id,
        success: record.success,
        execution_time_ms: record.execution_time_ms,
        result: record.result.map(proto_struct_to_json),
        step_name: record.step_name,
        from_state: record.from_state.map(proto_step_state_to_string),
        to_state: proto_step_state_to_string(record.to_state),
    })
}

// ============================================================================
// Health Response Conversions (Orchestration)
// ============================================================================

use tasker_shared::types::api::health::{PoolDetail, PoolUtilizationInfo};
use tasker_shared::types::api::orchestration::{
    DetailedHealthChecks, DetailedHealthResponse, HealthCheck, HealthInfo, HealthResponse,
    ReadinessChecks, ReadinessResponse,
};

/// Convert proto HealthResponse to domain.
pub fn proto_health_response_to_domain(response: proto::HealthResponse) -> HealthResponse {
    HealthResponse {
        status: response.status,
        timestamp: response.timestamp,
    }
}

/// Convert proto ReadinessResponse to domain.
///
/// # Errors
/// Returns `InvalidResponse` if required fields (checks, info) are missing.
pub fn proto_readiness_response_to_domain(
    response: proto::ReadinessResponse,
) -> Result<ReadinessResponse, ClientError> {
    let checks = response.checks.ok_or_else(|| {
        ClientError::invalid_response(
            "ReadinessResponse.checks",
            "Readiness response missing required health checks",
        )
    })?;
    let checks = proto_readiness_checks_to_domain(checks)?;

    let info = response.info.ok_or_else(|| {
        ClientError::invalid_response(
            "ReadinessResponse.info",
            "Readiness response missing required health info",
        )
    })?;
    let info = proto_health_info_to_domain(info)?;

    Ok(ReadinessResponse {
        status: response.status,
        timestamp: response.timestamp,
        checks,
        info,
    })
}

/// Convert proto DetailedHealthResponse to domain.
///
/// # Errors
/// Returns `InvalidResponse` if required fields (checks, info) are missing.
pub fn proto_detailed_health_response_to_domain(
    response: proto::DetailedHealthResponse,
) -> Result<DetailedHealthResponse, ClientError> {
    let checks = response.checks.ok_or_else(|| {
        ClientError::invalid_response(
            "DetailedHealthResponse.checks",
            "Detailed health response missing required health checks",
        )
    })?;
    let checks = proto_detailed_checks_to_domain(checks)?;

    let info = response.info.ok_or_else(|| {
        ClientError::invalid_response(
            "DetailedHealthResponse.info",
            "Detailed health response missing required health info",
        )
    })?;
    let info = proto_health_info_to_domain(info)?;

    Ok(DetailedHealthResponse {
        status: response.status,
        timestamp: response.timestamp,
        checks,
        info,
    })
}

fn proto_readiness_checks_to_domain(
    checks: proto::ReadinessChecks,
) -> Result<ReadinessChecks, ClientError> {
    Ok(ReadinessChecks {
        web_database: checks
            .web_database
            .map(proto_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("checks.web_database", "missing"))?,
        orchestration_database: checks
            .orchestration_database
            .map(proto_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("checks.orchestration_database", "missing")
            })?,
        circuit_breaker: checks
            .circuit_breaker
            .map(proto_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("checks.circuit_breaker", "missing"))?,
        orchestration_system: checks
            .orchestration_system
            .map(proto_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("checks.orchestration_system", "missing")
            })?,
        command_processor: checks
            .command_processor
            .map(proto_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("checks.command_processor", "missing"))?,
    })
}

fn proto_detailed_checks_to_domain(
    checks: proto::DetailedHealthChecks,
) -> Result<DetailedHealthChecks, ClientError> {
    Ok(DetailedHealthChecks {
        web_database: checks
            .web_database
            .map(proto_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("checks.web_database", "missing"))?,
        orchestration_database: checks
            .orchestration_database
            .map(proto_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("checks.orchestration_database", "missing")
            })?,
        circuit_breaker: checks
            .circuit_breaker
            .map(proto_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("checks.circuit_breaker", "missing"))?,
        orchestration_system: checks
            .orchestration_system
            .map(proto_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("checks.orchestration_system", "missing")
            })?,
        command_processor: checks
            .command_processor
            .map(proto_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("checks.command_processor", "missing"))?,
        pool_utilization: checks
            .pool_utilization
            .map(proto_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("checks.pool_utilization", "missing"))?,
        queue_depth: checks
            .queue_depth
            .map(proto_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("checks.queue_depth", "missing"))?,
        channel_saturation: checks
            .channel_saturation
            .map(proto_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("checks.channel_saturation", "missing"))?,
    })
}

fn proto_health_check_to_domain(check: proto::HealthCheck) -> HealthCheck {
    HealthCheck {
        status: check.status,
        message: check.message,
        duration_ms: check.duration_ms,
    }
}

fn proto_health_info_to_domain(info: proto::HealthInfo) -> Result<HealthInfo, ClientError> {
    // pool_utilization is optional - only present when pool stats are available
    let pool_utilization = info
        .pool_utilization
        .map(proto_pool_utilization_to_domain)
        .transpose()?;

    Ok(HealthInfo {
        version: info.version,
        environment: info.environment,
        operational_state: info.operational_state,
        web_database_pool_size: info.web_database_pool_size,
        orchestration_database_pool_size: info.orchestration_database_pool_size,
        circuit_breaker_state: info.circuit_breaker_state,
        pool_utilization,
    })
}

fn proto_pool_utilization_to_domain(
    info: proto::PoolUtilizationInfo,
) -> Result<PoolUtilizationInfo, ClientError> {
    Ok(PoolUtilizationInfo {
        tasker_pool: info
            .tasker_pool
            .map(proto_pool_detail_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response(
                    "PoolUtilizationInfo.tasker_pool",
                    "Pool utilization missing tasker pool details",
                )
            })?,
        pgmq_pool: info
            .pgmq_pool
            .map(proto_pool_detail_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response(
                    "PoolUtilizationInfo.pgmq_pool",
                    "Pool utilization missing pgmq pool details",
                )
            })?,
    })
}

fn proto_pool_detail_to_domain(pool: proto::PoolDetail) -> PoolDetail {
    PoolDetail {
        active_connections: pool.active_connections,
        idle_connections: pool.idle_connections,
        max_connections: pool.max_connections,
        utilization_percent: pool.utilization_percent,
        total_acquires: pool.total_acquires,
        slow_acquires: pool.slow_acquires,
        acquire_errors: pool.acquire_errors,
        average_acquire_time_ms: pool.average_acquire_time_ms,
        max_acquire_time_ms: pool.max_acquire_time_ms,
    }
}

// ============================================================================
// Worker Health Response Conversions
// ============================================================================

use tasker_shared::types::api::worker::{
    BasicHealthResponse as WorkerBasicHealth, DetailedHealthResponse as WorkerDetailedHealth,
    DistributedCacheInfo, HealthCheck as WorkerHealthCheck, ReadinessResponse as WorkerReadiness,
    WorkerDetailedChecks, WorkerReadinessChecks, WorkerSystemInfo,
};

/// Convert proto WorkerBasicHealthResponse to domain.
pub fn proto_worker_basic_health_to_domain(
    response: proto::WorkerBasicHealthResponse,
) -> WorkerBasicHealth {
    WorkerBasicHealth {
        status: response.status,
        timestamp: proto_timestamp_to_datetime(response.timestamp),
        worker_id: response.worker_id,
    }
}

/// Convert proto WorkerReadinessResponse to domain.
///
/// # Errors
/// Returns `InvalidResponse` if required fields (checks, system_info) are missing.
pub fn proto_worker_readiness_to_domain(
    response: proto::WorkerReadinessResponse,
) -> Result<WorkerReadiness, ClientError> {
    let checks = response.checks.ok_or_else(|| {
        ClientError::invalid_response(
            "WorkerReadinessResponse.checks",
            "Worker readiness response missing required health checks",
        )
    })?;
    let checks = proto_worker_readiness_checks_to_domain(checks)?;

    let system_info = response.system_info.ok_or_else(|| {
        ClientError::invalid_response(
            "WorkerReadinessResponse.system_info",
            "Worker readiness response missing required system info",
        )
    })?;
    let system_info = proto_worker_system_info_to_domain(system_info)?;

    Ok(WorkerReadiness {
        status: response.status,
        timestamp: proto_timestamp_to_datetime(response.timestamp),
        worker_id: response.worker_id,
        checks,
        system_info,
    })
}

/// Convert proto WorkerDetailedHealthResponse to domain.
///
/// # Errors
/// Returns `InvalidResponse` if required fields (checks, system_info) are missing.
pub fn proto_worker_detailed_health_to_domain(
    response: proto::WorkerDetailedHealthResponse,
) -> Result<WorkerDetailedHealth, ClientError> {
    let checks = response.checks.ok_or_else(|| {
        ClientError::invalid_response(
            "WorkerDetailedHealthResponse.checks",
            "Worker detailed health response missing required health checks",
        )
    })?;
    let checks = proto_worker_detailed_checks_to_domain(checks)?;

    let system_info = response.system_info.ok_or_else(|| {
        ClientError::invalid_response(
            "WorkerDetailedHealthResponse.system_info",
            "Worker detailed health response missing required system info",
        )
    })?;
    let system_info = proto_worker_system_info_to_domain(system_info)?;

    Ok(WorkerDetailedHealth {
        status: response.status,
        timestamp: proto_timestamp_to_datetime(response.timestamp),
        worker_id: response.worker_id,
        checks,
        system_info,
        // distributed_cache is optional - may not be configured
        distributed_cache: response
            .distributed_cache
            .map(proto_distributed_cache_to_domain),
    })
}

fn proto_worker_readiness_checks_to_domain(
    checks: proto::WorkerReadinessChecks,
) -> Result<WorkerReadinessChecks, ClientError> {
    Ok(WorkerReadinessChecks {
        database: checks
            .database
            .map(proto_worker_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("worker_checks.database", "missing"))?,
        command_processor: checks
            .command_processor
            .map(proto_worker_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("worker_checks.command_processor", "missing")
            })?,
        queue_processing: checks
            .queue_processing
            .map(proto_worker_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("worker_checks.queue_processing", "missing")
            })?,
    })
}

fn proto_worker_detailed_checks_to_domain(
    checks: proto::WorkerDetailedChecks,
) -> Result<WorkerDetailedChecks, ClientError> {
    Ok(WorkerDetailedChecks {
        database: checks
            .database
            .map(proto_worker_health_check_to_domain)
            .ok_or_else(|| ClientError::invalid_response("worker_checks.database", "missing"))?,
        command_processor: checks
            .command_processor
            .map(proto_worker_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("worker_checks.command_processor", "missing")
            })?,
        queue_processing: checks
            .queue_processing
            .map(proto_worker_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("worker_checks.queue_processing", "missing")
            })?,
        event_system: checks
            .event_system
            .map(proto_worker_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("worker_checks.event_system", "missing")
            })?,
        step_processing: checks
            .step_processing
            .map(proto_worker_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("worker_checks.step_processing", "missing")
            })?,
        circuit_breakers: checks
            .circuit_breakers
            .map(proto_worker_health_check_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response("worker_checks.circuit_breakers", "missing")
            })?,
    })
}

fn proto_worker_health_check_to_domain(check: proto::WorkerHealthCheck) -> WorkerHealthCheck {
    WorkerHealthCheck {
        status: check.status,
        message: check.message,
        duration_ms: check.duration_ms,
        last_checked: proto_timestamp_to_datetime(check.last_checked),
    }
}

fn proto_worker_system_info_to_domain(
    info: proto::WorkerSystemInfo,
) -> Result<WorkerSystemInfo, ClientError> {
    // pool_utilization is optional - only present when pool stats are available
    let pool_utilization = info
        .pool_utilization
        .map(proto_worker_pool_utilization_to_domain)
        .transpose()?;

    Ok(WorkerSystemInfo {
        version: info.version,
        environment: info.environment,
        uptime_seconds: info.uptime_seconds,
        worker_type: info.worker_type,
        database_pool_size: info.database_pool_size,
        command_processor_active: info.command_processor_active,
        supported_namespaces: info.supported_namespaces,
        pool_utilization,
    })
}

fn proto_worker_pool_utilization_to_domain(
    p: proto::WorkerPoolUtilizationInfo,
) -> Result<PoolUtilizationInfo, ClientError> {
    Ok(PoolUtilizationInfo {
        tasker_pool: p
            .tasker_pool
            .map(proto_worker_pool_detail_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response(
                    "WorkerPoolUtilizationInfo.tasker_pool",
                    "Worker pool utilization missing tasker pool details",
                )
            })?,
        pgmq_pool: p
            .pgmq_pool
            .map(proto_worker_pool_detail_to_domain)
            .ok_or_else(|| {
                ClientError::invalid_response(
                    "WorkerPoolUtilizationInfo.pgmq_pool",
                    "Worker pool utilization missing pgmq pool details",
                )
            })?,
    })
}

fn proto_worker_pool_detail_to_domain(pool: proto::WorkerPoolDetail) -> PoolDetail {
    PoolDetail {
        active_connections: pool.active_connections,
        idle_connections: pool.idle_connections,
        max_connections: pool.max_connections,
        utilization_percent: pool.utilization_percent,
        total_acquires: pool.total_acquires,
        slow_acquires: pool.slow_acquires,
        acquire_errors: pool.acquire_errors,
        average_acquire_time_ms: pool.average_acquire_time_ms,
        max_acquire_time_ms: pool.max_acquire_time_ms,
    }
}

fn proto_distributed_cache_to_domain(info: proto::DistributedCacheInfo) -> DistributedCacheInfo {
    DistributedCacheInfo {
        enabled: info.enabled,
        provider: info.provider,
        healthy: info.healthy,
    }
}

// ============================================================================
// Worker Template Response Conversions
// ============================================================================

use tasker_shared::models::core::task_template::{
    HandlerDefinition, ResolvedTaskTemplate, RetryConfiguration, StepDefinition,
    SystemDependencies, TaskTemplate,
};
use tasker_shared::types::api::worker::{
    TemplateListResponse as WorkerTemplateList, TemplateResponse as WorkerTemplateResponse,
};
use tasker_shared::types::base::{CacheStats, HandlerMetadata};

/// Convert proto WorkerTemplateListResponse to domain.
pub fn proto_worker_template_list_to_domain(
    response: proto::WorkerTemplateListResponse,
) -> WorkerTemplateList {
    WorkerTemplateList {
        supported_namespaces: response.supported_namespaces.clone(),
        template_count: response.template_count as usize,
        cache_stats: response.cache_stats.map(|c| CacheStats {
            total_cached: c.total_cached as usize,
            cache_hits: c.cache_hits,
            cache_misses: c.cache_misses,
            cache_evictions: c.cache_evictions,
            oldest_entry_age_seconds: c.oldest_entry_age_seconds,
            average_access_count: c.average_access_count,
            supported_namespaces: c.supported_namespaces,
        }),
        worker_capabilities: response.worker_capabilities,
    }
}

/// Convert proto WorkerTemplateResponse to domain.
pub fn proto_worker_template_to_domain(
    response: proto::WorkerTemplateResponse,
) -> Result<WorkerTemplateResponse, ClientError> {
    let proto_template = response
        .template
        .ok_or_else(|| ClientError::Internal("Server returned empty template".to_string()))?;
    let handler_meta = response.handler_metadata.unwrap_or_default();

    let steps: Vec<StepDefinition> = proto_template
        .steps
        .into_iter()
        .map(|s| StepDefinition {
            name: s.name,
            description: s.description,
            handler: HandlerDefinition {
                callable: String::new(),
                method: None,
                resolver: None,
                initialization: HashMap::new(),
            },
            step_type: Default::default(),
            system_dependency: None,
            dependencies: vec![],
            retry: RetryConfiguration {
                retryable: s.retryable,
                max_attempts: s.max_attempts as u32,
                ..Default::default()
            },
            timeout_seconds: None,
            publishes_events: vec![],
            batch_config: None,
        })
        .collect();

    let task_template = TaskTemplate {
        name: proto_template.name.clone(),
        namespace_name: proto_template.namespace.clone(),
        version: proto_template.version.clone(),
        description: proto_template.description,
        metadata: None,
        system_dependencies: SystemDependencies::default(),
        domain_events: vec![],
        input_schema: None,
        steps,
        environments: HashMap::new(),
        lifecycle: None,
    };

    let resolved = ResolvedTaskTemplate {
        template: task_template,
        environment: String::new(),
        resolved_at: chrono::Utc::now(),
    };

    Ok(WorkerTemplateResponse {
        template: resolved,
        handler_metadata: HandlerMetadata {
            namespace: handler_meta.namespace,
            name: handler_meta.handler_name,
            version: handler_meta.version,
            handler_class: String::new(),
            config_schema: None,
            default_dependent_system: None,
            registered_at: chrono::Utc::now(),
        },
        cached: response.cached,
        cache_age_seconds: response.cache_age_seconds,
        access_count: response.access_count,
    })
}

// ============================================================================
// Config Response Conversions
// ============================================================================

use tasker_shared::types::api::orchestration::{
    ConfigMetadata, SafeAuthConfig, SafeMessagingConfig, WorkerConfigResponse,
};

/// Convert proto WorkerGetConfigResponse to domain.
///
/// # Errors
/// Returns `InvalidResponse` if required config sections are missing.
pub fn proto_worker_config_to_domain(
    response: proto::WorkerGetConfigResponse,
) -> Result<WorkerConfigResponse, ClientError> {
    let metadata = response
        .metadata
        .map(|m| ConfigMetadata {
            timestamp: proto_timestamp_to_datetime(m.timestamp),
            environment: m.environment,
            version: m.version,
        })
        .ok_or_else(|| {
            ClientError::invalid_response(
                "WorkerGetConfigResponse.metadata",
                "Worker config response missing required metadata",
            )
        })?;

    let auth = response
        .auth
        .map(|a| SafeAuthConfig {
            enabled: a.enabled,
            verification_method: a.verification_method,
            jwt_issuer: a.jwt_issuer,
            jwt_audience: a.jwt_audience,
            api_key_header: a.api_key_header,
            api_key_count: a.api_key_count as usize,
            strict_validation: a.strict_validation,
            allowed_algorithms: a.allowed_algorithms,
        })
        .ok_or_else(|| {
            ClientError::invalid_response(
                "WorkerGetConfigResponse.auth",
                "Worker config response missing required auth section",
            )
        })?;

    let messaging = response
        .messaging
        .map(|m| SafeMessagingConfig {
            backend: m.backend,
            queues: m.queues,
        })
        .ok_or_else(|| {
            ClientError::invalid_response(
                "WorkerGetConfigResponse.messaging",
                "Worker config response missing required messaging section",
            )
        })?;

    Ok(WorkerConfigResponse {
        metadata,
        worker_id: response.worker_id,
        worker_type: response.worker_type,
        auth,
        messaging,
    })
}

// Note: Default value helpers removed - we now fail loudly on missing required fields
// per the "fail loudly" principle to avoid returning phantom data that looks valid.

// ============================================================================
// Template Response Conversions (Orchestration)
// ============================================================================

use tasker_shared::types::api::templates::{
    NamespaceSummary, StepDefinition as TemplateStepDefinition, TemplateDetail,
    TemplateListResponse, TemplateSummary,
};

/// Convert proto ListTemplatesResponse to domain.
pub fn proto_template_list_to_domain(
    response: proto::ListTemplatesResponse,
) -> Result<TemplateListResponse, ClientError> {
    Ok(TemplateListResponse {
        namespaces: response
            .namespaces
            .into_iter()
            .map(|ns| NamespaceSummary {
                name: ns.name,
                description: ns.description,
                template_count: ns.template_count as usize,
            })
            .collect(),
        templates: response
            .templates
            .into_iter()
            .map(|t| TemplateSummary {
                name: t.name,
                namespace: t.namespace,
                version: t.version,
                description: t.description,
                step_count: t.step_count as usize,
            })
            .collect(),
        total_count: response.total_count as usize,
    })
}

/// Convert proto TemplateDetail to domain.
pub fn proto_template_detail_to_domain(
    template: proto::TemplateDetail,
) -> Result<TemplateDetail, ClientError> {
    Ok(TemplateDetail {
        name: template.name,
        namespace: template.namespace,
        version: template.version,
        description: template.description,
        configuration: template.configuration.map(proto_struct_to_json),
        steps: template
            .steps
            .into_iter()
            .map(|s| TemplateStepDefinition {
                name: s.name,
                description: s.description,
                default_retryable: s.default_retryable,
                default_max_attempts: s.default_max_attempts,
            })
            .collect(),
    })
}

// ============================================================================
// Analytics Response Conversions
// ============================================================================

use tasker_shared::database::sql_functions::SystemHealthCounts;
use tasker_shared::types::api::orchestration::{
    BottleneckAnalysis, PerformanceMetrics, ResourceUtilization, SlowStepInfo, SlowTaskInfo,
};

/// Convert proto GetPerformanceMetricsResponse to domain.
pub fn proto_performance_metrics_to_domain(
    response: proto::GetPerformanceMetricsResponse,
) -> Result<PerformanceMetrics, ClientError> {
    Ok(PerformanceMetrics {
        total_tasks: response.total_tasks,
        active_tasks: response.active_tasks,
        completed_tasks: response.completed_tasks,
        failed_tasks: response.failed_tasks,
        completion_rate: response.completion_rate,
        error_rate: response.error_rate,
        average_task_duration_seconds: response.average_task_duration_seconds,
        average_step_duration_seconds: response.average_step_duration_seconds,
        tasks_per_hour: response.tasks_per_hour,
        steps_per_hour: response.steps_per_hour,
        system_health_score: response.system_health_score,
        analysis_period_start: response.analysis_period_start,
        calculated_at: response.calculated_at,
    })
}

/// Convert proto GetBottleneckAnalysisResponse to domain.
///
/// # Errors
/// Returns `InvalidResponse` if resource_utilization or system_health is missing.
pub fn proto_bottleneck_to_domain(
    response: proto::GetBottleneckAnalysisResponse,
) -> Result<BottleneckAnalysis, ClientError> {
    let resource_utilization = proto_resource_utilization_to_domain(response.resource_utilization)?;

    Ok(BottleneckAnalysis {
        slow_steps: response
            .slow_steps
            .into_iter()
            .map(|s| SlowStepInfo {
                namespace_name: s.namespace_name,
                task_name: s.task_name,
                version: s.version,
                step_name: s.step_name,
                average_duration_seconds: s.average_duration_seconds,
                max_duration_seconds: s.max_duration_seconds,
                execution_count: s.execution_count,
                error_count: s.error_count,
                error_rate: s.error_rate,
                last_executed_at: s.last_executed_at,
            })
            .collect(),
        slow_tasks: response
            .slow_tasks
            .into_iter()
            .map(|t| SlowTaskInfo {
                namespace_name: t.namespace_name,
                task_name: t.task_name,
                version: t.version,
                average_duration_seconds: t.average_duration_seconds,
                max_duration_seconds: t.max_duration_seconds,
                execution_count: t.execution_count,
                average_step_count: t.average_step_count,
                error_count: t.error_count,
                error_rate: t.error_rate,
                last_executed_at: t.last_executed_at,
            })
            .collect(),
        resource_utilization,
        recommendations: response.recommendations,
    })
}

fn proto_resource_utilization_to_domain(
    utilization: Option<proto::ResourceUtilization>,
) -> Result<ResourceUtilization, ClientError> {
    let u = utilization.ok_or_else(|| {
        ClientError::invalid_response(
            "BottleneckAnalysis.resource_utilization",
            "Bottleneck analysis missing required resource utilization",
        )
    })?;

    let health = u.system_health.ok_or_else(|| {
        ClientError::invalid_response(
            "ResourceUtilization.system_health",
            "Resource utilization missing required system health counts",
        )
    })?;

    Ok(ResourceUtilization {
        database_pool_utilization: u.database_pool_utilization,
        system_health: SystemHealthCounts {
            pending_tasks: health.pending_tasks,
            initializing_tasks: health.initializing_tasks,
            enqueuing_steps_tasks: health.enqueuing_steps_tasks,
            steps_in_process_tasks: health.steps_in_process_tasks,
            evaluating_results_tasks: health.evaluating_results_tasks,
            waiting_for_dependencies_tasks: health.waiting_for_dependencies_tasks,
            waiting_for_retry_tasks: health.waiting_for_retry_tasks,
            blocked_by_failures_tasks: health.blocked_by_failures_tasks,
            complete_tasks: health.complete_tasks,
            error_tasks: health.error_tasks,
            cancelled_tasks: health.cancelled_tasks,
            resolved_manually_tasks: health.resolved_manually_tasks,
            total_tasks: health.total_tasks,
            pending_steps: health.pending_steps,
            enqueued_steps: health.enqueued_steps,
            in_progress_steps: health.in_progress_steps,
            enqueued_for_orchestration_steps: health.enqueued_for_orchestration_steps,
            enqueued_as_error_for_orchestration_steps: health
                .enqueued_as_error_for_orchestration_steps,
            waiting_for_retry_steps: health.waiting_for_retry_steps,
            complete_steps: health.complete_steps,
            error_steps: health.error_steps,
            cancelled_steps: health.cancelled_steps,
            resolved_manually_steps: health.resolved_manually_steps,
            total_steps: health.total_steps,
        },
    })
}

// ============================================================================
// Config Response Conversions
// ============================================================================

use tasker_shared::types::api::orchestration::{
    OrchestrationConfigResponse, SafeCircuitBreakerConfig, SafeDatabasePoolConfig,
};

/// Convert proto GetConfigResponse to domain.
///
/// # Errors
/// Returns `InvalidResponse` if required config sections are missing.
pub fn proto_config_to_domain(
    response: proto::GetConfigResponse,
) -> Result<OrchestrationConfigResponse, ClientError> {
    let metadata = response.metadata.ok_or_else(|| {
        ClientError::invalid_response(
            "GetConfigResponse.metadata",
            "Config response missing required metadata",
        )
    })?;

    let auth = response
        .auth
        .map(|a| SafeAuthConfig {
            enabled: a.enabled,
            verification_method: a.verification_method,
            jwt_issuer: a.jwt_issuer,
            jwt_audience: a.jwt_audience,
            api_key_header: a.api_key_header,
            api_key_count: a.api_key_count as usize,
            strict_validation: a.strict_validation,
            allowed_algorithms: a.allowed_algorithms,
        })
        .ok_or_else(|| {
            ClientError::invalid_response(
                "GetConfigResponse.auth",
                "Config response missing required auth section",
            )
        })?;

    let circuit_breakers = response
        .circuit_breakers
        .map(|c| SafeCircuitBreakerConfig {
            enabled: c.enabled,
            failure_threshold: c.failure_threshold,
            success_threshold: c.success_threshold,
            timeout_seconds: c.timeout_seconds,
        })
        .ok_or_else(|| {
            ClientError::invalid_response(
                "GetConfigResponse.circuit_breakers",
                "Config response missing required circuit_breakers section",
            )
        })?;

    let database_pools = response
        .database_pools
        .map(|d| SafeDatabasePoolConfig {
            web_api_pool_size: d.web_api_pool_size,
            web_api_max_connections: d.web_api_max_connections,
        })
        .ok_or_else(|| {
            ClientError::invalid_response(
                "GetConfigResponse.database_pools",
                "Config response missing required database_pools section",
            )
        })?;

    let messaging = response
        .messaging
        .map(|m| SafeMessagingConfig {
            backend: m.backend,
            queues: m.queues,
        })
        .ok_or_else(|| {
            ClientError::invalid_response(
                "GetConfigResponse.messaging",
                "Config response missing required messaging section",
            )
        })?;

    Ok(OrchestrationConfigResponse {
        metadata: ConfigMetadata {
            timestamp: proto_timestamp_to_datetime(metadata.timestamp),
            environment: metadata.environment.clone(),
            version: metadata.version,
        },
        auth,
        circuit_breakers,
        database_pools,
        deployment_mode: metadata.environment,
        messaging,
    })
}

// ============================================================================
// DLQ Response Conversions
// ============================================================================

use tasker_shared::models::orchestration::{
    DlqEntry, DlqInvestigationQueueEntry, DlqReason, DlqResolutionStatus, DlqStats,
    StalenessHealthStatus, StalenessMonitoring,
};

/// Convert domain DlqResolutionStatus to proto.
pub fn dlq_resolution_status_to_proto(status: &DlqResolutionStatus) -> proto::DlqResolutionStatus {
    match status {
        DlqResolutionStatus::Pending => proto::DlqResolutionStatus::Pending,
        DlqResolutionStatus::ManuallyResolved => proto::DlqResolutionStatus::ManuallyResolved,
        DlqResolutionStatus::PermanentlyFailed => proto::DlqResolutionStatus::PermanentlyFailed,
        DlqResolutionStatus::Cancelled => proto::DlqResolutionStatus::Cancelled,
    }
}

fn proto_dlq_resolution_status_to_domain(status: i32) -> DlqResolutionStatus {
    match proto::DlqResolutionStatus::try_from(status) {
        Ok(proto::DlqResolutionStatus::Pending) => DlqResolutionStatus::Pending,
        Ok(proto::DlqResolutionStatus::ManuallyResolved) => DlqResolutionStatus::ManuallyResolved,
        Ok(proto::DlqResolutionStatus::PermanentlyFailed) => DlqResolutionStatus::PermanentlyFailed,
        Ok(proto::DlqResolutionStatus::Cancelled) => DlqResolutionStatus::Cancelled,
        _ => DlqResolutionStatus::Pending,
    }
}

fn proto_dlq_reason_to_domain(reason: i32) -> DlqReason {
    match proto::DlqReason::try_from(reason) {
        Ok(proto::DlqReason::StalenessTimeout) => DlqReason::StalenessTimeout,
        Ok(proto::DlqReason::MaxRetriesExceeded) => DlqReason::MaxRetriesExceeded,
        Ok(proto::DlqReason::DependencyCycleDetected) => DlqReason::DependencyCycleDetected,
        Ok(proto::DlqReason::WorkerUnavailable) => DlqReason::WorkerUnavailable,
        Ok(proto::DlqReason::ManualDlq) => DlqReason::ManualDlq,
        _ => DlqReason::StalenessTimeout,
    }
}

/// Convert proto DlqEntry to domain.
pub fn proto_dlq_entry_to_domain(entry: proto::DlqEntry) -> Result<DlqEntry, ClientError> {
    Ok(DlqEntry {
        dlq_entry_uuid: entry
            .dlq_entry_uuid
            .parse()
            .map_err(|_| ClientError::Internal("Invalid DLQ entry UUID".to_string()))?,
        task_uuid: entry
            .task_uuid
            .parse()
            .map_err(|_| ClientError::Internal("Invalid task UUID".to_string()))?,
        original_state: entry.original_state,
        dlq_reason: proto_dlq_reason_to_domain(entry.dlq_reason),
        dlq_timestamp: proto_timestamp_to_datetime(entry.dlq_timestamp).naive_utc(),
        resolution_status: proto_dlq_resolution_status_to_domain(entry.resolution_status),
        resolution_timestamp: entry
            .resolution_timestamp
            .map(|ts| proto_timestamp_to_datetime(Some(ts)).naive_utc()),
        resolution_notes: entry.resolution_notes,
        resolved_by: entry.resolved_by,
        task_snapshot: entry
            .task_snapshot
            .map(proto_struct_to_json)
            .unwrap_or(serde_json::Value::Null),
        metadata: entry.metadata.map(proto_struct_to_json),
        created_at: proto_timestamp_to_datetime(entry.created_at).naive_utc(),
        updated_at: proto_timestamp_to_datetime(entry.updated_at).naive_utc(),
    })
}

/// Convert proto DlqStats to domain.
pub fn proto_dlq_stats_to_domain(stats: proto::DlqStats) -> Result<DlqStats, ClientError> {
    Ok(DlqStats {
        dlq_reason: proto_dlq_reason_to_domain(stats.dlq_reason),
        total_entries: stats.total_entries,
        pending: stats.pending,
        manually_resolved: stats.manually_resolved,
        permanent_failures: stats.permanent_failures,
        cancelled: stats.cancelled,
        oldest_entry: stats
            .oldest_entry
            .map(|ts| proto_timestamp_to_datetime(Some(ts)).naive_utc()),
        newest_entry: stats
            .newest_entry
            .map(|ts| proto_timestamp_to_datetime(Some(ts)).naive_utc()),
        avg_resolution_time_minutes: stats.avg_resolution_time_minutes,
    })
}

/// Convert proto DlqInvestigationQueueEntry to domain.
pub fn proto_dlq_queue_entry_to_domain(
    entry: proto::DlqInvestigationQueueEntry,
) -> Result<DlqInvestigationQueueEntry, ClientError> {
    Ok(DlqInvestigationQueueEntry {
        dlq_entry_uuid: entry
            .dlq_entry_uuid
            .parse()
            .map_err(|_| ClientError::Internal("Invalid DLQ entry UUID".to_string()))?,
        task_uuid: entry
            .task_uuid
            .parse()
            .map_err(|_| ClientError::Internal("Invalid task UUID".to_string()))?,
        original_state: entry.original_state,
        dlq_reason: proto_dlq_reason_to_domain(entry.dlq_reason),
        dlq_timestamp: proto_timestamp_to_datetime(entry.dlq_timestamp).naive_utc(),
        minutes_in_dlq: entry.minutes_in_dlq,
        namespace_name: entry.namespace_name,
        task_name: entry.task_name,
        current_state: entry.current_state,
        time_in_state_minutes: entry.time_in_state_minutes,
        priority_score: entry.priority_score,
    })
}

fn proto_staleness_health_to_domain(status: i32) -> StalenessHealthStatus {
    match proto::StalenessHealthStatus::try_from(status) {
        Ok(proto::StalenessHealthStatus::Healthy) => StalenessHealthStatus::Healthy,
        Ok(proto::StalenessHealthStatus::Warning) => StalenessHealthStatus::Warning,
        Ok(proto::StalenessHealthStatus::Stale) => StalenessHealthStatus::Stale,
        _ => StalenessHealthStatus::Healthy,
    }
}

/// Convert proto StalenessMonitoringEntry to domain.
pub fn proto_staleness_to_domain(
    entry: proto::StalenessMonitoringEntry,
) -> Result<StalenessMonitoring, ClientError> {
    Ok(StalenessMonitoring {
        task_uuid: entry
            .task_uuid
            .parse()
            .map_err(|_| ClientError::Internal("Invalid task UUID".to_string()))?,
        namespace_name: entry.namespace_name,
        task_name: entry.task_name,
        current_state: entry.current_state,
        time_in_state_minutes: entry.time_in_state_minutes,
        task_age_minutes: entry.task_age_minutes,
        staleness_threshold_minutes: entry.staleness_threshold_minutes,
        health_status: proto_staleness_health_to_domain(entry.health_status),
        priority: entry.priority,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Test Helpers
    // ========================================================================

    fn make_timestamp(seconds: i64) -> prost_types::Timestamp {
        prost_types::Timestamp { seconds, nanos: 0 }
    }

    fn make_health_check(status: &str) -> proto::HealthCheck {
        proto::HealthCheck {
            status: status.to_string(),
            message: Some("ok".to_string()),
            duration_ms: 5,
        }
    }

    fn make_worker_health_check(status: &str) -> proto::WorkerHealthCheck {
        proto::WorkerHealthCheck {
            status: status.to_string(),
            message: Some("ok".to_string()),
            duration_ms: 3,
            last_checked: Some(make_timestamp(1704067200)),
        }
    }

    fn make_pool_detail() -> proto::PoolDetail {
        proto::PoolDetail {
            active_connections: 5,
            idle_connections: 10,
            max_connections: 20,
            utilization_percent: 25.0,
            total_acquires: 100,
            slow_acquires: 2,
            acquire_errors: 0,
            average_acquire_time_ms: 1.5,
            max_acquire_time_ms: 10.0,
        }
    }

    fn make_worker_pool_detail() -> proto::WorkerPoolDetail {
        proto::WorkerPoolDetail {
            active_connections: 3,
            idle_connections: 7,
            max_connections: 15,
            utilization_percent: 20.0,
            total_acquires: 50,
            slow_acquires: 1,
            acquire_errors: 0,
            average_acquire_time_ms: 2.0,
            max_acquire_time_ms: 8.0,
        }
    }

    fn make_minimal_task() -> proto::Task {
        proto::Task {
            task_uuid: "task-uuid-1".to_string(),
            name: "test_task".to_string(),
            namespace: "default".to_string(),
            version: "1.0.0".to_string(),
            state: proto::TaskState::Pending as i32,
            created_at: Some(make_timestamp(1704067200)),
            updated_at: Some(make_timestamp(1704067200)),
            completed_at: None,
            context: None,
            initiator: "test".to_string(),
            source_system: "unit-test".to_string(),
            reason: "testing".to_string(),
            priority: Some(3),
            tags: vec![],
            correlation_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            parent_correlation_id: None,
            total_steps: 2,
            pending_steps: 2,
            in_progress_steps: 0,
            completed_steps: 0,
            failed_steps: 0,
            ready_steps: 1,
            execution_status: "pending".to_string(),
            recommended_action: "wait".to_string(),
            completion_percentage: 0.0,
            health_status: "healthy".to_string(),
        }
    }

    fn make_minimal_step() -> proto::Step {
        proto::Step {
            step_uuid: "step-uuid-1".to_string(),
            task_uuid: "task-uuid-1".to_string(),
            name: "step_one".to_string(),
            state: proto::StepState::Pending as i32,
            created_at: Some(make_timestamp(1704067200)),
            updated_at: Some(make_timestamp(1704067200)),
            completed_at: None,
            results: None,
            dependencies_satisfied: true,
            retry_eligible: false,
            ready_for_execution: true,
            total_parents: 0,
            completed_parents: 0,
            attempts: 0,
            max_attempts: 3,
            last_failure_at: None,
            next_retry_at: None,
            last_attempted_at: None,
            backoff_request_seconds: None,
        }
    }

    // ========================================================================
    // Timestamp Conversions
    // ========================================================================

    #[test]
    fn test_proto_timestamp_to_datetime() {
        let ts = make_timestamp(1704067200);
        let dt = proto_timestamp_to_datetime(Some(ts));
        assert_eq!(dt.timestamp(), 1704067200);
    }

    #[test]
    fn test_proto_timestamp_none_returns_default() {
        let dt = proto_timestamp_to_datetime(None);
        assert_eq!(dt.timestamp(), 0);
    }

    #[test]
    fn test_proto_timestamp_with_nanos() {
        let ts = prost_types::Timestamp {
            seconds: 1704067200,
            nanos: 500_000_000,
        };
        let dt = proto_timestamp_to_datetime(Some(ts));
        assert_eq!(dt.timestamp(), 1704067200);
        assert_eq!(dt.timestamp_subsec_nanos(), 500_000_000);
    }

    #[test]
    fn test_proto_timestamp_to_datetime_opt_some() {
        let ts = make_timestamp(1704067200);
        let result = proto_timestamp_to_datetime_opt(Some(ts));
        assert!(result.is_some());
        assert_eq!(result.unwrap().timestamp(), 1704067200);
    }

    #[test]
    fn test_proto_timestamp_to_datetime_opt_none() {
        let result = proto_timestamp_to_datetime_opt(None);
        assert!(result.is_none());
    }

    #[test]
    fn test_proto_timestamp_to_string_some() {
        let ts = make_timestamp(1704067200);
        let s = proto_timestamp_to_string(Some(ts));
        assert!(s.contains("2024-01-01"));
    }

    #[test]
    fn test_proto_timestamp_to_string_none() {
        let s = proto_timestamp_to_string(None);
        assert!(s.contains("1970-01-01"));
    }

    #[test]
    fn test_proto_timestamp_to_string_opt_some() {
        let ts = make_timestamp(1704067200);
        let result = proto_timestamp_to_string_opt(Some(ts));
        assert!(result.is_some());
        assert!(result.unwrap().contains("2024-01-01"));
    }

    #[test]
    fn test_proto_timestamp_to_string_opt_none() {
        let result = proto_timestamp_to_string_opt(None);
        assert!(result.is_none());
    }

    // ========================================================================
    // JSON/Struct Conversions
    // ========================================================================

    #[test]
    fn test_json_struct_conversion() {
        use prost_types::value::Kind;

        let proto_struct = prost_types::Struct {
            fields: [
                (
                    "name".to_string(),
                    prost_types::Value {
                        kind: Some(Kind::StringValue("test".to_string())),
                    },
                ),
                (
                    "count".to_string(),
                    prost_types::Value {
                        kind: Some(Kind::NumberValue(42.0)),
                    },
                ),
                (
                    "active".to_string(),
                    prost_types::Value {
                        kind: Some(Kind::BoolValue(true)),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };

        let json = proto_struct_to_json_opt(Some(proto_struct));
        assert_eq!(json["name"], "test");
        assert_eq!(json["count"], 42.0);
        assert_eq!(json["active"], true);
    }

    #[test]
    fn test_proto_struct_to_json_opt_none() {
        let json = proto_struct_to_json_opt(None);
        assert!(json.is_null());
    }

    #[test]
    fn test_prost_value_null() {
        use prost_types::value::Kind;
        let value = prost_types::Value {
            kind: Some(Kind::NullValue(0)),
        };
        let json = prost_value_to_json(value);
        assert!(json.is_null());
    }

    #[test]
    fn test_prost_value_bool() {
        use prost_types::value::Kind;
        let value = prost_types::Value {
            kind: Some(Kind::BoolValue(false)),
        };
        let json = prost_value_to_json(value);
        assert_eq!(json, serde_json::Value::Bool(false));
    }

    #[test]
    fn test_prost_value_number() {
        use prost_types::value::Kind;
        let value = prost_types::Value {
            kind: Some(Kind::NumberValue(42.5)),
        };
        let json = prost_value_to_json(value);
        assert!(json.is_number());
    }

    #[test]
    fn test_prost_value_nan_becomes_null() {
        use prost_types::value::Kind;
        let value = prost_types::Value {
            kind: Some(Kind::NumberValue(f64::NAN)),
        };
        let json = prost_value_to_json(value);
        assert!(json.is_null());
    }

    #[test]
    fn test_prost_value_string() {
        use prost_types::value::Kind;
        let value = prost_types::Value {
            kind: Some(Kind::StringValue("hello".to_string())),
        };
        let json = prost_value_to_json(value);
        assert_eq!(json, serde_json::Value::String("hello".to_string()));
    }

    #[test]
    fn test_prost_value_list() {
        use prost_types::value::Kind;
        let value = prost_types::Value {
            kind: Some(Kind::ListValue(prost_types::ListValue {
                values: vec![
                    prost_types::Value {
                        kind: Some(Kind::NumberValue(1.0)),
                    },
                    prost_types::Value {
                        kind: Some(Kind::NumberValue(2.0)),
                    },
                ],
            })),
        };
        let json = prost_value_to_json(value);
        assert!(json.is_array());
        assert_eq!(json.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_prost_value_nested_struct() {
        use prost_types::value::Kind;
        let inner = prost_types::Struct {
            fields: [(
                "key".to_string(),
                prost_types::Value {
                    kind: Some(Kind::StringValue("val".to_string())),
                },
            )]
            .into_iter()
            .collect(),
        };
        let value = prost_types::Value {
            kind: Some(Kind::StructValue(inner)),
        };
        let json = prost_value_to_json(value);
        assert!(json.is_object());
        assert_eq!(json["key"], "val");
    }

    #[test]
    fn test_prost_value_none_kind() {
        let value = prost_types::Value { kind: None };
        let json = prost_value_to_json(value);
        assert!(json.is_null());
    }

    // ========================================================================
    // State String Conversions
    // ========================================================================

    #[test]
    fn test_all_task_states() {
        let cases = [
            (proto::TaskState::Pending, "pending"),
            (proto::TaskState::Initializing, "initializing"),
            (proto::TaskState::EnqueuingSteps, "enqueuing_steps"),
            (proto::TaskState::StepsInProcess, "steps_in_process"),
            (proto::TaskState::EvaluatingResults, "evaluating_results"),
            (
                proto::TaskState::WaitingForDependencies,
                "waiting_for_dependencies",
            ),
            (proto::TaskState::WaitingForRetry, "waiting_for_retry"),
            (proto::TaskState::BlockedByFailures, "blocked_by_failures"),
            (proto::TaskState::Complete, "complete"),
            (proto::TaskState::Error, "error"),
            (proto::TaskState::Cancelled, "cancelled"),
            (proto::TaskState::ResolvedManually, "resolved_manually"),
            (proto::TaskState::Unspecified, "unspecified"),
        ];
        for (state, expected) in cases {
            assert_eq!(
                proto_task_state_to_string(state as i32),
                expected,
                "TaskState::{state:?}"
            );
        }
    }

    #[test]
    fn test_task_state_invalid_value() {
        assert_eq!(proto_task_state_to_string(999), "unspecified");
    }

    #[test]
    fn test_all_step_states() {
        let cases = [
            (proto::StepState::Pending, "pending"),
            (proto::StepState::Enqueued, "enqueued"),
            (proto::StepState::InProgress, "in_progress"),
            (
                proto::StepState::EnqueuedForOrchestration,
                "enqueued_for_orchestration",
            ),
            (
                proto::StepState::EnqueuedAsErrorForOrchestration,
                "enqueued_as_error_for_orchestration",
            ),
            (proto::StepState::WaitingForRetry, "waiting_for_retry"),
            (proto::StepState::Complete, "complete"),
            (proto::StepState::Error, "error"),
            (proto::StepState::Cancelled, "cancelled"),
            (proto::StepState::ResolvedManually, "resolved_manually"),
            (proto::StepState::Unspecified, "unspecified"),
        ];
        for (state, expected) in cases {
            assert_eq!(
                proto_step_state_to_string(state as i32),
                expected,
                "StepState::{state:?}"
            );
        }
    }

    #[test]
    fn test_step_state_invalid_value() {
        assert_eq!(proto_step_state_to_string(999), "unspecified");
    }

    // ========================================================================
    // Task Response Conversions
    // ========================================================================

    #[test]
    fn test_proto_task_to_domain_minimal() {
        let task = make_minimal_task();
        let result = proto_task_to_domain(task).unwrap();
        assert_eq!(result.task_uuid, "task-uuid-1");
        assert_eq!(result.name, "test_task");
        assert_eq!(result.namespace, "default");
        assert_eq!(result.version, "1.0.0");
        assert_eq!(result.status, "pending");
        assert_eq!(result.total_steps, 2);
        assert!(result.completed_at.is_none());
        assert!(result.tags.is_none());
    }

    #[test]
    fn test_proto_task_to_domain_with_tags() {
        let mut task = make_minimal_task();
        task.tags = vec!["tag1".to_string(), "tag2".to_string()];
        let result = proto_task_to_domain(task).unwrap();
        assert_eq!(result.tags.unwrap(), vec!["tag1", "tag2"]);
    }

    #[test]
    fn test_proto_task_to_domain_with_context() {
        use prost_types::value::Kind;
        let mut task = make_minimal_task();
        task.context = Some(prost_types::Struct {
            fields: [(
                "key".to_string(),
                prost_types::Value {
                    kind: Some(Kind::StringValue("val".to_string())),
                },
            )]
            .into_iter()
            .collect(),
        });
        let result = proto_task_to_domain(task).unwrap();
        assert_eq!(result.context["key"], "val");
    }

    #[test]
    fn test_proto_task_to_domain_with_parent_correlation() {
        let mut task = make_minimal_task();
        task.parent_correlation_id = Some("660e8400-e29b-41d4-a716-446655440000".to_string());
        let result = proto_task_to_domain(task).unwrap();
        assert!(result.parent_correlation_id.is_some());
    }

    #[test]
    fn test_proto_task_to_domain_empty_parent_correlation_ignored() {
        let mut task = make_minimal_task();
        task.parent_correlation_id = Some(String::new());
        let result = proto_task_to_domain(task).unwrap();
        assert!(result.parent_correlation_id.is_none());
    }

    #[test]
    fn test_proto_task_to_domain_invalid_correlation_id() {
        let mut task = make_minimal_task();
        task.correlation_id = "not-a-uuid".to_string();
        let result = proto_task_to_domain(task);
        assert!(result.is_err());
    }

    #[test]
    fn test_proto_task_to_domain_invalid_parent_correlation_id() {
        let mut task = make_minimal_task();
        task.parent_correlation_id = Some("not-a-uuid".to_string());
        let result = proto_task_to_domain(task);
        assert!(result.is_err());
    }

    #[test]
    fn test_proto_get_task_response_to_domain() {
        let response = proto::GetTaskResponse {
            task: Some(make_minimal_task()),
            steps: vec![],
            context: None,
        };
        let result = proto_get_task_response_to_domain(response).unwrap();
        assert_eq!(result.task_uuid, "task-uuid-1");
    }

    #[test]
    fn test_proto_get_task_response_empty() {
        let response = proto::GetTaskResponse {
            task: None,
            steps: vec![],
            context: None,
        };
        let result = proto_get_task_response_to_domain(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_proto_create_task_response_to_domain() {
        let response = proto::CreateTaskResponse {
            task: Some(make_minimal_task()),
            backpressure: None,
        };
        let result = proto_create_task_response_to_domain(response).unwrap();
        assert_eq!(result.task_uuid, "task-uuid-1");
    }

    #[test]
    fn test_proto_create_task_response_empty() {
        let response = proto::CreateTaskResponse {
            task: None,
            backpressure: None,
        };
        let result = proto_create_task_response_to_domain(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_proto_list_tasks_response_to_domain() {
        let response = proto::ListTasksResponse {
            tasks: vec![make_minimal_task()],
            pagination: Some(proto::PaginationResponse {
                total: 1,
                count: 10,
                offset: 0,
                has_more: false,
            }),
        };
        let result = proto_list_tasks_response_to_domain(response).unwrap();
        assert_eq!(result.tasks.len(), 1);
        assert_eq!(result.pagination.total_count, 1);
        assert_eq!(result.pagination.page, 1);
        assert!(!result.pagination.has_next);
        assert!(!result.pagination.has_previous);
    }

    #[test]
    fn test_proto_list_tasks_pagination_second_page() {
        let response = proto::ListTasksResponse {
            tasks: vec![make_minimal_task()],
            pagination: Some(proto::PaginationResponse {
                total: 25,
                count: 10,
                offset: 10,
                has_more: true,
            }),
        };
        let result = proto_list_tasks_response_to_domain(response).unwrap();
        assert_eq!(result.pagination.page, 2);
        assert_eq!(result.pagination.total_pages, 3);
        assert!(result.pagination.has_next);
        assert!(result.pagination.has_previous);
    }

    #[test]
    fn test_proto_list_tasks_no_pagination() {
        let response = proto::ListTasksResponse {
            tasks: vec![],
            pagination: None,
        };
        let result = proto_list_tasks_response_to_domain(response).unwrap();
        assert!(result.tasks.is_empty());
        assert_eq!(result.pagination.page, 1);
    }

    // ========================================================================
    // Step Response Conversions
    // ========================================================================

    #[test]
    fn test_proto_step_to_domain() {
        let step = make_minimal_step();
        let result = proto_step_to_domain(step).unwrap();
        assert_eq!(result.step_uuid, "step-uuid-1");
        assert_eq!(result.task_uuid, "task-uuid-1");
        assert_eq!(result.name, "step_one");
        assert_eq!(result.current_state, "pending");
        assert!(result.dependencies_satisfied);
        assert!(result.ready_for_execution);
        assert_eq!(result.attempts, 0);
        assert_eq!(result.max_attempts, 3);
        assert!(result.completed_at.is_none());
        assert!(result.last_failure_at.is_none());
    }

    #[test]
    fn test_proto_step_to_domain_with_results() {
        use prost_types::value::Kind;
        let mut step = make_minimal_step();
        step.results = Some(prost_types::Struct {
            fields: [(
                "output".to_string(),
                prost_types::Value {
                    kind: Some(Kind::StringValue("done".to_string())),
                },
            )]
            .into_iter()
            .collect(),
        });
        let result = proto_step_to_domain(step).unwrap();
        assert!(result.results.is_some());
        assert_eq!(result.results.unwrap()["output"], "done");
    }

    #[test]
    fn test_proto_get_step_response_to_domain() {
        let response = proto::GetStepResponse {
            step: Some(make_minimal_step()),
        };
        let result = proto_get_step_response_to_domain(response).unwrap();
        assert_eq!(result.step_uuid, "step-uuid-1");
    }

    #[test]
    fn test_proto_get_step_response_empty() {
        let response = proto::GetStepResponse { step: None };
        let result = proto_get_step_response_to_domain(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_proto_audit_to_domain() {
        let record = proto::StepAuditRecord {
            audit_uuid: "audit-1".to_string(),
            step_uuid: "step-1".to_string(),
            transition_uuid: "trans-1".to_string(),
            task_uuid: "task-1".to_string(),
            recorded_at: Some(make_timestamp(1704067200)),
            worker_uuid: Some("worker-1".to_string()),
            correlation_id: Some("corr-1".to_string()),
            success: true,
            execution_time_ms: Some(150),
            result: None,
            step_name: "step_one".to_string(),
            from_state: Some(proto::StepState::InProgress as i32),
            to_state: proto::StepState::Complete as i32,
        };
        let result = proto_audit_to_domain(record).unwrap();
        assert_eq!(result.audit_uuid, "audit-1");
        assert!(result.success);
        assert_eq!(result.execution_time_ms, Some(150));
        assert_eq!(result.to_state, "complete");
        assert_eq!(result.from_state, Some("in_progress".to_string()));
    }

    // ========================================================================
    // Health Response Conversions (Orchestration)
    // ========================================================================

    #[test]
    fn test_proto_health_response_to_domain() {
        let response = proto::HealthResponse {
            status: "healthy".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };
        let result = proto_health_response_to_domain(response);
        assert_eq!(result.status, "healthy");
    }

    #[test]
    fn test_proto_readiness_response_to_domain() {
        let response = proto::ReadinessResponse {
            status: "ready".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            checks: Some(proto::ReadinessChecks {
                web_database: Some(make_health_check("healthy")),
                orchestration_database: Some(make_health_check("healthy")),
                circuit_breaker: Some(make_health_check("healthy")),
                orchestration_system: Some(make_health_check("healthy")),
                command_processor: Some(make_health_check("healthy")),
            }),
            info: Some(proto::HealthInfo {
                version: "0.1.0".to_string(),
                environment: "test".to_string(),
                operational_state: "running".to_string(),
                web_database_pool_size: 10,
                orchestration_database_pool_size: 10,
                circuit_breaker_state: "closed".to_string(),
                pool_utilization: None,
            }),
        };
        let result = proto_readiness_response_to_domain(response).unwrap();
        assert_eq!(result.status, "ready");
        assert_eq!(result.checks.web_database.status, "healthy");
        assert_eq!(result.info.version, "0.1.0");
    }

    #[test]
    fn test_proto_readiness_response_missing_checks() {
        let response = proto::ReadinessResponse {
            status: "ready".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            checks: None,
            info: Some(proto::HealthInfo {
                version: "0.1.0".to_string(),
                environment: "test".to_string(),
                operational_state: "running".to_string(),
                web_database_pool_size: 10,
                orchestration_database_pool_size: 10,
                circuit_breaker_state: "closed".to_string(),
                pool_utilization: None,
            }),
        };
        let result = proto_readiness_response_to_domain(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_proto_readiness_response_missing_info() {
        let response = proto::ReadinessResponse {
            status: "ready".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            checks: Some(proto::ReadinessChecks {
                web_database: Some(make_health_check("healthy")),
                orchestration_database: Some(make_health_check("healthy")),
                circuit_breaker: Some(make_health_check("healthy")),
                orchestration_system: Some(make_health_check("healthy")),
                command_processor: Some(make_health_check("healthy")),
            }),
            info: None,
        };
        let result = proto_readiness_response_to_domain(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_proto_readiness_checks_missing_sub_check() {
        let response = proto::ReadinessResponse {
            status: "ready".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            checks: Some(proto::ReadinessChecks {
                web_database: None,
                orchestration_database: Some(make_health_check("healthy")),
                circuit_breaker: Some(make_health_check("healthy")),
                orchestration_system: Some(make_health_check("healthy")),
                command_processor: Some(make_health_check("healthy")),
            }),
            info: Some(proto::HealthInfo {
                version: "0.1.0".to_string(),
                environment: "test".to_string(),
                operational_state: "running".to_string(),
                web_database_pool_size: 10,
                orchestration_database_pool_size: 10,
                circuit_breaker_state: "closed".to_string(),
                pool_utilization: None,
            }),
        };
        let result = proto_readiness_response_to_domain(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_proto_detailed_health_response_to_domain() {
        let response = proto::DetailedHealthResponse {
            status: "healthy".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            checks: Some(proto::DetailedHealthChecks {
                web_database: Some(make_health_check("healthy")),
                orchestration_database: Some(make_health_check("healthy")),
                circuit_breaker: Some(make_health_check("healthy")),
                orchestration_system: Some(make_health_check("healthy")),
                command_processor: Some(make_health_check("healthy")),
                pool_utilization: Some(make_health_check("healthy")),
                queue_depth: Some(make_health_check("healthy")),
                channel_saturation: Some(make_health_check("healthy")),
            }),
            info: Some(proto::HealthInfo {
                version: "0.1.0".to_string(),
                environment: "test".to_string(),
                operational_state: "running".to_string(),
                web_database_pool_size: 10,
                orchestration_database_pool_size: 10,
                circuit_breaker_state: "closed".to_string(),
                pool_utilization: None,
            }),
        };
        let result = proto_detailed_health_response_to_domain(response).unwrap();
        assert_eq!(result.status, "healthy");
        assert_eq!(result.checks.queue_depth.status, "healthy");
    }

    #[test]
    fn test_proto_detailed_health_missing_checks() {
        let response = proto::DetailedHealthResponse {
            status: "healthy".to_string(),
            timestamp: "now".to_string(),
            checks: None,
            info: Some(proto::HealthInfo {
                version: "0.1.0".to_string(),
                environment: "test".to_string(),
                operational_state: "running".to_string(),
                web_database_pool_size: 10,
                orchestration_database_pool_size: 10,
                circuit_breaker_state: "closed".to_string(),
                pool_utilization: None,
            }),
        };
        assert!(proto_detailed_health_response_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_detailed_checks_missing_sub_check() {
        let response = proto::DetailedHealthResponse {
            status: "healthy".to_string(),
            timestamp: "now".to_string(),
            checks: Some(proto::DetailedHealthChecks {
                web_database: Some(make_health_check("healthy")),
                orchestration_database: Some(make_health_check("healthy")),
                circuit_breaker: Some(make_health_check("healthy")),
                orchestration_system: Some(make_health_check("healthy")),
                command_processor: Some(make_health_check("healthy")),
                pool_utilization: Some(make_health_check("healthy")),
                queue_depth: None, // missing
                channel_saturation: Some(make_health_check("healthy")),
            }),
            info: Some(proto::HealthInfo {
                version: "0.1.0".to_string(),
                environment: "test".to_string(),
                operational_state: "running".to_string(),
                web_database_pool_size: 10,
                orchestration_database_pool_size: 10,
                circuit_breaker_state: "closed".to_string(),
                pool_utilization: None,
            }),
        };
        assert!(proto_detailed_health_response_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_health_info_with_pool_utilization() {
        let info = proto::HealthInfo {
            version: "0.1.0".to_string(),
            environment: "test".to_string(),
            operational_state: "running".to_string(),
            web_database_pool_size: 10,
            orchestration_database_pool_size: 10,
            circuit_breaker_state: "closed".to_string(),
            pool_utilization: Some(proto::PoolUtilizationInfo {
                tasker_pool: Some(make_pool_detail()),
                pgmq_pool: Some(make_pool_detail()),
            }),
        };
        let result = proto_health_info_to_domain(info).unwrap();
        assert!(result.pool_utilization.is_some());
        let pu = result.pool_utilization.unwrap();
        assert_eq!(pu.tasker_pool.active_connections, 5);
        assert_eq!(pu.pgmq_pool.max_connections, 20);
    }

    #[test]
    fn test_proto_pool_utilization_missing_tasker_pool() {
        let info = proto::PoolUtilizationInfo {
            tasker_pool: None,
            pgmq_pool: Some(make_pool_detail()),
        };
        assert!(proto_pool_utilization_to_domain(info).is_err());
    }

    #[test]
    fn test_proto_pool_utilization_missing_pgmq_pool() {
        let info = proto::PoolUtilizationInfo {
            tasker_pool: Some(make_pool_detail()),
            pgmq_pool: None,
        };
        assert!(proto_pool_utilization_to_domain(info).is_err());
    }

    // ========================================================================
    // Worker Health Response Conversions
    // ========================================================================

    #[test]
    fn test_proto_worker_basic_health_to_domain() {
        let response = proto::WorkerBasicHealthResponse {
            status: "healthy".to_string(),
            timestamp: Some(make_timestamp(1704067200)),
            worker_id: "worker-1".to_string(),
        };
        let result = proto_worker_basic_health_to_domain(response);
        assert_eq!(result.status, "healthy");
        assert_eq!(result.worker_id, "worker-1");
    }

    #[test]
    fn test_proto_worker_readiness_to_domain() {
        let response = proto::WorkerReadinessResponse {
            status: "ready".to_string(),
            timestamp: Some(make_timestamp(1704067200)),
            worker_id: "worker-1".to_string(),
            checks: Some(proto::WorkerReadinessChecks {
                database: Some(make_worker_health_check("healthy")),
                command_processor: Some(make_worker_health_check("healthy")),
                queue_processing: Some(make_worker_health_check("healthy")),
            }),
            system_info: Some(proto::WorkerSystemInfo {
                version: "0.1.0".to_string(),
                environment: "test".to_string(),
                uptime_seconds: 3600,
                worker_type: "rust".to_string(),
                database_pool_size: 10,
                command_processor_active: true,
                supported_namespaces: vec!["default".to_string()],
                pool_utilization: None,
            }),
        };
        let result = proto_worker_readiness_to_domain(response).unwrap();
        assert_eq!(result.status, "ready");
        assert_eq!(result.checks.database.status, "healthy");
        assert_eq!(result.system_info.worker_type, "rust");
    }

    #[test]
    fn test_proto_worker_readiness_missing_checks() {
        let response = proto::WorkerReadinessResponse {
            status: "ready".to_string(),
            timestamp: Some(make_timestamp(1704067200)),
            worker_id: "worker-1".to_string(),
            checks: None,
            system_info: Some(proto::WorkerSystemInfo {
                version: "0.1.0".to_string(),
                environment: "test".to_string(),
                uptime_seconds: 3600,
                worker_type: "rust".to_string(),
                database_pool_size: 10,
                command_processor_active: true,
                supported_namespaces: vec![],
                pool_utilization: None,
            }),
        };
        assert!(proto_worker_readiness_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_worker_readiness_missing_system_info() {
        let response = proto::WorkerReadinessResponse {
            status: "ready".to_string(),
            timestamp: Some(make_timestamp(1704067200)),
            worker_id: "worker-1".to_string(),
            checks: Some(proto::WorkerReadinessChecks {
                database: Some(make_worker_health_check("healthy")),
                command_processor: Some(make_worker_health_check("healthy")),
                queue_processing: Some(make_worker_health_check("healthy")),
            }),
            system_info: None,
        };
        assert!(proto_worker_readiness_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_worker_readiness_checks_missing_sub_check() {
        let checks = proto::WorkerReadinessChecks {
            database: None,
            command_processor: Some(make_worker_health_check("healthy")),
            queue_processing: Some(make_worker_health_check("healthy")),
        };
        assert!(proto_worker_readiness_checks_to_domain(checks).is_err());
    }

    #[test]
    fn test_proto_worker_detailed_health_to_domain() {
        let response = proto::WorkerDetailedHealthResponse {
            status: "healthy".to_string(),
            timestamp: Some(make_timestamp(1704067200)),
            worker_id: "worker-1".to_string(),
            checks: Some(proto::WorkerDetailedChecks {
                database: Some(make_worker_health_check("healthy")),
                command_processor: Some(make_worker_health_check("healthy")),
                queue_processing: Some(make_worker_health_check("healthy")),
                event_system: Some(make_worker_health_check("healthy")),
                step_processing: Some(make_worker_health_check("healthy")),
                circuit_breakers: Some(make_worker_health_check("healthy")),
            }),
            system_info: Some(proto::WorkerSystemInfo {
                version: "0.1.0".to_string(),
                environment: "test".to_string(),
                uptime_seconds: 7200,
                worker_type: "rust".to_string(),
                database_pool_size: 10,
                command_processor_active: true,
                supported_namespaces: vec!["default".to_string()],
                pool_utilization: None,
            }),
            distributed_cache: Some(proto::DistributedCacheInfo {
                enabled: true,
                provider: "redis".to_string(),
                healthy: true,
            }),
        };
        let result = proto_worker_detailed_health_to_domain(response).unwrap();
        assert_eq!(result.status, "healthy");
        assert_eq!(result.checks.event_system.status, "healthy");
        assert!(result.distributed_cache.is_some());
        let dc = result.distributed_cache.unwrap();
        assert!(dc.enabled);
        assert_eq!(dc.provider, "redis");
    }

    #[test]
    fn test_proto_worker_detailed_health_no_cache() {
        let response = proto::WorkerDetailedHealthResponse {
            status: "healthy".to_string(),
            timestamp: Some(make_timestamp(1704067200)),
            worker_id: "worker-1".to_string(),
            checks: Some(proto::WorkerDetailedChecks {
                database: Some(make_worker_health_check("healthy")),
                command_processor: Some(make_worker_health_check("healthy")),
                queue_processing: Some(make_worker_health_check("healthy")),
                event_system: Some(make_worker_health_check("healthy")),
                step_processing: Some(make_worker_health_check("healthy")),
                circuit_breakers: Some(make_worker_health_check("healthy")),
            }),
            system_info: Some(proto::WorkerSystemInfo {
                version: "0.1.0".to_string(),
                environment: "test".to_string(),
                uptime_seconds: 7200,
                worker_type: "rust".to_string(),
                database_pool_size: 10,
                command_processor_active: true,
                supported_namespaces: vec![],
                pool_utilization: None,
            }),
            distributed_cache: None,
        };
        let result = proto_worker_detailed_health_to_domain(response).unwrap();
        assert!(result.distributed_cache.is_none());
    }

    #[test]
    fn test_proto_worker_detailed_checks_missing_sub_check() {
        let checks = proto::WorkerDetailedChecks {
            database: Some(make_worker_health_check("healthy")),
            command_processor: Some(make_worker_health_check("healthy")),
            queue_processing: Some(make_worker_health_check("healthy")),
            event_system: None,
            step_processing: Some(make_worker_health_check("healthy")),
            circuit_breakers: Some(make_worker_health_check("healthy")),
        };
        assert!(proto_worker_detailed_checks_to_domain(checks).is_err());
    }

    #[test]
    fn test_proto_worker_system_info_with_pool_utilization() {
        let info = proto::WorkerSystemInfo {
            version: "0.1.0".to_string(),
            environment: "test".to_string(),
            uptime_seconds: 3600,
            worker_type: "rust".to_string(),
            database_pool_size: 10,
            command_processor_active: true,
            supported_namespaces: vec!["ns1".to_string()],
            pool_utilization: Some(proto::WorkerPoolUtilizationInfo {
                tasker_pool: Some(make_worker_pool_detail()),
                pgmq_pool: Some(make_worker_pool_detail()),
            }),
        };
        let result = proto_worker_system_info_to_domain(info).unwrap();
        assert!(result.pool_utilization.is_some());
    }

    #[test]
    fn test_proto_worker_pool_utilization_missing_pool() {
        let info = proto::WorkerPoolUtilizationInfo {
            tasker_pool: None,
            pgmq_pool: Some(make_worker_pool_detail()),
        };
        assert!(proto_worker_pool_utilization_to_domain(info).is_err());
    }

    // ========================================================================
    // Worker Template Response Conversions
    // ========================================================================

    #[test]
    fn test_proto_worker_template_list_to_domain() {
        let response = proto::WorkerTemplateListResponse {
            supported_namespaces: vec!["default".to_string()],
            template_count: 3,
            cache_stats: Some(proto::CacheStats {
                total_cached: 3,
                cache_hits: 100,
                cache_misses: 5,
                cache_evictions: 1,
                oldest_entry_age_seconds: 3600,
                average_access_count: 33.0,
                supported_namespaces: vec!["default".to_string()],
            }),
            worker_capabilities: vec!["python".to_string()],
        };
        let result = proto_worker_template_list_to_domain(response);
        assert_eq!(result.template_count, 3);
        assert!(result.cache_stats.is_some());
        let cs = result.cache_stats.unwrap();
        assert_eq!(cs.total_cached, 3);
        assert_eq!(cs.cache_hits, 100);
    }

    #[test]
    fn test_proto_worker_template_list_no_cache_stats() {
        let response = proto::WorkerTemplateListResponse {
            supported_namespaces: vec![],
            template_count: 0,
            cache_stats: None,
            worker_capabilities: vec![],
        };
        let result = proto_worker_template_list_to_domain(response);
        assert_eq!(result.template_count, 0);
        assert!(result.cache_stats.is_none());
    }

    #[test]
    fn test_proto_worker_template_to_domain() {
        let response = proto::WorkerTemplateResponse {
            template: Some(proto::WorkerResolvedTemplate {
                name: "test_template".to_string(),
                namespace: "default".to_string(),
                version: "1.0.0".to_string(),
                description: Some("A test".to_string()),
                steps: vec![proto::WorkerStepDefinition {
                    name: "step_1".to_string(),
                    description: Some("First step".to_string()),
                    retryable: true,
                    max_attempts: 3,
                }],
            }),
            handler_metadata: Some(proto::WorkerHandlerMetadata {
                namespace: "default".to_string(),
                handler_name: "test_handler".to_string(),
                description: None,
                step_names: vec![],
                version: "1.0.0".to_string(),
            }),
            cached: true,
            cache_age_seconds: Some(60),
            access_count: Some(10),
        };
        let result = proto_worker_template_to_domain(response).unwrap();
        assert_eq!(result.template.template.name, "test_template");
        assert!(result.cached);
        assert_eq!(result.cache_age_seconds, Some(60));
        assert_eq!(result.handler_metadata.name, "test_handler");
    }

    #[test]
    fn test_proto_worker_template_missing_template() {
        let response = proto::WorkerTemplateResponse {
            template: None,
            handler_metadata: None,
            cached: false,
            cache_age_seconds: None,
            access_count: None,
        };
        let result = proto_worker_template_to_domain(response);
        assert!(result.is_err());
    }

    // ========================================================================
    // Template Response Conversions (Orchestration)
    // ========================================================================

    #[test]
    fn test_proto_template_list_to_domain() {
        let response = proto::ListTemplatesResponse {
            namespaces: vec![proto::NamespaceSummary {
                name: "default".to_string(),
                description: Some("Default namespace".to_string()),
                template_count: 2,
            }],
            templates: vec![proto::TemplateSummary {
                name: "task_a".to_string(),
                namespace: "default".to_string(),
                version: "1.0.0".to_string(),
                description: Some("Task A".to_string()),
                step_count: 3,
            }],
            total_count: 1,
        };
        let result = proto_template_list_to_domain(response).unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.namespaces.len(), 1);
        assert_eq!(result.namespaces[0].name, "default");
        assert_eq!(result.templates[0].step_count, 3);
    }

    #[test]
    fn test_proto_template_detail_to_domain() {
        let template = proto::TemplateDetail {
            name: "my_task".to_string(),
            namespace: "default".to_string(),
            version: "2.0.0".to_string(),
            description: Some("Detailed task".to_string()),
            configuration: None,
            steps: vec![proto::StepDefinition {
                name: "step_1".to_string(),
                description: Some("First".to_string()),
                default_retryable: true,
                default_max_attempts: 5,
            }],
        };
        let result = proto_template_detail_to_domain(template).unwrap();
        assert_eq!(result.name, "my_task");
        assert_eq!(result.version, "2.0.0");
        assert_eq!(result.steps.len(), 1);
        assert!(result.steps[0].default_retryable);
    }

    #[test]
    fn test_proto_template_detail_with_configuration() {
        use prost_types::value::Kind;
        let template = proto::TemplateDetail {
            name: "configured".to_string(),
            namespace: "ns".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            configuration: Some(prost_types::Struct {
                fields: [(
                    "timeout".to_string(),
                    prost_types::Value {
                        kind: Some(Kind::NumberValue(30.0)),
                    },
                )]
                .into_iter()
                .collect(),
            }),
            steps: vec![],
        };
        let result = proto_template_detail_to_domain(template).unwrap();
        assert!(result.configuration.is_some());
        assert_eq!(result.configuration.unwrap()["timeout"], 30.0);
    }

    // ========================================================================
    // Config Response Conversions
    // ========================================================================

    #[test]
    fn test_proto_config_to_domain() {
        let response = proto::GetConfigResponse {
            metadata: Some(proto::ConfigMetadata {
                timestamp: Some(make_timestamp(1704067200)),
                environment: "test".to_string(),
                version: "0.1.0".to_string(),
            }),
            auth: Some(proto::SafeAuthConfig {
                enabled: true,
                verification_method: "jwt".to_string(),
                jwt_issuer: "tasker".to_string(),
                jwt_audience: "api".to_string(),
                api_key_header: "x-api-key".to_string(),
                api_key_count: 2,
                strict_validation: true,
                allowed_algorithms: vec!["HS256".to_string()],
            }),
            circuit_breakers: Some(proto::SafeCircuitBreakerConfig {
                enabled: true,
                failure_threshold: 5,
                success_threshold: 3,
                timeout_seconds: 60,
            }),
            database_pools: Some(proto::SafeDatabasePoolConfig {
                web_api_pool_size: 10,
                web_api_max_connections: 20,
            }),
            deployment_mode: "test".to_string(),
            messaging: Some(proto::SafeMessagingConfig {
                backend: "pgmq".to_string(),
                queues: vec!["tasks".to_string()],
            }),
        };
        let result = proto_config_to_domain(response).unwrap();
        assert_eq!(result.metadata.environment, "test");
        assert!(result.auth.enabled);
        assert!(result.circuit_breakers.enabled);
        assert_eq!(result.database_pools.web_api_pool_size, 10);
        assert_eq!(result.messaging.backend, "pgmq");
    }

    #[test]
    fn test_proto_config_missing_metadata() {
        let response = proto::GetConfigResponse {
            metadata: None,
            auth: Some(proto::SafeAuthConfig::default()),
            circuit_breakers: Some(proto::SafeCircuitBreakerConfig::default()),
            database_pools: Some(proto::SafeDatabasePoolConfig::default()),
            deployment_mode: "test".to_string(),
            messaging: Some(proto::SafeMessagingConfig::default()),
        };
        assert!(proto_config_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_config_missing_auth() {
        let response = proto::GetConfigResponse {
            metadata: Some(proto::ConfigMetadata {
                timestamp: Some(make_timestamp(1704067200)),
                environment: "test".to_string(),
                version: "0.1.0".to_string(),
            }),
            auth: None,
            circuit_breakers: Some(proto::SafeCircuitBreakerConfig::default()),
            database_pools: Some(proto::SafeDatabasePoolConfig::default()),
            deployment_mode: "test".to_string(),
            messaging: Some(proto::SafeMessagingConfig::default()),
        };
        assert!(proto_config_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_config_missing_circuit_breakers() {
        let response = proto::GetConfigResponse {
            metadata: Some(proto::ConfigMetadata {
                timestamp: Some(make_timestamp(1704067200)),
                environment: "test".to_string(),
                version: "0.1.0".to_string(),
            }),
            auth: Some(proto::SafeAuthConfig::default()),
            circuit_breakers: None,
            database_pools: Some(proto::SafeDatabasePoolConfig::default()),
            deployment_mode: "test".to_string(),
            messaging: Some(proto::SafeMessagingConfig::default()),
        };
        assert!(proto_config_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_config_missing_database_pools() {
        let response = proto::GetConfigResponse {
            metadata: Some(proto::ConfigMetadata {
                timestamp: Some(make_timestamp(1704067200)),
                environment: "test".to_string(),
                version: "0.1.0".to_string(),
            }),
            auth: Some(proto::SafeAuthConfig::default()),
            circuit_breakers: Some(proto::SafeCircuitBreakerConfig::default()),
            database_pools: None,
            deployment_mode: "test".to_string(),
            messaging: Some(proto::SafeMessagingConfig::default()),
        };
        assert!(proto_config_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_config_missing_messaging() {
        let response = proto::GetConfigResponse {
            metadata: Some(proto::ConfigMetadata {
                timestamp: Some(make_timestamp(1704067200)),
                environment: "test".to_string(),
                version: "0.1.0".to_string(),
            }),
            auth: Some(proto::SafeAuthConfig::default()),
            circuit_breakers: Some(proto::SafeCircuitBreakerConfig::default()),
            database_pools: Some(proto::SafeDatabasePoolConfig::default()),
            deployment_mode: "test".to_string(),
            messaging: None,
        };
        assert!(proto_config_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_worker_config_to_domain() {
        let response = proto::WorkerGetConfigResponse {
            metadata: Some(proto::ConfigMetadata {
                timestamp: Some(make_timestamp(1704067200)),
                environment: "test".to_string(),
                version: "0.1.0".to_string(),
            }),
            worker_id: "worker-1".to_string(),
            worker_type: "rust".to_string(),
            auth: Some(proto::SafeAuthConfig {
                enabled: true,
                verification_method: "api_key".to_string(),
                jwt_issuer: String::new(),
                jwt_audience: String::new(),
                api_key_header: "x-api-key".to_string(),
                api_key_count: 1,
                strict_validation: false,
                allowed_algorithms: vec![],
            }),
            messaging: Some(proto::SafeMessagingConfig {
                backend: "pgmq".to_string(),
                queues: vec!["worker_queue".to_string()],
            }),
        };
        let result = proto_worker_config_to_domain(response).unwrap();
        assert_eq!(result.worker_id, "worker-1");
        assert_eq!(result.worker_type, "rust");
        assert!(result.auth.enabled);
    }

    #[test]
    fn test_proto_worker_config_missing_metadata() {
        let response = proto::WorkerGetConfigResponse {
            metadata: None,
            worker_id: "w1".to_string(),
            worker_type: "rust".to_string(),
            auth: Some(proto::SafeAuthConfig::default()),
            messaging: Some(proto::SafeMessagingConfig::default()),
        };
        assert!(proto_worker_config_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_worker_config_missing_auth() {
        let response = proto::WorkerGetConfigResponse {
            metadata: Some(proto::ConfigMetadata {
                timestamp: Some(make_timestamp(1704067200)),
                environment: "test".to_string(),
                version: "0.1.0".to_string(),
            }),
            worker_id: "w1".to_string(),
            worker_type: "rust".to_string(),
            auth: None,
            messaging: Some(proto::SafeMessagingConfig::default()),
        };
        assert!(proto_worker_config_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_worker_config_missing_messaging() {
        let response = proto::WorkerGetConfigResponse {
            metadata: Some(proto::ConfigMetadata {
                timestamp: Some(make_timestamp(1704067200)),
                environment: "test".to_string(),
                version: "0.1.0".to_string(),
            }),
            worker_id: "w1".to_string(),
            worker_type: "rust".to_string(),
            auth: Some(proto::SafeAuthConfig::default()),
            messaging: None,
        };
        assert!(proto_worker_config_to_domain(response).is_err());
    }

    // ========================================================================
    // DLQ Response Conversions
    // ========================================================================

    #[test]
    fn test_dlq_resolution_status_to_proto_all_variants() {
        assert_eq!(
            dlq_resolution_status_to_proto(&DlqResolutionStatus::Pending),
            proto::DlqResolutionStatus::Pending
        );
        assert_eq!(
            dlq_resolution_status_to_proto(&DlqResolutionStatus::ManuallyResolved),
            proto::DlqResolutionStatus::ManuallyResolved
        );
        assert_eq!(
            dlq_resolution_status_to_proto(&DlqResolutionStatus::PermanentlyFailed),
            proto::DlqResolutionStatus::PermanentlyFailed
        );
        assert_eq!(
            dlq_resolution_status_to_proto(&DlqResolutionStatus::Cancelled),
            proto::DlqResolutionStatus::Cancelled
        );
    }

    #[test]
    fn test_proto_dlq_resolution_status_to_domain_all() {
        assert!(matches!(
            proto_dlq_resolution_status_to_domain(proto::DlqResolutionStatus::Pending as i32),
            DlqResolutionStatus::Pending
        ));
        assert!(matches!(
            proto_dlq_resolution_status_to_domain(
                proto::DlqResolutionStatus::ManuallyResolved as i32
            ),
            DlqResolutionStatus::ManuallyResolved
        ));
        assert!(matches!(
            proto_dlq_resolution_status_to_domain(
                proto::DlqResolutionStatus::PermanentlyFailed as i32
            ),
            DlqResolutionStatus::PermanentlyFailed
        ));
        assert!(matches!(
            proto_dlq_resolution_status_to_domain(proto::DlqResolutionStatus::Cancelled as i32),
            DlqResolutionStatus::Cancelled
        ));
    }

    #[test]
    fn test_proto_dlq_resolution_status_unknown_defaults_to_pending() {
        assert!(matches!(
            proto_dlq_resolution_status_to_domain(999),
            DlqResolutionStatus::Pending
        ));
    }

    #[test]
    fn test_proto_dlq_reason_to_domain_all() {
        assert!(matches!(
            proto_dlq_reason_to_domain(proto::DlqReason::StalenessTimeout as i32),
            DlqReason::StalenessTimeout
        ));
        assert!(matches!(
            proto_dlq_reason_to_domain(proto::DlqReason::MaxRetriesExceeded as i32),
            DlqReason::MaxRetriesExceeded
        ));
        assert!(matches!(
            proto_dlq_reason_to_domain(proto::DlqReason::DependencyCycleDetected as i32),
            DlqReason::DependencyCycleDetected
        ));
        assert!(matches!(
            proto_dlq_reason_to_domain(proto::DlqReason::WorkerUnavailable as i32),
            DlqReason::WorkerUnavailable
        ));
        assert!(matches!(
            proto_dlq_reason_to_domain(proto::DlqReason::ManualDlq as i32),
            DlqReason::ManualDlq
        ));
    }

    #[test]
    fn test_proto_dlq_reason_unknown_defaults_to_staleness() {
        assert!(matches!(
            proto_dlq_reason_to_domain(999),
            DlqReason::StalenessTimeout
        ));
    }

    #[test]
    fn test_proto_dlq_entry_to_domain() {
        let entry = proto::DlqEntry {
            dlq_entry_uuid: "550e8400-e29b-41d4-a716-446655440001".to_string(),
            task_uuid: "550e8400-e29b-41d4-a716-446655440002".to_string(),
            original_state: "steps_in_process".to_string(),
            dlq_reason: proto::DlqReason::StalenessTimeout as i32,
            dlq_timestamp: Some(make_timestamp(1704067200)),
            resolution_status: proto::DlqResolutionStatus::Pending as i32,
            resolution_timestamp: None,
            resolution_notes: Some("needs investigation".to_string()),
            resolved_by: None,
            task_snapshot: None,
            metadata: None,
            created_at: Some(make_timestamp(1704067200)),
            updated_at: Some(make_timestamp(1704067200)),
        };
        let result = proto_dlq_entry_to_domain(entry).unwrap();
        assert_eq!(
            result.dlq_entry_uuid.to_string(),
            "550e8400-e29b-41d4-a716-446655440001"
        );
        assert!(matches!(result.dlq_reason, DlqReason::StalenessTimeout));
        assert!(matches!(
            result.resolution_status,
            DlqResolutionStatus::Pending
        ));
        assert!(result.resolution_timestamp.is_none());
    }

    #[test]
    fn test_proto_dlq_entry_invalid_uuid() {
        let entry = proto::DlqEntry {
            dlq_entry_uuid: "not-a-uuid".to_string(),
            task_uuid: "550e8400-e29b-41d4-a716-446655440002".to_string(),
            original_state: "pending".to_string(),
            dlq_reason: proto::DlqReason::StalenessTimeout as i32,
            dlq_timestamp: Some(make_timestamp(1704067200)),
            resolution_status: proto::DlqResolutionStatus::Pending as i32,
            resolution_timestamp: None,
            resolution_notes: None,
            resolved_by: None,
            task_snapshot: None,
            metadata: None,
            created_at: Some(make_timestamp(1704067200)),
            updated_at: Some(make_timestamp(1704067200)),
        };
        assert!(proto_dlq_entry_to_domain(entry).is_err());
    }

    #[test]
    fn test_proto_dlq_entry_with_resolution() {
        let entry = proto::DlqEntry {
            dlq_entry_uuid: "550e8400-e29b-41d4-a716-446655440001".to_string(),
            task_uuid: "550e8400-e29b-41d4-a716-446655440002".to_string(),
            original_state: "error".to_string(),
            dlq_reason: proto::DlqReason::MaxRetriesExceeded as i32,
            dlq_timestamp: Some(make_timestamp(1704067200)),
            resolution_status: proto::DlqResolutionStatus::ManuallyResolved as i32,
            resolution_timestamp: Some(make_timestamp(1704153600)),
            resolution_notes: Some("resolved".to_string()),
            resolved_by: Some("admin".to_string()),
            task_snapshot: None,
            metadata: None,
            created_at: Some(make_timestamp(1704067200)),
            updated_at: Some(make_timestamp(1704153600)),
        };
        let result = proto_dlq_entry_to_domain(entry).unwrap();
        assert!(result.resolution_timestamp.is_some());
        assert_eq!(result.resolved_by, Some("admin".to_string()));
        assert!(matches!(
            result.resolution_status,
            DlqResolutionStatus::ManuallyResolved
        ));
    }

    #[test]
    fn test_proto_dlq_stats_to_domain() {
        let stats = proto::DlqStats {
            dlq_reason: proto::DlqReason::MaxRetriesExceeded as i32,
            total_entries: 10,
            pending: 5,
            manually_resolved: 3,
            permanent_failures: 1,
            cancelled: 1,
            oldest_entry: Some(make_timestamp(1704067200)),
            newest_entry: Some(make_timestamp(1704153600)),
            avg_resolution_time_minutes: Some(45.5),
        };
        let result = proto_dlq_stats_to_domain(stats).unwrap();
        assert!(matches!(result.dlq_reason, DlqReason::MaxRetriesExceeded));
        assert_eq!(result.total_entries, 10);
        assert_eq!(result.pending, 5);
        assert!(result.oldest_entry.is_some());
        assert_eq!(result.avg_resolution_time_minutes, Some(45.5));
    }

    #[test]
    fn test_proto_dlq_queue_entry_to_domain() {
        let entry = proto::DlqInvestigationQueueEntry {
            dlq_entry_uuid: "550e8400-e29b-41d4-a716-446655440001".to_string(),
            task_uuid: "550e8400-e29b-41d4-a716-446655440002".to_string(),
            original_state: "error".to_string(),
            dlq_reason: proto::DlqReason::WorkerUnavailable as i32,
            dlq_timestamp: Some(make_timestamp(1704067200)),
            minutes_in_dlq: 120.5,
            namespace_name: Some("default".to_string()),
            task_name: Some("my_task".to_string()),
            current_state: Some("error".to_string()),
            time_in_state_minutes: Some(60),
            priority_score: 85.0,
        };
        let result = proto_dlq_queue_entry_to_domain(entry).unwrap();
        assert!(matches!(result.dlq_reason, DlqReason::WorkerUnavailable));
        assert_eq!(result.minutes_in_dlq, 120.5);
        assert_eq!(result.priority_score, 85.0);
        assert_eq!(result.namespace_name, Some("default".to_string()));
    }

    #[test]
    fn test_proto_dlq_queue_entry_invalid_uuid() {
        let entry = proto::DlqInvestigationQueueEntry {
            dlq_entry_uuid: "bad".to_string(),
            task_uuid: "550e8400-e29b-41d4-a716-446655440002".to_string(),
            original_state: "error".to_string(),
            dlq_reason: proto::DlqReason::ManualDlq as i32,
            dlq_timestamp: Some(make_timestamp(1704067200)),
            minutes_in_dlq: 10.0,
            namespace_name: None,
            task_name: None,
            current_state: None,
            time_in_state_minutes: None,
            priority_score: 50.0,
        };
        assert!(proto_dlq_queue_entry_to_domain(entry).is_err());
    }

    // ========================================================================
    // Staleness Monitoring Conversions
    // ========================================================================

    #[test]
    fn test_proto_staleness_health_to_domain_all() {
        assert!(matches!(
            proto_staleness_health_to_domain(proto::StalenessHealthStatus::Healthy as i32),
            StalenessHealthStatus::Healthy
        ));
        assert!(matches!(
            proto_staleness_health_to_domain(proto::StalenessHealthStatus::Warning as i32),
            StalenessHealthStatus::Warning
        ));
        assert!(matches!(
            proto_staleness_health_to_domain(proto::StalenessHealthStatus::Stale as i32),
            StalenessHealthStatus::Stale
        ));
    }

    #[test]
    fn test_proto_staleness_health_unknown_defaults_to_healthy() {
        assert!(matches!(
            proto_staleness_health_to_domain(999),
            StalenessHealthStatus::Healthy
        ));
    }

    #[test]
    fn test_proto_staleness_to_domain() {
        let entry = proto::StalenessMonitoringEntry {
            task_uuid: "550e8400-e29b-41d4-a716-446655440001".to_string(),
            namespace_name: Some("default".to_string()),
            task_name: Some("stale_task".to_string()),
            current_state: "steps_in_process".to_string(),
            time_in_state_minutes: 120,
            task_age_minutes: 180,
            staleness_threshold_minutes: 60,
            health_status: proto::StalenessHealthStatus::Stale as i32,
            priority: 3,
        };
        let result = proto_staleness_to_domain(entry).unwrap();
        assert_eq!(
            result.task_uuid.to_string(),
            "550e8400-e29b-41d4-a716-446655440001"
        );
        assert!(matches!(result.health_status, StalenessHealthStatus::Stale));
        assert_eq!(result.time_in_state_minutes, 120);
        assert_eq!(result.staleness_threshold_minutes, 60);
    }

    #[test]
    fn test_proto_staleness_invalid_uuid() {
        let entry = proto::StalenessMonitoringEntry {
            task_uuid: "not-a-uuid".to_string(),
            namespace_name: None,
            task_name: None,
            current_state: "pending".to_string(),
            time_in_state_minutes: 0,
            task_age_minutes: 0,
            staleness_threshold_minutes: 60,
            health_status: proto::StalenessHealthStatus::Healthy as i32,
            priority: 1,
        };
        assert!(proto_staleness_to_domain(entry).is_err());
    }

    // ========================================================================
    // Analytics Response Conversions
    // ========================================================================

    #[test]
    fn test_proto_performance_metrics_to_domain() {
        let response = proto::GetPerformanceMetricsResponse {
            total_tasks: 100,
            active_tasks: 10,
            completed_tasks: 80,
            failed_tasks: 10,
            completion_rate: 0.8,
            error_rate: 0.1,
            average_task_duration_seconds: 5.5,
            average_step_duration_seconds: 1.2,
            tasks_per_hour: 50,
            steps_per_hour: 200,
            system_health_score: 0.95,
            analysis_period_start: "2024-01-01T00:00:00Z".to_string(),
            calculated_at: "2024-01-01T12:00:00Z".to_string(),
        };
        let result = proto_performance_metrics_to_domain(response).unwrap();
        assert_eq!(result.total_tasks, 100);
        assert_eq!(result.completion_rate, 0.8);
        assert_eq!(result.system_health_score, 0.95);
    }

    #[test]
    fn test_proto_bottleneck_to_domain() {
        let response = proto::GetBottleneckAnalysisResponse {
            slow_steps: vec![proto::SlowStepInfo {
                namespace_name: "default".to_string(),
                task_name: "slow_task".to_string(),
                version: "1.0.0".to_string(),
                step_name: "step_x".to_string(),
                average_duration_seconds: 10.5,
                max_duration_seconds: 30.0,
                execution_count: 50,
                error_count: 5,
                error_rate: 0.1,
                last_executed_at: Some("2024-01-01T12:00:00Z".to_string()),
            }],
            slow_tasks: vec![proto::SlowTaskInfo {
                namespace_name: "default".to_string(),
                task_name: "slow_task".to_string(),
                version: "1.0.0".to_string(),
                average_duration_seconds: 45.0,
                max_duration_seconds: 120.0,
                execution_count: 20,
                average_step_count: 5.0,
                error_count: 2,
                error_rate: 0.1,
                last_executed_at: Some("2024-01-01T12:00:00Z".to_string()),
            }],
            resource_utilization: Some(proto::ResourceUtilization {
                database_pool_utilization: 0.65,
                system_health: Some(proto::SystemHealthCounts {
                    pending_tasks: 5,
                    initializing_tasks: 1,
                    enqueuing_steps_tasks: 0,
                    steps_in_process_tasks: 3,
                    evaluating_results_tasks: 1,
                    waiting_for_dependencies_tasks: 0,
                    waiting_for_retry_tasks: 0,
                    blocked_by_failures_tasks: 0,
                    complete_tasks: 80,
                    error_tasks: 10,
                    cancelled_tasks: 0,
                    resolved_manually_tasks: 0,
                    total_tasks: 100,
                    pending_steps: 10,
                    enqueued_steps: 5,
                    in_progress_steps: 3,
                    enqueued_for_orchestration_steps: 0,
                    enqueued_as_error_for_orchestration_steps: 0,
                    waiting_for_retry_steps: 0,
                    complete_steps: 200,
                    error_steps: 15,
                    cancelled_steps: 0,
                    resolved_manually_steps: 0,
                    total_steps: 233,
                }),
            }),
            recommendations: vec!["Optimize step_x".to_string()],
        };
        let result = proto_bottleneck_to_domain(response).unwrap();
        assert_eq!(result.slow_steps.len(), 1);
        assert_eq!(result.slow_steps[0].step_name, "step_x");
        assert_eq!(result.slow_tasks.len(), 1);
        assert_eq!(result.resource_utilization.database_pool_utilization, 0.65);
        assert_eq!(result.resource_utilization.system_health.total_tasks, 100);
        assert_eq!(result.recommendations, vec!["Optimize step_x"]);
    }

    #[test]
    fn test_proto_bottleneck_missing_resource_utilization() {
        let response = proto::GetBottleneckAnalysisResponse {
            slow_steps: vec![],
            slow_tasks: vec![],
            resource_utilization: None,
            recommendations: vec![],
        };
        assert!(proto_bottleneck_to_domain(response).is_err());
    }

    #[test]
    fn test_proto_bottleneck_missing_system_health() {
        let response = proto::GetBottleneckAnalysisResponse {
            slow_steps: vec![],
            slow_tasks: vec![],
            resource_utilization: Some(proto::ResourceUtilization {
                database_pool_utilization: 0.5,
                system_health: None,
            }),
            recommendations: vec![],
        };
        assert!(proto_bottleneck_to_domain(response).is_err());
    }
}
