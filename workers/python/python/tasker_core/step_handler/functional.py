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
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, Protocol, cast, runtime_checkable

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


@runtime_checkable
class FunctionalHandler(Protocol):
    """Protocol for decorated handler functions.

    All ``@step_handler``, ``@decision_handler``, ``@batch_analyzer``, and
    ``@batch_worker`` decorated functions expose a ``_handler_class`` attribute
    containing the generated ``StepHandler`` subclass.
    """

    _handler_class: type[StepHandler]
    __name__: str
    __call__: Callable[..., Any]


#: Type alias for the decorator return — a callable that is also a
#: :class:`FunctionalHandler` (i.e. has ``_handler_class``).
HandlerDecorator = Callable[[Callable[..., Any]], FunctionalHandler]


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
    - BaseModel -> serialize via model_dump() then success
    - None -> success with empty dict
    """
    if isinstance(result, StepHandlerResult):
        return result
    if isinstance(result, dict):
        return StepHandlerResult.success(result)
    # Pydantic BaseModel — serialize to dict for FFI boundary
    if hasattr(result, "model_dump"):
        return StepHandlerResult.success(result.model_dump(mode="json"))
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
    Supports model-based injection for both dependencies and inputs.
    """
    kwargs: dict[str, Any] = {}

    # Inject dependency results (with optional model construction)
    dep_models: dict[str, type] = getattr(fn, "_dep_models", {})
    for param_name, step_name in dep_map.items():
        raw = context.get_dependency_result(step_name)
        model_cls = dep_models.get(param_name)
        if model_cls is not None and isinstance(raw, dict):
            kwargs[param_name] = model_cls.model_construct(**raw)  # type: ignore[attr-defined]  # Pydantic BaseModel
        else:
            kwargs[param_name] = raw

    # Inject inputs (model-based or string-based)
    input_model: type | None = getattr(fn, "_input_model", None)
    if input_model is not None:
        model_data = {}
        for field_name in input_model.model_fields:  # type: ignore[attr-defined]  # Pydantic BaseModel
            model_data[field_name] = context.get_input(field_name)
        kwargs["inputs"] = input_model(**model_data)
    else:
        for key in input_keys:
            kwargs[key] = context.get_input(key)

    # Always provide context if the function accepts it
    sig = inspect.signature(fn)
    if "context" in sig.parameters:
        kwargs["context"] = context
    elif "_context" in sig.parameters:
        kwargs["_context"] = context

    return kwargs


def _copy_fn_metadata(cls: type, fn: Callable[..., Any]) -> None:
    """Copy function metadata to a generated handler class."""
    cls.__name__ = getattr(fn, "__name__", cls.__name__)
    cls.__qualname__ = getattr(fn, "__qualname__", cls.__qualname__)
    cls.__doc__ = getattr(fn, "__doc__", cls.__doc__)


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

        _copy_fn_metadata(AsyncFunctionalHandler, fn)
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

        _copy_fn_metadata(SyncFunctionalHandler, fn)
        return SyncFunctionalHandler


# ============================================================================
# Public Decorators
# ============================================================================


def step_handler(
    name: str,
    version: str = "1.0.0",
) -> HandlerDecorator:
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

    def decorator(fn: Callable[..., Any]) -> FunctionalHandler:
        dep_map: dict[str, str] = getattr(fn, "_depends_on", {})
        input_keys: list[str] = getattr(fn, "_inputs", [])

        handler_cls = _make_handler_class(fn, name, version, dep_map, input_keys)
        fn._handler_class = handler_cls  # type: ignore[attr-defined]
        return cast(FunctionalHandler, fn)

    return decorator


