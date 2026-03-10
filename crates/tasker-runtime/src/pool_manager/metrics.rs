//! Access metrics for pool eviction decisions.

use std::time::Instant;

/// Tracks access patterns for a single resource pool.
#[derive(Debug, Clone)]
pub struct ResourceAccessMetrics {
    /// When the pool was created.
    pub creation_time: Instant,
    /// When the pool was last accessed.
    pub last_accessed: Instant,
    /// Total number of accesses.
    pub access_count: u64,
}

impl ResourceAccessMetrics {
    /// Create metrics for a newly created pool.
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            creation_time: now,
            last_accessed: now,
            access_count: 0,
        }
    }

    /// Record an access to the pool.
    pub fn record_access(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

impl Default for ResourceAccessMetrics {
    fn default() -> Self {
        Self::new()
    }
}
