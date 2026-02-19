"""DSL mirror of blog post_01_ecommerce handlers.

E-commerce checkout: validate_cart -> process_payment -> update_inventory ->
    create_order -> send_confirmation

These mirror the DETERMINISTIC outputs only. Fields with random IDs, timestamps,
and simulated external service responses use the same logic but won't be
byte-identical (parity tests compare structure and deterministic fields).
"""

from __future__ import annotations

import logging
import random
import re
import secrets
from datetime import datetime, timedelta, timezone
from typing import Any

from tasker_core.errors import PermanentError, RetryableError
from tasker_core.step_handler.functional import depends_on, inputs, step_handler

logger = logging.getLogger(__name__)

# Mock product database (same as verbose version)
PRODUCTS: dict[int, dict[str, Any]] = {
    1: {"id": 1, "name": "Widget A", "price": 29.99, "stock": 100, "active": True},
    2: {"id": 2, "name": "Widget B", "price": 49.99, "stock": 50, "active": True},
    3: {"id": 3, "name": "Widget C", "price": 19.99, "stock": 25, "active": True},
    4: {"id": 4, "name": "Widget D", "price": 39.99, "stock": 0, "active": True},
    5: {"id": 5, "name": "Widget E", "price": 59.99, "stock": 10, "active": False},
}


@step_handler("ecommerce_dsl.step_handlers.validate_cart")
@inputs("cart_items")
def validate_cart(cart_items, context):
    """Validate cart and calculate totals."""
    if not cart_items:
        raise PermanentError(
            message="Cart items are required but were not provided",
            error_code="MISSING_CART_ITEMS",
        )

    # Validate structure
    for index, item in enumerate(cart_items):
        if not item.get("product_id"):
            raise PermanentError(
                message=f"Product ID is required for cart item {index + 1}",
                error_code="MISSING_PRODUCT_ID",
            )
        quantity = item.get("quantity")
        if not quantity or quantity <= 0:
            raise PermanentError(
                message=f"Valid quantity is required for cart item {index + 1}",
                error_code="INVALID_QUANTITY",
            )

    # Validate each item
    validated_items = []
    for item in cart_items:
        product_id = item["product_id"]
        quantity = item["quantity"]
        product = PRODUCTS.get(product_id)

        if not product:
            raise PermanentError(
                message=f"Product {product_id} not found",
                error_code="PRODUCT_NOT_FOUND",
            )

        if not product["active"]:
            raise PermanentError(
                message=f"Product {product['name']} is no longer available",
                error_code="PRODUCT_INACTIVE",
            )

        if product["stock"] < quantity:
            raise RetryableError(
                message=(
                    f"Insufficient stock for {product['name']}. "
                    f"Available: {product['stock']}, Requested: {quantity}"
                ),
                retry_after=30,
            )

        validated_items.append(
            {
                "product_id": product["id"],
                "name": product["name"],
                "price": product["price"],
                "quantity": quantity,
                "line_total": round(product["price"] * quantity, 2),
            }
        )

    subtotal = sum(item["line_total"] for item in validated_items)
    tax_rate = 0.08
    tax = round(subtotal * tax_rate, 2)

    total_weight = sum(item["quantity"] * 0.5 for item in validated_items)
    if total_weight <= 2:
        shipping = 5.99
    elif total_weight <= 10:
        shipping = 9.99
    else:
        shipping = 14.99

    total = subtotal + tax + shipping

    return {
        "validated_items": validated_items,
        "subtotal": subtotal,
        "tax": tax,
        "shipping": shipping,
        "total": total,
        "item_count": len(validated_items),
        "validated_at": datetime.now(timezone.utc).isoformat(),
    }


@step_handler("ecommerce_dsl.step_handlers.process_payment")
@inputs("payment_info")
@depends_on(cart_validation="validate_cart")
def process_payment(payment_info, cart_validation, context):
    """Process the payment."""
    amount_to_charge = cart_validation.get("total") if cart_validation else None

    if not payment_info:
        raise PermanentError(
            message="Payment information is required but was not provided",
            error_code="MISSING_PAYMENT_INFO",
        )
    if not payment_info.get("method"):
        raise PermanentError(
            message="Payment method is required but was not provided",
            error_code="MISSING_PAYMENT_METHOD",
        )
    if not payment_info.get("token"):
        raise PermanentError(
            message="Payment token is required but was not provided",
            error_code="MISSING_PAYMENT_TOKEN",
        )
    if amount_to_charge is None:
        raise PermanentError(
            message="Cart total is required but was not found from validate_cart step",
            error_code="MISSING_CART_TOTAL",
        )

    provided_amount = payment_info.get("amount", 0)
    if abs(float(provided_amount) - float(amount_to_charge)) > 0.01:
        raise PermanentError(
            message=(
                f"Payment amount mismatch. "
                f"Expected: ${amount_to_charge}, Provided: ${provided_amount}"
            ),
            error_code="PAYMENT_AMOUNT_MISMATCH",
        )

    token = payment_info["token"]
    if token == "tok_test_declined":
        raise PermanentError(message="Payment declined: Card was declined by issuer", error_code="PAYMENT_DECLINED")
    if token == "tok_test_insufficient_funds":
        raise PermanentError(message="Payment declined: Insufficient funds", error_code="PAYMENT_DECLINED")
    if token == "tok_test_network_error":
        raise RetryableError(message="Payment service temporarily unavailable", retry_after=15)

    payment_id = f"pay_{secrets.token_hex(12)}"
    transaction_id = f"txn_{secrets.token_hex(12)}"

    return {
        "payment_id": payment_id,
        "amount_charged": amount_to_charge,
        "currency": "USD",
        "payment_method_type": payment_info["method"],
        "transaction_id": transaction_id,
        "processed_at": datetime.now(timezone.utc).isoformat(),
        "status": "completed",
    }