def depends_on(
    **deps: str | tuple[str, type],
) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    """Declare dependency step results to inject as named parameters.

    Each keyword argument maps a parameter name to either:
    - A step name string: the raw dependency result dict is injected.
    - A ``(step_name, ModelClass)`` tuple: the result dict is used to construct
      a model instance which is injected as a typed parameter.

    Args:
        **deps: Mapping of parameter_name="step_name" or
            parameter_name=("step_name", ModelClass).

    Example:
        >>> @step_handler("process_order")
        ... @depends_on(cart="validate_cart", user="fetch_user")
        ... def process_order(cart, user, context):
        ...     return {"order": f"for {user['name']}", "total": cart["total"]}
        >>>
        >>> # With model-based injection:
        >>> @step_handler("execute_refund")
        ... @depends_on(approval=("get_manager_approval", ApproveRefundResult))
        ... def execute_refund(approval: ApproveRefundResult, context):
        ...     return {"approved": approval.approved}
    """

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        existing = getattr(fn, "_depends_on", {})
        existing_models = getattr(fn, "_dep_models", {})
        dep_map: dict[str, str] = {}
        dep_models: dict[str, type] = {}
        for param_name, value in deps.items():
            if isinstance(value, tuple) and len(value) == 2:
                step_name, model_cls = value
                dep_map[param_name] = step_name
                dep_models[param_name] = model_cls
            else:
                dep_map[param_name] = value
        fn._depends_on = {**existing, **dep_map}  # type: ignore[attr-defined]
        fn._dep_models = {**existing_models, **dep_models}  # type: ignore[attr-defined]
        return fn

    return decorator


def inputs(*keys_or_model: str | type) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    """Declare task context inputs to inject as named parameters.

    Accepts either string key names or a single model class:

    - **String keys**: Each key is looked up via ``context.get_input(key)``
      and injected as an individual keyword argument.
    - **Model class**: A single class (e.g. a Pydantic ``BaseModel`` subclass)
      whose fields are looked up from ``context.get_input()`` and injected
      as a single ``inputs`` keyword argument containing the constructed model.

    Args:
        *keys_or_model: Input key names to inject, or a single model class.

    Example:
        >>> @step_handler("validate_payment")
        ... @inputs("payment_info", "billing_address")
        ... def validate_payment(payment_info, billing_address, context):
        ...     if not payment_info:
        ...         raise PermanentError("Payment info required")
        ...     return {"valid": True}
        >>>
        >>> # With model-based injection:
        >>> @step_handler("validate_refund")
        ... @inputs(ValidateRefundInput)
        ... def validate_refund(inputs: ValidateRefundInput, context):
        ...     return {"ticket": inputs.ticket_id}
    """

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        if len(keys_or_model) == 1 and isinstance(keys_or_model[0], type):
            fn._input_model = keys_or_model[0]  # type: ignore[attr-defined]
        else:
            existing = getattr(fn, "_inputs", [])
            fn._inputs = [*existing, *keys_or_model]  # type: ignore[attr-defined]
        return fn

    return decorator


# ============================================================================
# Specialized Decorators
# ============================================================================


def decision_handler(
    name: str,
    version: str = "1.0.0",
) -> HandlerDecorator:
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

    def decorator(fn: Callable[..., Any]) -> FunctionalHandler:
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

            _copy_fn_metadata(AsyncDecisionHandler, fn)
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

            _copy_fn_metadata(SyncDecisionHandler, fn)
            fn._handler_class = SyncDecisionHandler  # type: ignore[attr-defined]

        return cast(FunctionalHandler, fn)

    return decorator


def batch_analyzer(
    name: str,
    worker_template: str,
    version: str = "1.0.0",
) -> HandlerDecorator:
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

    def decorator(fn: Callable[..., Any]) -> FunctionalHandler:
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
                                batch_metadata=raw_result.metadata or {},
                            )
                            return self.batch_analyzer_success(
                                outcome,
                                worker_template_name=worker_template,
                            )
                        return _wrap_result(raw_result)
                    except Exception as exc:
                        return _wrap_exception(exc)

            _copy_fn_metadata(AsyncBatchAnalyzerHandler, fn)
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
                                batch_metadata=raw_result.metadata or {},
                            )
                            return self.batch_analyzer_success(
                                outcome,
                                worker_template_name=worker_template,
                            )
                        return _wrap_result(raw_result)
                    except Exception as exc:
                        return _wrap_exception(exc)

            _copy_fn_metadata(SyncBatchAnalyzerHandler, fn)
            fn._handler_class = SyncBatchAnalyzerHandler  # type: ignore[attr-defined]

        return cast(FunctionalHandler, fn)

    return decorator


