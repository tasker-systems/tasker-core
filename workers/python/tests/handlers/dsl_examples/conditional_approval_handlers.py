"""DSL mirror of conditional_approval_handlers using @step_handler and @decision_handler.

Thresholds:
- < $1,000: auto_approve only
- $1,000-$4,999: manager_approval only
- >= $5,000: manager_approval + finance_review
"""

from __future__ import annotations

from typing import Any

from tasker_core.step_handler.functional import (
    Decision,
    decision_handler,
    depends_on,
    inputs,
    step_handler,
)
from tasker_core.types import StepHandlerResult

SMALL_THRESHOLD = 1000.0
LARGE_THRESHOLD = 5000.0


@step_handler("conditional_approval_dsl_py.step_handlers.validate_request")
@inputs("amount", "requester", "purpose")
def validate_request(amount, requester, purpose, _context):
    """Validate the approval request."""
    errors: list[str] = []

    if amount is None:
        errors.append("amount is required")
    elif not isinstance(amount, (int, float)) or amount <= 0:
        errors.append("amount must be a positive number")

    if not requester:
        errors.append("requester is required")

    if not purpose:
        errors.append("purpose is required")

    if errors:
        return StepHandlerResult.failure(
            message=f"Validation failed: {', '.join(errors)}",
            error_type="validation_error",
            retryable=False,
        )

    return {
        "validated": True,
        "amount": amount,
        "requester": requester,
        "purpose": purpose,
    }


@decision_handler("conditional_approval_dsl_py.step_handlers.routing_decision")
@depends_on(validate_result="validate_request_dsl_py")
def routing_decision(validate_result, _context):
    """Determine the approval routing based on amount."""
    if validate_result is None:
        return StepHandlerResult.failure(
            message="Missing validation result from validate_request_dsl_py",
            error_type="dependency_error",
            retryable=True,
        )

    amount = validate_result.get("amount")
    if amount is None:
        return StepHandlerResult.failure(
            message="Missing amount in validation result",
            error_type="dependency_error",
            retryable=True,
        )

    if amount < SMALL_THRESHOLD:
        return Decision.route(
            ["auto_approve_dsl_py"],
            approval_path="auto",
            amount=amount,
            threshold_used="small",
        )
    elif amount < LARGE_THRESHOLD:
        return Decision.route(
            ["manager_approval_dsl_py"],
            approval_path="manager",
            amount=amount,
            threshold_used="medium",
        )
    else:
        return Decision.route(
            ["manager_approval_dsl_py", "finance_review_dsl_py"],
            approval_path="dual",
            amount=amount,
            threshold_used="large",
        )


@step_handler("conditional_approval_dsl_py.step_handlers.auto_approve")
@depends_on(routing_result="routing_decision_dsl_py")
def auto_approve(routing_result, _context):
    """Auto-approve the request."""
    if routing_result is None:
        return StepHandlerResult.failure(
            message="Missing routing decision result",
            error_type="dependency_error",
            retryable=True,
        )

    routing_context = routing_result.get("routing_context", {})
    amount = routing_context.get("amount", 0)

    return {
        "approved": True,
        "approval_type": "auto",
        "approved_amount": amount,
        "approver": "system",
        "notes": "Auto-approved for amounts under $1,000",
    }


@step_handler("conditional_approval_dsl_py.step_handlers.manager_approval")
@depends_on(routing_result="routing_decision_dsl_py")
def manager_approval(routing_result, _context):
    """Process manager approval."""
    if routing_result is None:
        return StepHandlerResult.failure(
            message="Missing routing decision result",
            error_type="dependency_error",
            retryable=True,
        )

    routing_context = routing_result.get("routing_context", {})
    amount = routing_context.get("amount", 0)

    return {
        "approved": True,
        "approval_type": "manager",
        "approved_amount": amount,
        "approver": "manager@example.com",
        "notes": "Manager approved after review",
    }


@step_handler("conditional_approval_dsl_py.step_handlers.finance_review")
@depends_on(routing_result="routing_decision_dsl_py")
def finance_review(routing_result, _context):
    """Process finance review."""
    if routing_result is None:
        return StepHandlerResult.failure(
            message="Missing routing decision result",
            error_type="dependency_error",
            retryable=True,
        )

    routing_context = routing_result.get("routing_context", {})
    amount = routing_context.get("amount", 0)

    return {
        "approved": True,
        "approval_type": "finance",
        "approved_amount": amount,
        "approver": "finance@example.com",
        "budget_code": "CAPEX-2024",
        "notes": "Finance review completed for large amount",
    }


@step_handler("conditional_approval_dsl_py.step_handlers.finalize_approval")
@depends_on(
    auto_result="auto_approve_dsl_py",
    manager_result="manager_approval_dsl_py",
    finance_result="finance_review_dsl_py",
)
def finalize_approval(auto_result, manager_result, finance_result, _context):
    """Finalize the approval process."""
    approvals: list[dict[str, Any]] = []

    if auto_result:
        approvals.append(
            {
                "type": "auto",
                "approved": auto_result.get("approved", False),
                "approver": auto_result.get("approver"),
            }
        )

    if manager_result:
        approvals.append(
            {
                "type": "manager",
                "approved": manager_result.get("approved", False),
                "approver": manager_result.get("approver"),
            }
        )

    if finance_result:
        approvals.append(
            {
                "type": "finance",
                "approved": finance_result.get("approved", False),
                "approver": finance_result.get("approver"),
            }
        )

    all_approved = all(a.get("approved", False) for a in approvals)

    return {
        "final_status": "approved" if all_approved else "rejected",
        "approval_count": len(approvals),
        "approvals": approvals,
        "all_approved": all_approved,
    }


__all__ = [
    "validate_request",
    "routing_decision",
    "auto_approve",
    "manager_approval",
    "finance_review",
    "finalize_approval",
]
