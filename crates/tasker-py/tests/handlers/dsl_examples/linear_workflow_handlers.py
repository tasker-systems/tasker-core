"""DSL mirror of linear_workflow_handlers using @step_handler decorator.

Same mathematical operations as the verbose version:
    LinearStep1 (square) -> LinearStep2 (add) -> LinearStep3 (multiply) -> LinearStep4 (divide)

For input even_number=4:
    Step1: 4^2 = 16
    Step2: 16 + 10 = 26
    Step3: 26 * 3 = 78
    Step4: 78 / 2 = 39.0
"""

from __future__ import annotations

from tasker_core.step_handler.functional import (
    depends_on,
    inputs,
    step_handler,
)
from tasker_core.types import StepHandlerResult


@step_handler("linear_workflow_dsl_py.step_handlers.linear_step_1")
@inputs("even_number")
def linear_step_1(even_number, _context):
    """Square the even number."""
    if even_number is None:
        return StepHandlerResult.failure(
            message="Task context must contain an even_number",
            error_type="validation_error",
            retryable=False,
        )

    if not isinstance(even_number, int) or even_number % 2 != 0:
        return StepHandlerResult.failure(
            message=f"even_number must be an even integer, got: {even_number}",
            error_type="validation_error",
            retryable=False,
        )

    result = even_number * even_number

    return {
        "result": result,
        "operation": "square",
        "step_type": "initial",
        "input_refs": {"even_number": "task.context.even_number"},
    }


@step_handler("linear_workflow_dsl_py.step_handlers.linear_step_2")
@depends_on(step1_output="linear_step_1_dsl_py")
def linear_step_2(step1_output, _context):
    """Add 10 to the squared result."""
    if step1_output is None:
        return StepHandlerResult.failure(
            message="Missing result from linear_step_1_dsl_py",
            error_type="dependency_error",
            retryable=True,
        )

    squared_value = step1_output.get("result") if isinstance(step1_output, dict) else step1_output

    if squared_value is None:
        return StepHandlerResult.failure(
            message="Missing 'result' field in linear_step_1_dsl_py output",
            error_type="dependency_error",
            retryable=True,
        )

    constant = 10
    result = squared_value + constant

    return {
        "result": result,
        "operation": "add",
        "constant": constant,
        "input_value": squared_value,
    }


@step_handler("linear_workflow_dsl_py.step_handlers.linear_step_3")
@depends_on(step2_output="linear_step_2_dsl_py")
def linear_step_3(step2_output, _context):
    """Multiply by 3."""
    if step2_output is None:
        return StepHandlerResult.failure(
            message="Missing result from linear_step_2_dsl_py",
            error_type="dependency_error",
            retryable=True,
        )

    added_value = step2_output.get("result") if isinstance(step2_output, dict) else step2_output

    if added_value is None:
        return StepHandlerResult.failure(
            message="Missing 'result' field in linear_step_2_dsl_py output",
            error_type="dependency_error",
            retryable=True,
        )

    factor = 3
    result = added_value * factor

    return {
        "result": result,
        "operation": "multiply",
        "factor": factor,
        "input_value": added_value,
    }


@step_handler("linear_workflow_dsl_py.step_handlers.linear_step_4")
@depends_on(step3_output="linear_step_3_dsl_py")
def linear_step_4(step3_output, _context):
    """Divide by 2 for final result."""
    if step3_output is None:
        return StepHandlerResult.failure(
            message="Missing result from linear_step_3_dsl_py",
            error_type="dependency_error",
            retryable=True,
        )

    multiplied_value = (
        step3_output.get("result") if isinstance(step3_output, dict) else step3_output
    )

    if multiplied_value is None:
        return StepHandlerResult.failure(
            message="Missing 'result' field in linear_step_3_dsl_py output",
            error_type="dependency_error",
            retryable=True,
        )

    divisor = 2
    result = multiplied_value / divisor

    return {
        "result": result,
        "operation": "divide",
        "divisor": divisor,
        "input_value": multiplied_value,
    }


__all__ = [
    "linear_step_1",
    "linear_step_2",
    "linear_step_3",
    "linear_step_4",
]
