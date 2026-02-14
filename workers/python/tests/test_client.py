"""Unit tests for the TaskerClient high-level client wrapper.

These tests mock the raw FFI functions and verify that TaskerClient
constructs correct request dicts, applies defaults, and wraps responses
into typed dataclass objects.
"""

from __future__ import annotations

from unittest.mock import MagicMock, patch

import pytest

from tasker_core.client import (
    HealthResponse,
    PaginationInfo,
    StepAuditResponse,
    StepResponse,
    TaskerClient,
    TaskListResponse,
    TaskResponse,
)


@pytest.fixture
def client() -> TaskerClient:
    """Provide a TaskerClient with default settings."""
    return TaskerClient()


@pytest.fixture
def custom_client() -> TaskerClient:
    """Provide a TaskerClient with custom initiator/source_system."""
    return TaskerClient(initiator="my-app", source_system="my-system")


@pytest.fixture
def mock_task_response() -> dict:
    return {
        "task_uuid": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
        "name": "test_task",
        "namespace": "test",
        "version": "1.0.0",
        "status": "pending",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z",
        "context": {"key": "value"},
        "initiator": "tasker-core-python",
        "source_system": "tasker-core",
        "reason": "Task requested",
        "correlation_id": "corr-id-123",
        "total_steps": 3,
        "pending_steps": 3,
        "in_progress_steps": 0,
        "completed_steps": 0,
        "failed_steps": 0,
        "ready_steps": 1,
        "execution_status": "pending",
        "recommended_action": "wait",
        "completion_percentage": 0.0,
        "health_status": "healthy",
        "steps": [],
    }


@pytest.fixture
def mock_step_response() -> dict:
    return {
        "step_uuid": "11111111-2222-3333-4444-555555555555",
        "task_uuid": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
        "name": "validate_input",
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-01-01T00:00:00Z",
        "current_state": "pending",
        "dependencies_satisfied": True,
        "retry_eligible": False,
        "ready_for_execution": True,
        "total_parents": 0,
        "completed_parents": 0,
        "attempts": 0,
        "max_attempts": 3,
    }


@pytest.fixture
def mock_audit_response() -> dict:
    return {
        "audit_uuid": "audit-uuid-1",
        "workflow_step_uuid": "11111111-2222-3333-4444-555555555555",
        "transition_uuid": "trans-uuid-1",
        "task_uuid": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
        "recorded_at": "2026-01-01T00:00:01Z",
        "success": True,
        "step_name": "validate_input",
        "to_state": "complete",
    }


class TestTaskerClientCreateTask:
    """Tests for TaskerClient.create_task."""

    @patch("tasker_core._tasker_core.client_create_task")
    def test_creates_task_with_defaults(
        self, mock_ffi: MagicMock, client: TaskerClient, mock_task_response: dict
    ):
        mock_ffi.return_value = mock_task_response

        result = client.create_task("test_task", namespace="test", context={"key": "value"})

        mock_ffi.assert_called_once()
        call_args = mock_ffi.call_args[0][0]
        assert call_args["name"] == "test_task"
        assert call_args["namespace"] == "test"
        assert call_args["context"] == {"key": "value"}
        assert call_args["initiator"] == "tasker-core-python"
        assert call_args["source_system"] == "tasker-core"
        assert call_args["reason"] == "Task requested"
        assert call_args["version"] == "1.0.0"

        assert isinstance(result, TaskResponse)
        assert result.task_uuid == "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
        assert result.name == "test_task"

    @patch("tasker_core._tasker_core.client_create_task")
    def test_uses_custom_initiator_and_source(
        self,
        mock_ffi: MagicMock,
        custom_client: TaskerClient,
        mock_task_response: dict,
    ):
        mock_ffi.return_value = mock_task_response

        custom_client.create_task("test_task")

        call_args = mock_ffi.call_args[0][0]
        assert call_args["initiator"] == "my-app"
        assert call_args["source_system"] == "my-system"

    @patch("tasker_core._tasker_core.client_create_task")
    def test_allows_overriding_defaults(
        self, mock_ffi: MagicMock, client: TaskerClient, mock_task_response: dict
    ):
        mock_ffi.return_value = mock_task_response

        client.create_task(
            "test_task",
            namespace="custom",
            version="2.0.0",
            reason="Custom reason",
        )

        call_args = mock_ffi.call_args[0][0]
        assert call_args["namespace"] == "custom"
        assert call_args["version"] == "2.0.0"
        assert call_args["reason"] == "Custom reason"

    @patch("tasker_core._tasker_core.client_create_task")
    def test_default_context_is_empty_dict(
        self, mock_ffi: MagicMock, client: TaskerClient, mock_task_response: dict
    ):
        mock_ffi.return_value = mock_task_response

        client.create_task("test_task")

        call_args = mock_ffi.call_args[0][0]
        assert call_args["context"] == {}


