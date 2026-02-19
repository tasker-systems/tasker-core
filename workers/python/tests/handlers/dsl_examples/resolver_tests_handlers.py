"""DSL mirror of resolver_tests_handlers.

Note: The multi-method dispatch pattern (validate, process, refund methods)
is a class-based feature that doesn't map to the decorator API. The DSL
version only mirrors the default `call` method behavior.
"""

from __future__ import annotations

from tasker_core.step_handler.functional import inputs, step_handler


@step_handler("resolver_tests_dsl.step_handlers.multi_method")
@inputs("data")
def multi_method(data, context):
    """Default entry point - standard processing."""
    if data is None:
        data = {}

    return {
        "invoked_method": "call",
        "handler": "MultiMethodHandler",
        "message": "Default call method invoked",
        "input_received": data,
        "step_uuid": str(context.step_uuid),
    }


@step_handler("resolver_tests_dsl.step_handlers.alternate_method")
def alternate_method(context):
    """Default entry point for alternate handler."""
    return {
        "invoked_method": "call",
        "handler": "AlternateMethodHandler",
        "message": "Alternate handler default method",
        "step_uuid": str(context.step_uuid),
    }


__all__ = [
    "multi_method",
    "alternate_method",
]
