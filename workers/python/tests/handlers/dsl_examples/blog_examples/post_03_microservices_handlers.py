"""DSL mirror of blog post_03_microservices handlers.

User Registration workflow:
1. create_user_account -> 2. setup_billing_profile + 3. initialize_preferences (parallel)
    -> 4. send_welcome_sequence -> 5. update_user_status
"""

from __future__ import annotations

import re
import uuid
from datetime import datetime, timedelta, timezone

from tasker_core.step_handler.functional import depends_on, inputs, step_handler
from tasker_core.types import StepHandlerResult

# Simulated existing users
EXISTING_USERS = {
    "existing@example.com": {
        "id": "user_existing_001",
        "email": "existing@example.com",
        "name": "Existing User",
        "plan": "free",
        "created_at": "2025-01-01T00:00:00Z",
    }
}

BILLING_TIERS = {
    "free": {"price": 0, "features": ["basic_features"], "billing_required": False},
    "pro": {
        "price": 29.99,
        "features": ["basic_features", "advanced_analytics"],
        "billing_required": True,
    },
    "enterprise": {
        "price": 299.99,
        "features": [
            "basic_features",
            "advanced_analytics",
            "priority_support",
            "custom_integrations",
        ],
        "billing_required": True,
    },
}

DEFAULT_PREFERENCES = {
    "free": {
        "email_notifications": True,
        "marketing_emails": False,
        "product_updates": True,
        "weekly_digest": False,
        "theme": "light",
        "language": "en",
        "timezone": "UTC",
    },
    "pro": {
        "email_notifications": True,
        "marketing_emails": True,
        "product_updates": True,
        "weekly_digest": True,
        "theme": "dark",
        "language": "en",
        "timezone": "UTC",
        "api_notifications": True,
    },
    "enterprise": {
        "email_notifications": True,
        "marketing_emails": True,
        "product_updates": True,
        "weekly_digest": True,
        "theme": "dark",
        "language": "en",
        "timezone": "UTC",
        "api_notifications": True,
        "audit_logs": True,
        "advanced_reports": True,
    },
}


@step_handler("microservices_dsl_py.step_handlers.create_user_account")
@inputs("user_info")
def create_user_account(user_info, _context):
    """Create user account with idempotency handling."""
    if user_info is None:
        user_info = {}

    email = user_info.get("email")
    name = user_info.get("name")

    if not email:
        return StepHandlerResult.failure(
            message="Email is required but was not provided",
            error_type="MISSING_EMAIL",
            retryable=False,
        )
    if not name:
        return StepHandlerResult.failure(
            message="Name is required but was not provided",
            error_type="MISSING_NAME",
            retryable=False,
        )

    email_pattern = r"^[\w+\-.]+@[a-z\d\-]+(\.[a-z\d\-]+)*\.[a-z]+$"
    if not re.match(email_pattern, email, re.IGNORECASE):
        return StepHandlerResult.failure(
            message=f"Invalid email format: {email}",
            error_type="INVALID_EMAIL_FORMAT",
            retryable=False,
        )

    plan = user_info.get("plan", "free")
    phone = user_info.get("phone")
    source = user_info.get("source", "web")

    if email in EXISTING_USERS:
        existing_user = EXISTING_USERS[email]
        if (
            existing_user["email"] == email
            and existing_user["name"] == name
            and existing_user["plan"] == plan
        ):
            return {
                "user_id": existing_user["id"],
                "email": existing_user["email"],
                "plan": existing_user["plan"],
                "status": "already_exists",
                "created_at": existing_user["created_at"],
            }
        else:
            return StepHandlerResult.failure(
                message=f"User with email {email} already exists with different data",
                error_type="USER_CONFLICT",
                retryable=False,
            )

    now = datetime.now(timezone.utc).isoformat()
    user_id = f"user_{uuid.uuid4().hex[:12]}"

    return {
        "user_id": user_id,
        "email": email,
        "name": name,
        "plan": plan,
        "phone": phone,
        "source": source,
        "status": "created",
        "created_at": now,
    }


@step_handler("microservices_dsl_py.step_handlers.setup_billing_profile")
@depends_on(user_data="create_user_account_dsl_py")
def setup_billing_profile(user_data, _context):
    """Setup billing profile with graceful degradation for free plans."""
    if not user_data:
        return StepHandlerResult.failure(
            message="User data not found from create_user_account step",
            error_type="MISSING_USER_DATA",
            retryable=False,
        )

    user_id = user_data.get("user_id")
    plan = user_data.get("plan", "free")
    tier_config = BILLING_TIERS.get(plan, BILLING_TIERS["free"])

    if tier_config["billing_required"]:
        now = datetime.now(timezone.utc)
        next_billing = (now + timedelta(days=30)).isoformat()
        billing_id = f"billing_{uuid.uuid4().hex[:12]}"

        return {
            "billing_id": billing_id,
            "user_id": user_id,
            "plan": plan,
            "price": tier_config["price"],
            "currency": "USD",
            "billing_cycle": "monthly",
            "features": tier_config["features"],
            "status": "active",
            "next_billing_date": next_billing,
            "created_at": now.isoformat(),
        }
    else:
        return {
            "user_id": user_id,
            "plan": plan,
            "billing_required": False,
            "status": "skipped_free_plan",
            "message": "Free plan users do not require billing setup",
        }


