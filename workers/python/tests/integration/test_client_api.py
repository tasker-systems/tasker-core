"""Client API FFI Integration Tests (TAS-231).

Tests the client FFI functions against a running orchestration server.
Verifies full round-trip: Python -> PyO3 FFI -> Rust -> REST API -> PostgreSQL -> response.

Prerequisites:
- Python FFI extension built: uv run maturin develop
- DATABASE_URL set and database accessible
- Orchestration server running (default: http://localhost:8080)
- FFI_CLIENT_TESTS=true environment variable

Run: FFI_CLIENT_TESTS=true DATABASE_URL=... uv run pytest tests/integration/test_client_api.py -v
"""

from __future__ import annotations

import uuid

import pytest


@pytest.mark.client_integration
@pytest.mark.usefixtures("bootstrapped_worker")
class TestClientHealthCheck:
    """Test client health check against orchestration API."""

    def test_health_check_returns_healthy(self):
        """Health check returns a dict with status information."""
        from tasker_core._tasker_core import client_health_check

        result = client_health_check()
        assert isinstance(result, dict)
        assert "healthy" in result


@pytest.mark.client_integration
@pytest.mark.usefixtures("bootstrapped_worker")
class TestClientTaskLifecycle:
    """Test full task lifecycle through client FFI."""

    def test_create_task(self, shared_state):
        """Create a task via the orchestration API."""
        from tasker_core._tasker_core import client_create_task

        request = {
            "name": "success_only_py",
            "namespace": "test_scenarios_py",
            "version": "1.0.0",
            "context": {"test_run": "client_api_integration", "run_id": str(uuid.uuid4())},
            "initiator": "python-client-test",
            "source_system": "integration-test",
            "reason": "TAS-231 client API integration test",
        }

        result = client_create_task(request)
        assert isinstance(result, dict)
        assert "task_uuid" in result
        assert result["name"] == "success_only_py"
        assert result["namespace"] == "test_scenarios_py"

        # Save for subsequent tests
        shared_state["task_uuid"] = result["task_uuid"]

    def test_get_task(self, shared_state):
        """Get the created task by UUID."""
        from tasker_core._tasker_core import client_get_task

        task_uuid = shared_state.get("task_uuid")
        if not task_uuid:
            pytest.skip("No task_uuid from create_task test")

        result = client_get_task(task_uuid)
        assert isinstance(result, dict)
        assert result["task_uuid"] == task_uuid
        assert result["name"] == "success_only_py"
        assert result["namespace"] == "test_scenarios_py"
        assert result["version"] == "1.0.0"
        assert "created_at" in result
        assert "updated_at" in result
        assert "correlation_id" in result
        assert isinstance(result["total_steps"], int)

    def test_list_tasks(self):
        """List tasks with pagination."""
        from tasker_core._tasker_core import client_list_tasks

        result = client_list_tasks(50, 0, None, None)
        assert isinstance(result, dict)
        assert "tasks" in result
        assert isinstance(result["tasks"], list)
        assert "pagination" in result
        assert isinstance(result["pagination"]["total_count"], int)
        assert result["pagination"]["total_count"] >= 1

    def test_list_task_steps(self, shared_state):
        """List workflow steps for the created task."""
        from tasker_core._tasker_core import client_list_task_steps

        task_uuid = shared_state.get("task_uuid")
        if not task_uuid:
            pytest.skip("No task_uuid from create_task test")

        result = client_list_task_steps(task_uuid)
        assert isinstance(result, list)

        if len(result) > 0:
            step = result[0]
            assert "step_uuid" in step
            assert step["task_uuid"] == task_uuid
            assert "name" in step
            # Save for subsequent tests
            shared_state["step_uuid"] = step["step_uuid"]

    def test_get_step(self, shared_state):
        """Get a specific workflow step."""
        from tasker_core._tasker_core import client_get_step

        task_uuid = shared_state.get("task_uuid")
        step_uuid = shared_state.get("step_uuid")
        if not task_uuid or not step_uuid:
            pytest.skip("No task_uuid/step_uuid from previous tests")

        result = client_get_step(task_uuid, step_uuid)
        assert isinstance(result, dict)
        assert result["step_uuid"] == step_uuid
        assert result["task_uuid"] == task_uuid
        assert "name" in result
        assert "current_state" in result
        assert isinstance(result["attempts"], int)
        assert isinstance(result["max_attempts"], int)

    def test_get_step_audit_history(self, shared_state):
        """Get audit history for a workflow step."""
        from tasker_core._tasker_core import client_get_step_audit_history

        task_uuid = shared_state.get("task_uuid")
        step_uuid = shared_state.get("step_uuid")
        if not task_uuid or not step_uuid:
            pytest.skip("No task_uuid/step_uuid from previous tests")

        result = client_get_step_audit_history(task_uuid, step_uuid)
        assert isinstance(result, list)
        # May be empty for newly created tasks

    def test_cancel_task(self, shared_state):
        """Cancel the created task."""
        from tasker_core._tasker_core import client_cancel_task

        task_uuid = shared_state.get("task_uuid")
        if not task_uuid:
            pytest.skip("No task_uuid from create_task test")

        result = client_cancel_task(task_uuid)
        assert isinstance(result, dict)


@pytest.mark.client_integration
@pytest.mark.usefixtures("bootstrapped_worker")
class TestClientErrorHandling:
    """Test client error handling for edge cases."""

    def test_get_nonexistent_task(self):
        """Getting a non-existent task should raise or return error."""
        from tasker_core._tasker_core import client_get_task

        # Should either raise an exception or return an error response
        try:
            result = client_get_task("00000000-0000-0000-0000-000000000000")
            # If it returns, it should still be a valid response
            assert isinstance(result, dict)
        except RuntimeError:
            pass  # Expected - server returns 404/error


# =============================================================================
# Fixtures
# =============================================================================


@pytest.fixture(scope="module")
def bootstrapped_worker():
    """Bootstrap the worker for the entire test module.

    This fixture starts the worker system (which initializes the client)
    and stops it after all tests complete.
    """
    from tasker_core._tasker_core import bootstrap_worker, stop_worker

    result = bootstrap_worker(None)
    assert isinstance(result, dict), f"Bootstrap returned unexpected type: {type(result)}"
    assert result.get("status") == "started", f"Bootstrap failed: {result}"

    yield result

    stop_worker()


@pytest.fixture(scope="module")
def shared_state():
    """Shared mutable state for ordered test methods within a module.

    Allows tests to pass data (task_uuid, step_uuid) to subsequent tests.
    """
    return {}
