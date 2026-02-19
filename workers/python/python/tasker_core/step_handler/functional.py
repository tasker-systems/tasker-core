"""Functional/decorator API for step handlers (TAS-294).

This module provides a decorator-based alternative to the class-based handler API.
It reduces boilerplate for common handler patterns while preserving full access
to the underlying StepContext for advanced use cases.

The decorators auto-wrap return values and classify exceptions:
- dict return -> StepHandlerResult.success(dict)
- StepHandlerResult return -> pass through unchanged
- PermanentError raised -> StepHandlerResult.failure(retryable=False)
- RetryableError raised -> StepHandlerResult.failure(retryable=True)
- Other exceptions -> StepHandlerResult.failure(retryable=True)

Example:
    >>> from tasker_core.step_handler.functional import step_handler, depends_on, inputs
    >>>
    >>> @step_handler("process_payment")
    ... @depends_on(cart="validate_cart")
    ... @inputs("payment_info")
    ... async def process_payment(cart, payment_info, context):
    ...     if not payment_info:
    ...         raise PermanentError("Payment info required")
    ...     result = await charge_card(payment_info, cart["total"])
    ...     return {"payment_id": result["id"], "amount": cart["total"]}
"""

from __future__ import annotations

import asyncio
import inspect
import traceback
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, Callable

from tasker_core.errors import PermanentError, RetryableError, TaskerError
from tasker_core.step_handler.base import StepHandler
from tasker_core.types import (
    DecisionPointOutcome,
    DecisionType,
    ErrorType,
    StepHandlerResult,
)

if TYPE_CHECKING:
    from tasker_core.types import StepContext


# ============================================================================
# Helper Types
# ============================================================================


@dataclass(frozen=True)
class Decision:
    """Helper for decision handler return values.

    Provides simple factory methods for creating decision outcomes
    without needing to import DecisionPointOutcome directly.

    Example:
        >>> @decision_handler("route_order")
        ... @depends_on(order="validate_order")
        ... def route_order(order, context):
        ...     if order["tier"] == "premium":
        ...         return Decision.route(["process_premium"], tier="premium")
        ...     return Decision.route(["process_standard"])
    """

    outcome: DecisionPointOutcome

    @staticmethod
    def route(
        steps: list[str],
        **routing_context: Any,
    ) -> Decision:
        """Route to the specified steps.

        Args:
            steps: Step names to execute.
            **routing_context: Additional context for routing decisions.
        """
        return Decision(
            outcome=DecisionPointOutcome.create_steps(
                step_names=steps,
                routing_context=routing_context if routing_context else {},
            )
        )

    @staticmethod
    def skip(reason: str, **routing_context: Any) -> Decision:
        """Skip all branches.

        Args:
            reason: Human-readable reason for skipping.
            **routing_context: Additional context for routing decisions.
        """
        return Decision(
            outcome=DecisionPointOutcome.no_branches(
                reason=reason,
                routing_context=routing_context if routing_context else {},
            )
        )


@dataclass
class BatchConfig:
    """Configuration returned by batch analyzer handlers.

    Example:
        >>> @batch_analyzer("analyze_csv", worker_template="process_csv_batch")
        ... def analyze_csv(context):
        ...     row_count = count_rows(context.get_input("file_path"))
        ...     return BatchConfig(total_items=row_count, batch_size=1000)
    """

    total_items: int
    batch_size: int
    metadata: dict[str, Any] = field(default_factory=dict)


# ============================================================================
# Internal: Auto-wrapping logic
# ============================================================================


def _wrap_result(result: Any) -> StepHandlerResult:
    """Convert a handler return value to StepHandlerResult.

    - StepHandlerResult -> pass through
    - dict -> success
    - None -> success with empty dict
    """
    if isinstance(result, StepHandlerResult):
        return result
    if isinstance(result, dict):
        return StepHandlerResult.success(result)
    if result is None:
        return StepHandlerResult.success({})
    # Fallback: wrap in a dict
    return StepHandlerResult.success({"result": result})


