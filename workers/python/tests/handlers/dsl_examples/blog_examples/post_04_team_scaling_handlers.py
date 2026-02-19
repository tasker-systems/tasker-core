"""DSL mirror of blog post_04_team_scaling handlers.

Customer Success namespace (5 handlers):
    validate_refund_request -> check_refund_policy -> get_manager_approval
        -> execute_refund_workflow -> update_ticket_status

Payments namespace (4 handlers):
    validate_payment_eligibility -> process_gateway_refund
        -> update_payment_records -> notify_customer
"""

from __future__ import annotations

import re
import uuid
from datetime import datetime, timedelta, timezone

from tasker_core.step_handler.functional import depends_on, inputs, step_handler
from tasker_core.types import StepHandlerResult

# Policy rules by customer tier
REFUND_POLICIES = {
    "standard": {"window_days": 30, "requires_approval": True, "max_amount": 10_000},
    "gold": {"window_days": 60, "requires_approval": False, "max_amount": 50_000},
    "premium": {"window_days": 90, "requires_approval": False, "max_amount": 100_000},
}


# ============================================================================
# Customer Success namespace
# ============================================================================


@step_handler("team_scaling_dsl.customer_success.step_handlers.validate_refund_request")
@inputs("ticket_id", "customer_id", "refund_amount", "refund_reason")
def cs_validate_refund_request(ticket_id, customer_id, refund_amount, refund_reason, context):
    """Validate customer refund request details."""
    missing_fields = []
    if not ticket_id:
        missing_fields.append("ticket_id")
    if not customer_id:
        missing_fields.append("customer_id")
    if not refund_amount:
        missing_fields.append("refund_amount")

    if missing_fields:
        return StepHandlerResult.failure(
            message=f"Missing required fields for refund validation: {', '.join(missing_fields)}",
            error_type="MISSING_REQUIRED_FIELDS",
            retryable=False,
        )

    # Simulate validation
    if "ticket_closed" in ticket_id:
        return StepHandlerResult.failure(message="Cannot process refund for closed ticket", error_type="TICKET_CLOSED", retryable=False)
    if "ticket_cancelled" in ticket_id:
        return StepHandlerResult.failure(message="Cannot process refund for cancelled ticket", error_type="TICKET_CANCELLED", retryable=False)

    # Determine customer tier
    if "vip" in customer_id.lower() or "premium" in customer_id.lower():
        customer_tier = "premium"
    elif "gold" in customer_id.lower():
        customer_tier = "gold"
    else:
        customer_tier = "standard"

    purchase_date = (datetime.now(timezone.utc) - timedelta(days=30)).isoformat()

    return {
        "request_validated": True,
        "ticket_id": ticket_id,
        "customer_id": customer_id,
        "ticket_status": "open",
        "customer_tier": customer_tier,
        "original_purchase_date": purchase_date,
        "payment_id": f"pay_{uuid.uuid4().hex[:12]}",
        "validation_timestamp": datetime.now(timezone.utc).isoformat(),
        "namespace": "customer_success",
    }


@step_handler("team_scaling_dsl.customer_success.step_handlers.check_refund_policy")
@depends_on(validation_result="validate_refund_request")
@inputs("refund_amount", "refund_reason")
def cs_check_refund_policy(validation_result, refund_amount, refund_reason, context):
    """Check if refund request complies with policy rules."""
    if not validation_result or not validation_result.get("request_validated"):
        return StepHandlerResult.failure(message="Request validation must be completed before policy check", error_type="MISSING_VALIDATION", retryable=False)

    customer_tier = validation_result.get("customer_tier", "standard")
    purchase_date_str = validation_result.get("original_purchase_date")
    policy = REFUND_POLICIES.get(customer_tier, REFUND_POLICIES["standard"])

    purchase_date = datetime.fromisoformat(purchase_date_str.replace("Z", "+00:00"))
    now = datetime.now(timezone.utc)
    days_since_purchase = (now - purchase_date).days

    within_window = days_since_purchase <= policy["window_days"]
    within_amount_limit = refund_amount <= policy["max_amount"]

    if not within_window:
        return StepHandlerResult.failure(
            message=f"Refund request outside policy window: {days_since_purchase} days (max: {policy['window_days']} days)",
            error_type="OUTSIDE_REFUND_WINDOW",
            retryable=False,
        )

    if not within_amount_limit:
        return StepHandlerResult.failure(
            message=f"Refund amount exceeds policy limit: ${refund_amount / 100:.2f} (max: ${policy['max_amount'] / 100:.2f})",
            error_type="EXCEEDS_AMOUNT_LIMIT",
            retryable=False,
        )

    return {
        "policy_checked": True,
        "policy_compliant": True,
        "customer_tier": customer_tier,
        "refund_window_days": policy["window_days"],
        "days_since_purchase": days_since_purchase,
        "within_refund_window": within_window,
        "requires_approval": policy["requires_approval"],
        "max_allowed_amount": policy["max_amount"],
        "policy_checked_at": now.isoformat(),
        "namespace": "customer_success",
    }