class TestTaskerClientGetTask:
    """Tests for TaskerClient.get_task."""

    @patch("tasker_core._tasker_core.client_get_task")
    def test_gets_task_and_wraps_response(
        self, mock_ffi: MagicMock, client: TaskerClient, mock_task_response: dict
    ):
        mock_ffi.return_value = mock_task_response

        result = client.get_task("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")

        mock_ffi.assert_called_once_with("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        assert isinstance(result, TaskResponse)
        assert result.task_uuid == "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
        assert result.namespace == "test"


class TestTaskerClientListTasks:
    """Tests for TaskerClient.list_tasks."""

    @patch("tasker_core._tasker_core.client_list_tasks")
    def test_lists_tasks_with_defaults(
        self, mock_ffi: MagicMock, client: TaskerClient, mock_task_response: dict
    ):
        mock_ffi.return_value = {
            "tasks": [mock_task_response],
            "pagination": {
                "page": 1,
                "per_page": 50,
                "total_count": 1,
                "total_pages": 1,
                "has_next": False,
                "has_previous": False,
            },
        }

        result = client.list_tasks()

        mock_ffi.assert_called_once_with(50, 0, None, None)
        assert isinstance(result, TaskListResponse)
        assert len(result.tasks) == 1
        assert isinstance(result.tasks[0], TaskResponse)
        assert isinstance(result.pagination, PaginationInfo)
        assert result.pagination.total_count == 1

    @patch("tasker_core._tasker_core.client_list_tasks")
    def test_passes_filter_arguments(self, mock_ffi: MagicMock, client: TaskerClient):
        mock_ffi.return_value = {"tasks": [], "pagination": {}}

        client.list_tasks(limit=10, offset=5, namespace="test", status="pending")

        mock_ffi.assert_called_once_with(10, 5, "test", "pending")


class TestTaskerClientCancelTask:
    """Tests for TaskerClient.cancel_task."""

    @patch("tasker_core._tasker_core.client_cancel_task")
    def test_cancels_task(self, mock_ffi: MagicMock, client: TaskerClient):
        mock_ffi.return_value = {"cancelled": True}

        result = client.cancel_task("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")

        mock_ffi.assert_called_once_with("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
        assert result == {"cancelled": True}


class TestTaskerClientListTaskSteps:
    """Tests for TaskerClient.list_task_steps."""

    @patch("tasker_core._tasker_core.client_list_task_steps")
    def test_lists_steps_and_wraps_each(
        self, mock_ffi: MagicMock, client: TaskerClient, mock_step_response: dict
    ):
        mock_ffi.return_value = [mock_step_response]

        result = client.list_task_steps("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")

        assert isinstance(result, list)
        assert len(result) == 1
        assert isinstance(result[0], StepResponse)
        assert result[0].step_uuid == "11111111-2222-3333-4444-555555555555"
        assert result[0].name == "validate_input"

    @patch("tasker_core._tasker_core.client_list_task_steps")
    def test_returns_empty_list(self, mock_ffi: MagicMock, client: TaskerClient):
        mock_ffi.return_value = []

        result = client.list_task_steps("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")

        assert result == []


class TestTaskerClientGetStep:
    """Tests for TaskerClient.get_step."""

    @patch("tasker_core._tasker_core.client_get_step")
    def test_gets_step_and_wraps_response(
        self, mock_ffi: MagicMock, client: TaskerClient, mock_step_response: dict
    ):
        mock_ffi.return_value = mock_step_response

        result = client.get_step("task-uuid", "step-uuid")

        mock_ffi.assert_called_once_with("task-uuid", "step-uuid")
        assert isinstance(result, StepResponse)
        assert result.current_state == "pending"


class TestTaskerClientGetStepAuditHistory:
    """Tests for TaskerClient.get_step_audit_history."""

    @patch("tasker_core._tasker_core.client_get_step_audit_history")
    def test_gets_audit_history_and_wraps_entries(
        self, mock_ffi: MagicMock, client: TaskerClient, mock_audit_response: dict
    ):
        mock_ffi.return_value = [mock_audit_response]

        result = client.get_step_audit_history("task-uuid", "step-uuid")

        assert isinstance(result, list)
        assert len(result) == 1
        assert isinstance(result[0], StepAuditResponse)
        assert result[0].step_name == "validate_input"
        assert result[0].success is True

    @patch("tasker_core._tasker_core.client_get_step_audit_history")
    def test_returns_empty_list(self, mock_ffi: MagicMock, client: TaskerClient):
        mock_ffi.return_value = []

        result = client.get_step_audit_history("task-uuid", "step-uuid")

        assert result == []


class TestTaskerClientHealthCheck:
    """Tests for TaskerClient.health_check."""

    @patch("tasker_core._tasker_core.client_health_check")
    def test_health_check_wraps_response(self, mock_ffi: MagicMock, client: TaskerClient):
        mock_ffi.return_value = {
            "healthy": True,
            "status": "ok",
            "timestamp": "2026-01-01T00:00:00Z",
        }

        result = client.health_check()

        assert isinstance(result, HealthResponse)
        assert result.healthy is True
        assert result.status == "ok"


class TestResponseDataclasses:
    """Tests for response dataclass construction."""

    def test_task_response_from_dict_with_missing_optional_fields(self):
        data = {"task_uuid": "abc-123", "name": "test"}
        result = TaskResponse.from_dict(data)

        assert result.task_uuid == "abc-123"
        assert result.name == "test"
        assert result.completed_at is None
        assert result.tags is None
        assert result.steps == []

    def test_pagination_info_from_dict(self):
        data = {"page": 2, "per_page": 25, "total_count": 100, "total_pages": 4}
        result = PaginationInfo.from_dict(data)

        assert result.page == 2
        assert result.per_page == 25
        assert result.total_count == 100
        assert result.has_next is False  # default

    def test_step_response_from_dict(self):
        data = {
            "step_uuid": "step-1",
            "task_uuid": "task-1",
            "name": "validate",
            "current_state": "complete",
            "dependencies_satisfied": True,
            "retry_eligible": False,
            "ready_for_execution": False,
            "total_parents": 0,
            "completed_parents": 0,
            "attempts": 1,
            "max_attempts": 3,
        }
        result = StepResponse.from_dict(data)

        assert result.step_uuid == "step-1"
        assert result.current_state == "complete"
        assert result.attempts == 1

    def test_health_response_from_dict_with_partial_data(self):
        data = {"healthy": True}
        result = HealthResponse.from_dict(data)

        assert result.healthy is True
        assert result.status == ""
        assert result.timestamp == ""

    def test_task_list_response_from_dict(self):
        data = {
            "tasks": [{"task_uuid": "abc", "name": "test"}],
            "pagination": {"total_count": 1, "page": 1},
        }
        result = TaskListResponse.from_dict(data)

        assert len(result.tasks) == 1
        assert isinstance(result.tasks[0], TaskResponse)
        assert isinstance(result.pagination, PaginationInfo)
        assert result.pagination.total_count == 1

    def test_response_dataclasses_are_frozen(self):
        response = TaskResponse.from_dict({"task_uuid": "abc"})
        with pytest.raises(AttributeError):
            response.task_uuid = "changed"  # type: ignore[misc]