def _wrap_exception(exc: Exception) -> StepHandlerResult:
    """Convert an exception to a failure StepHandlerResult."""
    if isinstance(exc, PermanentError):
        return StepHandlerResult.failure(
            message=str(exc),
            error_type=ErrorType.PERMANENT_ERROR,
            retryable=False,
            metadata=exc.metadata if hasattr(exc, "metadata") else {},
        )
    if isinstance(exc, RetryableError):
        return StepHandlerResult.failure(
            message=str(exc),
            error_type=ErrorType.RETRYABLE_ERROR,
            retryable=True,
            metadata=exc.metadata if hasattr(exc, "metadata") else {},
        )
    if isinstance(exc, TaskerError):
        return StepHandlerResult.failure(
            message=str(exc),
            error_type=ErrorType.HANDLER_ERROR,
            retryable=getattr(exc, "retryable", True),
            metadata=getattr(exc, "metadata", {}),
        )
    # Unknown exception: retryable by default (safe default)
    return StepHandlerResult.failure(
        message=str(exc),
        error_type=ErrorType.HANDLER_ERROR,
        retryable=True,
        metadata={"exception_type": type(exc).__name__, "traceback": traceback.format_exc()},
    )


def _inject_args(
    fn: Callable[..., Any],
    context: StepContext,
    dep_map: dict[str, str],
    input_keys: list[str],
) -> dict[str, Any]:
    """Build keyword arguments for a functional handler.

    Injects dependency results, input values, and context.
    """
    kwargs: dict[str, Any] = {}

    # Inject dependency results
    for param_name, step_name in dep_map.items():
        kwargs[param_name] = context.get_dependency_result(step_name)

    # Inject input values
    for key in input_keys:
        kwargs[key] = context.get_input(key)

    # Always provide context if the function accepts it
    sig = inspect.signature(fn)
    if "context" in sig.parameters:
        kwargs["context"] = context

    return kwargs


def _make_handler_class(
    fn: Callable[..., Any],
    name: str,
    version: str,
    dep_map: dict[str, str],
    input_keys: list[str],
    result_transformer: Callable[[Any], StepHandlerResult] | None = None,
) -> type[StepHandler]:
    """Create a StepHandler subclass from a decorated function.

    The generated class:
    - Sets handler_name and handler_version
    - Injects dependencies and inputs as keyword arguments
    - Auto-wraps return values and classifies exceptions
    - Supports both sync and async handler functions
    """
    is_async = asyncio.iscoroutinefunction(fn)
    transformer = result_transformer or _wrap_result

    if is_async:

        class AsyncFunctionalHandler(StepHandler):
            handler_name = name
            handler_version = version

            async def call(self, context: StepContext) -> StepHandlerResult:
                try:
                    kwargs = _inject_args(fn, context, dep_map, input_keys)
                    raw_result = await fn(**kwargs)
                    return transformer(raw_result)
                except (PermanentError, RetryableError, TaskerError) as exc:
                    return _wrap_exception(exc)
                except Exception as exc:
                    return _wrap_exception(exc)

        AsyncFunctionalHandler.__name__ = fn.__name__
        AsyncFunctionalHandler.__qualname__ = fn.__qualname__
        AsyncFunctionalHandler.__doc__ = fn.__doc__
        return AsyncFunctionalHandler
    else:

        class SyncFunctionalHandler(StepHandler):
            handler_name = name
            handler_version = version

            def call(self, context: StepContext) -> StepHandlerResult:
                try:
                    kwargs = _inject_args(fn, context, dep_map, input_keys)
                    raw_result = fn(**kwargs)
                    return transformer(raw_result)
                except (PermanentError, RetryableError, TaskerError) as exc:
                    return _wrap_exception(exc)
                except Exception as exc:
                    return _wrap_exception(exc)

        SyncFunctionalHandler.__name__ = fn.__name__
        SyncFunctionalHandler.__qualname__ = fn.__qualname__
        SyncFunctionalHandler.__doc__ = fn.__doc__
        return SyncFunctionalHandler


# ============================================================================
# Public Decorators
# ============================================================================


