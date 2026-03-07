"""Unit tests for the Worker class — pure Python logic only (TAS-281).

These tests verify property accessors, state transitions, and singleton
behavior that don't require FFI. Integration tests that exercise the
full FFI pipeline live in tests/integration/test_worker_lifecycle.py.
"""

from __future__ import annotations

import pytest

from tasker_core.worker import Worker


@pytest.fixture(autouse=True)
def _reset_singleton():
    """Ensure singleton is clean between tests."""
    Worker._instance = None
    yield
    Worker._instance = None


class TestWorkerPreStartState:
    """Test Worker state before start() is called."""

    def test_worker_id_before_start(self):
        worker = Worker()
        assert worker.worker_id == ""

    def test_is_running_before_start(self):
        worker = Worker()
        assert worker.is_running is False

    def test_bootstrap_result_before_start(self):
        worker = Worker()
        with pytest.raises(RuntimeError, match="Worker has not been started"):
            _ = worker.bootstrap_result

    def test_stop_before_start_is_safe(self):
        worker = Worker()
        worker.stop()  # should not raise
        assert worker.is_running is False


class TestWorkerSingleton:
    """Test singleton behavior — no FFI needed for these."""

    def test_instance_returns_none_when_not_started(self):
        assert Worker.instance() is None

    def test_reset_instance_when_no_instance(self):
        Worker.reset_instance()  # should not raise
        assert Worker.instance() is None
