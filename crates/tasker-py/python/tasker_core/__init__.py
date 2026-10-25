"""
Tasker Core Python Worker

This package provides Python bindings for the tasker-core workflow
orchestration system, enabling Python step handlers to integrate
with Rust-based orchestration.

Example:
    >>> import tasker_core
    >>> tasker_core.version()
    '0.1.0'
    >>> tasker_core.health_check()
    True

    >>> # Bootstrap the worker system (Phase 2+)
    >>> result = tasker_core.bootstrap_worker()
    >>> print(f"Worker started: {result.worker_id}")

    >>> # Use structured logging
    >>> tasker_core.log_info("Processing started", {"correlation_id": "abc-123"})

    >>> # Poll for step events (Phase 3+)
    >>> from tasker_core import EventPoller
    >>> poller = EventPoller()
    >>> poller.on_step_event(handle_step)
    >>> poller.start()

    >>> # Create custom step handlers (Phase 4+)
    >>> from tasker_core import StepHandler, StepContext, StepHandlerResult, ErrorType
    >>> class MyHandler(StepHandler):
    ...     handler_name = "my_handler"
    ...     def call(self, context: StepContext) -> StepHandlerResult:
    ...         return StepHandlerResult.success({"done": True})

    >>> # Stop the worker
    >>> tasker_core.stop_worker()
"""

from __future__ import annotations

# Import from the internal FFI module (Phase 1, Phase 3, Phase 5)
from tasker_core._tasker_core import (
    __version__,
    check_starvation_warnings,
    cleanup_timeouts,
    complete_step_event,
    get_ffi_dispatch_metrics,
    get_rust_version,
    get_version,
    health_check,
    poll_in_process_events,
    poll_step_events,
)

# Import Phase 6b: Specialized handlers and batch processing
from tasker_core.batch_processing import (
    Batchable,
    BatchAggregationScenario,
    BatchWorkerConfig,
)

# Import bootstrap functions (Phase 2)
from tasker_core.bootstrap import (
    bootstrap_worker,
    get_worker_status,
    is_worker_running,
    stop_worker,
    transition_to_graceful_shutdown,
)

# Import client wrapper (TAS-231)
from tasker_core.client import (
    HealthResponse as ClientHealthResponse,
)
from tasker_core.client import (
    PaginationInfo as ClientPaginationInfo,
)
from tasker_core.client import (
    StepAuditResponse as ClientStepAuditResponse,
)
from tasker_core.client import (
    StepResponse as ClientStepResponse,
)
from tasker_core.client import (
    TaskerClient,
)
from tasker_core.client import (
    TaskListResponse as ClientTaskListResponse,
)
from tasker_core.client import (
    TaskResponse as ClientTaskResponse,
)

# Import Phase 5: Domain events and observability
from tasker_core.domain_events import (
    BasePublisher,
    BaseSubscriber,
    EventDeclaration,
    InProcessDomainEventPoller,
    PublishContext,
    StepEventContext,
    StepResult,
)

# Import error classification (Phase 6a - parity with Ruby)
from tasker_core.errors import (
    AuthenticationError,
    AuthorizationError,
    BusinessLogicError,
    ConfigurationError,
    NetworkError,
    NotFoundError,
    PermanentError,
    RateLimitError,
    ResourceContentionError,
    RetryableError,
    ServiceUnavailableError,
)
from tasker_core.errors import TimeoutError as StepTimeoutError
from tasker_core.errors import ValidationError as StepValidationError
from tasker_core.errors.error_classifier import (
    ErrorClassifier,
    get_classifier,
    is_permanent,
    is_retryable,
)

# Import Phase 4: Handler system
from tasker_core.event_bridge import EventBridge, EventNames
from tasker_core.event_poller import EventPoller

# Import exceptions (Phase 2)
from tasker_core.exceptions import (
    ConversionError,
    FFIError,
    TaskerError,
    WorkerAlreadyRunningError,
    WorkerBootstrapError,
    WorkerNotInitializedError,
)
from tasker_core.handler import HandlerRegistry, StepHandler

# Import logging functions (Phase 2)
from tasker_core.logging import (
    log_debug,
    log_error,
    log_info,
    log_trace,
    log_warn,
)

# Import model wrappers (Phase 6a - parity with Ruby)
from tasker_core.models import (
    DependencyResultsWrapper,
    HandlerWrapper,
    StepDefinitionWrapper,
    TaskSequenceStepWrapper,
    TaskWrapper,
    WorkflowStepWrapper,
)
from tasker_core.observability import (
    get_health_check,
    get_metrics,
    get_worker_config,
)

# Import TAS-93: Resolver chain infrastructure
from tasker_core.registry import (
    BaseResolver,
    ClassLookupResolver,
    ExplicitMappingResolver,
    HandlerDefinition,
    MethodDispatchWrapper,
    RegistryResolver,
    ResolutionError,
    ResolverChain,
    ResolverNotFoundError,
)
from tasker_core.step_execution_subscriber import (
    StepExecutionError,
    StepExecutionSubscriber,
)
from tasker_core.step_handler import ApiHandler, ApiResponse, DecisionHandler