def step_handler(
    name: str,
    version: str = "1.0.0",
) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    """Decorator that wraps a function as a StepHandler subclass.

    The decorated function receives injected dependencies, inputs, and context
    as keyword arguments. Return values are auto-wrapped as success results,
    and exceptions are auto-classified as failure results.

    Args:
        name: Handler name (must match step definition).
        version: Handler version (default: "1.0.0").

    Returns:
        Decorator that attaches ``_handler_class`` to the function.

    Example:
        >>> @step_handler("process_payment")
        ... @depends_on(cart="validate_cart")
        ... @inputs("payment_info")
        ... async def process_payment(cart, payment_info, context):
        ...     if not payment_info:
        ...         raise PermanentError("Payment info required")
        ...     result = await charge_card(payment_info, cart["total"])
        ...     return {"payment_id": result["id"], "amount": cart["total"]}
        >>>
        >>> # The handler class is accessible for registration:
        >>> registry.register(process_payment._handler_class)
    """

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        dep_map: dict[str, str] = getattr(fn, "_depends_on", {})
        input_keys: list[str] = getattr(fn, "_inputs", [])

        handler_cls = _make_handler_class(fn, name, version, dep_map, input_keys)
        fn._handler_class = handler_cls  # type: ignore[attr-defined]
        return fn

    return decorator


def depends_on(**deps: str) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    """Declare dependency step results to inject as named parameters.

    Each keyword argument maps a parameter name to a step name.
    The dependency result is fetched via ``context.get_dependency_result(step_name)``
    and injected as the named parameter. Missing dependencies inject ``None``.

    Args:
        **deps: Mapping of parameter_name="step_name".

    Example:
        >>> @step_handler("process_order")
        ... @depends_on(cart="validate_cart", user="fetch_user")
        ... def process_order(cart, user, context):
        ...     return {"order": f"for {user['name']}", "total": cart["total"]}
    """

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        existing = getattr(fn, "_depends_on", {})
        fn._depends_on = {**existing, **deps}  # type: ignore[attr-defined]
        return fn

    return decorator


def inputs(*keys: str) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    """Declare task context inputs to inject as named parameters.

    Each key is looked up via ``context.get_input(key)`` and injected
    as a parameter with the same name. Missing inputs inject ``None``.

    Args:
        *keys: Input key names to inject.

    Example:
        >>> @step_handler("validate_payment")
        ... @inputs("payment_info", "billing_address")
        ... def validate_payment(payment_info, billing_address, context):
        ...     if not payment_info:
        ...         raise PermanentError("Payment info required")
        ...     return {"valid": True}
    """

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        existing = getattr(fn, "_inputs", [])
        fn._inputs = [*existing, *keys]  # type: ignore[attr-defined]
        return fn

    return decorator


# ============================================================================
# Specialized Decorators
# ============================================================================


