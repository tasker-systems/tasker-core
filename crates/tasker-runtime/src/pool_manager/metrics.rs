//! Access metrics for pool eviction and observability.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Tracks access patterns for a single resource pool.
#[derive(Debug)]
pub struct ResourceAccessMetrics {
    /// When the pool was created.
    pub creation_time: Instant,
    /// When the pool was last accessed.
    pub last_accessed: Instant,
    /// Total number of accesses.
    pub access_count: u64,
    /// Currently in-flight operations (for liveness protection).
    pub active_checkouts: u64,
    /// Estimated connection count for budget enforcement.
    pub estimated_connections: u32,
}

impl ResourceAccessMetrics {
    /// Create metrics for a newly created pool.
    pub fn new(estimated_connections: u32) -> Self {
        let now = Instant::now();
        Self {
            creation_time: now,
            last_accessed: now,
            access_count: 0,
            active_checkouts: 0,
            estimated_connections,
        }
    }

    /// Record an access to the pool.
    pub fn record_access(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

/// Aggregate metrics for the pool manager (safe for telemetry export).
#[derive(Debug)]
pub struct PoolManagerMetrics {
    /// Total number of pools currently managed.
    pub total_pools: AtomicU64,
    /// Number of static (non-evictable) pools.
    pub static_pools: AtomicU64,
    /// Number of dynamic (evictable) pools.
    pub dynamic_pools: AtomicU64,
    /// Estimated total connections across all pools.
    pub estimated_total_connections: AtomicU64,
    /// Number of admission rejections (autoscaling signal).
    pub admission_rejections: AtomicU64,
    /// Number of evictions performed.
    pub evictions_performed: AtomicU64,
}

impl PoolManagerMetrics {
    /// Create a new metrics instance with all counters at zero.
    pub fn new() -> Self {
        Self {
            total_pools: AtomicU64::new(0),
            static_pools: AtomicU64::new(0),
            dynamic_pools: AtomicU64::new(0),
            estimated_total_connections: AtomicU64::new(0),
            admission_rejections: AtomicU64::new(0),
            evictions_performed: AtomicU64::new(0),
        }
    }

    /// Take a point-in-time snapshot of all counters.
    pub fn snapshot(&self) -> PoolManagerMetricsSnapshot {
        PoolManagerMetricsSnapshot {
            total_pools: self.total_pools.load(Ordering::Relaxed),
            static_pools: self.static_pools.load(Ordering::Relaxed),
            dynamic_pools: self.dynamic_pools.load(Ordering::Relaxed),
            estimated_total_connections: self.estimated_total_connections.load(Ordering::Relaxed),
            admission_rejections: self.admission_rejections.load(Ordering::Relaxed),
            evictions_performed: self.evictions_performed.load(Ordering::Relaxed),
        }
    }
}

impl Default for PoolManagerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Point-in-time snapshot of pool manager metrics.
#[derive(Debug, Clone)]
pub struct PoolManagerMetricsSnapshot {
    /// Total number of pools currently managed.
    pub total_pools: u64,
    /// Number of static (non-evictable) pools.
    pub static_pools: u64,
    /// Number of dynamic (evictable) pools.
    pub dynamic_pools: u64,
    /// Estimated total connections across all pools.
    pub estimated_total_connections: u64,
    /// Number of admission rejections (autoscaling signal).
    pub admission_rejections: u64,
    /// Number of evictions performed.
    pub evictions_performed: u64,
}
