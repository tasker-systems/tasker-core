//! Resource pool manager with lifecycle management, eviction, and admission control.
//!
//! Wraps `tasker_secure::ResourceRegistry` with dynamic pool creation,
//! eviction policies, and connection budget enforcement.

mod lifecycle;
mod metrics;

pub use lifecycle::{AdmissionStrategy, EvictionStrategy, PoolManagerConfig, ResourceOrigin};
pub use metrics::{PoolManagerMetrics, PoolManagerMetricsSnapshot, ResourceAccessMetrics};

use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;

use crate::sources::ResourceHandleResolver;
use tasker_secure::{ResourceError, ResourceHandle, ResourceRegistry, ResourceSummary};

/// Manages resource pool lifecycle with eviction and admission control.
///
/// Wraps a `ResourceRegistry` and adds:
/// - Dynamic pool creation for generative workflows
/// - Eviction of idle dynamic pools based on configurable strategy
/// - Admission control when at capacity
/// - Connection budget enforcement across all pools
#[derive(Debug)]
pub struct ResourcePoolManager {
    registry: Arc<ResourceRegistry>,
    config: PoolManagerConfig,
    origins: RwLock<HashMap<String, ResourceOrigin>>,
    access_metrics: RwLock<HashMap<String, ResourceAccessMetrics>>,
    pool_metrics: PoolManagerMetrics,
}

impl ResourcePoolManager {
    /// Create a new pool manager wrapping the given registry.
    pub fn new(registry: Arc<ResourceRegistry>, config: PoolManagerConfig) -> Self {
        Self {
            registry,
            config,
            origins: RwLock::new(HashMap::new()),
            access_metrics: RwLock::new(HashMap::new()),
            pool_metrics: PoolManagerMetrics::new(),
        }
    }

    /// Register a resource handle with admission control and connection budget enforcement.
    ///
    /// Admission is rejected when:
    /// - The pool count is at `max_pools` (with `Reject` strategy), or
    /// - The connection budget would be exceeded.
    ///
    /// With `EvictOne` strategy, the manager attempts to evict an idle dynamic
    /// resource before rejecting.
    pub async fn register(
        &self,
        name: &str,
        handle: Arc<dyn ResourceHandle>,
        origin: ResourceOrigin,
        estimated_connections: u32,
    ) -> Result<(), ResourceError> {
        // Check admission constraints
        let origins = self.origins.read().await;
        let current_count = origins.len();
        let metrics_map = self.access_metrics.read().await;
        let current_connections: u64 = metrics_map
            .values()
            .map(|m| u64::from(m.estimated_connections))
            .sum();
        drop(metrics_map);
        drop(origins);

        let pool_full = current_count >= self.config.max_pools;
        let budget_exceeded = current_connections + u64::from(estimated_connections)
            > self.config.max_total_connections as u64;

        if pool_full || budget_exceeded {
            match self.config.admission_strategy {
                AdmissionStrategy::Reject => {
                    self.pool_metrics
                        .admission_rejections
                        .fetch_add(1, Ordering::Relaxed);
                    return Err(ResourceError::InitializationFailed {
                        name: name.to_string(),
                        message: if pool_full {
                            format!(
                                "Pool capacity exhausted ({}/{})",
                                current_count, self.config.max_pools
                            )
                        } else {
                            format!(
                                "Connection budget exceeded ({}/{})",
                                current_connections, self.config.max_total_connections
                            )
                        },
                    });
                }
                AdmissionStrategy::EvictOne => {
                    if !self.try_evict_one().await {
                        self.pool_metrics
                            .admission_rejections
                            .fetch_add(1, Ordering::Relaxed);
                        return Err(ResourceError::InitializationFailed {
                            name: name.to_string(),
                            message: "No eligible resources to evict".to_string(),
                        });
                    }
                }
            }
        }

        // Admit the resource
        self.registry.register(name, handle).await;

        let is_static = matches!(origin, ResourceOrigin::Static);
        let mut origins = self.origins.write().await;
        origins.insert(name.to_string(), origin);
        drop(origins);

        let mut metrics_map = self.access_metrics.write().await;
        metrics_map.insert(
            name.to_string(),
            ResourceAccessMetrics::new(estimated_connections),
        );
        drop(metrics_map);

        self.pool_metrics
            .total_pools
            .fetch_add(1, Ordering::Relaxed);
        if is_static {
            self.pool_metrics
                .static_pools
                .fetch_add(1, Ordering::Relaxed);
        } else {
            self.pool_metrics
                .dynamic_pools
                .fetch_add(1, Ordering::Relaxed);
        }
        self.pool_metrics
            .estimated_total_connections
            .fetch_add(u64::from(estimated_connections), Ordering::Relaxed);

        Ok(())
    }

    /// Get a resource handle by name, updating access metrics.
    pub async fn get(&self, name: &str) -> Result<Arc<dyn ResourceHandle>, ResourceError> {
        let handle = self
            .registry
            .get(name)
            .ok_or_else(|| ResourceError::ResourceNotFound {
                name: name.to_string(),
            })?;

        let mut metrics_map = self.access_metrics.write().await;
        if let Some(m) = metrics_map.get_mut(name) {
            m.record_access();
        }

        Ok(handle)
    }