@step_handler("ecommerce_dsl.step_handlers.update_inventory")
@inputs("customer_info")
@depends_on(cart_validation="validate_cart")
def ecommerce_update_inventory(customer_info, cart_validation, context):
    """Update inventory for cart items."""
    if not cart_validation or not cart_validation.get("validated_items"):
        raise PermanentError(
            message="Validated cart items are required but were not found from validate_cart step",
            error_code="MISSING_VALIDATED_ITEMS",
        )
    if not customer_info:
        raise PermanentError(
            message="Customer information is required but was not provided",
            error_code="MISSING_CUSTOMER_INFO",
        )

    validated_items = cart_validation.get("validated_items", [])
    updated_products = []
    inventory_changes = []

    for item in validated_items:
        product_id = item["product_id"]
        quantity = item["quantity"]
        product = PRODUCTS.get(product_id)

        if not product:
            raise PermanentError(message=f"Product {product_id} not found", error_code="PRODUCT_NOT_FOUND")

        stock_level = product["stock"]
        if stock_level < quantity:
            raise RetryableError(
                message=f"Stock not available for {product['name']}. Available: {stock_level}, Needed: {quantity}",
                retry_after=30,
            )

        reservation_id = f"rsv_{secrets.token_hex(8)}"
        updated_products.append(
            {
                "product_id": product["id"],
                "name": product["name"],
                "previous_stock": stock_level,
                "new_stock": stock_level - quantity,
                "quantity_reserved": quantity,
                "reservation_id": reservation_id,
            }
        )
        inventory_changes.append(
            {
                "product_id": product["id"],
                "change_type": "reservation",
                "quantity": -quantity,
                "reason": "order_checkout",
                "timestamp": datetime.now(timezone.utc).isoformat(),
                "reservation_id": reservation_id,
                "inventory_log_id": f"log_{secrets.token_hex(6)}",
            }
        )

    total_items_reserved = sum(p["quantity_reserved"] for p in updated_products)

    return {
        "updated_products": updated_products,
        "total_items_reserved": total_items_reserved,
        "inventory_changes": inventory_changes,
        "inventory_log_id": f"log_{secrets.token_hex(8)}",
        "updated_at": datetime.now(timezone.utc).isoformat(),
    }


@step_handler("ecommerce_dsl.step_handlers.create_order")
@inputs("customer_info")
@depends_on(
    cart_validation="validate_cart",
    payment_result="process_payment",
    inventory_result="update_inventory",
)
def create_order(customer_info, cart_validation, payment_result, inventory_result, context):
    """Create the order record."""
    if not customer_info:
        raise PermanentError(message="Customer information is required but was not provided", error_code="MISSING_CUSTOMER_INFO")
    if not cart_validation or not cart_validation.get("validated_items"):
        raise PermanentError(message="Cart validation results are required but were not found from validate_cart step", error_code="MISSING_CART_VALIDATION")
    if not payment_result or not payment_result.get("payment_id"):
        raise PermanentError(message="Payment results are required but were not found from process_payment step", error_code="MISSING_PAYMENT_RESULT")
    if not inventory_result or not inventory_result.get("updated_products"):
        raise PermanentError(message="Inventory results are required but were not found from update_inventory step", error_code="MISSING_INVENTORY_RESULT")

    order_id = random.randint(1000, 9999)
    today = datetime.now(timezone.utc).strftime("%Y%m%d")
    suffix = secrets.token_hex(4).upper()
    order_number = f"ORD-{today}-{suffix}"
    delivery_date = datetime.now(timezone.utc) + timedelta(days=7)

    return {
        "order_id": order_id,
        "order_number": order_number,
        "status": "confirmed",
        "total_amount": cart_validation.get("total"),
        "customer_email": customer_info.get("email"),
        "created_at": datetime.now(timezone.utc).isoformat(),
        "estimated_delivery": delivery_date.strftime("%B %d, %Y"),
    }


@step_handler("ecommerce_dsl.step_handlers.send_confirmation")
@inputs("customer_info")
@depends_on(order_result="create_order", cart_validation="validate_cart")
def send_confirmation(customer_info, order_result, cart_validation, context):
    """Send the confirmation email."""
    if not customer_info or not customer_info.get("email"):
        raise PermanentError(message="Customer email is required but was not provided", error_code="MISSING_CUSTOMER_EMAIL")
    if not order_result or not order_result.get("order_id"):
        raise PermanentError(message="Order results are required but were not found from create_order step", error_code="MISSING_ORDER_RESULT")
    if not cart_validation or not cart_validation.get("validated_items"):
        raise PermanentError(message="Cart validation results are required but were not found from validate_cart step", error_code="MISSING_CART_VALIDATION")

    customer_email = customer_info.get("email")

    # Validate email format
    if not customer_email or not re.match(r"^[^@\s]+@[^@\s]+$", customer_email):
        raise PermanentError(message="Invalid email address provided", error_code="INVALID_EMAIL")

    return {
        "email_sent": True,
        "recipient": customer_email,
        "email_type": "order_confirmation",
        "sent_at": datetime.now(timezone.utc).isoformat(),
        "message_id": f"mock_{secrets.token_hex(8)}",
    }


__all__ = [
    "validate_cart",
    "process_payment",
    "ecommerce_update_inventory",
    "create_order",
    "send_confirmation",
]
