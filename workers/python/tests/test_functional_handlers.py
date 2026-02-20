"""Tests for TAS-294 functional/decorator handler API.

Tests:
1. Basic @step_handler with auto-wrapping
2. Dependency injection via @depends_on
3. Input injection via @inputs
4. Error auto-classification (PermanentError, RetryableError, generic)
5. Decision handler helpers (Decision.route, Decision.skip)
6. Batch analyzer/worker helpers
7. Passthrough when returning StepHandlerResult directly
8. Missing dependency returns None
9. Async handler support
"""

from __future__ import annotations

import asyncio
from typing import cast
from uuid import uuid4

from tasker_core.errors import PermanentError, RetryableError
from tasker_core.step_handler.functional import (
    BatchConfig,
    Decision,
    batch_analyzer,
    batch_worker,
    decision_handler,
    depends_on,
    inputs,
    step_handler,
)
from tasker_core.types import (
    FfiStepEvent,
    StepContext,
    StepHandlerResult,
)

# ============================================================================
# Test Helpers
# ============================================================================


def _call_sync(handler, ctx: StepContext) -> StepHandlerResult:
    """Call a sync handler and cast the result to StepHandlerResult.

    StepHandler.call() returns StepHandlerCallResult (sync | async union).
    In sync tests the result is always StepHandlerResult; this helper
    makes that explicit so basedpyright resolves attributes correctly.
    """
    return cast(StepHandlerResult, handler.call(ctx))


def _make_context(
    handler_name: str = "test_handler",
    input_data: dict | None = None,
    dependency_results: dict | None = None,
    step_config: dict | None = None,
) -> StepContext:
    """Create a StepContext for testing."""
    task_uuid = str(uuid4())
    step_uuid = str(uuid4())
    correlation_id = str(uuid4())

    # Build a minimal FFI event payload
    task_sequence_step = {
        "task": {"task": {"context": input_data or {}}},
        "dependency_results": dependency_results or {},
        "step_definition": {"handler": {"initialization": step_config or {}}},
        "workflow_step": {"attempts": 0, "max_attempts": 3, "inputs": {}},
    }

    event = FfiStepEvent(
        event_id=str(uuid4()),
        task_uuid=task_uuid,
        step_uuid=step_uuid,
        correlation_id=correlation_id,
        task_sequence_step=task_sequence_step,
    )

    return StepContext.from_ffi_event(event, handler_name)


# ============================================================================
# Tests: Basic @step_handler
# ============================================================================


class TestStepHandlerDecorator:
    """Tests for the @step_handler decorator."""

    def test_basic_dict_return(self):
        """Dict return is auto-wrapped as success."""

        @step_handler("my_handler")
        def my_handler(_context):
            return {"processed": True}

        assert hasattr(my_handler, "_handler_class")
        handler = my_handler._handler_class()
        assert handler.handler_name == "my_handler"
        assert handler.handler_version == "1.0.0"

        ctx = _make_context("my_handler")
        result = _call_sync(handler, ctx)
        assert result.is_success is True
        assert result.result == {"processed": True}

    def test_custom_version(self):
        """Custom version is set on the handler class."""

        @step_handler("versioned", version="2.0.0")
        def versioned(_context):
            return {}

        handler = versioned._handler_class()
        assert handler.handler_version == "2.0.0"

    def test_none_return_wraps_empty_dict(self):
        """None return wraps as success with empty dict."""

        @step_handler("no_return")
        def no_return(context):
            pass

        handler = no_return._handler_class()
        result = _call_sync(handler, _make_context())
        assert result.is_success is True
        assert result.result == {}

    def test_passthrough_step_handler_result(self):
        """Returning StepHandlerResult directly is not double-wrapped."""

        @step_handler("passthrough")
        def passthrough(_context):
            return StepHandlerResult.success({"direct": True})

        handler = passthrough._handler_class()
        result = _call_sync(handler, _make_context())
        assert result.is_success is True
        assert result.result == {"direct": True}


# ============================================================================
# Tests: Dependency Injection
# ============================================================================