def batch_worker(
    name: str,
    version: str = "1.0.0",
) -> HandlerDecorator:
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

    def decorator(fn: Callable[..., Any]) -> FunctionalHandler:
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

            _copy_fn_metadata(AsyncBatchHandler, fn)
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

            _copy_fn_metadata(SyncBatchHandler, fn)
            fn._handler_class = SyncBatchHandler  # type: ignore[attr-defined]

        return cast(FunctionalHandler, fn)

    return decorator


def api_handler(
    name: str,
    base_url: str,
    version: str = "1.0.0",
    timeout: float = 30.0,
    default_headers: dict[str, str] | None = None,
) -> HandlerDecorator:
    """Decorator for API handlers with HTTP client functionality.

    The decorated function receives injected dependencies, inputs, context,
    and an ``api`` object providing pre-configured HTTP methods and result
    helpers from the APIMixin.

    The ``api`` object exposes:
    - HTTP methods: ``get``, ``post``, ``put``, ``patch``, ``delete``, ``request``
    - Result helpers: ``api_success``, ``api_failure``, ``connection_error``, ``timeout_error``

    Args:
        name: Handler name (must match step definition).
        base_url: Base URL for API calls.
        version: Handler version (default: "1.0.0").
        timeout: Default request timeout in seconds (default: 30.0).
        default_headers: Default headers to include in all requests.

    Example:
        >>> @api_handler("fetch_user", base_url="https://api.example.com")
        ... @depends_on(user_id="validate_user")
        ... def fetch_user(user_id, api, context):
        ...     response = api.get(f"/users/{user_id}")
        ...     if response.ok:
        ...         return api.api_success(response)
        ...     return api.api_failure(response)
    """

    def decorator(fn: Callable[..., Any]) -> FunctionalHandler:
        dep_map: dict[str, str] = getattr(fn, "_depends_on", {})
        input_keys: list[str] = getattr(fn, "_inputs", [])
        is_async = inspect.iscoroutinefunction(fn)
        headers = default_headers or {}

        from tasker_core.step_handler.mixins.api import APIMixin

        if is_async:

            class AsyncApiHandler(APIMixin, StepHandler):
                handler_name = name
                handler_version = version

                async def call(self, context: StepContext) -> StepHandlerResult:
                    try:
                        kwargs = _inject_args(fn, context, dep_map, input_keys)
                        kwargs["api"] = self
                        raw_result = await fn(**kwargs)
                        return _wrap_result(raw_result)
                    except (PermanentError, RetryableError, TaskerError) as exc:
                        return _wrap_exception(exc)
                    except Exception as exc:
                        return _wrap_exception(exc)

            AsyncApiHandler.base_url = base_url
            AsyncApiHandler.default_timeout = timeout
            AsyncApiHandler.default_headers = headers
            _copy_fn_metadata(AsyncApiHandler, fn)
            fn._handler_class = AsyncApiHandler  # type: ignore[attr-defined]
        else:

            class SyncApiHandler(APIMixin, StepHandler):
                handler_name = name
                handler_version = version

                def call(self, context: StepContext) -> StepHandlerResult:
                    try:
                        kwargs = _inject_args(fn, context, dep_map, input_keys)
                        kwargs["api"] = self
                        raw_result = fn(**kwargs)
                        return _wrap_result(raw_result)
                    except (PermanentError, RetryableError, TaskerError) as exc:
                        return _wrap_exception(exc)
                    except Exception as exc:
                        return _wrap_exception(exc)

            SyncApiHandler.base_url = base_url
            SyncApiHandler.default_timeout = timeout
            SyncApiHandler.default_headers = headers
            _copy_fn_metadata(SyncApiHandler, fn)
            fn._handler_class = SyncApiHandler  # type: ignore[attr-defined]

        return cast(FunctionalHandler, fn)

    return decorator


__all__ = [
    "BatchConfig",
    "Decision",
    "FunctionalHandler",
    "HandlerDecorator",
    "api_handler",
    "batch_analyzer",
    "batch_worker",
    "decision_handler",
    "depends_on",
    "inputs",
    "step_handler",
]
