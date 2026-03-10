//! Pool lifecycle configuration types.

use std::time::Duration;

/// Whether a resource was statically configured or dynamically created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceOrigin {
    /// From worker.toml configuration — never evicted.
    Static,
    /// Created at runtime by generative workflows — subject to eviction.
    Dynamic,
}

/// Strategy for evicting idle resource pools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictionStrategy {
    /// Least Recently Used — evict the pool that was accessed longest ago.
    Lru,
    /// Least Frequently Used — evict the pool with the fewest accesses.
    Lfu,
    /// First In, First Out — evict the oldest pool.
    Fifo,
}

/// Strategy for admitting new resource pools when at capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionStrategy {
    /// Reject new pools when at capacity.
    Reject,
    /// Evict an existing pool to make room.
    EvictOne,
}

/// Configuration for the resource pool manager.
#[derive(Debug, Clone)]
pub struct PoolManagerConfig {
    /// Maximum number of distinct resource pools.
    pub max_pools: usize,
    /// Maximum total connections across all pools.
    pub max_total_connections: usize,
    /// Idle timeout before a dynamic pool becomes eligible for eviction.
    pub idle_timeout: Duration,
    /// Interval between eviction sweeps.
    pub sweep_interval: Duration,
    /// Strategy for choosing which pool to evict.
    pub eviction_strategy: EvictionStrategy,
    /// Strategy for handling new pool requests when at capacity.
    pub admission_strategy: AdmissionStrategy,
}

impl Default for PoolManagerConfig {
    fn default() -> Self {
        Self {
            max_pools: 32,
            max_total_connections: 256,
            idle_timeout: Duration::from_secs(300),
            sweep_interval: Duration::from_secs(60),
            eviction_strategy: EvictionStrategy::Lru,
            admission_strategy: AdmissionStrategy::EvictOne,
        }
    }
}