class TestDependsOn:
    """Tests for the @depends_on decorator."""

    def test_dependency_injection(self):
        """Dependencies are injected from context."""

        @step_handler("with_deps")
        @depends_on(cart="validate_cart")
        def with_deps(cart, _context):
            return {"total": cart["total"]}

        handler = with_deps._handler_class()
        ctx = _make_context(
            "with_deps",
            dependency_results={"validate_cart": {"result": {"total": 99.99}}},
        )
        result = _call_sync(handler, ctx)
        assert result.is_success is True
        assert result.result == {"total": 99.99}

    def test_missing_dependency_injects_none(self):
        """Missing dependency injects None."""

        @step_handler("missing_dep")
        @depends_on(cart="validate_cart")
        def missing_dep(cart, _context):
            return {"cart_is_none": cart is None}

        handler = missing_dep._handler_class()
        ctx = _make_context("missing_dep", dependency_results={})
        result = _call_sync(handler, ctx)
        assert result.is_success is True
        assert result.result == {"cart_is_none": True}

    def test_multiple_dependencies(self):
        """Multiple dependencies are all injected."""

        @step_handler("multi_deps")
        @depends_on(cart="validate_cart", user="fetch_user")
        def multi_deps(cart, user, _context):
            return {"cart": cart, "user": user}

        handler = multi_deps._handler_class()
        ctx = _make_context(
            "multi_deps",
            dependency_results={
                "validate_cart": {"result": {"total": 50}},
                "fetch_user": {"result": {"name": "Alice"}},
            },
        )
        result = _call_sync(handler, ctx)
        assert result.is_success is True
        assert result.result is not None
        assert result.result["cart"] == {"total": 50}
        assert result.result["user"] == {"name": "Alice"}


# ============================================================================
# Tests: Input Injection
# ============================================================================


class TestInputs:
    """Tests for the @inputs decorator."""

    def test_input_injection(self):
        """Inputs are injected from task context."""

        @step_handler("with_inputs")
        @inputs("payment_info")
        def with_inputs(payment_info, _context):
            return {"payment": payment_info}

        handler = with_inputs._handler_class()
        ctx = _make_context(
            "with_inputs",
            input_data={"payment_info": {"card": "1234"}},
        )
        result = _call_sync(handler, ctx)
        assert result.is_success is True
        assert result.result == {"payment": {"card": "1234"}}

    def test_missing_input_injects_none(self):
        """Missing input injects None."""

        @step_handler("missing_input")
        @inputs("nonexistent")
        def missing_input(nonexistent, _context):
            return {"is_none": nonexistent is None}

        handler = missing_input._handler_class()
        result = _call_sync(handler, _make_context("missing_input"))
        assert result.is_success is True
        assert result.result == {"is_none": True}


# ============================================================================
# Tests: Error Classification
# ============================================================================


class TestErrorClassification:
    """Tests for automatic error classification."""

    def test_permanent_error(self):
        """PermanentError → failure(retryable=False)."""

        @step_handler("perm_err")
        def perm_err(_context):
            raise PermanentError("Invalid input")

        handler = perm_err._handler_class()
        result = _call_sync(handler, _make_context())
        assert result.is_success is False
        assert result.retryable is False
        assert result.error_message is not None
        assert "Invalid input" in result.error_message

    def test_retryable_error(self):
        """RetryableError → failure(retryable=True)."""

        @step_handler("retry_err")
        def retry_err(_context):
            raise RetryableError("Service unavailable")

        handler = retry_err._handler_class()
        result = _call_sync(handler, _make_context())
        assert result.is_success is False
        assert result.retryable is True
        assert result.error_message is not None
        assert "Service unavailable" in result.error_message

    def test_generic_exception(self):
        """Generic exception → failure(retryable=True) (safe default)."""

        @step_handler("generic_err")
        def generic_err(_context):
            raise ValueError("Something went wrong")

        handler = generic_err._handler_class()
        result = _call_sync(handler, _make_context())
        assert result.is_success is False
        assert result.retryable is True
        assert result.error_message is not None
        assert "Something went wrong" in result.error_message


# ============================================================================
# Tests: Decision Handler
# ============================================================================


