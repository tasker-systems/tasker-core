//! DLQ service gRPC implementation.
//!
//! Provides DLQ investigation tracking operations via gRPC.

use crate::grpc::conversions::{datetime_to_timestamp, json_to_struct, parse_uuid};
use crate::grpc::interceptors::AuthInterceptor;
use crate::grpc::state::GrpcState;
use tasker_shared::models::orchestration::dlq::{
    DlqEntry, DlqInvestigationQueueEntry, DlqInvestigationUpdate, DlqListParams, DlqStats,
    StalenessMonitoring,
};
use tasker_shared::proto::v1::{self as proto, dlq_service_server::DlqService as DlqServiceTrait};
use tasker_shared::types::Permission;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info};

/// gRPC DLQ service implementation.
#[derive(Debug)]
pub struct DlqServiceImpl {
    state: GrpcState,
    auth_interceptor: AuthInterceptor,
}

impl DlqServiceImpl {
    /// Create a new DLQ service.
    pub fn new(state: GrpcState) -> Self {
        let auth_interceptor = AuthInterceptor::new(state.services.security_service.clone());
        Self {
            state,
            auth_interceptor,
        }
    }

    /// Authenticate the request and check permissions.
    async fn authenticate_and_authorize<T>(
        &self,
        request: &Request<T>,
        required_permission: Permission,
    ) -> Result<tasker_shared::types::SecurityContext, Status> {
        let ctx = self.auth_interceptor.authenticate(request).await?;

        // Check permission
        if !ctx.has_permission(&required_permission) {
            return Err(Status::permission_denied(
                "Insufficient permissions for this operation",
            ));
        }

        Ok(ctx)
    }
}

#[tonic::async_trait]
impl DlqServiceTrait for DlqServiceImpl {
    /// List DLQ entries with optional filtering.
    async fn list_entries(
        &self,
        request: Request<proto::ListDlqEntriesRequest>,
    ) -> Result<Response<proto::ListDlqEntriesResponse>, Status> {
        // Authenticate and authorize
        let _ctx = self
            .authenticate_and_authorize(&request, Permission::DlqRead)
            .await?;

        let req = request.into_inner();
        debug!(
            resolution_status = ?req.resolution_status,
            limit = req.limit,
            offset = req.offset,
            "gRPC list DLQ entries"
        );

        // Build query parameters
        let resolution_status = req
            .resolution_status
            .and_then(|s| proto::DlqResolutionStatus::try_from(s).ok())
            .and_then(dlq_resolution_status_from_proto);

        let params = DlqListParams {
            resolution_status,
            limit: req.limit.unwrap_or(50) as i64,
            offset: req.offset.unwrap_or(0) as i64,
        };

        // List DLQ entries via model layer
        let entries = DlqEntry::list(&self.state.services.read_pool, params)
            .await
            .map_err(|e| {
                error!("Failed to list DLQ entries: {}", e);
                Status::internal("Failed to list DLQ entries")
            })?;

        info!(count = entries.len(), "Successfully listed DLQ entries");

        // Convert to proto
        let proto_entries = entries.iter().map(dlq_entry_to_proto).collect();

        Ok(Response::new(proto::ListDlqEntriesResponse {
            entries: proto_entries,
        }))
    }

    /// Get DLQ entry for a specific task (most recent).
    async fn get_entry_by_task(
        &self,
        request: Request<proto::GetDlqEntryByTaskRequest>,
    ) -> Result<Response<proto::GetDlqEntryByTaskResponse>, Status> {
        // Authenticate and authorize
        let _ctx = self
            .authenticate_and_authorize(&request, Permission::DlqRead)
            .await?;

        let req = request.into_inner();
        let task_uuid = parse_uuid(&req.task_uuid)?;

        debug!(task_uuid = %task_uuid, "gRPC get DLQ entry by task");

        // Get DLQ entry via model layer
        let entry = DlqEntry::find_by_task(&self.state.services.read_pool, task_uuid)
            .await
            .map_err(|e| {
                error!("Failed to get DLQ entry for task {}: {}", task_uuid, e);
                Status::internal("Failed to get DLQ entry")
            })?;

        match entry {
            Some(entry) => {
                info!(
                    dlq_entry_uuid = %entry.dlq_entry_uuid,
                    task_uuid = %task_uuid,
                    "Successfully retrieved DLQ entry"
                );
                Ok(Response::new(proto::GetDlqEntryByTaskResponse {
                    entry: Some(dlq_entry_to_proto(&entry)),
                }))
            }
            None => Err(Status::not_found(format!(
                "DLQ entry not found for task {}",
                task_uuid
            ))),
        }
    }

