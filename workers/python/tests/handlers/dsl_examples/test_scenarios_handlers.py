"""DSL mirror of test_scenarios_handlers using @step_handler decorator.

- SuccessStepHandler: Always succeeds
- RetryableErrorStepHandler: Returns retryable errors
- PermanentErrorStepHandler: Returns permanent errors
"""

from __future__ import annotations

from datetime import datetime, timezone

from tasker_core.errors import PermanentError, RetryableError
from tasker_core.step_handler.functional import inputs, step_handler
from tasker_core.types import StepHandlerResult


@step_handler("test_scenarios_dsl.step_handlers.success_step")
@inputs("message")
def success_step(message, context):
    """Execute successfully."""
    if message is None:
        message = "Step completed successfully"

    return {
        "status": "success",
        "message": message,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "handler": "SuccessStepHandler",
    }


@step_handler("test_scenarios_dsl.step_handlers.retryable_error_step")
@inputs("error_message")
def retryable_error_step(error_message, context):
    """Return a retryable error."""
    if error_message is None:
        error_message = "Temporary failure - please retry"

    return StepHandlerResult.failure(
        message=error_message,
        error_type="temporary_error",
        retryable=True,
        metadata={
            "handler": "RetryableErrorStepHandler",
            "timestamp": datetime.now(timezone.utc).isoformat(),
        },
    )


@step_handler("test_scenarios_dsl.step_handlers.permanent_error_step")
@inputs("error_message")
def permanent_error_step(error_message, context):
    """Return a permanent error."""
    if error_message is None:
        error_message = "Permanent failure - do not retry"

    return StepHandlerResult.failure(
        message=error_message,
        error_type="permanent_error",
        retryable=False,
        metadata={
            "handler": "PermanentErrorStepHandler",
            "timestamp": datetime.now(timezone.utc).isoformat(),
        },
    )


__all__ = [
    "success_step",
    "retryable_error_step",
    "permanent_error_step",
]