class TestDecisionHandler:
    """Tests for @decision_handler and Decision helpers."""

    def test_decision_route(self):
        """Decision.route() creates a create_steps outcome via DecisionMixin."""

        @decision_handler("route_order")
        def route_order(_context):
            return Decision.route(["process_premium"], tier="premium")

        handler = route_order._handler_class()
        result = _call_sync(handler, _make_context())
        assert result.is_success is True
        assert result.result is not None
        # DecisionMixin format: type/step_names in outcome, routing_context at result level
        outcome = result.result["decision_point_outcome"]
        assert outcome["type"] == "create_steps"
        assert outcome["step_names"] == ["process_premium"]
        assert result.result["routing_context"]["tier"] == "premium"
        # DecisionMixin adds handler metadata
        assert result.metadata["decision_handler"] == "route_order"
        assert result.metadata["decision_version"] == "1.0.0"

    def test_decision_skip(self):
        """Decision.skip() creates a no_branches outcome via DecisionMixin."""

        @decision_handler("skip_handler")
        def skip_handler(_context):
            return Decision.skip("No items to process")

        handler = skip_handler._handler_class()
        result = _call_sync(handler, _make_context())
        assert result.is_success is True
        assert result.result is not None
        outcome = result.result["decision_point_outcome"]
        assert outcome["type"] == "no_branches"
        # reason is at result level in DecisionMixin format
        assert result.result["reason"] == "No items to process"

    def test_decision_with_dependencies(self):
        """Decision handler with dependency injection."""

        @decision_handler("route_with_deps")
        @depends_on(order="validate_order")
        def route_with_deps(order, _context):
            if order and order.get("tier") == "premium":
                return Decision.route(["process_premium"])
            return Decision.route(["process_standard"])

        handler = route_with_deps._handler_class()
        ctx = _make_context(
            "route_with_deps",
            dependency_results={"validate_order": {"result": {"tier": "premium"}}},
        )
        result = _call_sync(handler, ctx)
        assert result.is_success is True
        assert result.result is not None
        outcome = result.result["decision_point_outcome"]
        assert outcome["step_names"] == ["process_premium"]


# ============================================================================
# Tests: Batch Analyzer
# ============================================================================


class TestBatchAnalyzer:
    """Tests for @batch_analyzer."""

    def test_batch_config_auto_generates_cursors(self):
        """BatchConfig return auto-generates cursor configs via Batchable mixin."""

        @batch_analyzer("analyze", worker_template="process_batch")
        def analyze(_context):
            return BatchConfig(total_items=250, batch_size=100)

        handler = analyze._handler_class()
        result = _call_sync(handler, _make_context())
        assert result.is_success is True
        assert result.result is not None

        # Batchable mixin adds these at result level
        assert result.result["worker_count"] == 3
        assert result.result["total_items"] == 250
        assert result.metadata["batch_analyzer"] is True

        outcome = result.result["batch_processing_outcome"]
        assert outcome["type"] == "create_batches"
        assert outcome["worker_template_name"] == "process_batch"
        assert outcome["total_items"] == 250
        assert outcome["worker_count"] == 3
        assert len(outcome["cursor_configs"]) == 3

        # Verify cursor ranges
        configs = outcome["cursor_configs"]
        assert configs[0]["start_cursor"] == 0
        assert configs[0]["end_cursor"] == 100
        assert configs[1]["start_cursor"] == 100
        assert configs[1]["end_cursor"] == 200
        assert configs[2]["start_cursor"] == 200
        assert configs[2]["end_cursor"] == 250


# ============================================================================
# Tests: Batch Worker
# ============================================================================


class TestBatchWorker:
    """Tests for @batch_worker."""

    def test_batch_worker_receives_context(self):
        """Batch worker handler receives batch_context parameter."""

        @batch_worker("process_batch")
        def process_batch(batch_context, _context):
            # batch_context may be None if no batch data in step_config
            if batch_context is None:
                return {"no_batch": True}
            return {
                "start": batch_context.start_cursor,
                "end": batch_context.end_cursor,
            }

        handler = process_batch._handler_class()
        # Without batch context in step_config, batch_context is None
        result = _call_sync(handler, _make_context())
        assert result.is_success is True
        assert result.result == {"no_batch": True}

    def test_batch_worker_with_batch_data(self):
        """Batch worker extracts batch context from step_config."""

        @batch_worker("process_batch")
        def process_batch(batch_context, _context):
            return {
                "start": batch_context.start_cursor,
                "end": batch_context.end_cursor,
                "batch_id": batch_context.batch_id,
            }

        handler = process_batch._handler_class()
        ctx = _make_context(
            "process_batch",
            step_config={
                "batch_context": {
                    "batch_id": "batch_001",
                    "cursor_config": {
                        "start_cursor": 100,
                        "end_cursor": 200,
                        "step_size": 1,
                    },
                    "batch_index": 1,
                    "total_batches": 3,
                }
            },
        )
        result = _call_sync(handler, ctx)
        assert result.is_success is True
        assert result.result is not None
        assert result.result["start"] == 100
        assert result.result["end"] == 200
        assert result.result["batch_id"] == "batch_001"


