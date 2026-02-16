"""Worker lifecycle integration tests (TAS-281).

Tests the Worker class against the real FFI layer to verify the full
bootstrap -> event pipeline -> shutdown lifecycle works end-to-end.

Prerequisites:
- Python FFI extension built: uv run maturin develop
- DATABASE_URL set and database accessible
- Orchestration server running (default: http://localhost:8080)
- FFI_CLIENT_TESTS=true environment variable

Run: FFI_CLIENT_TESTS=true DATABASE_URL=... uv run pytest tests/integration/test_worker_lifecycle.py -v
"""

from __future__ import annotations

import pytest

from tasker_core.worker import Worker


@pytest.fixture(autouse=True)
def _reset_singleton():
    """Ensure singleton is clean between tests."""
    Worker.reset_instance()
    yield
    Worker.reset_instance()


@pytest.mark.client_integration
class TestWorkerLifecycle:
    """Test Worker start/stop lifecycle with real FFI."""

    def test_start_and_stop(self):
        worker = Worker.start()

        assert worker.is_running
        assert worker.worker_id != ""
        assert worker.bootstrap_result.success

        worker.stop()
        assert not worker.is_running

    def test_context_manager(self):
        with Worker.start() as worker:
            assert worker.is_running
            assert worker.worker_id != ""

        assert not worker.is_running

    def test_stop_idempotent(self):
        worker = Worker.start()
        worker.stop()
        worker.stop()  # second stop should not raise
        assert not worker.is_running

    def test_context_manager_on_exception(self):
        with pytest.raises(ValueError, match="boom"), Worker.start() as worker:
            raise ValueError("boom")

        # Worker should still be stopped cleanly
        assert not worker.is_running


@pytest.mark.client_integration
class TestWorkerSingleton:
    """Test singleton behavior with real FFI."""

    def test_start_returns_same_instance(self):
        w1 = Worker.start()
        w2 = Worker.start()
        assert w1 is w2
        assert w1.is_running
        w1.stop()

    def test_instance_returns_running_worker(self):
        assert Worker.instance() is None
        worker = Worker.start()
        assert Worker.instance() is worker
        worker.stop()
        assert Worker.instance() is None

    def test_stop_clears_singleton(self):
        worker = Worker.start()
        worker.stop()
        # After stop, a new start() creates a fresh worker
        worker2 = Worker.start()
        assert worker2 is not worker
        assert worker2.is_running
        worker2.stop()


@pytest.mark.client_integration
class TestWorkerHandlerDiscovery:
    """Test handler discovery modes with real FFI."""

    def test_start_with_template_discovery(self):
        """Default start() uses template-based handler discovery."""
        with Worker.start() as worker:
            assert worker.is_running

    def test_start_with_explicit_packages(self):
        """handler_packages scans additional packages after template discovery."""
        # Use a non-existent package -- discover_handlers logs a warning
        # but doesn't fail, so this verifies the code path works
        with Worker.start(handler_packages=["nonexistent.handlers"]) as worker:
            assert worker.is_running
