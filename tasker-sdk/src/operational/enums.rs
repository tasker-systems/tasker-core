//! Enum parsing utilities for DLQ resolution status and task status strings.

use tasker_shared::models::orchestration::DlqResolutionStatus;

/// Parse a DLQ resolution status string into the enum.
///
/// Accepts: "pending", "manually_resolved", "permanently_failed", "cancelled" (case-insensitive).
pub fn parse_dlq_resolution_status(s: &str) -> Result<DlqResolutionStatus, String> {
    match s.to_lowercase().as_str() {
        "pending" => Ok(DlqResolutionStatus::Pending),
        "manually_resolved" => Ok(DlqResolutionStatus::ManuallyResolved),
        "permanently_failed" => Ok(DlqResolutionStatus::PermanentlyFailed),
        "cancelled" => Ok(DlqResolutionStatus::Cancelled),
        _ => Err(format!(
            "Unknown DLQ resolution status '{}'. Valid values: pending, manually_resolved, permanently_failed, cancelled",
            s
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dlq_resolution_status_valid() {
        assert_eq!(
            parse_dlq_resolution_status("pending").unwrap(),
            DlqResolutionStatus::Pending
        );
        assert_eq!(
            parse_dlq_resolution_status("manually_resolved").unwrap(),
            DlqResolutionStatus::ManuallyResolved
        );
        assert_eq!(
            parse_dlq_resolution_status("permanently_failed").unwrap(),
            DlqResolutionStatus::PermanentlyFailed
        );
        assert_eq!(
            parse_dlq_resolution_status("cancelled").unwrap(),
            DlqResolutionStatus::Cancelled
        );
    }

    #[test]
    fn test_parse_dlq_resolution_status_case_insensitive() {
        assert_eq!(
            parse_dlq_resolution_status("PENDING").unwrap(),
            DlqResolutionStatus::Pending
        );
        assert_eq!(
            parse_dlq_resolution_status("Manually_Resolved").unwrap(),
            DlqResolutionStatus::ManuallyResolved
        );
    }

    #[test]
    fn test_parse_dlq_resolution_status_invalid() {
        let result = parse_dlq_resolution_status("invalid");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Unknown DLQ resolution status"));
    }
}