    /// Get a resource handle by name, or initialize it via the given source.
    ///
    /// Flow:
    /// 1. Try `self.get(name)` — if found, return it
    /// 2. If `ResourceNotFound` and `source` is `Some`, call `source.resolve(name)`
    /// 3. Register the returned handle as `Dynamic` with 1 estimated connection
    /// 4. Return the handle
    /// 5. If `ResourceNotFound` and `source` is `None`, propagate the error
    pub async fn get_or_initialize(
        &self,
        name: &str,
        source: Option<&dyn ResourceHandleResolver>,
    ) -> Result<Arc<dyn ResourceHandle>, ResourceError> {
        match self.get(name).await {
            Ok(handle) => Ok(handle),
            Err(ResourceError::ResourceNotFound { .. }) => {
                let Some(source) = source else {
                    return Err(ResourceError::ResourceNotFound {
                        name: name.to_string(),
                    });
                };

                let handle = source.resolve(name).await.map_err(|e| {
                    ResourceError::InitializationFailed {
                        name: name.to_string(),
                        message: e.to_string(),
                    }
                })?;

                self.register(name, handle.clone(), ResourceOrigin::Dynamic, 1)
                    .await?;

                Ok(handle)
            }
            Err(other) => Err(other),
        }
    }

    /// Evict a specific resource pool by name.
    ///
    /// Static resources cannot be evicted and will return an error.
    pub async fn evict(&self, name: &str) -> Result<(), ResourceError> {
        let origins = self.origins.read().await;
        if let Some(ResourceOrigin::Static) = origins.get(name) {
            return Err(ResourceError::InitializationFailed {
                name: name.to_string(),
                message: "Cannot evict static resource".to_string(),
            });
        }
        drop(origins);
        self.do_evict(name).await;
        Ok(())
    }

    /// Run an eviction sweep based on the configured strategy.
    ///
    /// Returns `(candidates_found, evicted_count)`. Only idle dynamic resources
    /// with zero active checkouts are eligible for eviction.
    pub async fn sweep(&self) -> (usize, usize) {
        let now = Instant::now();
        let origins = self.origins.read().await;
        let metrics_map = self.access_metrics.read().await;

        let mut candidates: Vec<(String, Instant, u64)> = Vec::new();
        for (name, origin) in origins.iter() {
            if matches!(origin, ResourceOrigin::Static) {
                continue;
            }
            if let Some(m) = metrics_map.get(name) {
                let idle_duration = now.duration_since(m.last_accessed);
                if idle_duration >= self.config.idle_timeout && m.active_checkouts == 0 {
                    candidates.push((name.clone(), m.last_accessed, m.access_count));
                }
            }
        }

        let candidates_found = candidates.len();
        drop(metrics_map);
        drop(origins);

        if candidates_found == 0 {
            return (0, 0);
        }

        // Sort by eviction strategy
        match self.config.eviction_strategy {
            EvictionStrategy::Lru => candidates.sort_by_key(|(_, t, _)| *t),
            EvictionStrategy::Lfu => candidates.sort_by_key(|(_, _, count)| *count),
            EvictionStrategy::Fifo => {
                // Re-read metrics for creation_time sorting
                let metrics_map = self.access_metrics.read().await;
                candidates.sort_by_key(|(name, _, _)| {
                    metrics_map
                        .get(name)
                        .map(|m| m.creation_time)
                        .unwrap_or_else(Instant::now)
                });
            }
        }

        let mut evicted = 0;
        for (name, _, _) in &candidates {
            self.do_evict(name).await;
            evicted += 1;
        }

        (candidates_found, evicted)
    }

    /// List current pool summaries for introspection.
    pub fn current_pools(&self) -> Vec<ResourceSummary> {
        self.registry.list_resources()
    }

    /// Refresh credentials for a single resource by name.
    pub async fn refresh_resource(&self, name: &str) -> Result<(), ResourceError> {
        self.registry.refresh_resource(name).await
    }

    /// Access aggregate pool metrics.
    pub fn pool_metrics(&self) -> &PoolManagerMetrics {
        &self.pool_metrics
    }

    /// Remove a resource from the registry and update all bookkeeping.
    async fn do_evict(&self, name: &str) {
        self.registry.remove(name).await;

        let mut origins = self.origins.write().await;
        let was_static = matches!(origins.remove(name), Some(ResourceOrigin::Static));
        drop(origins);

        let mut metrics_map = self.access_metrics.write().await;
        let connections = metrics_map
            .remove(name)
            .map(|m| u64::from(m.estimated_connections))
            .unwrap_or(0);
        drop(metrics_map);

        self.pool_metrics
            .total_pools
            .fetch_sub(1, Ordering::Relaxed);
        if was_static {
            self.pool_metrics
                .static_pools
                .fetch_sub(1, Ordering::Relaxed);
        } else {
            self.pool_metrics
                .dynamic_pools
                .fetch_sub(1, Ordering::Relaxed);
        }
        self.pool_metrics
            .estimated_total_connections
            .fetch_sub(connections, Ordering::Relaxed);
        self.pool_metrics
            .evictions_performed
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Attempt to evict a single idle dynamic resource using LRU ordering.
    ///
    /// Returns `true` if an eviction occurred, `false` if no eligible candidate.
    async fn try_evict_one(&self) -> bool {
        let now = Instant::now();
        let origins = self.origins.read().await;
        let metrics_map = self.access_metrics.read().await;

        let mut best_candidate: Option<(String, Instant)> = None;

        for (name, origin) in origins.iter() {
            if matches!(origin, ResourceOrigin::Static) {
                continue;
            }
            if let Some(m) = metrics_map.get(name) {
                let idle_duration = now.duration_since(m.last_accessed);
                if idle_duration >= self.config.idle_timeout
                    && m.active_checkouts == 0
                    && best_candidate
                        .as_ref()
                        .is_none_or(|(_, t)| m.last_accessed < *t)
                {
                    best_candidate = Some((name.clone(), m.last_accessed));
                }
            }
        }

        drop(metrics_map);
        drop(origins);

        if let Some((name, _)) = best_candidate {
            self.do_evict(&name).await;
            true
        } else {
            false
        }
    }
}
