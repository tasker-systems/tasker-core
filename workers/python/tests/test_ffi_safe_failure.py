"""Tests for FFI safe-failure pattern in StepExecutionSubscriber.

These tests verify the fixes to the FFI fallback path:
1. _build_ffi_safe_failure produces a dict matching StepExecutionResult's shape
   - execution_time_ms is in metadata (not top-level)
   - task_uuid and worker_id are NOT at top level
   - error.error_type is FFI_SERIALIZATION_ERROR
   - error.message is truncated to 500 chars
2. When both primary and fallback FFI submissions are rejected, RuntimeError is raised
3. Serialization and FFI transport errors have separate try/except blocks
"""

from __future__ import annotations

from unittest.mock import patch
from uuid import uuid4

import pytest

from tasker_core import (
    EventBridge,
    FfiStepEvent,
    HandlerRegistry,
    StepExecutionSubscriber,
    StepHandlerResult,
)


def create_test_event(
    handler_name: str = "test_handler",
) -> FfiStepEvent:
    """Create a test FfiStepEvent with standard nested structure."""
    return FfiStepEvent(
        event_id=str(uuid4()),
        task_uuid=str(uuid4()),
        step_uuid=str(uuid4()),
        correlation_id=str(uuid4()),
        task_sequence_step={
            "workflow_step": {
                "name": "test_step",
                "attempts": 0,
                "max_attempts": 3,
            },
            "step_definition": {
                "name": "test_step",
                "handler": {
                    "callable": handler_name,
                    "initialization": {},
                },
            },
            "task": {
                "context": {},
            },
            "dependency_results": {},
        },
    )


class TestBuildFfiSafeFailure:
    """Tests for _build_ffi_safe_failure fallback structure."""

    def setup_method(self):
        """Reset singletons before each test."""
        EventBridge.reset_instance()
        HandlerRegistry.reset_instance()

    def teardown_method(self):
        """Clean up after each test."""
        EventBridge.reset_instance()
        HandlerRegistry.reset_instance()

    def test_step_uuid_at_top_level(self):
        """Verify step_uuid is present at the top level of the fallback dict."""
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({"data": "value"})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert result["step_uuid"] == event.step_uuid

    def test_no_task_uuid_at_top_level(self):
        """Verify task_uuid is NOT present at the top level.

        The Rust StepExecutionResult struct does not have task_uuid at the top
        level -- it only appears in the step relationship. Including it would
        cause deserialization to fail or create ambiguity.
        """
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert "task_uuid" not in result

    def test_no_execution_time_ms_at_top_level(self):
        """Verify execution_time_ms is NOT at the top level.

        It should be nested inside metadata.
        """
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert "execution_time_ms" not in result

    def test_no_worker_id_at_top_level(self):
        """Verify worker_id is NOT at the top level.

        It should be nested inside metadata.
        """
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert "worker_id" not in result

    def test_metadata_contains_execution_time_ms_zero(self):
        """Verify metadata.execution_time_ms is 0 in fallback results.

        The fallback does not have real timing data so it uses 0.
        """
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert result["metadata"]["execution_time_ms"] == 0

    def test_metadata_contains_worker_id(self):
        """Verify metadata.worker_id is set to the subscriber's worker_id."""
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "my-worker-42")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert result["metadata"]["worker_id"] == "my-worker-42"

    def test_error_type_is_ffi_serialization_error(self):
        """Verify error.error_type is FFI_SERIALIZATION_ERROR."""
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert result["error"]["error_type"] == "FFI_SERIALIZATION_ERROR"

    def test_error_message_truncated_to_500_chars(self):
        """Verify error.message is truncated to 500 characters."""
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        long_message = "x" * 1000
        error = ValueError(long_message)

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert len(result["error"]["message"]) <= 500

    def test_metadata_custom_contains_original_success(self):
        """Verify metadata.custom tracks whether the original result was successful."""
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({"data": "value"})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert result["metadata"]["custom"]["original_success"] == "True"

    def test_success_is_false(self):
        """Verify success is always False in fallback results."""
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert result["success"] is False

    def test_status_is_error(self):
        """Verify status is 'error' in fallback results."""
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert result["status"] == "error"

    def test_error_not_retryable(self):
        """Verify error.retryable is False in fallback results.

        FFI serialization errors are not transient -- retrying will fail the same way.
        """
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        assert result["error"]["retryable"] is False
        assert result["metadata"]["retryable"] is False

    def test_metadata_completed_at_is_iso8601(self):
        """Verify metadata.completed_at is an ISO 8601 timestamp."""
        bridge = EventBridge.instance()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({})
        error = ValueError("test error")

        result = subscriber._build_ffi_safe_failure(event, handler_result, error)

        # Should be parseable as ISO 8601
        import datetime

        completed_at = result["metadata"]["completed_at"]
        parsed = datetime.datetime.fromisoformat(completed_at)
        assert parsed.tzinfo is not None  # Should be timezone-aware (UTC)