# ============================================================================
# Tests: Async Handlers
# ============================================================================


class TestAsyncHandlers:
    """Tests for async handler support."""

    def test_async_step_handler(self):
        """Async handler is awaited properly."""

        @step_handler("async_handler")
        async def async_handler(_context):
            await asyncio.sleep(0)
            return {"async": True}

        handler = async_handler._handler_class()
        ctx = _make_context()
        coro = handler.call(ctx)
        assert asyncio.iscoroutine(coro)
        result = asyncio.run(coro)
        assert result.is_success is True
        assert result.result == {"async": True}

    def test_async_error_handling(self):
        """Async handler errors are caught and classified."""

        @step_handler("async_error")
        async def async_error(_context):
            await asyncio.sleep(0)
            raise PermanentError("Async permanent error")

        handler = async_error._handler_class()
        ctx = _make_context()
        coro = handler.call(ctx)
        assert asyncio.iscoroutine(coro)
        result = asyncio.run(coro)
        assert result.is_success is False
        assert result.retryable is False

    def test_async_with_deps_and_inputs(self):
        """Async handler with dependency and input injection."""

        @step_handler("async_full")
        @depends_on(data="fetch_data")
        @inputs("query")
        async def async_full(data, query, _context):
            await asyncio.sleep(0)
            return {"data": data, "query": query}

        handler = async_full._handler_class()
        ctx = _make_context(
            "async_full",
            input_data={"query": "search_term"},
            dependency_results={"fetch_data": {"result": {"items": [1, 2, 3]}}},
        )
        coro = handler.call(ctx)
        assert asyncio.iscoroutine(coro)
        result = asyncio.run(coro)
        assert result.is_success is True
        assert result.result is not None
        assert result.result["data"] == {"items": [1, 2, 3]}
        assert result.result["query"] == "search_term"


# ============================================================================
# Tests: Handler Class Compatibility
# ============================================================================


class TestHandlerClassCompatibility:
    """Tests that generated handler classes work with existing infrastructure."""

    def test_handler_class_is_step_handler_subclass(self):
        """Generated class is a StepHandler subclass."""
        from tasker_core.step_handler.base import StepHandler as BaseStepHandler

        @step_handler("compat_test")
        def compat_test(_context):
            return {}

        assert issubclass(compat_test._handler_class, BaseStepHandler)

    def test_handler_class_instantiable(self):
        """Generated class can be instantiated."""

        @step_handler("instantiate_test")
        def instantiate_test(_context):
            return {}

        handler = instantiate_test._handler_class()
        assert handler.name == "instantiate_test"
        assert handler.version == "1.0.0"
        assert handler.capabilities == ["process"]

    def test_handler_without_context_param(self):
        """Handler that doesn't accept context still works."""

        @step_handler("no_ctx")
        @inputs("value")
        def no_ctx(value):
            return {"value": value}

        handler = no_ctx._handler_class()
        ctx = _make_context("no_ctx", input_data={"value": 42})
        result = _call_sync(handler, ctx)
        assert result.is_success is True
        assert result.result == {"value": 42}

    def test_combined_deps_and_inputs(self):
        """Dependencies and inputs work together."""

        @step_handler("combined")
        @depends_on(prev="step_1")
        @inputs("config_key")
        def combined(prev, config_key, _context):
            return {"prev": prev, "config": config_key}

        handler = combined._handler_class()
        ctx = _make_context(
            "combined",
            input_data={"config_key": "abc"},
            dependency_results={"step_1": {"result": {"count": 5}}},
        )
        result = _call_sync(handler, ctx)
        assert result.is_success is True
        assert result.result is not None
        assert result.result["prev"] == {"count": 5}
        assert result.result["config"] == "abc"