@step_handler("team_scaling_dsl.customer_success.step_handlers.get_manager_approval")
@depends_on(policy_result="check_refund_policy", validation_result="validate_refund_request")
@inputs("refund_amount", "refund_reason")
def cs_get_manager_approval(policy_result, validation_result, refund_amount, refund_reason, context):
    """Get manager approval for refund if required."""
    if not policy_result or not policy_result.get("policy_checked"):
        return StepHandlerResult.failure(message="Policy check must be completed before approval", error_type="MISSING_POLICY_CHECK", retryable=False)

    requires_approval = policy_result.get("requires_approval")
    customer_tier = policy_result.get("customer_tier")
    now = datetime.now(timezone.utc).isoformat()

    if requires_approval:
        ticket_id = validation_result.get("ticket_id", "") if validation_result else ""
        customer_id = validation_result.get("customer_id", "") if validation_result else ""

        if "ticket_denied" in ticket_id:
            return StepHandlerResult.failure(message="Manager denied refund request: Manager denied refund request", error_type="APPROVAL_DENIED", retryable=False)

        approval_id = f"appr_{uuid.uuid4().hex[:16]}"
        manager_id = f"mgr_{(hash(ticket_id) % 5) + 1}"

        return {
            "approval_obtained": True,
            "approval_required": True,
            "auto_approved": False,
            "approval_id": approval_id,
            "manager_id": manager_id,
            "manager_notes": f"Approved refund request for customer {customer_id}",
            "approved_at": now,
            "namespace": "customer_success",
        }
    else:
        return {
            "approval_obtained": True,
            "approval_required": False,
            "auto_approved": True,
            "approval_id": None,
            "manager_id": None,
            "manager_notes": f"Auto-approved for customer tier {customer_tier}",
            "approved_at": now,
            "namespace": "customer_success",
        }


@step_handler("team_scaling_dsl.customer_success.step_handlers.execute_refund_workflow")
@depends_on(approval_result="get_manager_approval", validation_result="validate_refund_request")
@inputs("refund_amount", "refund_reason", "customer_email", "ticket_id", "correlation_id")
def cs_execute_refund_workflow(approval_result, validation_result, refund_amount, refund_reason, customer_email, ticket_id, correlation_id, context):
    """Execute cross-namespace refund workflow delegation."""
    if not approval_result or not approval_result.get("approval_obtained"):
        return StepHandlerResult.failure(message="Manager approval must be obtained before executing refund", error_type="MISSING_APPROVAL", retryable=False)

    payment_id = validation_result.get("payment_id") if validation_result else None
    if not payment_id:
        return StepHandlerResult.failure(message="Payment ID not found in validation results", error_type="MISSING_PAYMENT_ID", retryable=False)

    approval_id = approval_result.get("approval_id")
    if refund_reason is None:
        refund_reason = "customer_request"
    if customer_email is None:
        customer_email = "customer@example.com"
    if correlation_id is None:
        correlation_id = f"cs-{uuid.uuid4().hex[:16]}"

    task_id = f"task_{uuid.uuid4()}"
    now = datetime.now(timezone.utc).isoformat()

    return {
        "task_delegated": True,
        "target_namespace": "payments",
        "target_workflow": "process_refund",
        "delegated_task_id": task_id,
        "delegated_task_status": "created",
        "delegation_timestamp": now,
        "correlation_id": correlation_id,
        "namespace": "customer_success",
    }