class TestFallbackRejectionRaisesRuntimeError:
    """Tests that RuntimeError is raised when both FFI submissions are rejected."""

    def setup_method(self):
        """Reset singletons before each test."""
        EventBridge.reset_instance()
        HandlerRegistry.reset_instance()

    def teardown_method(self):
        """Clean up after each test."""
        EventBridge.reset_instance()
        HandlerRegistry.reset_instance()

    @patch("tasker_core.step_execution_subscriber._complete_step_event")
    def test_fallback_rejection_raises_runtime_error(self, mock_complete):
        """When primary FFI throws and fallback returns False, RuntimeError is raised.

        This prevents step orphaning -- the caller must know the step was not
        delivered to the orchestrator.
        """
        # First call: primary FFI transport throws
        # Second call: fallback submission returns False (rejected)
        mock_complete.side_effect = [RuntimeError("primary FFI failure"), False]
        # We need the fallback to be called, which means the first call raises,
        # then the second call (with fallback dict) returns False.
        # But actually, the code uses the same mock for both calls.
        # Let's trace the code path more carefully:
        #
        # 1. result_dict = result.model_dump(mode="json")  -- succeeds
        # 2. success = _complete_step_event(event_id, result_dict)  -- raises RuntimeError
        # 3. fallback = _build_ffi_safe_failure(...)
        # 4. success = _complete_step_event(event_id, fallback)  -- returns False
        # 5. raises RuntimeError("Both primary and fallback...")

        bridge = EventBridge.instance()
        bridge.start()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({"data": "value"})

        with pytest.raises(RuntimeError, match="orphaned"):
            subscriber._submit_result(event, handler_result, execution_time_ms=100)

    @patch("tasker_core.step_execution_subscriber._complete_step_event")
    def test_fallback_exception_is_raised(self, mock_complete):
        """When both primary and fallback FFI calls throw, the fallback error propagates.

        This ensures that a complete FFI failure is not silently swallowed.
        """
        # Both calls raise exceptions
        mock_complete.side_effect = [
            RuntimeError("primary FFI failure"),
            RuntimeError("fallback also failed"),
        ]

        bridge = EventBridge.instance()
        bridge.start()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({"data": "value"})

        with pytest.raises(RuntimeError, match="fallback also failed"):
            subscriber._submit_result(event, handler_result, execution_time_ms=100)


class TestSplitTryBlocks:
    """Tests that serialization and FFI transport errors are distinguishable.

    The _submit_result method has separate try/except blocks for:
    1. Pydantic serialization (model_dump)
    2. FFI transport (_complete_step_event)

    This means a serialization failure triggers the fallback path without
    attempting the primary FFI call, while an FFI transport failure triggers
    the fallback with the original serialized data available for logging.
    """

    def setup_method(self):
        """Reset singletons before each test."""
        EventBridge.reset_instance()
        HandlerRegistry.reset_instance()

    def teardown_method(self):
        """Clean up after each test."""
        EventBridge.reset_instance()
        HandlerRegistry.reset_instance()

    @patch("tasker_core.step_execution_subscriber._complete_step_event")
    def test_serialization_failure_uses_fallback_dict(self, mock_complete):
        """When model_dump fails, the fallback dict is submitted instead.

        The fallback dict is constructed by _build_ffi_safe_failure and
        should be submitted to _complete_step_event.
        """
        mock_complete.return_value = True

        bridge = EventBridge.instance()
        bridge.start()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({"data": "value"})

        # Patch model_dump to simulate serialization failure
        with patch.object(
            type(handler_result),
            "model_dump",
            side_effect=TypeError("Cannot serialize"),
        ):
            # This will be a StepExecutionResult object, so we need to patch
            # the right thing. The result is created inside _submit_result.
            # Actually, let's just patch the StepExecutionResult class's model_dump.
            pass

        # Alternative approach: patch at the module level
        from tasker_core.types import StepExecutionResult

        def failing_model_dump(_self, **_kwargs):
            raise TypeError("Cannot serialize complex type")

        with patch.object(StepExecutionResult, "model_dump", failing_model_dump):
            subscriber._submit_result(event, handler_result, execution_time_ms=100)

        # The fallback dict should have been submitted
        mock_complete.assert_called_once()
        call_args = mock_complete.call_args
        result_dict = call_args[0][1]
        assert result_dict["error"]["error_type"] == "FFI_SERIALIZATION_ERROR"
        assert "Cannot serialize" in result_dict["metadata"]["custom"]["ffi_serialization_error"]

    @patch("tasker_core.step_execution_subscriber._complete_step_event")
    def test_ffi_transport_failure_triggers_separate_fallback(self, mock_complete):
        """When FFI transport fails but serialization succeeded, the error path is distinct.

        The first _complete_step_event call raises (transport failure).
        The second call with the fallback dict succeeds.
        """
        # First call raises (transport), second call succeeds (fallback)
        mock_complete.side_effect = [RuntimeError("Transport error"), True]

        bridge = EventBridge.instance()
        bridge.start()
        registry = HandlerRegistry.instance()
        subscriber = StepExecutionSubscriber(bridge, registry, "worker-001")

        event = create_test_event()
        handler_result = StepHandlerResult.success({"data": "value"})

        # Should not raise -- fallback succeeds
        subscriber._submit_result(event, handler_result, execution_time_ms=100)

        # Two calls: primary (failed) and fallback (succeeded)
        assert mock_complete.call_count == 2

        # The second (fallback) call should have the safe failure dict
        fallback_call = mock_complete.call_args_list[1]
        fallback_dict = fallback_call[0][1]
        assert fallback_dict["error"]["error_type"] == "FFI_SERIALIZATION_ERROR"
