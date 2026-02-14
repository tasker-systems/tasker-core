"""High-level client wrapper for orchestration API operations.

The raw FFI exposes ``tasker_core._tasker_core.client_create_task(dict)`` and
similar functions that require callers to construct complete request dicts with
all required fields (initiator, source_system, reason, etc.) and return plain
dicts.  This module provides a :class:`TaskerClient` class with typed methods,
sensible defaults, and dataclass response objects.

Example::

    from tasker_core.client import TaskerClient

    client = TaskerClient()
    task = client.create_task("process_order", namespace="ecommerce", context={"order_id": 123})
    print(task.task_uuid)
    print(task.status)
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

# ---------------------------------------------------------------------------
# Response dataclasses
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class PaginationInfo:
    """Pagination metadata in list responses."""

    page: int = 0
    per_page: int = 50
    total_count: int = 0
    total_pages: int = 0
    has_next: bool = False
    has_previous: bool = False

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> PaginationInfo:
        return cls(
            page=data.get("page", 0),
            per_page=data.get("per_page", 50),
            total_count=data.get("total_count", 0),
            total_pages=data.get("total_pages", 0),
            has_next=data.get("has_next", False),
            has_previous=data.get("has_previous", False),
        )


@dataclass(frozen=True)
class TaskResponse:
    """Task response from the orchestration API."""

    task_uuid: str = ""
    name: str = ""
    namespace: str = ""
    version: str = ""
    status: str = ""
    created_at: str = ""
    updated_at: str = ""
    completed_at: str | None = None
    context: dict[str, Any] = field(default_factory=dict)
    initiator: str = ""
    source_system: str = ""
    reason: str = ""
    priority: int | None = None
    tags: list[str] | None = None
    correlation_id: str = ""
    parent_correlation_id: str | None = None
    total_steps: int = 0
    pending_steps: int = 0
    in_progress_steps: int = 0
    completed_steps: int = 0
    failed_steps: int = 0
    ready_steps: int = 0
    execution_status: str = ""
    recommended_action: str = ""
    completion_percentage: float = 0.0
    health_status: str = ""
    steps: list[dict[str, Any]] = field(default_factory=list)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> TaskResponse:
        return cls(
            task_uuid=data.get("task_uuid", ""),
            name=data.get("name", ""),
            namespace=data.get("namespace", ""),
            version=data.get("version", ""),
            status=data.get("status", ""),
            created_at=data.get("created_at", ""),
            updated_at=data.get("updated_at", ""),
            completed_at=data.get("completed_at"),
            context=data.get("context", {}),
            initiator=data.get("initiator", ""),
            source_system=data.get("source_system", ""),
            reason=data.get("reason", ""),
            priority=data.get("priority"),
            tags=data.get("tags"),
            correlation_id=data.get("correlation_id", ""),
            parent_correlation_id=data.get("parent_correlation_id"),
            total_steps=data.get("total_steps", 0),
            pending_steps=data.get("pending_steps", 0),
            in_progress_steps=data.get("in_progress_steps", 0),
            completed_steps=data.get("completed_steps", 0),
            failed_steps=data.get("failed_steps", 0),
            ready_steps=data.get("ready_steps", 0),
            execution_status=data.get("execution_status", ""),
            recommended_action=data.get("recommended_action", ""),
            completion_percentage=data.get("completion_percentage", 0.0),
            health_status=data.get("health_status", ""),
            steps=data.get("steps", []),
        )


@dataclass(frozen=True)
class TaskListResponse:
    """Task list response with pagination."""

    tasks: list[TaskResponse] = field(default_factory=list)
    pagination: PaginationInfo = field(default_factory=PaginationInfo)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> TaskListResponse:
        tasks = [
            TaskResponse.from_dict(t) if isinstance(t, dict) else t for t in data.get("tasks", [])
        ]
        pagination_data = data.get("pagination", {})
        pagination = (
            PaginationInfo.from_dict(pagination_data)
            if isinstance(pagination_data, dict)
            else PaginationInfo()
        )
        return cls(tasks=tasks, pagination=pagination)


@dataclass(frozen=True)
class StepResponse:
    """Step response from the orchestration API."""

    step_uuid: str = ""
    task_uuid: str = ""
    name: str = ""
    created_at: str = ""
    updated_at: str = ""
    completed_at: str | None = None
    results: dict[str, Any] | None = None
    current_state: str = ""
    dependencies_satisfied: bool = False
    retry_eligible: bool = False
    ready_for_execution: bool = False
    total_parents: int = 0
    completed_parents: int = 0
    attempts: int = 0
    max_attempts: int = 0
    last_failure_at: str | None = None
    next_retry_at: str | None = None
    last_attempted_at: str | None = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> StepResponse:
        return cls(
            step_uuid=data.get("step_uuid", ""),
            task_uuid=data.get("task_uuid", ""),
            name=data.get("name", ""),
            created_at=data.get("created_at", ""),
            updated_at=data.get("updated_at", ""),
            completed_at=data.get("completed_at"),
            results=data.get("results"),
            current_state=data.get("current_state", ""),
            dependencies_satisfied=data.get("dependencies_satisfied", False),
            retry_eligible=data.get("retry_eligible", False),
            ready_for_execution=data.get("ready_for_execution", False),
            total_parents=data.get("total_parents", 0),
            completed_parents=data.get("completed_parents", 0),
            attempts=data.get("attempts", 0),
            max_attempts=data.get("max_attempts", 0),
            last_failure_at=data.get("last_failure_at"),
            next_retry_at=data.get("next_retry_at"),
            last_attempted_at=data.get("last_attempted_at"),
        )


@dataclass(frozen=True)
class StepAuditResponse:
    """Step audit history entry (SOC2 compliance)."""

    audit_uuid: str = ""
    workflow_step_uuid: str = ""
    transition_uuid: str = ""
    task_uuid: str = ""
    recorded_at: str = ""
    worker_uuid: str | None = None
    correlation_id: str | None = None
    success: bool = False
    execution_time_ms: int | None = None
    result: dict[str, Any] | None = None
    step_name: str = ""
    from_state: str | None = None
    to_state: str = ""

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> StepAuditResponse:
        return cls(
            audit_uuid=data.get("audit_uuid", ""),
            workflow_step_uuid=data.get("workflow_step_uuid", ""),
            transition_uuid=data.get("transition_uuid", ""),
            task_uuid=data.get("task_uuid", ""),
            recorded_at=data.get("recorded_at", ""),
            worker_uuid=data.get("worker_uuid"),
            correlation_id=data.get("correlation_id"),
            success=data.get("success", False),
            execution_time_ms=data.get("execution_time_ms"),
            result=data.get("result"),
            step_name=data.get("step_name", ""),
            from_state=data.get("from_state"),
            to_state=data.get("to_state", ""),
        )


@dataclass(frozen=True)
class HealthResponse:
    """Health check response from the orchestration API."""

    healthy: bool = False
    status: str = ""
    timestamp: str = ""

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> HealthResponse:
        return cls(
            healthy=data.get("healthy", False),
            status=data.get("status", ""),
            timestamp=data.get("timestamp", ""),
        )


# ---------------------------------------------------------------------------
# Client class
# ---------------------------------------------------------------------------


class TaskerClient:
    """High-level client for orchestration API operations.

    Wraps the raw FFI functions with sensible defaults and typed responses.

    Args:
        initiator: Default initiator field for task creation requests.
        source_system: Default source_system field for task creation requests.

    Example::

        client = TaskerClient()
        task = client.create_task("process_order", namespace="ecommerce")
        print(task.task_uuid)

        steps = client.list_task_steps(task.task_uuid)
        for step in steps:
            print(f"{step.name}: {step.current_state}")
    """

    def __init__(
        self,
        initiator: str = "tasker-core-python",
        source_system: str = "tasker-core",
    ) -> None:
        self.initiator = initiator
        self.source_system = source_system

    def create_task(
        self,
        name: str,
        *,
        namespace: str = "default",
        context: dict[str, Any] | None = None,
        version: str = "1.0.0",
        reason: str = "Task requested",
        **kwargs: Any,
    ) -> TaskResponse:
        """Create a task via the orchestration API.

        Args:
            name: Named task template name.
            namespace: Task namespace.
            context: Workflow context passed to step handlers.
            version: Template version.
            reason: Reason for creating the task.
            **kwargs: Additional fields merged into the request dict.

        Returns:
            Typed task response.
        """
        from tasker_core._tasker_core import client_create_task  # type: ignore[attr-defined]

        request: dict[str, Any] = {
            "name": name,
            "namespace": namespace,
            "version": version,
            "context": context or {},
            "initiator": self.initiator,
            "source_system": self.source_system,
            "reason": reason,
        }
        request.update(kwargs)

        result = client_create_task(request)
        return TaskResponse.from_dict(result) if isinstance(result, dict) else result

    def get_task(self, task_uuid: str) -> TaskResponse:
        """Get a task by UUID."""
        from tasker_core._tasker_core import client_get_task  # type: ignore[attr-defined]

        result = client_get_task(task_uuid)
        return TaskResponse.from_dict(result) if isinstance(result, dict) else result

    def list_tasks(
        self,
        *,
        limit: int = 50,
        offset: int = 0,
        namespace: str | None = None,
        status: str | None = None,
    ) -> TaskListResponse:
        """List tasks with optional filtering and pagination."""
        from tasker_core._tasker_core import client_list_tasks  # type: ignore[attr-defined]

        result = client_list_tasks(limit, offset, namespace, status)
        return TaskListResponse.from_dict(result) if isinstance(result, dict) else result

    def cancel_task(self, task_uuid: str) -> dict[str, Any]:
        """Cancel a task by UUID."""
        from tasker_core._tasker_core import client_cancel_task  # type: ignore[attr-defined]

        return client_cancel_task(task_uuid)  # type: ignore[no-any-return]

    def list_task_steps(self, task_uuid: str) -> list[StepResponse]:
        """List workflow steps for a task."""
        from tasker_core._tasker_core import client_list_task_steps  # type: ignore[attr-defined]

        result = client_list_task_steps(task_uuid)
        if isinstance(result, list):
            return [StepResponse.from_dict(s) if isinstance(s, dict) else s for s in result]
        return result  # type: ignore[no-any-return]

    def get_step(self, task_uuid: str, step_uuid: str) -> StepResponse:
        """Get a specific workflow step."""
        from tasker_core._tasker_core import client_get_step  # type: ignore[attr-defined]

        result = client_get_step(task_uuid, step_uuid)
        return StepResponse.from_dict(result) if isinstance(result, dict) else result

    def get_step_audit_history(self, task_uuid: str, step_uuid: str) -> list[StepAuditResponse]:
        """Get audit history for a workflow step."""
        from tasker_core._tasker_core import (  # type: ignore[attr-defined]
            client_get_step_audit_history,
        )

        result = client_get_step_audit_history(task_uuid, step_uuid)
        if isinstance(result, list):
            return [StepAuditResponse.from_dict(e) if isinstance(e, dict) else e for e in result]
        return result  # type: ignore[no-any-return]

    def health_check(self) -> HealthResponse:
        """Check orchestration API health."""
        from tasker_core._tasker_core import client_health_check  # type: ignore[attr-defined]

        result = client_health_check()
        return HealthResponse.from_dict(result) if isinstance(result, dict) else result
