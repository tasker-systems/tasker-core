"""DSL mirror of domain_event_handlers using @step_handler.

Domain event publishing is handled by the worker's post-execution callback
system based on YAML declarations, not by the handler itself. The handlers
just return business result data.

Note: Some fields (order_id, transaction_id, notification_id, timestamps)
are generated dynamically, so parity tests compare structural keys and
deterministic fields only.
"""

from __future__ import annotations

import uuid
from datetime import datetime, timezone

from tasker_core.step_handler.functional import inputs, step_handler


@step_handler("domain_events_dsl_py.step_handlers.validate_order")
@inputs("order_id", "customer_id", "amount")
def validate_order(order_id, customer_id, amount, context):
    """Validate the order."""
    if order_id is None:
        order_id = str(uuid.uuid4())
    if customer_id is None:
        customer_id = "unknown"
    if amount is None:
        amount = 0

    step_config = context.step_config or {}
    validation_mode = step_config.get("validation_mode", "standard")

    validation_checks = ["order_id_present", "customer_id_present"]

    if validation_mode == "strict" and amount <= 0:
        from tasker_core.types import StepHandlerResult

        return StepHandlerResult.failure(
            message="Amount must be positive in strict mode",
            error_type="ValidationError",
            retryable=False,
            metadata={"validation_mode": validation_mode},
        )

    if amount > 0:
        validation_checks.append("amount_positive")

    return {
        "order_id": order_id,
        "validation_timestamp": datetime.now(timezone.utc).isoformat(),
        "validation_checks": validation_checks,
        "validated": True,
    }


@step_handler("domain_events_dsl_py.step_handlers.process_payment")
@inputs("order_id", "amount", "simulate_failure")
def process_payment(order_id, amount, simulate_failure, context):
    """Process the payment."""
    if order_id is None:
        order_id = "unknown"
    if amount is None:
        amount = 0
    if simulate_failure is None:
        simulate_failure = False

    if simulate_failure:
        from tasker_core.types import StepHandlerResult

        return StepHandlerResult.failure(
            message="Simulated payment failure",
            error_type="PaymentError",
            retryable=True,
            metadata={
                "order_id": order_id,
                "failed_at": datetime.now(timezone.utc).isoformat(),
            },
        )

    transaction_id = f"TXN-{uuid.uuid4()}"

    return {
        "transaction_id": transaction_id,
        "amount": amount,
        "payment_method": "credit_card",
        "processed_at": datetime.now(timezone.utc).isoformat(),
        "status": "success",
    }


@step_handler("domain_events_dsl_py.step_handlers.update_inventory")
@inputs("order_id")
def update_inventory(order_id, context):
    """Update inventory for the order."""
    if order_id is None:
        order_id = "unknown"

    items = [
        {"sku": "ITEM-001", "quantity": 1},
        {"sku": "ITEM-002", "quantity": 2},
    ]

    return {
        "order_id": order_id,
        "items": items,
        "success": True,
        "updated_at": datetime.now(timezone.utc).isoformat(),
    }


@step_handler("domain_events_dsl_py.step_handlers.send_notification")
@inputs("customer_id")
def send_notification(customer_id, context):
    """Send customer notification."""
    if customer_id is None:
        customer_id = "unknown"

    step_config = context.step_config or {}
    notification_type = step_config.get("notification_type", "email")
    notification_id = f"NOTIF-{uuid.uuid4()}"

    return {
        "notification_id": notification_id,
        "channel": notification_type,
        "recipient": customer_id,
        "sent_at": datetime.now(timezone.utc).isoformat(),
        "status": "delivered",
    }


__all__ = [
    "validate_order",
    "process_payment",
    "update_inventory",
    "send_notification",
]
