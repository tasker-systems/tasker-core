"""DSL example handlers (TAS-294 Phase 2).

These handlers mirror the verbose class-based handlers in tests/handlers/examples/
using the decorator-based functional API from tasker_core.step_handler.functional.

Every DSL handler produces identical output to its verbose counterpart for the same
input context. Parity is verified by tests/test_handler_parity.py.
"""

# Linear workflow (4 handlers)
# Batch processing (3 handlers - analyzer is decorator, worker+aggregator are class-based)
from .batch_processing_handlers import (
    CsvBatchProcessorDslHandler,
    CsvResultsAggregatorDslHandler,
    csv_analyzer,
)
from .blog_examples.post_01_ecommerce_handlers import (
    create_order as ecommerce_create_order,
)
from .blog_examples.post_01_ecommerce_handlers import (
    ecommerce_update_inventory,
)
from .blog_examples.post_01_ecommerce_handlers import (
    process_payment as ecommerce_process_payment,
)
from .blog_examples.post_01_ecommerce_handlers import (
    send_confirmation as ecommerce_send_confirmation,
)

# Blog examples - Post 01: E-commerce (5 handlers)
from .blog_examples.post_01_ecommerce_handlers import (
    validate_cart as ecommerce_validate_cart,
)
from .blog_examples.post_02_data_pipeline_handlers import (
    aggregate_metrics as data_pipeline_aggregate_metrics,
)
from .blog_examples.post_02_data_pipeline_handlers import (
    extract_customer_data as data_pipeline_extract_customers,
)
from .blog_examples.post_02_data_pipeline_handlers import (
    extract_inventory_data as data_pipeline_extract_inventory,
)

# Blog examples - Post 02: Data Pipeline (8 handlers)
from .blog_examples.post_02_data_pipeline_handlers import (
    extract_sales_data as data_pipeline_extract_sales,
)
from .blog_examples.post_02_data_pipeline_handlers import (
    generate_insights as data_pipeline_generate_insights,
)
from .blog_examples.post_02_data_pipeline_handlers import (
    transform_customers as data_pipeline_transform_customers,
)
from .blog_examples.post_02_data_pipeline_handlers import (
    transform_inventory as data_pipeline_transform_inventory,
)
from .blog_examples.post_02_data_pipeline_handlers import (
    transform_sales as data_pipeline_transform_sales,
)

# Blog examples - Post 03: Microservices (5 handlers)
from .blog_examples.post_03_microservices_handlers import (
    create_user_account as microservices_create_user_account,
)
from .blog_examples.post_03_microservices_handlers import (
    initialize_preferences as microservices_initialize_preferences,
)
from .blog_examples.post_03_microservices_handlers import (
    send_welcome_sequence as microservices_send_welcome_sequence,
)
from .blog_examples.post_03_microservices_handlers import (
    setup_billing_profile as microservices_setup_billing_profile,
)
from .blog_examples.post_03_microservices_handlers import (
    update_user_status as microservices_update_user_status,
)

# Blog examples - Post 04: Team Scaling (9 handlers)
from .blog_examples.post_04_team_scaling_handlers import (
    cs_check_refund_policy,
    cs_execute_refund_workflow,
    cs_get_manager_approval,
    cs_update_ticket_status,
    cs_validate_refund_request,
    pay_notify_customer,
    pay_process_gateway_refund,
    pay_update_payment_records,
    pay_validate_payment_eligibility,
)

# Checkpoint yield (3 handlers - analyzer is decorator, worker+aggregator are class-based)
from .checkpoint_yield_handlers import (
    CheckpointYieldAggregatorDslHandler,
    CheckpointYieldWorkerDslHandler,
    checkpoint_yield_analyzer,
)

# Conditional approval (6 handlers)
from .conditional_approval_handlers import (
    auto_approve,
    finalize_approval,
    finance_review,
    manager_approval,
    routing_decision,
    validate_request,
)

# Diamond workflow - unit test (4 handlers)
# Diamond workflow - E2E (4 handlers)
from .diamond_workflow_handlers import (
    diamond_branch_b,
    diamond_branch_c,
    diamond_end,
    diamond_init,
    diamond_merge,
    diamond_path_a,
    diamond_path_b,
    diamond_start,
)

# Domain events (4 handlers)
from .domain_event_handlers import (
    process_payment,
    send_notification,
    update_inventory,
    validate_order,
)
from .linear_workflow_handlers import (
    linear_step_1,
    linear_step_2,
    linear_step_3,
    linear_step_4,
)

# Resolver tests (2 handlers)
from .resolver_tests_handlers import (
    alternate_method,
    multi_method,
)

# Test scenarios (3 handlers)
from .test_scenarios_handlers import (
    permanent_error_step,
    retryable_error_step,
    success_step,
)

__all__ = [
    # Linear workflow
    "linear_step_1",
    "linear_step_2",
    "linear_step_3",
    "linear_step_4",
    # Diamond - unit test
    "diamond_init",
    "diamond_path_a",
    "diamond_path_b",
    "diamond_merge",
    # Diamond - E2E
    "diamond_start",
    "diamond_branch_b",
    "diamond_branch_c",
    "diamond_end",
    # Test scenarios
    "success_step",
    "retryable_error_step",
    "permanent_error_step",
    # Conditional approval
    "validate_request",
    "routing_decision",
    "auto_approve",
    "manager_approval",
    "finance_review",
    "finalize_approval",
    # Batch processing
    "csv_analyzer",
    "CsvBatchProcessorDslHandler",
    "CsvResultsAggregatorDslHandler",
    # Checkpoint yield
    "checkpoint_yield_analyzer",
    "CheckpointYieldWorkerDslHandler",
    "CheckpointYieldAggregatorDslHandler",
    # Domain events
    "validate_order",
    "process_payment",
    "update_inventory",
    "send_notification",
    # Resolver tests
    "multi_method",
    "alternate_method",
    # Blog - E-commerce
    "ecommerce_validate_cart",
    "ecommerce_process_payment",
    "ecommerce_update_inventory",
    "ecommerce_create_order",
    "ecommerce_send_confirmation",
    # Blog - Data Pipeline
    "data_pipeline_extract_sales",
    "data_pipeline_extract_inventory",
    "data_pipeline_extract_customers",
    "data_pipeline_transform_sales",
    "data_pipeline_transform_inventory",
    "data_pipeline_transform_customers",
    "data_pipeline_aggregate_metrics",
    "data_pipeline_generate_insights",
    # Blog - Microservices
    "microservices_create_user_account",
    "microservices_setup_billing_profile",
    "microservices_initialize_preferences",
    "microservices_send_welcome_sequence",
    "microservices_update_user_status",
    # Blog - Team Scaling (Customer Success)
    "cs_validate_refund_request",
    "cs_check_refund_policy",
    "cs_get_manager_approval",
    "cs_execute_refund_workflow",
    "cs_update_ticket_status",
    # Blog - Team Scaling (Payments)
    "pay_validate_payment_eligibility",
    "pay_process_gateway_refund",
    "pay_update_payment_records",
    "pay_notify_customer",
]