@step_handler("team_scaling_dsl.customer_success.step_handlers.update_ticket_status")
@depends_on(delegation_result="execute_refund_workflow", validation_result="validate_refund_request")
@inputs("refund_amount", "refund_reason")
def cs_update_ticket_status(delegation_result, validation_result, refund_amount, refund_reason, context):
    """Update customer support ticket status."""
    if not delegation_result or not delegation_result.get("task_delegated"):
        return StepHandlerResult.failure(message="Refund workflow must be executed before updating ticket", error_type="MISSING_DELEGATION", retryable=False)

    ticket_id = validation_result.get("ticket_id") if validation_result else None
    delegated_task_id = delegation_result.get("delegated_task_id")
    correlation_id = delegation_result.get("correlation_id")
    now = datetime.now(timezone.utc).isoformat()

    if ticket_id and "ticket_locked" in ticket_id:
        return StepHandlerResult.failure(message="Ticket locked by another agent, will retry", error_type="TICKET_LOCKED", retryable=True)

    resolution_note = (
        f"Refund of ${refund_amount / 100:.2f} processed successfully. "
        f"Delegated task ID: {delegated_task_id}. "
        f"Correlation ID: {correlation_id}"
    )

    return {
        "ticket_updated": True,
        "ticket_id": ticket_id,
        "previous_status": "in_progress",
        "new_status": "resolved",
        "resolution_note": resolution_note,
        "updated_at": now,
        "refund_completed": True,
        "delegated_task_id": delegated_task_id,
        "namespace": "customer_success",
    }


# ============================================================================
# Payments namespace
# ============================================================================


@step_handler("team_scaling_dsl.payments.step_handlers.validate_payment_eligibility")
@inputs("payment_id", "refund_amount", "refund_reason", "partial_refund")
def pay_validate_payment_eligibility(payment_id, refund_amount, refund_reason, partial_refund, context):
    """Validate payment eligibility for refund."""
    if partial_refund is None:
        partial_refund = False

    missing_fields = []
    if not payment_id:
        missing_fields.append("payment_id")
    if not refund_amount:
        missing_fields.append("refund_amount")

    if missing_fields:
        return StepHandlerResult.failure(message=f"Missing required fields for payment validation: {', '.join(missing_fields)}", error_type="MISSING_REQUIRED_FIELDS", retryable=False)

    if refund_amount <= 0:
        return StepHandlerResult.failure(message=f"Refund amount must be positive, got: {refund_amount}", error_type="INVALID_REFUND_AMOUNT", retryable=False)

    if not re.match(r"^pay_[a-zA-Z0-9_]+$", payment_id):
        return StepHandlerResult.failure(message=f"Invalid payment ID format: {payment_id}", error_type="INVALID_PAYMENT_ID", retryable=False)

    if "pay_test_insufficient" in payment_id:
        return StepHandlerResult.failure(message="Insufficient funds available for refund", error_type="INSUFFICIENT_FUNDS", retryable=False)
    if "pay_test_ineligible" in payment_id:
        return StepHandlerResult.failure(message="Payment is not eligible for refund: Payment is past refund window", error_type="PAYMENT_INELIGIBLE", retryable=False)

    return {
        "payment_validated": True,
        "payment_id": payment_id,
        "original_amount": refund_amount + 1000,
        "refund_amount": refund_amount,
        "payment_method": "credit_card",
        "gateway_provider": "MockPaymentGateway",
        "eligibility_status": "eligible",
        "validation_timestamp": datetime.now(timezone.utc).isoformat(),
        "namespace": "payments",
    }