    /// Update DLQ investigation status and notes.
    async fn update_investigation(
        &self,
        request: Request<proto::UpdateDlqInvestigationRequest>,
    ) -> Result<Response<proto::UpdateDlqInvestigationResponse>, Status> {
        // Authenticate and authorize
        let _ctx = self
            .authenticate_and_authorize(&request, Permission::DlqUpdate)
            .await?;

        let req = request.into_inner();
        let dlq_entry_uuid = parse_uuid(&req.dlq_entry_uuid)?;

        debug!(
            dlq_entry_uuid = %dlq_entry_uuid,
            resolution_status = ?req.resolution_status,
            "gRPC update DLQ investigation"
        );

        // Build update request
        let resolution_status = req
            .resolution_status
            .and_then(|s| proto::DlqResolutionStatus::try_from(s).ok())
            .and_then(dlq_resolution_status_from_proto);

        let metadata = req.metadata.map(crate::grpc::conversions::struct_to_json);

        let update = DlqInvestigationUpdate {
            resolution_status,
            resolution_notes: req.resolution_notes,
            resolved_by: req.resolved_by,
            metadata,
        };

        // Update via model layer
        let updated =
            DlqEntry::update_investigation(&self.state.services.write_pool, dlq_entry_uuid, update)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to update DLQ investigation {}: {}",
                        dlq_entry_uuid, e
                    );
                    Status::internal("Failed to update DLQ investigation")
                })?;

        if !updated {
            return Err(Status::not_found(format!(
                "DLQ entry not found: {}",
                dlq_entry_uuid
            )));
        }

        info!(
            dlq_entry_uuid = %dlq_entry_uuid,
            "Successfully updated DLQ investigation"
        );

        Ok(Response::new(proto::UpdateDlqInvestigationResponse {
            success: true,
            message: "Investigation status updated successfully".to_string(),
            dlq_entry_uuid: dlq_entry_uuid.to_string(),
        }))
    }

    /// Get DLQ statistics by reason.
    async fn get_stats(
        &self,
        request: Request<proto::GetDlqStatsRequest>,
    ) -> Result<Response<proto::GetDlqStatsResponse>, Status> {
        // Authenticate and authorize
        let _ctx = self
            .authenticate_and_authorize(&request, Permission::DlqStats)
            .await?;

        debug!("gRPC get DLQ stats");

        // Get stats via model layer
        let stats = DlqEntry::get_stats(&self.state.services.read_pool)
            .await
            .map_err(|e| {
                error!("Failed to get DLQ stats: {}", e);
                Status::internal("Failed to get DLQ stats")
            })?;

        info!(
            stats_count = stats.len(),
            "Successfully retrieved DLQ stats"
        );

        // Convert to proto
        let proto_stats = stats.iter().map(dlq_stats_to_proto).collect();

        Ok(Response::new(proto::GetDlqStatsResponse {
            stats: proto_stats,
        }))
    }

    /// Get prioritized investigation queue for operator triage.
    async fn get_investigation_queue(
        &self,
        request: Request<proto::GetDlqInvestigationQueueRequest>,
    ) -> Result<Response<proto::GetDlqInvestigationQueueResponse>, Status> {
        // Authenticate and authorize
        let _ctx = self
            .authenticate_and_authorize(&request, Permission::DlqRead)
            .await?;

        let req = request.into_inner();
        let limit = req.limit.map(|l| l as i64);

        debug!(limit = ?limit, "gRPC get investigation queue");

        // Get investigation queue via model layer
        let queue = DlqEntry::list_investigation_queue(&self.state.services.read_pool, limit)
            .await
            .map_err(|e| {
                error!("Failed to get investigation queue: {}", e);
                Status::internal("Failed to get investigation queue")
            })?;

        info!(
            queue_size = queue.len(),
            "Successfully retrieved investigation queue"
        );

        // Convert to proto
        let proto_entries = queue
            .iter()
            .map(dlq_investigation_queue_entry_to_proto)
            .collect();

        Ok(Response::new(proto::GetDlqInvestigationQueueResponse {
            entries: proto_entries,
        }))
    }

    /// Get task staleness monitoring (proactive health check).
    async fn get_staleness_monitoring(
        &self,
        request: Request<proto::GetStalenessMonitoringRequest>,
    ) -> Result<Response<proto::GetStalenessMonitoringResponse>, Status> {
        // Authenticate and authorize
        let _ctx = self
            .authenticate_and_authorize(&request, Permission::DlqRead)
            .await?;

        let req = request.into_inner();
        let limit = req.limit.map(|l| l as i64);

        debug!(limit = ?limit, "gRPC get staleness monitoring");

        // Get staleness monitoring via model layer
        let monitoring = DlqEntry::get_staleness_monitoring(&self.state.services.read_pool, limit)
            .await
            .map_err(|e| {
                error!("Failed to get staleness monitoring: {}", e);
                Status::internal("Failed to get staleness monitoring")
            })?;

        info!(
            monitoring_count = monitoring.len(),
            "Successfully retrieved staleness monitoring"
        );

        // Convert to proto
        let proto_entries = monitoring
            .iter()
            .map(staleness_monitoring_entry_to_proto)
            .collect();

        Ok(Response::new(proto::GetStalenessMonitoringResponse {
            entries: proto_entries,
        }))
    }
}