def decision_handler(
    name: str,
    version: str = "1.0.0",
) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    """Decorator for decision point handlers.

    The decorated function should return a ``Decision.route(...)`` or
    ``Decision.skip(...)`` value. The result is automatically wrapped
    into the proper decision point outcome format using the DecisionMixin,
    ensuring Rust-compatible output structure.

    Args:
        name: Handler name.
        version: Handler version.

    Example:
        >>> @decision_handler("route_order")
        ... @depends_on(order="validate_order")
        ... def route_order(order, context):
        ...     if order["tier"] == "premium":
        ...         return Decision.route(["process_premium"], tier="premium")
        ...     return Decision.route(["process_standard"])
    """

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        dep_map: dict[str, str] = getattr(fn, "_depends_on", {})
        input_keys: list[str] = getattr(fn, "_inputs", [])
        is_async = asyncio.iscoroutinefunction(fn)

        from tasker_core.step_handler.mixins.decision import DecisionMixin

        if is_async:

            class AsyncDecisionHandler(DecisionMixin, StepHandler):
                handler_name = name
                handler_version = version

                async def call(self, context: StepContext) -> StepHandlerResult:
                    try:
                        kwargs = _inject_args(fn, context, dep_map, input_keys)
                        raw_result = await fn(**kwargs)
                        if isinstance(raw_result, StepHandlerResult):
                            return raw_result
                        if isinstance(raw_result, Decision):
                            outcome = raw_result.outcome
                            if outcome.decision_type == DecisionType.CREATE_STEPS:
                                return self.decision_success(
                                    outcome.next_step_names,
                                    routing_context=outcome.routing_context,
                                )
                            else:
                                return self.skip_branches(
                                    reason=outcome.reason or "No branches",
                                    routing_context=outcome.routing_context,
                                )
                        return _wrap_result(raw_result)
                    except Exception as exc:
                        return _wrap_exception(exc)

            AsyncDecisionHandler.__name__ = fn.__name__
            AsyncDecisionHandler.__qualname__ = fn.__qualname__
            AsyncDecisionHandler.__doc__ = fn.__doc__
            fn._handler_class = AsyncDecisionHandler  # type: ignore[attr-defined]
        else:

            class SyncDecisionHandler(DecisionMixin, StepHandler):
                handler_name = name
                handler_version = version

                def call(self, context: StepContext) -> StepHandlerResult:
                    try:
                        kwargs = _inject_args(fn, context, dep_map, input_keys)
                        raw_result = fn(**kwargs)
                        if isinstance(raw_result, StepHandlerResult):
                            return raw_result
                        if isinstance(raw_result, Decision):
                            outcome = raw_result.outcome
                            if outcome.decision_type == DecisionType.CREATE_STEPS:
                                return self.decision_success(
                                    outcome.next_step_names,
                                    routing_context=outcome.routing_context,
                                )
                            else:
                                return self.skip_branches(
                                    reason=outcome.reason or "No branches",
                                    routing_context=outcome.routing_context,
                                )
                        return _wrap_result(raw_result)
                    except Exception as exc:
                        return _wrap_exception(exc)

            SyncDecisionHandler.__name__ = fn.__name__
            SyncDecisionHandler.__qualname__ = fn.__qualname__
            SyncDecisionHandler.__doc__ = fn.__doc__
            fn._handler_class = SyncDecisionHandler  # type: ignore[attr-defined]

        return fn

    return decorator


def batch_analyzer(
    name: str,
    worker_template: str,
    version: str = "1.0.0",
) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    """Decorator for batch analyzer handlers.

    The decorated function should return a ``BatchConfig`` with
    ``total_items`` and ``batch_size``. The decorator automatically
    generates cursor configs and creates the batch processing outcome
    by delegating to the Batchable mixin's well-tested methods.

    Args:
        name: Handler name.
        worker_template: Name of the worker template step.
        version: Handler version.

    Example:
        >>> @batch_analyzer("analyze_csv", worker_template="process_csv_batch")
        ... def analyze_csv(context):
        ...     row_count = count_rows(context.get_input("file_path"))
        ...     return BatchConfig(total_items=row_count, batch_size=1000)
    """

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        dep_map: dict[str, str] = getattr(fn, "_depends_on", {})
        input_keys: list[str] = getattr(fn, "_inputs", [])
        is_async = asyncio.iscoroutinefunction(fn)

        from tasker_core.batch_processing.batchable import Batchable

        if is_async:

            class AsyncBatchAnalyzerHandler(Batchable, StepHandler):
                handler_name = name
                handler_version = version

                async def call(self, context: StepContext) -> StepHandlerResult:
                    try:
                        kwargs = _inject_args(fn, context, dep_map, input_keys)
                        raw_result = await fn(**kwargs)
                        if isinstance(raw_result, StepHandlerResult):
                            return raw_result
                        if isinstance(raw_result, BatchConfig):
                            outcome = self.create_batch_outcome(
                                total_items=raw_result.total_items,
                                batch_size=raw_result.batch_size,
                            )
                            return self.batch_analyzer_success(
                                outcome,
                                worker_template_name=worker_template,
                                metadata=raw_result.metadata or None,
                            )
                        return _wrap_result(raw_result)
                    except Exception as exc:
                        return _wrap_exception(exc)

            AsyncBatchAnalyzerHandler.__name__ = fn.__name__
            AsyncBatchAnalyzerHandler.__qualname__ = fn.__qualname__
            AsyncBatchAnalyzerHandler.__doc__ = fn.__doc__
            fn._handler_class = AsyncBatchAnalyzerHandler  # type: ignore[attr-defined]
        else:

            class SyncBatchAnalyzerHandler(Batchable, StepHandler):
                handler_name = name
                handler_version = version

                def call(self, context: StepContext) -> StepHandlerResult:
                    try:
                        kwargs = _inject_args(fn, context, dep_map, input_keys)
                        raw_result = fn(**kwargs)
                        if isinstance(raw_result, StepHandlerResult):
                            return raw_result
                        if isinstance(raw_result, BatchConfig):
                            outcome = self.create_batch_outcome(
                                total_items=raw_result.total_items,
                                batch_size=raw_result.batch_size,
                            )
                            return self.batch_analyzer_success(
                                outcome,
                                worker_template_name=worker_template,
                                metadata=raw_result.metadata or None,
                            )
                        return _wrap_result(raw_result)
                    except Exception as exc:
                        return _wrap_exception(exc)

            SyncBatchAnalyzerHandler.__name__ = fn.__name__
            SyncBatchAnalyzerHandler.__qualname__ = fn.__qualname__
            SyncBatchAnalyzerHandler.__doc__ = fn.__doc__
            fn._handler_class = SyncBatchAnalyzerHandler  # type: ignore[attr-defined]

        return fn

    return decorator


