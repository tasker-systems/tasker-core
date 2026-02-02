//! # Database Health Evaluator
//!
//! TAS-75 Phase 5: Evaluates database connection health and circuit breaker state.
//!
//! This module provides functions to check database health without blocking the
//! API hot path. The evaluator is called by the background `StatusEvaluator` task.

use sqlx::PgPool;
use std::time::{Duration, Instant};
use tracing::{debug, error};

use super::types::{DatabaseHealthConfig, DatabaseHealthStatus};
use crate::api_common::{CircuitState, WebDatabaseCircuitBreaker};

/// Evaluate database health status
///
/// Performs a lightweight health check query and combines with circuit breaker state.
///
/// # Arguments
/// * `pool` - Database connection pool
/// * `circuit_breaker` - Circuit breaker for database operations
/// * `config` - Health check configuration
///
/// # Returns
/// `DatabaseHealthStatus` with current health information
pub async fn evaluate_db_status(
    pool: &PgPool,
    circuit_breaker: &WebDatabaseCircuitBreaker,
    config: &DatabaseHealthConfig,
) -> DatabaseHealthStatus {
    let start = Instant::now();

    // Get circuit breaker state
    let circuit_breaker_open = circuit_breaker.is_circuit_open();
    let circuit_breaker_failures = circuit_breaker.current_failures();

    // If circuit breaker is open, skip the actual check
    if circuit_breaker_open {
        debug!(
            circuit_breaker_state = ?CircuitState::Open,
            failures = circuit_breaker_failures,
            "Database health check skipped - circuit breaker open"
        );

        return DatabaseHealthStatus {
            evaluated: true, // We did evaluate - we know the circuit breaker is open
            is_connected: false,
            circuit_breaker_open: true,
            circuit_breaker_failures,
            last_check_duration_ms: 0,
            error_message: Some("Circuit breaker open".to_string()),
        };
    }

    // Perform the health check query with timeout
    let timeout = Duration::from_millis(config.query_timeout_ms);
    let check_result = tokio::time::timeout(timeout, check_database_connection(pool)).await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match check_result {
        Ok(Ok(())) => {
            debug!(
                duration_ms = duration_ms,
                "Database health check successful"
            );

            DatabaseHealthStatus {
                evaluated: true,
                is_connected: true,
                circuit_breaker_open: false,
                circuit_breaker_failures,
                last_check_duration_ms: duration_ms,
                error_message: None,
            }
        }
        Ok(Err(e)) => {
            error!(
                error = %e,
                duration_ms = duration_ms,
                "Database health check failed"
            );

            DatabaseHealthStatus {
                evaluated: true,
                is_connected: false,
                circuit_breaker_open,
                circuit_breaker_failures,
                last_check_duration_ms: duration_ms,
                error_message: Some(e.to_string()),
            }
        }
        Err(_elapsed) => {
            error!(
                timeout_ms = config.query_timeout_ms,
                duration_ms = duration_ms,
                "Database health check timed out"
            );

            DatabaseHealthStatus {
                evaluated: true,
                is_connected: false,
                circuit_breaker_open,
                circuit_breaker_failures,
                last_check_duration_ms: duration_ms,
                error_message: Some(format!(
                    "Health check timed out after {}ms",
                    config.query_timeout_ms
                )),
            }
        }
    }
}

/// Perform a simple database connection check
///
/// Uses a lightweight query to verify database connectivity.
async fn check_database_connection(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DatabaseHealthConfig::default();
        assert_eq!(config.query_timeout_ms, 1000);
    }

    #[tokio::test]
    async fn test_evaluate_with_open_circuit_breaker() {
        // Create a circuit breaker and force it open
        let cb = WebDatabaseCircuitBreaker::new(2, Duration::from_secs(30), "test");
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_circuit_open());

        // Create a dummy pool (won't be used since circuit is open)
        // We can't easily create a real pool in unit tests, so we test the
        // circuit breaker path which doesn't require a pool
        // Note: We verify the circuit breaker state directly since we can't mock
        // the pool easily for the full evaluate function
        assert!(cb.is_circuit_open());
        assert_eq!(cb.current_failures(), 2);
    }

    #[test]
    fn test_circuit_breaker_state_detection() {
        let cb = WebDatabaseCircuitBreaker::new(3, Duration::from_secs(30), "test");

        // Initially closed
        assert!(!cb.is_circuit_open());
        assert_eq!(cb.current_failures(), 0);

        // Add failures
        cb.record_failure();
        cb.record_failure();
        assert!(!cb.is_circuit_open());

        // Third failure opens the circuit
        cb.record_failure();
        assert!(cb.is_circuit_open());
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_evaluate_db_status_healthy(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cb = WebDatabaseCircuitBreaker::new(5, Duration::from_secs(30), "test");
        let config = DatabaseHealthConfig::default();

        let status = evaluate_db_status(&pool, &cb, &config).await;

        assert!(status.evaluated);
        assert!(status.is_connected);
        assert!(!status.circuit_breaker_open);
        assert_eq!(status.circuit_breaker_failures, 0);
        assert!(status.error_message.is_none());
        assert!(status.last_check_duration_ms < config.query_timeout_ms);
        Ok(())
    }

    #[sqlx::test(migrator = "tasker_shared::database::migrator::MIGRATOR")]
    async fn test_evaluate_db_status_circuit_breaker_open(
        pool: sqlx::PgPool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cb = WebDatabaseCircuitBreaker::new(2, Duration::from_secs(30), "test");
        cb.record_failure();
        cb.record_failure();

        let config = DatabaseHealthConfig::default();
        let status = evaluate_db_status(&pool, &cb, &config).await;

        assert!(status.evaluated);
        assert!(!status.is_connected);
        assert!(status.circuit_breaker_open);
        assert_eq!(status.circuit_breaker_failures, 2);
        assert!(status.error_message.is_some());
        assert!(status
            .error_message
            .unwrap()
            .contains("Circuit breaker open"));
        assert_eq!(status.last_check_duration_ms, 0);
        Ok(())
    }

    #[test]
    fn test_circuit_breaker_success_resets_failures() {
        let cb = WebDatabaseCircuitBreaker::new(3, Duration::from_secs(30), "test");

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.current_failures(), 2);

        cb.record_success();
        assert_eq!(cb.current_failures(), 0);
        assert!(!cb.is_circuit_open());
    }

    #[test]
    fn test_database_health_status_fields() {
        let status = DatabaseHealthStatus {
            evaluated: true,
            is_connected: true,
            circuit_breaker_open: false,
            circuit_breaker_failures: 0,
            last_check_duration_ms: 5,
            error_message: None,
        };

        assert!(status.evaluated);
        assert!(status.is_connected);
        assert!(!status.circuit_breaker_open);
        assert_eq!(status.last_check_duration_ms, 5);
    }

    #[test]
    fn test_database_health_status_with_error() {
        let status = DatabaseHealthStatus {
            evaluated: true,
            is_connected: false,
            circuit_breaker_open: false,
            circuit_breaker_failures: 1,
            last_check_duration_ms: 50,
            error_message: Some("connection refused".to_string()),
        };

        assert!(!status.is_connected);
        assert_eq!(status.circuit_breaker_failures, 1);
        assert!(status.error_message.unwrap().contains("connection refused"));
    }
}