// ============================================================================
// Conversion Helpers
// ============================================================================

/// Convert domain DlqEntry to proto DlqEntry.
fn dlq_entry_to_proto(entry: &DlqEntry) -> proto::DlqEntry {
    proto::DlqEntry {
        dlq_entry_uuid: entry.dlq_entry_uuid.to_string(),
        task_uuid: entry.task_uuid.to_string(),
        original_state: entry.original_state.clone(),
        dlq_reason: dlq_reason_to_proto(entry.dlq_reason) as i32,
        dlq_timestamp: Some(datetime_to_timestamp(entry.dlq_timestamp.and_utc())),
        resolution_status: dlq_resolution_status_to_proto(entry.resolution_status) as i32,
        resolution_timestamp: entry
            .resolution_timestamp
            .map(|dt| datetime_to_timestamp(dt.and_utc())),
        resolution_notes: entry.resolution_notes.clone(),
        resolved_by: entry.resolved_by.clone(),
        task_snapshot: json_to_struct(entry.task_snapshot.clone()),
        metadata: entry.metadata.clone().and_then(json_to_struct),
        created_at: Some(datetime_to_timestamp(entry.created_at.and_utc())),
        updated_at: Some(datetime_to_timestamp(entry.updated_at.and_utc())),
    }
}

/// Convert domain DlqResolutionStatus to proto DlqResolutionStatus.
fn dlq_resolution_status_to_proto(
    status: tasker_shared::models::orchestration::dlq::DlqResolutionStatus,
) -> proto::DlqResolutionStatus {
    use tasker_shared::models::orchestration::dlq::DlqResolutionStatus as DomainStatus;
    match status {
        DomainStatus::Pending => proto::DlqResolutionStatus::Pending,
        DomainStatus::ManuallyResolved => proto::DlqResolutionStatus::ManuallyResolved,
        DomainStatus::PermanentlyFailed => proto::DlqResolutionStatus::PermanentlyFailed,
        DomainStatus::Cancelled => proto::DlqResolutionStatus::Cancelled,
    }
}

