//! # Orchestration Statistics
//!
//! Runtime statistics tracking for the orchestration event system.
//! Provides atomic counters for event processing metrics and component aggregation.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tasker_shared::{DeploymentMode, EventSystemStatistics};

use crate::orchestration::orchestration_queues::{
    OrchestrationFallbackPoller, OrchestrationListenerStats, OrchestrationPollerStats,
    OrchestrationQueueListener,
};

/// Runtime statistics for orchestration event system
#[derive(Debug, Default)]
pub struct OrchestrationStatistics {
    /// Events processed counter (system-level)
    pub(crate) events_processed: AtomicU64,
    /// Events failed counter (system-level)
    pub(crate) events_failed: AtomicU64,
    /// Operations coordinated counter (system-level)
    pub(crate) operations_coordinated: AtomicU64,
    /// Last processing timestamp as epoch nanos (0 = never processed)
    pub(crate) last_processing_time_epoch_nanos: AtomicU64,
    /// Processing latencies for rate calculation (system-level)
    pub(crate) processing_latencies: std::sync::Mutex<VecDeque<Duration>>,
}

impl Clone for OrchestrationStatistics {
    fn clone(&self) -> Self {
        Self {
            events_processed: AtomicU64::new(self.events_processed.load(Ordering::Relaxed)),
            events_failed: AtomicU64::new(self.events_failed.load(Ordering::Relaxed)),
            operations_coordinated: AtomicU64::new(
                self.operations_coordinated.load(Ordering::Relaxed),
            ),
            last_processing_time_epoch_nanos: AtomicU64::new(
                self.last_processing_time_epoch_nanos
                    .load(Ordering::Relaxed),
            ),
            processing_latencies: std::sync::Mutex::new(
                self.processing_latencies
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone(),
            ),
        }
    }
}

impl EventSystemStatistics for OrchestrationStatistics {
    fn events_processed(&self) -> u64 {
        self.events_processed.load(Ordering::Relaxed)
    }

    fn events_failed(&self) -> u64 {
        self.events_failed.load(Ordering::Relaxed)
    }

    fn processing_rate(&self) -> f64 {
        let latencies = self
            .processing_latencies
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if latencies.is_empty() {
            return 0.0;
        }

        // Calculate events per second based on recent latencies
        let recent_latencies: Vec<_> = latencies.iter().rev().take(100).collect();
        if recent_latencies.is_empty() {
            return 0.0;
        }

        let total_time: Duration = recent_latencies.iter().copied().sum();
        if total_time.as_secs_f64() > 0.0 {
            recent_latencies.len() as f64 / total_time.as_secs_f64()
        } else {
            0.0
        }
    }

    fn average_latency_ms(&self) -> f64 {
        let latencies = self
            .processing_latencies
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if latencies.is_empty() {
            return 0.0;
        }

        let sum: Duration = latencies.iter().sum();
        sum.as_millis() as f64 / latencies.len() as f64
    }

    fn deployment_mode_score(&self) -> f64 {
        // Score based on success rate and processing efficiency
        let total_events = self.events_processed() + self.events_failed();
        if total_events == 0 {
            return 1.0; // No events yet, assume perfect
        }

        let success_rate = self.events_processed() as f64 / total_events as f64;
        let latency = self.average_latency_ms();

        // High score for high success rate and low latency
        let latency_score = if latency > 0.0 { 100.0 / latency } else { 1.0 };
        (success_rate + latency_score.min(1.0)) / 2.0
    }
}

