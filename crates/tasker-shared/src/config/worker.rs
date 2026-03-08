//! # Worker Configuration
//!
//! TAS-221: Worker adapter types (StepProcessingConfig, HealthMonitoringConfig) removed.
//! These were type adapters (u32 → u64/usize) for V2 config fields that are no longer
//! present in WorkerConfig. Worker step processing and health monitoring configuration
//! is now handled directly by the event system metadata.