/// Convert proto DlqResolutionStatus to domain DlqResolutionStatus.
fn dlq_resolution_status_from_proto(
    status: proto::DlqResolutionStatus,
) -> Option<tasker_shared::models::orchestration::dlq::DlqResolutionStatus> {
    use tasker_shared::models::orchestration::dlq::DlqResolutionStatus as DomainStatus;
    match status {
        proto::DlqResolutionStatus::Unspecified => None,
        proto::DlqResolutionStatus::Pending => Some(DomainStatus::Pending),
        proto::DlqResolutionStatus::ManuallyResolved => Some(DomainStatus::ManuallyResolved),
        proto::DlqResolutionStatus::PermanentlyFailed => Some(DomainStatus::PermanentlyFailed),
        proto::DlqResolutionStatus::Cancelled => Some(DomainStatus::Cancelled),
    }
}

/// Convert domain DlqReason to proto DlqReason.
fn dlq_reason_to_proto(
    reason: tasker_shared::models::orchestration::dlq::DlqReason,
) -> proto::DlqReason {
    use tasker_shared::models::orchestration::dlq::DlqReason as DomainReason;
    match reason {
        DomainReason::StalenessTimeout => proto::DlqReason::StalenessTimeout,
        DomainReason::MaxRetriesExceeded => proto::DlqReason::MaxRetriesExceeded,
        DomainReason::DependencyCycleDetected => proto::DlqReason::DependencyCycleDetected,
        DomainReason::WorkerUnavailable => proto::DlqReason::WorkerUnavailable,
        DomainReason::ManualDlq => proto::DlqReason::ManualDlq,
    }
}

/// Convert domain DlqStats to proto DlqStats.
fn dlq_stats_to_proto(stats: &DlqStats) -> proto::DlqStats {
    proto::DlqStats {
        dlq_reason: dlq_reason_to_proto(stats.dlq_reason) as i32,
        total_entries: stats.total_entries,
        pending: stats.pending,
        manually_resolved: stats.manually_resolved,
        permanent_failures: stats.permanent_failures,
        cancelled: stats.cancelled,
        oldest_entry: stats
            .oldest_entry
            .map(|dt| datetime_to_timestamp(dt.and_utc())),
        newest_entry: stats
            .newest_entry
            .map(|dt| datetime_to_timestamp(dt.and_utc())),
        avg_resolution_time_minutes: stats.avg_resolution_time_minutes,
    }
}

/// Convert domain DlqInvestigationQueueEntry to proto DlqInvestigationQueueEntry.
fn dlq_investigation_queue_entry_to_proto(
    entry: &DlqInvestigationQueueEntry,
) -> proto::DlqInvestigationQueueEntry {
    proto::DlqInvestigationQueueEntry {
        dlq_entry_uuid: entry.dlq_entry_uuid.to_string(),
        task_uuid: entry.task_uuid.to_string(),
        original_state: entry.original_state.clone(),
        dlq_reason: dlq_reason_to_proto(entry.dlq_reason) as i32,
        dlq_timestamp: Some(datetime_to_timestamp(entry.dlq_timestamp.and_utc())),
        minutes_in_dlq: entry.minutes_in_dlq,
        namespace_name: entry.namespace_name.clone(),
        task_name: entry.task_name.clone(),
        current_state: entry.current_state.clone(),
        time_in_state_minutes: entry.time_in_state_minutes,
        priority_score: entry.priority_score,
    }
}

/// Convert domain StalenessMonitoring to proto StalenessMonitoringEntry.
fn staleness_monitoring_entry_to_proto(
    entry: &StalenessMonitoring,
) -> proto::StalenessMonitoringEntry {
    proto::StalenessMonitoringEntry {
        task_uuid: entry.task_uuid.to_string(),
        namespace_name: entry.namespace_name.clone(),
        task_name: entry.task_name.clone(),
        current_state: entry.current_state.clone(),
        time_in_state_minutes: entry.time_in_state_minutes,
        task_age_minutes: entry.task_age_minutes,
        staleness_threshold_minutes: entry.staleness_threshold_minutes,
        health_status: staleness_health_status_to_proto(entry.health_status) as i32,
        priority: entry.priority,
    }
}