def batch_worker(
    name: str,
    version: str = "1.0.0",
) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    """Decorator for batch worker handlers.

    The decorated function receives a ``batch_context`` parameter
    (a ``BatchWorkerContext``) auto-extracted from the step context
    via the Batchable mixin's ``get_batch_context()`` method,
    in addition to any declared dependencies and inputs.

    Args:
        name: Handler name.
        version: Handler version.

    Example:
        >>> @batch_worker("process_csv_batch")
        ... def process_csv_batch(batch_context, context):
        ...     start = batch_context.start_cursor
        ...     end = batch_context.end_cursor
        ...     rows = read_csv_range(start, end)
        ...     return {"items_processed": len(rows)}
    """

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        dep_map: dict[str, str] = getattr(fn, "_depends_on", {})
        input_keys: list[str] = getattr(fn, "_inputs", [])
        is_async = asyncio.iscoroutinefunction(fn)
        transformer = _wrap_result

        from tasker_core.batch_processing.batchable import Batchable

        if is_async:

            class AsyncBatchHandler(Batchable, StepHandler):
                handler_name = name
                handler_version = version

                async def call(self, context: StepContext) -> StepHandlerResult:
                    try:
                        kwargs = _inject_args(fn, context, dep_map, input_keys)
                        kwargs["batch_context"] = self.get_batch_context(context)
                        raw_result = await fn(**kwargs)
                        return transformer(raw_result)
                    except Exception as exc:
                        return _wrap_exception(exc)

            AsyncBatchHandler.__name__ = fn.__name__
            AsyncBatchHandler.__qualname__ = fn.__qualname__
            AsyncBatchHandler.__doc__ = fn.__doc__
            fn._handler_class = AsyncBatchHandler  # type: ignore[attr-defined]
        else:

            class SyncBatchHandler(Batchable, StepHandler):
                handler_name = name
                handler_version = version

                def call(self, context: StepContext) -> StepHandlerResult:
                    try:
                        kwargs = _inject_args(fn, context, dep_map, input_keys)
                        kwargs["batch_context"] = self.get_batch_context(context)
                        raw_result = fn(**kwargs)
                        return transformer(raw_result)
                    except Exception as exc:
                        return _wrap_exception(exc)

            SyncBatchHandler.__name__ = fn.__name__
            SyncBatchHandler.__qualname__ = fn.__qualname__
            SyncBatchHandler.__doc__ = fn.__doc__
            fn._handler_class = SyncBatchHandler  # type: ignore[attr-defined]

        return fn

    return decorator


__all__ = [
    "BatchConfig",
    "Decision",
    "batch_analyzer",
    "batch_worker",
    "decision_handler",
    "depends_on",
    "inputs",
    "step_handler",
]