impl OrchestrationStatistics {
    /// Create statistics aggregated from component statistics
    pub async fn with_component_aggregation(
        &self,
        fallback_poller: Option<&OrchestrationFallbackPoller>,
        queue_listener: Option<&OrchestrationQueueListener>,
    ) -> OrchestrationStatistics {
        let aggregated = self.clone();

        // Aggregate fallback poller statistics
        if let Some(poller) = fallback_poller {
            let poller_stats = poller.stats().await;
            // Add poller-specific events to system total
            let poller_processed = poller_stats.messages_processed.load(Ordering::Relaxed);
            aggregated
                .events_processed
                .fetch_add(poller_processed, Ordering::Relaxed);

            // Add poller errors to system failed events
            let poller_errors = poller_stats.polling_errors.load(Ordering::Relaxed);
            aggregated
                .events_failed
                .fetch_add(poller_errors, Ordering::Relaxed);
        }

        // Aggregate queue listener statistics
        if let Some(listener) = queue_listener {
            let listener_stats = listener.stats().await;
            // Add listener-specific events to system total
            let listener_processed = listener_stats.events_received.load(Ordering::Relaxed);
            aggregated
                .events_processed
                .fetch_add(listener_processed, Ordering::Relaxed);

            // Add listener errors to system failed events
            let listener_errors = listener_stats.connection_errors.load(Ordering::Relaxed);
            aggregated
                .events_failed
                .fetch_add(listener_errors, Ordering::Relaxed);
        }

        aggregated
    }
}

/// Detailed component statistics for monitoring and debugging
#[derive(Debug)]
pub struct OrchestrationComponentStatistics {
    /// Fallback poller statistics (if active)
    pub fallback_poller_stats: Option<OrchestrationPollerStats>,
    /// Queue listener statistics (if active)
    pub queue_listener_stats: Option<OrchestrationListenerStats>,
    /// System uptime since start
    pub system_uptime: Option<Duration>,
    /// Current deployment mode
    pub deployment_mode: DeploymentMode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_statistics_zeroed() {
        let stats = OrchestrationStatistics::default();
        assert_eq!(stats.events_processed.load(Ordering::Relaxed), 0);
        assert_eq!(stats.events_failed.load(Ordering::Relaxed), 0);
        assert_eq!(stats.operations_coordinated.load(Ordering::Relaxed), 0);
        assert_eq!(
            stats
                .last_processing_time_epoch_nanos
                .load(Ordering::Relaxed),
            0
        );
        assert!(stats.processing_latencies.lock().unwrap().is_empty());
    }

    #[test]
    fn test_clone_preserves_counter_values() {
        let stats = OrchestrationStatistics::default();
        stats.events_processed.store(42, Ordering::Relaxed);
        stats.events_failed.store(7, Ordering::Relaxed);
        stats.operations_coordinated.store(100, Ordering::Relaxed);
        stats
            .last_processing_time_epoch_nanos
            .store(999, Ordering::Relaxed);

        let cloned = stats.clone();

        assert_eq!(cloned.events_processed.load(Ordering::Relaxed), 42);
        assert_eq!(cloned.events_failed.load(Ordering::Relaxed), 7);
        assert_eq!(cloned.operations_coordinated.load(Ordering::Relaxed), 100);
        assert_eq!(
            cloned
                .last_processing_time_epoch_nanos
                .load(Ordering::Relaxed),
            999
        );
    }

    #[test]
    fn test_clone_preserves_latencies() {
        let stats = OrchestrationStatistics::default();
        {
            let mut latencies = stats.processing_latencies.lock().unwrap();
            latencies.push_back(Duration::from_millis(10));
            latencies.push_back(Duration::from_millis(20));
            latencies.push_back(Duration::from_millis(30));
        }

        let cloned = stats.clone();
        let cloned_latencies = cloned.processing_latencies.lock().unwrap();
        assert_eq!(cloned_latencies.len(), 3);
        assert_eq!(cloned_latencies[0], Duration::from_millis(10));
        assert_eq!(cloned_latencies[1], Duration::from_millis(20));
        assert_eq!(cloned_latencies[2], Duration::from_millis(30));
    }