# Import template discovery (for manual usage - auto-bootstrap handles this)
from tasker_core.template_discovery import (
    HandlerDiscovery,
    TemplateParser,
    TemplatePath,
)

# Import types (Phase 2 + Phase 3 + Phase 4 + Phase 5 + Phase 6b)
from tasker_core.types import (
    BatchAnalyzerOutcome,
    BatchWorkerContext,
    BatchWorkerOutcome,
    BootstrapConfig,
    BootstrapResult,
    ComponentHealth,
    CursorConfig,
    DecisionPointOutcome,
    DecisionType,
    DomainEventMetadata,
    ErrorType,
    ExecutionResult,
    FfiDispatchMetrics,
    FfiStepEvent,
    HealthCheck,
    InProcessDomainEvent,
    LogContext,
    ResultStatus,
    StarvationWarning,
    StepContext,
    StepError,
    StepExecutionResult,
    StepHandlerCallResult,
    StepHandlerResult,
    WorkerConfig,
    WorkerMetrics,
    WorkerState,
    WorkerStatus,
)

# Import high-level Worker lifecycle manager (TAS-281)
from tasker_core.worker import Worker

__all__ = [
    "ApiHandler",
    "ApiResponse",
    "AuthenticationError",
    "AuthorizationError",
    "BasePublisher",
    "BaseResolver",
    "BaseSubscriber",
    "BatchAggregationScenario",
    "BatchAnalyzerOutcome",
    "BatchWorkerConfig",
    "BatchWorkerContext",
    "BatchWorkerOutcome",
    "Batchable",
    "BootstrapConfig",
    "BootstrapResult",
    "BusinessLogicError",
    "ClassLookupResolver",
    "ClientHealthResponse",
    "ClientPaginationInfo",
    "ClientStepAuditResponse",
    "ClientStepResponse",
    "ClientTaskListResponse",
    "ClientTaskResponse",
    "ComponentHealth",
    "ConfigurationError",
    "ConversionError",
    "CursorConfig",
    "DecisionHandler",
    "DecisionPointOutcome",
    "DecisionType",
    "DependencyResultsWrapper",
    "DomainEventMetadata",
    "ErrorClassifier",
    "ErrorType",
    "EventBridge",
    "EventDeclaration",
    "EventNames",
    "EventPoller",
    "ExecutionResult",
    "ExplicitMappingResolver",
    "FFIError",
    "FfiDispatchMetrics",
    "FfiStepEvent",
    "HandlerDefinition",
    "HandlerDiscovery",
    "HandlerRegistry",
    "HandlerWrapper",
    "HealthCheck",
    "InProcessDomainEvent",
    "InProcessDomainEventPoller",
    "LogContext",
    "MethodDispatchWrapper",
    "NetworkError",
    "NotFoundError",
    "PermanentError",
    "PublishContext",
    "RateLimitError",
    "RegistryResolver",
    "ResolutionError",
    "ResolverChain",
    "ResolverNotFoundError",
    "ResourceContentionError",
    "ResultStatus",
    "RetryableError",
    "ServiceUnavailableError",
    "StarvationWarning",
    "StepContext",
    "StepDefinitionWrapper",
    "StepError",
    "StepEventContext",
    "StepExecutionError",
    "StepExecutionResult",
    "StepExecutionSubscriber",
    "StepHandler",
    "StepHandlerCallResult",
    "StepHandlerResult",
    "StepResult",
    "StepTimeoutError",
    "StepValidationError",
    "TaskSequenceStepWrapper",
    "TaskWrapper",
    "TaskerClient",
    "TaskerError",
    "TemplateParser",
    "TemplatePath",
    "Worker",
    "WorkerAlreadyRunningError",
    "WorkerBootstrapError",
    "WorkerConfig",
    "WorkerMetrics",
    "WorkerNotInitializedError",
    "WorkerState",
    "WorkerStatus",
    "WorkflowStepWrapper",
    "__version__",
    "bootstrap_worker",
    "check_starvation_warnings",
    "cleanup_timeouts",
    "complete_step_event",
    "get_classifier",
    "get_ffi_dispatch_metrics",
    "get_health_check",
    "get_metrics",
    "get_rust_version",
    "get_version",
    "get_worker_config",
    "get_worker_status",
    "health_check",
    "is_permanent",
    "is_retryable",
    "is_worker_running",
    "log_debug",
    "log_error",
    "log_info",
    "log_trace",
    "log_warn",
    "poll_in_process_events",
    "poll_step_events",
    "stop_worker",
    "transition_to_graceful_shutdown",
    "version",
]


def version() -> str:
    """Return the package version.

    Returns:
        The version string (e.g., "0.1.0")

    Example:
        >>> import tasker_core
        >>> tasker_core.version()
        '0.1.0'
    """
    return get_version()