@step_handler("microservices_dsl_py.step_handlers.initialize_preferences")
@depends_on(user_data="create_user_account_dsl_py")
@inputs("user_info")
def initialize_preferences(user_data, user_info, _context):
    """Initialize user preferences with plan-based defaults."""
    if not user_data:
        return StepHandlerResult.failure(
            message="User data not found from create_user_account step",
            error_type="MISSING_USER_DATA",
            retryable=False,
        )

    user_id = user_data.get("user_id")
    plan = user_data.get("plan", "free")

    if user_info is None:
        user_info = {}
    custom_prefs = user_info.get("preferences", {})

    default_prefs = DEFAULT_PREFERENCES.get(plan, DEFAULT_PREFERENCES["free"])
    final_prefs = {**default_prefs, **custom_prefs}

    now = datetime.now(timezone.utc).isoformat()
    preferences_id = f"prefs_{uuid.uuid4().hex[:12]}"

    return {
        "preferences_id": preferences_id,
        "user_id": user_id,
        "plan": plan,
        "preferences": final_prefs,
        "defaults_applied": len(default_prefs),
        "customizations": len(custom_prefs),
        "status": "active",
        "created_at": now,
        "updated_at": now,
    }


@step_handler("microservices_dsl_py.step_handlers.send_welcome_sequence")
@depends_on(
    user_data="create_user_account_dsl_py",
    billing_data="setup_billing_profile_dsl_py",
    preferences_data="initialize_preferences_dsl_py",
)
def send_welcome_sequence(user_data, billing_data, preferences_data, _context):
    """Send welcome sequence via multiple notification channels."""
    missing = []
    if not user_data:
        missing.append("create_user_account")
    if not billing_data:
        missing.append("setup_billing_profile")
    if not preferences_data:
        missing.append("initialize_preferences")

    if missing:
        return StepHandlerResult.failure(
            message=f"Missing results from steps: {', '.join(missing)}",
            error_type="MISSING_DEPENDENCY_DATA",
            retryable=False,
        )

    user_id = user_data.get("user_id")
    plan = user_data.get("plan", "free")
    prefs = preferences_data.get("preferences", {})

    channels_used = []
    messages_sent_count = 0
    now = datetime.now(timezone.utc).isoformat()

    if prefs.get("email_notifications", True):
        channels_used.append("email")
        messages_sent_count += 1

    channels_used.append("in_app")
    messages_sent_count += 1

    if plan == "enterprise":
        channels_used.append("sms")
        messages_sent_count += 1

    welcome_sequence_id = f"welcome_{uuid.uuid4().hex[:12]}"

    return {
        "user_id": user_id,
        "plan": plan,
        "channels_used": channels_used,
        "messages_sent": messages_sent_count,
        "welcome_sequence_id": welcome_sequence_id,
        "status": "sent",
        "sent_at": now,
    }


@step_handler("microservices_dsl_py.step_handlers.update_user_status")
@depends_on(
    user_data="create_user_account_dsl_py",
    billing_data="setup_billing_profile_dsl_py",
    preferences_data="initialize_preferences_dsl_py",
    welcome_data="send_welcome_sequence_dsl_py",
)
def update_user_status(user_data, billing_data, preferences_data, welcome_data, _context):
    """Update user status to active after workflow completion."""
    missing = []
    if not user_data:
        missing.append("create_user_account")
    if not billing_data:
        missing.append("setup_billing_profile")
    if not preferences_data:
        missing.append("initialize_preferences")
    if not welcome_data:
        missing.append("send_welcome_sequence")

    if missing:
        return StepHandlerResult.failure(
            message=f"Cannot complete registration: missing results from steps: {', '.join(missing)}",
            error_type="INCOMPLETE_WORKFLOW",
            retryable=False,
        )

    user_id = user_data.get("user_id")
    plan = user_data.get("plan", "free")
    email = user_data.get("email")
    now = datetime.now(timezone.utc).isoformat()

    # Build registration summary
    summary: dict = {
        "user_id": user_id,
        "email": email,
        "plan": plan,
        "registration_status": "complete",
    }

    if plan != "free" and billing_data.get("billing_id"):
        summary["billing_id"] = billing_data.get("billing_id")
        summary["next_billing_date"] = billing_data.get("next_billing_date")

    prefs = preferences_data.get("preferences", {})
    summary["preferences_count"] = len(prefs) if isinstance(prefs, dict) else 0
    summary["welcome_sent"] = True
    summary["notification_channels"] = welcome_data.get("channels_used", [])
    summary["user_created_at"] = user_data.get("created_at")
    summary["registration_completed_at"] = now

    return {
        "user_id": user_id,
        "status": "active",
        "plan": plan,
        "registration_summary": summary,
        "activation_timestamp": now,
        "all_services_coordinated": True,
        "services_completed": [
            "user_service",
            "billing_service",
            "preferences_service",
            "notification_service",
        ],
    }


__all__ = [
    "create_user_account",
    "setup_billing_profile",
    "initialize_preferences",
    "send_welcome_sequence",
    "update_user_status",
]