    #[test]
    fn test_events_processed_counter() {
        let stats = OrchestrationStatistics::default();
        stats.events_processed.fetch_add(5, Ordering::Relaxed);
        assert_eq!(stats.events_processed(), 5);

        stats.events_processed.fetch_add(3, Ordering::Relaxed);
        assert_eq!(stats.events_processed(), 8);
    }

    #[test]
    fn test_events_failed_counter() {
        let stats = OrchestrationStatistics::default();
        stats.events_failed.fetch_add(2, Ordering::Relaxed);
        assert_eq!(stats.events_failed(), 2);

        stats.events_failed.fetch_add(1, Ordering::Relaxed);
        assert_eq!(stats.events_failed(), 3);
    }

    #[test]
    fn test_processing_rate_empty_returns_zero() {
        let stats = OrchestrationStatistics::default();
        assert_eq!(stats.processing_rate(), 0.0);
    }

    #[test]
    fn test_processing_rate_with_known_latencies() {
        let stats = OrchestrationStatistics::default();
        {
            let mut latencies = stats.processing_latencies.lock().unwrap();
            // 10 events, each taking 100ms = 1 second total → 10 events/sec
            for _ in 0..10 {
                latencies.push_back(Duration::from_millis(100));
            }
        }

        let rate = stats.processing_rate();
        // 10 events / 1.0 sec = 10.0 events/sec
        assert!((rate - 10.0).abs() < 0.01, "Expected ~10.0, got {}", rate);
    }

    #[test]
    fn test_average_latency_empty_returns_zero() {
        let stats = OrchestrationStatistics::default();
        assert_eq!(stats.average_latency_ms(), 0.0);
    }

    #[test]
    fn test_average_latency_calculation() {
        let stats = OrchestrationStatistics::default();
        {
            let mut latencies = stats.processing_latencies.lock().unwrap();
            latencies.push_back(Duration::from_millis(10));
            latencies.push_back(Duration::from_millis(20));
            latencies.push_back(Duration::from_millis(30));
        }

        let avg = stats.average_latency_ms();
        // (10 + 20 + 30) / 3 = 20.0
        assert!((avg - 20.0).abs() < 0.01, "Expected ~20.0, got {}", avg);
    }

    #[test]
    fn test_deployment_mode_score_no_events() {
        let stats = OrchestrationStatistics::default();
        // No events yet → returns 1.0 (assume perfect)
        assert_eq!(stats.deployment_mode_score(), 1.0);
    }

    #[test]
    fn test_deployment_mode_score_all_success_low_latency() {
        let stats = OrchestrationStatistics::default();
        stats.events_processed.store(100, Ordering::Relaxed);
        // No failures
        {
            let mut latencies = stats.processing_latencies.lock().unwrap();
            // Very low latency: 1ms each
            for _ in 0..100 {
                latencies.push_back(Duration::from_millis(1));
            }
        }

        let score = stats.deployment_mode_score();
        // success_rate = 100/100 = 1.0
        // avg_latency = 1.0ms
        // latency_score = min(100.0 / 1.0, 1.0) = 1.0
        // score = (1.0 + 1.0) / 2.0 = 1.0
        assert!((score - 1.0).abs() < 0.01, "Expected ~1.0, got {}", score);
    }

    #[test]
    fn test_deployment_mode_score_with_failures() {
        let stats = OrchestrationStatistics::default();
        stats.events_processed.store(50, Ordering::Relaxed);
        stats.events_failed.store(50, Ordering::Relaxed);
        {
            let mut latencies = stats.processing_latencies.lock().unwrap();
            for _ in 0..50 {
                latencies.push_back(Duration::from_millis(1));
            }
        }

        let score = stats.deployment_mode_score();
        // success_rate = 50/100 = 0.5
        // avg_latency = 1.0ms
        // latency_score = min(100.0 / 1.0, 1.0) = 1.0
        // score = (0.5 + 1.0) / 2.0 = 0.75
        assert!((score - 0.75).abs() < 0.01, "Expected ~0.75, got {}", score);
    }
}