@step_handler("team_scaling_dsl.payments.step_handlers.process_gateway_refund")
@depends_on(validation_result="validate_payment_eligibility")
@inputs("refund_reason", "partial_refund")
def pay_process_gateway_refund(validation_result, refund_reason, partial_refund, context):
    """Process refund through payment gateway."""
    if not validation_result or not validation_result.get("payment_validated"):
        return StepHandlerResult.failure(message="Payment validation must be completed before processing refund", error_type="MISSING_VALIDATION", retryable=False)

    payment_id = validation_result.get("payment_id")
    refund_amount = validation_result.get("refund_amount")
    if refund_reason is None:
        refund_reason = "customer_request"

    if payment_id and "pay_test_gateway_timeout" in payment_id:
        return StepHandlerResult.failure(message="Gateway timeout, will retry", error_type="GATEWAY_TIMEOUT", retryable=True)
    if payment_id and "pay_test_gateway_error" in payment_id:
        return StepHandlerResult.failure(message="Gateway refund failed: Gateway error", error_type="GATEWAY_REFUND_FAILED", retryable=False)

    now = datetime.now(timezone.utc)

    return {
        "refund_processed": True,
        "refund_id": f"rfnd_{uuid.uuid4().hex[:24]}",
        "payment_id": payment_id,
        "refund_amount": refund_amount,
        "refund_status": "processed",
        "gateway_transaction_id": f"gtx_{uuid.uuid4().hex[:20]}",
        "gateway_provider": "MockPaymentGateway",
        "processed_at": now.isoformat(),
        "estimated_arrival": (now + timedelta(days=5)).isoformat(),
        "namespace": "payments",
    }


@step_handler("team_scaling_dsl.payments.step_handlers.update_payment_records")
@depends_on(refund_result="process_gateway_refund", validation_result="validate_payment_eligibility")
@inputs("refund_reason")
def pay_update_payment_records(refund_result, validation_result, refund_reason, context):
    """Update internal payment records after refund."""
    if not refund_result or not refund_result.get("refund_processed"):
        return StepHandlerResult.failure(message="Gateway refund must be completed before updating records", error_type="MISSING_REFUND", retryable=False)

    payment_id = refund_result.get("payment_id")
    refund_id = refund_result.get("refund_id")
    if refund_reason is None:
        refund_reason = "customer_request"

    if payment_id and "pay_test_record_lock" in payment_id:
        return StepHandlerResult.failure(message="Payment record locked, will retry", error_type="RECORD_LOCKED", retryable=True)

    now = datetime.now(timezone.utc).isoformat()

    return {
        "records_updated": True,
        "payment_id": payment_id,
        "refund_id": refund_id,
        "record_id": f"rec_{uuid.uuid4().hex[:16]}",
        "payment_status": "refunded",
        "refund_status": "completed",
        "history_entries_created": 2,
        "updated_at": now,
        "namespace": "payments",
    }


@step_handler("team_scaling_dsl.payments.step_handlers.notify_customer")
@depends_on(refund_result="process_gateway_refund")
@inputs("customer_email", "refund_reason")
def pay_notify_customer(refund_result, customer_email, refund_reason, context):
    """Send refund confirmation notification to customer."""
    if not refund_result or not refund_result.get("refund_processed"):
        return StepHandlerResult.failure(message="Refund must be processed before sending notification", error_type="MISSING_REFUND", retryable=False)

    if not customer_email:
        return StepHandlerResult.failure(message="Customer email is required for notification", error_type="MISSING_CUSTOMER_EMAIL", retryable=False)

    if not re.match(r"^[^@\s]+@[^@\s]+$", customer_email):
        return StepHandlerResult.failure(message=f"Invalid customer email format: {customer_email}", error_type="INVALID_EMAIL_FORMAT", retryable=False)

    refund_id = refund_result.get("refund_id")
    refund_amount = refund_result.get("refund_amount")
    now = datetime.now(timezone.utc).isoformat()

    return {
        "notification_sent": True,
        "customer_email": customer_email,
        "message_id": f"msg_{uuid.uuid4().hex[:24]}",
        "notification_type": "refund_confirmation",
        "sent_at": now,
        "delivery_status": "delivered",
        "refund_id": refund_id,
        "refund_amount": refund_amount,
        "namespace": "payments",
    }


__all__ = [
    # Customer Success
    "cs_validate_refund_request",
    "cs_check_refund_policy",
    "cs_get_manager_approval",
    "cs_execute_refund_workflow",
    "cs_update_ticket_status",
    # Payments
    "pay_validate_payment_eligibility",
    "pay_process_gateway_refund",
    "pay_update_payment_records",
    "pay_notify_customer",
]
