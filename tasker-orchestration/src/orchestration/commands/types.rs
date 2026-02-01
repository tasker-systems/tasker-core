//! # TAS-148: Orchestration Command Types
//!
//! This module contains the command types and result structures for orchestration operations.
//! These types are separated from the command processing logic for clarity and maintainability.
//!
//! ## Command Pattern
//!
//! The `OrchestrationCommand` enum represents all commands that can be sent to the
//! orchestration command processor. Commands fall into two categories:
//!
//! - **Request-response**: System commands (`GetProcessingStats`, `HealthCheck`, `Shutdown`)
//!   and direct processing commands (`InitializeTask`, `ProcessStepResult`, `FinalizeTask`)
//!   include a `CommandResponder` for async communication of results.
//! - **Fire-and-forget**: Message-based commands (`*FromMessage`, `*FromMessageEvent`) are
//!   sent without response channels. All production callers (OrchestrationEventSystem,
//!   FallbackPoller) use fire-and-forget semantics. (TAS-165)
//!
//! ## Result Types
//!
//! Each request-response command has a corresponding result type:
//! - `TaskInitializeResult`: Task initialization outcomes
//! - `StepProcessResult`: Step result processing outcomes
//! - `TaskFinalizationResult`: Task finalization outcomes
//!
//! ## Provider Abstraction
//!
//! TAS-133 introduced provider-agnostic messaging types:
//! - `QueuedMessage<T>`: Provider-agnostic message with explicit `MessageHandle`
//! - `MessageEvent`: Signal-only notification for PGMQ large message flow

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::oneshot;
use uuid::Uuid;

use tasker_shared::messaging::service::{MessageEvent, QueuedMessage};
use tasker_shared::messaging::{StepExecutionResult, TaskRequestMessage};
use tasker_shared::TaskerResult;

/// Type alias for command response channels
pub type CommandResponder<T> = oneshot::Sender<TaskerResult<T>>;

/// Commands for orchestration operations (TAS-40 Command Pattern)
///
/// These commands replace direct method calls with async command pattern,
/// eliminating polling while preserving sophisticated orchestration logic.
#[derive(Debug)]
pub enum OrchestrationCommand {
    /// Initialize a new task - delegates to TaskRequestProcessor
    InitializeTask {
        request: TaskRequestMessage, // Use existing message format
        resp: CommandResponder<TaskInitializeResult>,
    },
    /// Process a step execution result - delegates to StepResultProcessor
    ProcessStepResult {
        result: StepExecutionResult, // Use existing result format
        resp: CommandResponder<StepProcessResult>,
    },
    /// Finalize a completed task - uses FinalizationClaimer for atomic operation
    FinalizeTask {
        task_uuid: Uuid,
        resp: CommandResponder<TaskFinalizationResult>,
    },
    /// Process step result from message - delegates full message lifecycle to worker
    ///
    /// TAS-133: Uses provider-agnostic QueuedMessage with explicit MessageHandle.
    /// TAS-165: Fire-and-forget (no response channel) - all callers use fire-and-forget semantics.
    ProcessStepResultFromMessage {
        message: QueuedMessage<serde_json::Value>,
    },
    /// Initialize task from message - delegates full message lifecycle to worker
    ///
    /// TAS-133: Uses provider-agnostic QueuedMessage with explicit MessageHandle.
    /// TAS-165: Fire-and-forget (no response channel) - all callers use fire-and-forget semantics.
    InitializeTaskFromMessage {
        message: QueuedMessage<serde_json::Value>,
    },
    /// Finalize task from message - delegates full message lifecycle to worker
    ///
    /// TAS-133: Uses provider-agnostic QueuedMessage with explicit MessageHandle.
    /// TAS-165: Fire-and-forget (no response channel) - all callers use fire-and-forget semantics.
    FinalizeTaskFromMessage {
        message: QueuedMessage<serde_json::Value>,
    },
    /// Process step result from message event - delegates full message lifecycle to worker
    ///
    /// TAS-133: Uses provider-agnostic MessageEvent for multi-backend support.
    /// TAS-165: Fire-and-forget (no response channel) - all callers use fire-and-forget semantics.
    ProcessStepResultFromMessageEvent { message_event: MessageEvent },
    /// Initialize task from message event - delegates full message lifecycle to worker
    ///
    /// TAS-133: Uses provider-agnostic MessageEvent for multi-backend support.
    /// TAS-165: Fire-and-forget (no response channel) - all callers use fire-and-forget semantics.
    InitializeTaskFromMessageEvent { message_event: MessageEvent },
    /// Finalize task from message event - delegates full message lifecycle to worker
    ///
    /// TAS-133: Uses provider-agnostic MessageEvent for multi-backend support.
    /// TAS-165: Fire-and-forget (no response channel) - all callers use fire-and-forget semantics.
    FinalizeTaskFromMessageEvent { message_event: MessageEvent },
    /// Get orchestration processing statistics
    GetProcessingStats {
        resp: CommandResponder<OrchestrationProcessingStats>,
    },
    /// Perform health check
    HealthCheck {
        resp: CommandResponder<SystemHealth>,
    },
    /// Shutdown orchestration processor
    Shutdown { resp: CommandResponder<()> },
}

/// Result types matching existing orchestration patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskInitializeResult {
    Success { task_uuid: Uuid, message: String },
    Failed { error: String },
    Skipped { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepProcessResult {
    Success { message: String },
    Failed { error: String },
    Skipped { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskFinalizationResult {
    Success {
        task_uuid: Uuid,
        final_status: String,
        completion_time: Option<chrono::DateTime<chrono::Utc>>,
    },
    NotClaimed {
        reason: String,
        already_claimed_by: Option<Uuid>,
    },
    Failed {
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationProcessingStats {
    pub task_requests_processed: u64,
    pub step_results_processed: u64,
    pub tasks_finalized: u64,
    pub processing_errors: u64,
    pub current_queue_sizes: HashMap<String, i64>,
}

/// TAS-75: Enhanced system health status
///
/// This struct contains comprehensive health information derived from
/// cached health status data updated by the background StatusEvaluator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    /// Overall health status: "healthy", "degraded", or "unhealthy"
    pub status: String,

    /// Whether the database is connected (from cached DB health check)
    pub database_connected: bool,

    /// Whether message queues are healthy (not in Critical/Overflow)
    pub message_queues_healthy: bool,

    /// Number of active orchestration processors
    pub active_processors: u32,

    // TAS-75: Enhanced health fields from cached status
    /// Circuit breaker state for database operations
    pub circuit_breaker_open: bool,

    /// Number of consecutive circuit breaker failures
    pub circuit_breaker_failures: u32,

    /// Command channel saturation percentage (0.0-100.0)
    pub command_channel_saturation_percent: f64,

    /// Whether backpressure is currently active
    pub backpressure_active: bool,

    /// Queue depth tier: "Unknown", "Normal", "Warning", "Critical", "Overflow"
    pub queue_depth_tier: String,

    /// Maximum queue depth across all monitored queues
    pub queue_depth_max: i64,

    /// Name of the queue with the highest depth
    pub queue_depth_worst_queue: String,

    /// Whether health data has been evaluated (false means Unknown state)
    pub health_evaluated: bool,
}
