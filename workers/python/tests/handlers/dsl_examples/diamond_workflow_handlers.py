"""DSL mirror of diamond_handlers.py and diamond_workflow_handlers.py.

Covers both unit-test diamond (init/pathA/pathB/merge) and
E2E diamond (start/branchB/branchC/end) patterns.
"""

from __future__ import annotations

from tasker_core.step_handler.functional import (
    depends_on,
    inputs,
    step_handler,
)
from tasker_core.types import StepHandlerResult

# ============================================================================
# Unit-test diamond (diamond_handlers.py mirror)
# ============================================================================


@step_handler("diamond_init_dsl")
@inputs("initial_value")
def diamond_init(initial_value, context):
    """Initialize the diamond workflow."""
    if initial_value is None:
        initial_value = 100
    return {
        "initialized": True,
        "value": initial_value,
        "metadata": {
            "workflow": "diamond",
            "init_timestamp": "2025-01-01T00:00:00Z",
        },
    }


@step_handler("diamond_path_a_dsl")
@depends_on(init_result="diamond_init")
def diamond_path_a(init_result, context):
    """Process data via path A (multiplication)."""
    if init_result is None:
        init_result = {}
    value = init_result.get("value", 0)
    result = value * 2
    return {
        "path_a_result": result,
        "operation": "multiply_by_2",
        "input_value": value,
    }


@step_handler("diamond_path_b_dsl")
@depends_on(init_result="diamond_init")
def diamond_path_b(init_result, context):
    """Process data via path B (addition)."""
    if init_result is None:
        init_result = {}
    value = init_result.get("value", 0)
    result = value + 50
    return {
        "path_b_result": result,
        "operation": "add_50",
        "input_value": value,
    }


@step_handler("diamond_merge_dsl")
@depends_on(path_a="diamond_path_a", path_b="diamond_path_b")
def diamond_merge(path_a, path_b, context):
    """Merge results from both paths."""
    if path_a is None:
        path_a = {}
    if path_b is None:
        path_b = {}

    a_result = path_a.get("path_a_result", 0)
    b_result = path_b.get("path_b_result", 0)

    if a_result == 0 and b_result == 0:
        return StepHandlerResult.failure(
            message="Missing results from both paths",
            error_type="dependency_error",
            retryable=True,
        )

    merged = a_result + b_result

    return {
        "merged_result": merged,
        "path_a_value": a_result,
        "path_b_value": b_result,
        "merge_summary": {
            "total": merged,
            "path_a_operation": path_a.get("operation", "unknown"),
            "path_b_operation": path_b.get("operation", "unknown"),
        },
    }


# ============================================================================
# E2E diamond (diamond_workflow_handlers.py mirror)
# ============================================================================


@step_handler("diamond_workflow_dsl.step_handlers.diamond_start")
@inputs("even_number")
def diamond_start(even_number, context):
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
    }


@step_handler("diamond_workflow_dsl.step_handlers.diamond_branch_b")
@depends_on(start_output="diamond_start_py")
def diamond_branch_b(start_output, context):
    """Add 25 to the squared result."""
    if start_output is None:
        return StepHandlerResult.failure(
            message="Missing result from diamond_start_py",
            error_type="dependency_error",
            retryable=True,
        )

    squared_value = (
        start_output.get("result") if isinstance(start_output, dict) else start_output
    )

    if squared_value is None:
        return StepHandlerResult.failure(
            message="Missing 'result' field in diamond_start_py output",
            error_type="dependency_error",
            retryable=True,
        )

    constant = 25
    result = squared_value + constant

    return {
        "result": result,
        "operation": "add",
        "constant": constant,
        "input_value": squared_value,
        "branch": "B",
    }


@step_handler("diamond_workflow_dsl.step_handlers.diamond_branch_c")
@depends_on(start_output="diamond_start_py")
def diamond_branch_c(start_output, context):
    """Multiply the squared result by 2."""
    if start_output is None:
        return StepHandlerResult.failure(
            message="Missing result from diamond_start_py",
            error_type="dependency_error",
            retryable=True,
        )

    squared_value = (
        start_output.get("result") if isinstance(start_output, dict) else start_output
    )

    if squared_value is None:
        return StepHandlerResult.failure(
            message="Missing 'result' field in diamond_start_py output",
            error_type="dependency_error",
            retryable=True,
        )

    factor = 2
    result = squared_value * factor

    return {
        "result": result,
        "operation": "multiply",
        "factor": factor,
        "input_value": squared_value,
        "branch": "C",
    }


@step_handler("diamond_workflow_dsl.step_handlers.diamond_end")
@depends_on(branch_b_output="diamond_branch_b_py", branch_c_output="diamond_branch_c_py")
def diamond_end(branch_b_output, branch_c_output, context):
    """Calculate average of both branch results."""
    b_value = None
    c_value = None

    if branch_b_output is not None:
        b_value = (
            branch_b_output.get("result")
            if isinstance(branch_b_output, dict)
            else branch_b_output
        )

    if branch_c_output is not None:
        c_value = (
            branch_c_output.get("result")
            if isinstance(branch_c_output, dict)
            else branch_c_output
        )

    if b_value is None or c_value is None:
        missing = []
        if b_value is None:
            missing.append("diamond_branch_b_py")
        if c_value is None:
            missing.append("diamond_branch_c_py")
        return StepHandlerResult.failure(
            message=f"Missing results from: {', '.join(missing)}",
            error_type="dependency_error",
            retryable=True,
        )

    result = (b_value + c_value) / 2

    return {
        "result": result,
        "operation": "average",
        "branch_b_value": b_value,
        "branch_c_value": c_value,
        "step_type": "convergence",
    }


__all__ = [
    # Unit test diamond
    "diamond_init",
    "diamond_path_a",
    "diamond_path_b",
    "diamond_merge",
    # E2E diamond
    "diamond_start",
    "diamond_branch_b",
    "diamond_branch_c",
    "diamond_end",
]