/// Convert domain StalenessHealthStatus to proto StalenessHealthStatus.
fn staleness_health_status_to_proto(
    status: tasker_shared::models::orchestration::dlq::StalenessHealthStatus,
) -> proto::StalenessHealthStatus {
    use tasker_shared::models::orchestration::dlq::StalenessHealthStatus as DomainStatus;
    match status {
        DomainStatus::Healthy => proto::StalenessHealthStatus::Healthy,
        DomainStatus::Warning => proto::StalenessHealthStatus::Warning,
        DomainStatus::Stale => proto::StalenessHealthStatus::Stale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;
    use tasker_shared::models::orchestration::dlq::{
        DlqReason, DlqResolutionStatus, StalenessHealthStatus,
    };
    use uuid::Uuid;

    fn sample_timestamp() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-01-31 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
    }

    // ---- Resolution status conversion tests ----

    #[test]
    fn test_resolution_status_to_proto_pending() {
        assert!(matches!(
            dlq_resolution_status_to_proto(DlqResolutionStatus::Pending),
            proto::DlqResolutionStatus::Pending
        ));
    }

    #[test]
    fn test_resolution_status_to_proto_manually_resolved() {
        assert!(matches!(
            dlq_resolution_status_to_proto(DlqResolutionStatus::ManuallyResolved),
            proto::DlqResolutionStatus::ManuallyResolved
        ));
    }

    #[test]
    fn test_resolution_status_to_proto_permanently_failed() {
        assert!(matches!(
            dlq_resolution_status_to_proto(DlqResolutionStatus::PermanentlyFailed),
            proto::DlqResolutionStatus::PermanentlyFailed
        ));
    }

    #[test]
    fn test_resolution_status_to_proto_cancelled() {
        assert!(matches!(
            dlq_resolution_status_to_proto(DlqResolutionStatus::Cancelled),
            proto::DlqResolutionStatus::Cancelled
        ));
    }

    #[test]
    fn test_resolution_status_from_proto_unspecified() {
        assert!(dlq_resolution_status_from_proto(proto::DlqResolutionStatus::Unspecified).is_none());
    }

    #[test]
    fn test_resolution_status_from_proto_pending() {
        let result = dlq_resolution_status_from_proto(proto::DlqResolutionStatus::Pending);
        assert!(matches!(result, Some(DlqResolutionStatus::Pending)));
    }

    #[test]
    fn test_resolution_status_from_proto_manually_resolved() {
        let result = dlq_resolution_status_from_proto(proto::DlqResolutionStatus::ManuallyResolved);
        assert!(matches!(result, Some(DlqResolutionStatus::ManuallyResolved)));
    }

    #[test]
    fn test_resolution_status_from_proto_permanently_failed() {
        let result =
            dlq_resolution_status_from_proto(proto::DlqResolutionStatus::PermanentlyFailed);
        assert!(matches!(
            result,
            Some(DlqResolutionStatus::PermanentlyFailed)
        ));
    }

    #[test]
    fn test_resolution_status_from_proto_cancelled() {
        let result = dlq_resolution_status_from_proto(proto::DlqResolutionStatus::Cancelled);
        assert!(matches!(result, Some(DlqResolutionStatus::Cancelled)));
    }

    // ---- DLQ reason conversion tests ----

    #[test]
    fn test_reason_to_proto_staleness_timeout() {
        assert!(matches!(
            dlq_reason_to_proto(DlqReason::StalenessTimeout),
            proto::DlqReason::StalenessTimeout
        ));
    }

    #[test]
    fn test_reason_to_proto_max_retries() {
        assert!(matches!(
            dlq_reason_to_proto(DlqReason::MaxRetriesExceeded),
            proto::DlqReason::MaxRetriesExceeded
        ));
    }

    #[test]
    fn test_reason_to_proto_cycle_detected() {
        assert!(matches!(
            dlq_reason_to_proto(DlqReason::DependencyCycleDetected),
            proto::DlqReason::DependencyCycleDetected
        ));
    }

    #[test]
    fn test_reason_to_proto_worker_unavailable() {
        assert!(matches!(
            dlq_reason_to_proto(DlqReason::WorkerUnavailable),
            proto::DlqReason::WorkerUnavailable
        ));
    }

    #[test]
    fn test_reason_to_proto_manual_dlq() {
        assert!(matches!(
            dlq_reason_to_proto(DlqReason::ManualDlq),
            proto::DlqReason::ManualDlq
        ));
    }

    // ---- Staleness health status conversion tests ----

    #[test]
    fn test_staleness_health_healthy() {
        assert!(matches!(
            staleness_health_status_to_proto(StalenessHealthStatus::Healthy),
            proto::StalenessHealthStatus::Healthy
        ));
    }

    #[test]
    fn test_staleness_health_warning() {
        assert!(matches!(
            staleness_health_status_to_proto(StalenessHealthStatus::Warning),
            proto::StalenessHealthStatus::Warning
        ));
    }

    #[test]
    fn test_staleness_health_stale() {
        assert!(matches!(
            staleness_health_status_to_proto(StalenessHealthStatus::Stale),
            proto::StalenessHealthStatus::Stale
        ));
    }

    // ---- Complex struct conversion tests ----

    #[test]
    fn test_dlq_entry_to_proto() {
        let entry = DlqEntry {
            dlq_entry_uuid: Uuid::nil(),
            task_uuid: Uuid::nil(),
            original_state: "error".to_string(),
            dlq_reason: DlqReason::StalenessTimeout,
            dlq_timestamp: sample_timestamp(),
            resolution_status: DlqResolutionStatus::Pending,
            resolution_timestamp: None,
            resolution_notes: Some("investigating".to_string()),
            resolved_by: None,
            task_snapshot: serde_json::json!({"key": "value"}),
            metadata: None,
            created_at: sample_timestamp(),
            updated_at: sample_timestamp(),
        };

        let proto = dlq_entry_to_proto(&entry);
        assert_eq!(proto.dlq_entry_uuid, Uuid::nil().to_string());
        assert_eq!(proto.task_uuid, Uuid::nil().to_string());
        assert_eq!(proto.original_state, "error");
        assert_eq!(
            proto.dlq_reason,
            proto::DlqReason::StalenessTimeout as i32
        );
        assert_eq!(
            proto.resolution_status,
            proto::DlqResolutionStatus::Pending as i32
        );
        assert!(proto.resolution_timestamp.is_none());
        assert_eq!(proto.resolution_notes, Some("investigating".to_string()));
        assert!(proto.resolved_by.is_none());
        assert!(proto.created_at.is_some());
        assert!(proto.updated_at.is_some());
        assert!(proto.dlq_timestamp.is_some());
        assert!(proto.metadata.is_none());
    }

    #[test]
    fn test_dlq_entry_to_proto_with_resolution() {
        let entry = DlqEntry {
            dlq_entry_uuid: Uuid::nil(),
            task_uuid: Uuid::nil(),
            original_state: "steps_in_process".to_string(),
            dlq_reason: DlqReason::MaxRetriesExceeded,
            dlq_timestamp: sample_timestamp(),
            resolution_status: DlqResolutionStatus::ManuallyResolved,
            resolution_timestamp: Some(sample_timestamp()),
            resolution_notes: Some("resolved by operator".to_string()),
            resolved_by: Some("admin".to_string()),
            task_snapshot: serde_json::json!({}),
            metadata: Some(serde_json::json!({"trace": "abc123"})),
            created_at: sample_timestamp(),
            updated_at: sample_timestamp(),
        };

        let proto = dlq_entry_to_proto(&entry);
        assert!(proto.resolution_timestamp.is_some());
        assert_eq!(proto.resolved_by, Some("admin".to_string()));
        assert!(proto.metadata.is_some());
    }

    #[test]
    fn test_dlq_stats_to_proto() {
        let stats = DlqStats {
            dlq_reason: DlqReason::WorkerUnavailable,
            total_entries: 42,
            pending: 10,
            manually_resolved: 20,
            permanent_failures: 5,
            cancelled: 7,
            oldest_entry: Some(sample_timestamp()),
            newest_entry: Some(sample_timestamp()),
            avg_resolution_time_minutes: Some(15.5),
        };

        let proto = dlq_stats_to_proto(&stats);
        assert_eq!(proto.dlq_reason, proto::DlqReason::WorkerUnavailable as i32);
        assert_eq!(proto.total_entries, 42);
        assert_eq!(proto.pending, 10);
        assert_eq!(proto.manually_resolved, 20);
        assert_eq!(proto.permanent_failures, 5);
        assert_eq!(proto.cancelled, 7);
        assert!(proto.oldest_entry.is_some());
        assert!(proto.newest_entry.is_some());
        assert_eq!(proto.avg_resolution_time_minutes, Some(15.5));
    }

    #[test]
    fn test_dlq_stats_to_proto_no_optional_fields() {
        let stats = DlqStats {
            dlq_reason: DlqReason::ManualDlq,
            total_entries: 0,
            pending: 0,
            manually_resolved: 0,
            permanent_failures: 0,
            cancelled: 0,
            oldest_entry: None,
            newest_entry: None,
            avg_resolution_time_minutes: None,
        };

        let proto = dlq_stats_to_proto(&stats);
        assert!(proto.oldest_entry.is_none());
        assert!(proto.newest_entry.is_none());
        assert!(proto.avg_resolution_time_minutes.is_none());
    }

    #[test]
    fn test_investigation_queue_entry_to_proto() {
        let entry = DlqInvestigationQueueEntry {
            dlq_entry_uuid: Uuid::nil(),
            task_uuid: Uuid::nil(),
            original_state: "error".to_string(),
            dlq_reason: DlqReason::DependencyCycleDetected,
            dlq_timestamp: sample_timestamp(),
            minutes_in_dlq: 120.5,
            namespace_name: Some("default".to_string()),
            task_name: Some("process_data".to_string()),
            current_state: Some("error".to_string()),
            time_in_state_minutes: Some(60),
            priority_score: 95.0,
        };

        let proto = dlq_investigation_queue_entry_to_proto(&entry);
        assert_eq!(proto.dlq_entry_uuid, Uuid::nil().to_string());
        assert_eq!(proto.minutes_in_dlq, 120.5);
        assert_eq!(proto.namespace_name, Some("default".to_string()));
        assert_eq!(proto.task_name, Some("process_data".to_string()));
        assert_eq!(proto.priority_score, 95.0);
        assert_eq!(
            proto.dlq_reason,
            proto::DlqReason::DependencyCycleDetected as i32
        );
    }

    #[test]
    fn test_staleness_monitoring_entry_to_proto() {
        let entry = StalenessMonitoring {
            task_uuid: Uuid::nil(),
            namespace_name: Some("production".to_string()),
            task_name: Some("daily_report".to_string()),
            current_state: "steps_in_process".to_string(),
            time_in_state_minutes: 45,
            task_age_minutes: 120,
            staleness_threshold_minutes: 30,
            health_status: StalenessHealthStatus::Stale,
            priority: 3,
        };

        let proto = staleness_monitoring_entry_to_proto(&entry);
        assert_eq!(proto.task_uuid, Uuid::nil().to_string());
        assert_eq!(proto.namespace_name, Some("production".to_string()));
        assert_eq!(proto.current_state, "steps_in_process");
        assert_eq!(proto.time_in_state_minutes, 45);
        assert_eq!(proto.task_age_minutes, 120);
        assert_eq!(proto.staleness_threshold_minutes, 30);
        assert_eq!(
            proto.health_status,
            proto::StalenessHealthStatus::Stale as i32
        );
        assert_eq!(proto.priority, 3);
    }

    // ---- DlqServiceImpl construction test ----

    #[test]
    fn test_dlq_service_impl_debug() {
        // Verify Debug is derived (compile-time check via format)
        let _: fn(&DlqServiceImpl, &mut std::fmt::Formatter<'_>) -> std::fmt::Result =
            <DlqServiceImpl as std::fmt::Debug>::fmt;
    }
}
