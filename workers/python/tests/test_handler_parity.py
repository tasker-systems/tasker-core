"""Parity tests: DSL decorator handlers vs. verbose class-based handlers (TAS-294 Phase 2).

Each test case instantiates both the verbose (class-based) handler and the DSL
(decorator-based) handler, feeds them identical StepContext objects, and asserts
that the result_data dictionaries match on all deterministic keys.

Fields with non-deterministic values (UUIDs, timestamps) are compared structurally
(same keys present, same types) rather than by exact value.

Usage:
    cd workers/python
    DATABASE_URL=postgresql://tasker:tasker@localhost/tasker_rust_test \
        uv run python -m pytest tests/test_handler_parity.py -v
"""

from __future__ import annotations

from uuid import uuid4

import pytest

from tasker_core.types import (
    FfiStepEvent,
    StepContext,
    StepHandlerResult,
)

# Blog examples - Post 01: E-commerce
from tests.handlers.dsl_examples.blog_examples.post_01_ecommerce_handlers import (
    validate_cart as dsl_validate_cart,
)
from tests.handlers.dsl_examples.blog_examples.post_02_data_pipeline_handlers import (
    extract_customer_data as dsl_extract_customers,
)
from tests.handlers.dsl_examples.blog_examples.post_02_data_pipeline_handlers import (
    extract_inventory_data as dsl_extract_inventory,
)

# Blog examples - Post 02: Data Pipeline
from tests.handlers.dsl_examples.blog_examples.post_02_data_pipeline_handlers import (
    extract_sales_data as dsl_extract_sales,
)

# Blog examples - Post 03: Microservices
from tests.handlers.dsl_examples.blog_examples.post_03_microservices_handlers import (
    create_user_account as dsl_create_user_account,
)
from tests.handlers.dsl_examples.conditional_approval_handlers import (
    auto_approve,
    finalize_approval,
    routing_decision,
    validate_request,
)
from tests.handlers.dsl_examples.diamond_workflow_handlers import (
    diamond_branch_b,
    diamond_branch_c,
    diamond_end,
    diamond_init,
    diamond_merge,
    diamond_path_a,
    diamond_path_b,
    diamond_start,
)
from tests.handlers.dsl_examples.domain_event_handlers import (
    process_payment as dsl_process_payment,
)
from tests.handlers.dsl_examples.domain_event_handlers import (
    validate_order,
)

# ============================================================================
# DSL handler imports
# ============================================================================
from tests.handlers.dsl_examples.linear_workflow_handlers import (
    linear_step_1,
    linear_step_2,
    linear_step_3,
    linear_step_4,
)
from tests.handlers.dsl_examples.resolver_tests_handlers import (
    alternate_method,
    multi_method,
)
from tests.handlers.dsl_examples.test_scenarios_handlers import (
    permanent_error_step,
    retryable_error_step,
    success_step,
)
from tests.handlers.examples.blog_examples.post_01_ecommerce import (
    ValidateCartHandler as EcommerceValidateCartHandler,
)
from tests.handlers.examples.blog_examples.post_02_data_pipeline import (
    ExtractCustomerDataHandler as DataPipelineExtractCustomerHandler,
)
from tests.handlers.examples.blog_examples.post_02_data_pipeline import (
    ExtractInventoryDataHandler as DataPipelineExtractInventoryHandler,
)
from tests.handlers.examples.blog_examples.post_02_data_pipeline import (
    ExtractSalesDataHandler as DataPipelineExtractSalesHandler,
)
from tests.handlers.examples.blog_examples.post_03_microservices import (
    CreateUserAccountHandler as MicroservicesCreateUserAccountHandler,
)
from tests.handlers.examples.conditional_approval_handlers import (
    AutoApproveHandler,
    FinalizeApprovalHandler,
    RoutingDecisionHandler,
    ValidateRequestHandler,
)
from tests.handlers.examples.diamond_handlers import (
    DiamondInitHandler,
    DiamondMergeHandler,
    DiamondPathAHandler,
    DiamondPathBHandler,
)
from tests.handlers.examples.diamond_workflow_handlers import (
    DiamondBranchBHandler,
    DiamondBranchCHandler,
    DiamondEndHandler,
    DiamondStartHandler,
)
from tests.handlers.examples.domain_event_handlers import (
    ProcessPaymentHandler,
    ValidateOrderHandler,
)

# ============================================================================
# Verbose handler imports
# ============================================================================
from tests.handlers.examples.linear_workflow_handlers import (
    LinearStep1Handler,
    LinearStep2Handler,
    LinearStep3Handler,
    LinearStep4Handler,
)
from tests.handlers.examples.resolver_tests_handlers import (
    AlternateMethodHandler,
    MultiMethodHandler,
)
from tests.handlers.examples.test_scenarios_handlers import (
    PermanentErrorStepHandler,
    RetryableErrorStepHandler,
    SuccessStepHandler,
)

# ============================================================================
# Helpers
# ============================================================================

# Keys known to contain non-deterministic values (UUIDs, timestamps, random IDs)
NON_DETERMINISTIC_KEYS = frozenset(
    {
        "timestamp",
        "validated_at",
        "processed_at",
        "sent_at",
        "created_at",
        "updated_at",
        "extracted_at",
        "generated_at",
        "activation_timestamp",
        "validation_timestamp",
        "policy_checked_at",
        "approved_at",
        "delegation_timestamp",
        "estimated_delivery",
        "next_billing_date",
        "original_purchase_date",
        "order_id",
        "order_number",
        "payment_id",
        "transaction_id",
        "billing_id",
        "preferences_id",
        "welcome_sequence_id",
        "message_id",
        "notification_id",
        "reservation_id",
        "inventory_log_id",
        "approval_id",
        "manager_id",
        "delegated_task_id",
        "correlation_id",
        "record_id",
        "refund_id",
        "gateway_transaction_id",
        "estimated_arrival",
        "user_id",
        "step_uuid",
        "user_created_at",
        "registration_completed_at",
    }
)


def _make_context(
    handler_name: str = "test_handler",
    input_data: dict | None = None,
    dependency_results: dict | None = None,
    step_config: dict | None = None,
    step_inputs: dict | None = None,
) -> StepContext:
    """Create a StepContext for testing.

    dependency_results format for verbose handlers that use context.dependency_results.get():
        {"step_name": {"field": value}}

    dependency_results format for verbose handlers that use context.get_dependency_result():
        {"step_name": {"result": {"field": value}}}

    The @depends_on decorator uses get_dependency_result() internally, which unwraps
    the {"result": ...} wrapper. So for handlers that use both patterns, we need to
    provide the format matching how the verbose handler accesses dependencies.
    """
    task_uuid = str(uuid4())
    step_uuid = str(uuid4())
    correlation_id = str(uuid4())

    task_sequence_step = {
        "task": {"task": {"context": input_data or {}}},
        "dependency_results": dependency_results or {},
        "step_definition": {"handler": {"initialization": step_config or {}}},
        "workflow_step": {"attempts": 0, "max_attempts": 3, "inputs": step_inputs or {}},
    }

    event = FfiStepEvent(
        event_id=str(uuid4()),
        task_uuid=task_uuid,
        step_uuid=step_uuid,
        correlation_id=correlation_id,
        task_sequence_step=task_sequence_step,
    )

    return StepContext.from_ffi_event(event, handler_name)


def _assert_result_parity(
    verbose_result: StepHandlerResult,
    dsl_result: StepHandlerResult,
    description: str,
    skip_non_deterministic: bool = True,
):
    """Assert two StepHandlerResults have matching structure and deterministic values.

    Both results must have the same:
    - is_success status
    - retryable flag
    - error_type (for failures)
    - result keys
    - deterministic field values
    """
    # Compare success/failure status
    assert verbose_result.is_success == dsl_result.is_success, (
        f"[{description}] is_success mismatch: "
        f"verbose={verbose_result.is_success}, dsl={dsl_result.is_success}"
    )

    # Compare retryable flag
    assert verbose_result.retryable == dsl_result.retryable, (
        f"[{description}] retryable mismatch: "
        f"verbose={verbose_result.retryable}, dsl={dsl_result.retryable}"
    )

    # Compare error_type for failures
    if not verbose_result.is_success:
        assert verbose_result.error_type == dsl_result.error_type, (
            f"[{description}] error_type mismatch: "
            f"verbose={verbose_result.error_type}, dsl={dsl_result.error_type}"
        )

    # Compare result data
    v_data = verbose_result.result or {}
    d_data = dsl_result.result or {}

    if skip_non_deterministic:
        _assert_dict_parity(v_data, d_data, description)
    else:
        assert v_data == d_data, (
            f"[{description}] result_data mismatch:\nverbose={v_data}\ndsl={d_data}"
        )


def _assert_dict_parity(verbose_dict: dict, dsl_dict: dict, description: str, path: str = ""):
    """Recursively compare dicts, skipping non-deterministic values."""
    prefix = f"{path}." if path else ""

    # Same keys
    v_keys = set(verbose_dict.keys())
    d_keys = set(dsl_dict.keys())
    assert v_keys == d_keys, (
        f"[{description}] key mismatch at '{path}': "
        f"verbose_only={v_keys - d_keys}, dsl_only={d_keys - v_keys}"
    )

    for key in v_keys:
        full_key = f"{prefix}{key}"
        v_val = verbose_dict[key]
        d_val = dsl_dict[key]

        if key in NON_DETERMINISTIC_KEYS:
            # Just check same type (both present and same general type)
            assert type(v_val) is type(d_val), (
                f"[{description}] type mismatch for non-deterministic key '{full_key}': "
                f"verbose={type(v_val).__name__}, dsl={type(d_val).__name__}"
            )
            continue

        if isinstance(v_val, dict) and isinstance(d_val, dict):
            _assert_dict_parity(v_val, d_val, description, full_key)
        elif isinstance(v_val, list) and isinstance(d_val, list):
            assert len(v_val) == len(d_val), (
                f"[{description}] list length mismatch at '{full_key}': "
                f"verbose={len(v_val)}, dsl={len(d_val)}"
            )
            for i, (v_item, d_item) in enumerate(zip(v_val, d_val, strict=True)):
                if isinstance(v_item, dict) and isinstance(d_item, dict):
                    _assert_dict_parity(v_item, d_item, description, f"{full_key}[{i}]")
                else:
                    assert v_item == d_item, (
                        f"[{description}] list item mismatch at '{full_key}[{i}]': "
                        f"verbose={v_item!r}, dsl={d_item!r}"
                    )
        else:
            assert v_val == d_val, (
                f"[{description}] value mismatch at '{full_key}': verbose={v_val!r}, dsl={d_val!r}"
            )


def _run_verbose(handler_class, context: StepContext) -> StepHandlerResult:
    """Instantiate and call a verbose class-based handler."""
    handler = handler_class()
    return handler.call(context)


def _run_dsl(dsl_fn, context: StepContext) -> StepHandlerResult:
    """Instantiate and call a DSL decorator handler."""
    handler = dsl_fn._handler_class()
    return handler.call(context)


# ============================================================================
# Linear Workflow Parity Tests
# ============================================================================


class TestLinearWorkflowParity:
    """Parity: linear_workflow_handlers (verbose) vs linear_workflow_handlers (DSL)."""

    def test_linear_step_1_success(self):
        """Step 1: square even_number=4 -> 16."""
        ctx = _make_context(
            handler_name="linear_workflow.step_handlers.LinearStep1Handler",
            input_data={"even_number": 4},
        )
        verbose_result = _run_verbose(LinearStep1Handler, ctx)

        dsl_ctx = _make_context(
            handler_name="linear_workflow_dsl.step_handlers.linear_step_1",
            input_data={"even_number": 4},
        )
        dsl_result = _run_dsl(linear_step_1, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "linear_step_1_success")

    def test_linear_step_1_missing_input(self):
        """Step 1: missing even_number -> failure."""
        ctx = _make_context(handler_name="linear_workflow.step_handlers.LinearStep1Handler")
        verbose_result = _run_verbose(LinearStep1Handler, ctx)

        dsl_ctx = _make_context(handler_name="linear_workflow_dsl.step_handlers.linear_step_1")
        dsl_result = _run_dsl(linear_step_1, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "linear_step_1_missing")

    def test_linear_step_2_success(self):
        """Step 2: 16 + 10 = 26."""
        deps = {"linear_step_1_py": {"result": {"result": 16, "operation": "square"}}}
        ctx = _make_context(
            handler_name="linear_workflow.step_handlers.LinearStep2Handler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(LinearStep2Handler, ctx)

        dsl_ctx = _make_context(
            handler_name="linear_workflow_dsl.step_handlers.linear_step_2",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(linear_step_2, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "linear_step_2_success")

    def test_linear_step_3_success(self):
        """Step 3: 26 * 3 = 78."""
        deps = {"linear_step_2_py": {"result": {"result": 26, "operation": "add"}}}
        ctx = _make_context(
            handler_name="linear_workflow.step_handlers.LinearStep3Handler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(LinearStep3Handler, ctx)

        dsl_ctx = _make_context(
            handler_name="linear_workflow_dsl.step_handlers.linear_step_3",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(linear_step_3, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "linear_step_3_success")

    def test_linear_step_4_success(self):
        """Step 4: 78 / 2 = 39.0."""
        deps = {"linear_step_3_py": {"result": {"result": 78, "operation": "multiply"}}}
        ctx = _make_context(
            handler_name="linear_workflow.step_handlers.LinearStep4Handler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(LinearStep4Handler, ctx)

        dsl_ctx = _make_context(
            handler_name="linear_workflow_dsl.step_handlers.linear_step_4",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(linear_step_4, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "linear_step_4_success")

    def test_linear_full_chain(self):
        """Run the complete chain with even_number=4, verify final result=39.0."""
        # Step 1: square
        ctx1_v = _make_context(
            handler_name="linear_workflow.step_handlers.LinearStep1Handler",
            input_data={"even_number": 4},
        )
        ctx1_d = _make_context(
            handler_name="linear_workflow_dsl.step_handlers.linear_step_1",
            input_data={"even_number": 4},
        )
        r1_v = _run_verbose(LinearStep1Handler, ctx1_v)
        r1_d = _run_dsl(linear_step_1, ctx1_d)
        _assert_result_parity(r1_v, r1_d, "chain_step_1")

        # Step 2: add
        step1_output = r1_v.result
        deps2 = {"linear_step_1_py": {"result": step1_output}}
        ctx2_v = _make_context(
            handler_name="linear_workflow.step_handlers.LinearStep2Handler",
            dependency_results=deps2,
        )
        ctx2_d = _make_context(
            handler_name="linear_workflow_dsl.step_handlers.linear_step_2",
            dependency_results=deps2,
        )
        r2_v = _run_verbose(LinearStep2Handler, ctx2_v)
        r2_d = _run_dsl(linear_step_2, ctx2_d)
        _assert_result_parity(r2_v, r2_d, "chain_step_2")

        # Step 3: multiply
        step2_output = r2_v.result
        deps3 = {"linear_step_2_py": {"result": step2_output}}
        ctx3_v = _make_context(
            handler_name="linear_workflow.step_handlers.LinearStep3Handler",
            dependency_results=deps3,
        )
        ctx3_d = _make_context(
            handler_name="linear_workflow_dsl.step_handlers.linear_step_3",
            dependency_results=deps3,
        )
        r3_v = _run_verbose(LinearStep3Handler, ctx3_v)
        r3_d = _run_dsl(linear_step_3, ctx3_d)
        _assert_result_parity(r3_v, r3_d, "chain_step_3")

        # Step 4: divide
        step3_output = r3_v.result
        deps4 = {"linear_step_3_py": {"result": step3_output}}
        ctx4_v = _make_context(
            handler_name="linear_workflow.step_handlers.LinearStep4Handler",
            dependency_results=deps4,
        )
        ctx4_d = _make_context(
            handler_name="linear_workflow_dsl.step_handlers.linear_step_4",
            dependency_results=deps4,
        )
        r4_v = _run_verbose(LinearStep4Handler, ctx4_v)
        r4_d = _run_dsl(linear_step_4, ctx4_d)
        _assert_result_parity(r4_v, r4_d, "chain_step_4")

        # Verify final value
        assert r4_v.result is not None
        assert r4_d.result is not None
        assert r4_v.result["result"] == 39.0
        assert r4_d.result["result"] == 39.0


# ============================================================================
# Diamond Workflow Parity Tests (Unit Test Diamond)
# ============================================================================


class TestDiamondUnitParity:
    """Parity: diamond_handlers (verbose) vs diamond_workflow_handlers (DSL, unit section)."""

    def test_diamond_init(self):
        """Init with initial_value=100."""
        # Verbose uses context.input_data.get() directly (no {"result": ...} wrapper for deps)
        ctx = _make_context(
            handler_name="diamond_init",
            input_data={"initial_value": 100},
        )
        verbose_result = _run_verbose(DiamondInitHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="diamond_init_dsl",
            input_data={"initial_value": 100},
        )
        dsl_result = _run_dsl(diamond_init, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "diamond_init")

    def test_diamond_path_a(self):
        """Path A: 100 * 2 = 200."""
        # Verbose DiamondPathAHandler uses context.dependency_results.get("diamond_init", {})
        # which returns raw dict. DSL uses @depends_on(init_result="diamond_init") which
        # calls get_dependency_result() and unwraps {"result": ...}.
        # So we need dependency_results WITHOUT the {"result": ...} wrapper for verbose,
        # but WITH it for DSL.
        init_output = {
            "initialized": True,
            "value": 100,
            "metadata": {"workflow": "diamond", "init_timestamp": "2025-01-01T00:00:00Z"},
        }

        # Verbose: accesses context.dependency_results.get("diamond_init", {}) -> gets raw dict
        verbose_deps = {"diamond_init": init_output}
        ctx = _make_context(
            handler_name="diamond_path_a",
            dependency_results=verbose_deps,
        )
        verbose_result = _run_verbose(DiamondPathAHandler, ctx)

        # DSL: @depends_on(init_result="diamond_init") calls get_dependency_result("diamond_init")
        # which unwraps {"result": ...}
        dsl_deps = {"diamond_init": {"result": init_output}}
        dsl_ctx = _make_context(
            handler_name="diamond_path_a_dsl",
            dependency_results=dsl_deps,
        )
        dsl_result = _run_dsl(diamond_path_a, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "diamond_path_a")

    def test_diamond_path_b(self):
        """Path B: 100 + 50 = 150."""
        init_output = {
            "initialized": True,
            "value": 100,
            "metadata": {"workflow": "diamond", "init_timestamp": "2025-01-01T00:00:00Z"},
        }

        verbose_deps = {"diamond_init": init_output}
        ctx = _make_context(
            handler_name="diamond_path_b",
            dependency_results=verbose_deps,
        )
        verbose_result = _run_verbose(DiamondPathBHandler, ctx)

        dsl_deps = {"diamond_init": {"result": init_output}}
        dsl_ctx = _make_context(
            handler_name="diamond_path_b_dsl",
            dependency_results=dsl_deps,
        )
        dsl_result = _run_dsl(diamond_path_b, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "diamond_path_b")

    def test_diamond_merge(self):
        """Merge: 200 + 150 = 350."""
        path_a_output = {"path_a_result": 200, "operation": "multiply_by_2", "input_value": 100}
        path_b_output = {"path_b_result": 150, "operation": "add_50", "input_value": 100}

        # Verbose uses context.dependency_results.get() directly
        verbose_deps = {"diamond_path_a": path_a_output, "diamond_path_b": path_b_output}
        ctx = _make_context(
            handler_name="diamond_merge",
            dependency_results=verbose_deps,
        )
        verbose_result = _run_verbose(DiamondMergeHandler, ctx)

        # DSL uses get_dependency_result() via @depends_on
        dsl_deps = {
            "diamond_path_a": {"result": path_a_output},
            "diamond_path_b": {"result": path_b_output},
        }
        dsl_ctx = _make_context(
            handler_name="diamond_merge_dsl",
            dependency_results=dsl_deps,
        )
        dsl_result = _run_dsl(diamond_merge, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "diamond_merge")


# ============================================================================
# Diamond Workflow Parity Tests (E2E Diamond)
# ============================================================================


class TestDiamondE2EParity:
    """Parity: diamond_workflow_handlers (verbose) vs diamond_workflow_handlers (DSL, E2E section)."""

    def test_diamond_start(self):
        """Start: square even_number=4 -> 16."""
        ctx = _make_context(
            handler_name="diamond_workflow.step_handlers.DiamondStartHandler",
            input_data={"even_number": 4},
        )
        verbose_result = _run_verbose(DiamondStartHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="diamond_workflow_dsl.step_handlers.diamond_start",
            input_data={"even_number": 4},
        )
        dsl_result = _run_dsl(diamond_start, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "diamond_start")

    def test_diamond_branch_b(self):
        """Branch B: 16 + 25 = 41."""
        deps = {
            "diamond_start_py": {
                "result": {"result": 16, "operation": "square", "step_type": "initial"}
            }
        }
        ctx = _make_context(
            handler_name="diamond_workflow.step_handlers.DiamondBranchBHandler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(DiamondBranchBHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="diamond_workflow_dsl.step_handlers.diamond_branch_b",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(diamond_branch_b, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "diamond_branch_b")

    def test_diamond_branch_c(self):
        """Branch C: 16 * 2 = 32."""
        deps = {
            "diamond_start_py": {
                "result": {"result": 16, "operation": "square", "step_type": "initial"}
            }
        }
        ctx = _make_context(
            handler_name="diamond_workflow.step_handlers.DiamondBranchCHandler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(DiamondBranchCHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="diamond_workflow_dsl.step_handlers.diamond_branch_c",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(diamond_branch_c, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "diamond_branch_c")

    def test_diamond_end(self):
        """End: (41 + 32) / 2 = 36.5."""
        deps = {
            "diamond_branch_b_py": {"result": {"result": 41, "operation": "add", "branch": "B"}},
            "diamond_branch_c_py": {
                "result": {"result": 32, "operation": "multiply", "branch": "C"}
            },
        }
        ctx = _make_context(
            handler_name="diamond_workflow.step_handlers.DiamondEndHandler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(DiamondEndHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="diamond_workflow_dsl.step_handlers.diamond_end",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(diamond_end, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "diamond_end")
        assert verbose_result.result is not None
        assert verbose_result.result["result"] == 36.5


# ============================================================================
# Test Scenarios Parity Tests
# ============================================================================


class TestScenariosParity:
    """Parity: test_scenarios_handlers (verbose) vs test_scenarios_handlers (DSL)."""

    def test_success_step(self):
        """Success handler with custom message."""
        ctx = _make_context(
            handler_name="test_scenarios.step_handlers.SuccessStepHandler",
            input_data={"message": "Hello from parity test"},
        )
        verbose_result = _run_verbose(SuccessStepHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="test_scenarios_dsl.step_handlers.success_step",
            input_data={"message": "Hello from parity test"},
        )
        dsl_result = _run_dsl(success_step, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "success_step")

    def test_retryable_error(self):
        """Retryable error handler."""
        ctx = _make_context(
            handler_name="test_scenarios.step_handlers.RetryableErrorStepHandler",
            input_data={"error_message": "Something went wrong temporarily"},
        )
        verbose_result = _run_verbose(RetryableErrorStepHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="test_scenarios_dsl.step_handlers.retryable_error_step",
            input_data={"error_message": "Something went wrong temporarily"},
        )
        dsl_result = _run_dsl(retryable_error_step, dsl_ctx)

        # Both should be failures with retryable=True
        assert not verbose_result.is_success
        assert not dsl_result.is_success
        assert verbose_result.retryable == dsl_result.retryable
        assert verbose_result.error_type == dsl_result.error_type
        assert verbose_result.error_message == dsl_result.error_message

    def test_permanent_error(self):
        """Permanent error handler."""
        ctx = _make_context(
            handler_name="test_scenarios.step_handlers.PermanentErrorStepHandler",
            input_data={"error_message": "Fatal error occurred"},
        )
        verbose_result = _run_verbose(PermanentErrorStepHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="test_scenarios_dsl.step_handlers.permanent_error_step",
            input_data={"error_message": "Fatal error occurred"},
        )
        dsl_result = _run_dsl(permanent_error_step, dsl_ctx)

        assert not verbose_result.is_success
        assert not dsl_result.is_success
        assert verbose_result.retryable == dsl_result.retryable
        assert verbose_result.error_type == dsl_result.error_type
        assert verbose_result.error_message == dsl_result.error_message


# ============================================================================
# Conditional Approval Parity Tests
# ============================================================================


class TestConditionalApprovalParity:
    """Parity: conditional_approval_handlers (verbose) vs conditional_approval_handlers (DSL)."""

    def test_validate_request_success(self):
        """Validate a valid request."""
        input_data = {"amount": 500, "requester": "alice", "purpose": "office supplies"}
        ctx = _make_context(
            handler_name="conditional_approval.step_handlers.ValidateRequestHandler",
            input_data=input_data,
        )
        verbose_result = _run_verbose(ValidateRequestHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="conditional_approval_dsl.step_handlers.validate_request",
            input_data=input_data,
        )
        dsl_result = _run_dsl(validate_request, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "validate_request_success")

    def test_validate_request_missing_fields(self):
        """Validate request with missing fields."""
        ctx = _make_context(
            handler_name="conditional_approval.step_handlers.ValidateRequestHandler",
            input_data={},
        )
        verbose_result = _run_verbose(ValidateRequestHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="conditional_approval_dsl.step_handlers.validate_request",
            input_data={},
        )
        dsl_result = _run_dsl(validate_request, dsl_ctx)

        assert not verbose_result.is_success
        assert not dsl_result.is_success
        assert verbose_result.retryable == dsl_result.retryable

    def test_routing_decision_auto_approve(self):
        """Route small amount to auto_approve."""
        validate_output = {
            "validated": True,
            "amount": 500,
            "requester": "alice",
            "purpose": "office supplies",
        }
        deps = {"validate_request_py": {"result": validate_output}}

        ctx = _make_context(
            handler_name="conditional_approval.step_handlers.RoutingDecisionHandler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(RoutingDecisionHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="conditional_approval_dsl.step_handlers.routing_decision",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(routing_decision, dsl_ctx)

        # Both should succeed and route to auto_approve
        assert verbose_result.is_success
        assert dsl_result.is_success

        # Check the decision output has matching routing
        assert verbose_result.result is not None
        assert dsl_result.result is not None
        v_data = verbose_result.result
        d_data = dsl_result.result

        # The decision handler returns a decision_point_outcome structure
        # Both should route to ["auto_approve_py"]
        v_outcome = v_data.get("decision_point_outcome", {})
        d_outcome = d_data.get("decision_point_outcome", {})

        assert v_outcome.get("step_names") == d_outcome.get("step_names")

    def test_routing_decision_manager(self):
        """Route medium amount to manager_approval."""
        validate_output = {
            "validated": True,
            "amount": 2000,
            "requester": "bob",
            "purpose": "new equipment",
        }
        deps = {"validate_request_py": {"result": validate_output}}

        ctx = _make_context(
            handler_name="conditional_approval.step_handlers.RoutingDecisionHandler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(RoutingDecisionHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="conditional_approval_dsl.step_handlers.routing_decision",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(routing_decision, dsl_ctx)

        assert verbose_result.is_success
        assert dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None

        v_outcome = verbose_result.result.get("decision_point_outcome", {})
        d_outcome = dsl_result.result.get("decision_point_outcome", {})
        assert v_outcome.get("step_names") == d_outcome.get("step_names")

    def test_routing_decision_dual_approval(self):
        """Route large amount to manager + finance."""
        validate_output = {
            "validated": True,
            "amount": 10000,
            "requester": "carol",
            "purpose": "server farm",
        }
        deps = {"validate_request_py": {"result": validate_output}}

        ctx = _make_context(
            handler_name="conditional_approval.step_handlers.RoutingDecisionHandler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(RoutingDecisionHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="conditional_approval_dsl.step_handlers.routing_decision",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(routing_decision, dsl_ctx)

        assert verbose_result.is_success
        assert dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None

        v_outcome = verbose_result.result.get("decision_point_outcome", {})
        d_outcome = dsl_result.result.get("decision_point_outcome", {})
        assert v_outcome.get("step_names") == d_outcome.get("step_names")

    def test_auto_approve(self):
        """Auto-approve step."""
        routing_output = {
            "routing_context": {"approval_path": "auto", "amount": 500, "threshold_used": "small"},
            "decision_point_outcome": {"type": "route", "step_names": ["auto_approve_py"]},
        }
        deps = {"routing_decision_py": {"result": routing_output}}

        ctx = _make_context(
            handler_name="conditional_approval.step_handlers.AutoApproveHandler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(AutoApproveHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="conditional_approval_dsl.step_handlers.auto_approve",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(auto_approve, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "auto_approve")

    def test_finalize_approval_auto(self):
        """Finalize with auto-approval result."""
        auto_output = {
            "approved": True,
            "approval_type": "auto",
            "approved_amount": 500,
            "approver": "system",
            "notes": "Auto-approved for amounts under $1,000",
        }
        deps = {
            "auto_approve_py": {"result": auto_output},
            "manager_approval_py": {"result": None},
            "finance_review_py": {"result": None},
        }

        ctx = _make_context(
            handler_name="conditional_approval.step_handlers.FinalizeApprovalHandler",
            dependency_results=deps,
        )
        verbose_result = _run_verbose(FinalizeApprovalHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="conditional_approval_dsl.step_handlers.finalize_approval",
            dependency_results=deps,
        )
        dsl_result = _run_dsl(finalize_approval, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "finalize_approval_auto")


# ============================================================================
# Domain Events Parity Tests
# ============================================================================


class TestDomainEventParity:
    """Parity: domain_event_handlers (verbose) vs domain_event_handlers (DSL)."""

    def test_validate_order(self):
        """Validate order with given inputs."""
        input_data = {"order_id": "ORD-123", "customer_id": "CUST-456", "amount": 99.99}
        ctx = _make_context(
            handler_name="domain_events_py.step_handlers.validate_order",
            input_data=input_data,
        )
        verbose_result = _run_verbose(ValidateOrderHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="domain_events_dsl_py.step_handlers.validate_order",
            input_data=input_data,
        )
        dsl_result = _run_dsl(validate_order, dsl_ctx)

        # Both succeed with same structure
        assert verbose_result.is_success == dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None
        assert set(verbose_result.result.keys()) == set(dsl_result.result.keys())

    def test_process_payment(self):
        """Process payment (non-failure path)."""
        input_data = {"order_id": "ORD-123", "amount": 99.99, "simulate_failure": False}
        ctx = _make_context(
            handler_name="domain_events_py.step_handlers.process_payment",
            input_data=input_data,
        )
        verbose_result = _run_verbose(ProcessPaymentHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="domain_events_dsl_py.step_handlers.process_payment",
            input_data=input_data,
        )
        dsl_result = _run_dsl(dsl_process_payment, dsl_ctx)

        assert verbose_result.is_success == dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None
        assert set(verbose_result.result.keys()) == set(dsl_result.result.keys())

    def test_process_payment_failure(self):
        """Process payment with simulated failure."""
        input_data = {"order_id": "ORD-123", "amount": 99.99, "simulate_failure": True}
        ctx = _make_context(
            handler_name="domain_events_py.step_handlers.process_payment",
            input_data=input_data,
        )
        verbose_result = _run_verbose(ProcessPaymentHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="domain_events_dsl_py.step_handlers.process_payment",
            input_data=input_data,
        )
        dsl_result = _run_dsl(dsl_process_payment, dsl_ctx)

        assert not verbose_result.is_success
        assert not dsl_result.is_success
        assert verbose_result.retryable == dsl_result.retryable


# ============================================================================
# Resolver Tests Parity
# ============================================================================


class TestResolverParity:
    """Parity: resolver_tests_handlers (verbose) vs resolver_tests_handlers (DSL)."""

    def test_multi_method_default(self):
        """Multi-method handler default call."""
        input_data = {"data": {"key": "value"}}
        ctx = _make_context(
            handler_name="resolver_tests.step_handlers.MultiMethodHandler",
            input_data=input_data,
        )
        verbose_result = _run_verbose(MultiMethodHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="resolver_tests_dsl.step_handlers.multi_method",
            input_data=input_data,
        )
        dsl_result = _run_dsl(multi_method, dsl_ctx)

        # Both should succeed
        assert verbose_result.is_success == dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None

        # Check deterministic fields match
        v_data = verbose_result.result
        d_data = dsl_result.result
        assert v_data["invoked_method"] == d_data["invoked_method"]
        assert v_data["handler"] == d_data["handler"]

    def test_alternate_method_default(self):
        """Alternate method handler default call."""
        ctx = _make_context(
            handler_name="resolver_tests.step_handlers.AlternateMethodHandler",
        )
        verbose_result = _run_verbose(AlternateMethodHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="resolver_tests_dsl.step_handlers.alternate_method",
        )
        dsl_result = _run_dsl(alternate_method, dsl_ctx)

        assert verbose_result.is_success == dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None
        v_data = verbose_result.result
        d_data = dsl_result.result
        assert v_data["invoked_method"] == d_data["invoked_method"]
        assert v_data["handler"] == d_data["handler"]


# ============================================================================
# Blog Examples - Post 01: E-commerce (validate_cart only - others have random IDs)
# ============================================================================


class TestBlogEcommerceParity:
    """Parity: blog post_01 handlers (verbose) vs (DSL)."""

    def test_validate_cart_success(self):
        """Validate cart with valid items."""
        input_data = {
            "cart_items": [
                {"product_id": 1, "quantity": 2},
                {"product_id": 2, "quantity": 1},
            ]
        }
        ctx = _make_context(
            handler_name="ecommerce.step_handlers.ValidateCartHandler",
            input_data=input_data,
        )
        verbose_result = _run_verbose(EcommerceValidateCartHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="ecommerce_dsl.step_handlers.validate_cart",
            input_data=input_data,
        )
        dsl_result = _run_dsl(dsl_validate_cart, dsl_ctx)

        _assert_result_parity(verbose_result, dsl_result, "ecommerce_validate_cart")

    def test_validate_cart_empty(self):
        """Validate cart with empty items -> both handlers error.

        Note: Both verbose and DSL handlers raise PermanentError(error_code=...)
        but PermanentError does not accept error_code kwarg (pre-existing bug).
        The verbose handler propagates the TypeError, while the DSL decorator
        auto-catches it and returns a failure result. We verify both error.
        """
        input_data = {"cart_items": []}

        # Verbose handler raises TypeError (uncaught by class-based pattern)
        ctx = _make_context(
            handler_name="ecommerce.step_handlers.ValidateCartHandler",
            input_data=input_data,
        )
        with pytest.raises(TypeError):
            _run_verbose(EcommerceValidateCartHandler, ctx)

        # DSL handler catches the TypeError and returns failure
        dsl_ctx = _make_context(
            handler_name="ecommerce_dsl.step_handlers.validate_cart",
            input_data=input_data,
        )
        dsl_result = _run_dsl(dsl_validate_cart, dsl_ctx)
        assert not dsl_result.is_success


# ============================================================================
# Blog Examples - Post 02: Data Pipeline (extract steps - no dependencies)
# ============================================================================


class TestBlogDataPipelineParity:
    """Parity: blog post_02 extract handlers (verbose) vs (DSL)."""

    def test_extract_sales_data(self):
        """Extract sales data (no inputs needed)."""
        ctx = _make_context(
            handler_name="data_pipeline.step_handlers.ExtractSalesDataHandler",
        )
        verbose_result = _run_verbose(DataPipelineExtractSalesHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="data_pipeline_dsl.step_handlers.extract_sales_data",
        )
        dsl_result = _run_dsl(dsl_extract_sales, dsl_ctx)

        assert verbose_result.is_success == dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None
        # Structural parity: same keys, same records
        v_data = verbose_result.result
        d_data = dsl_result.result
        assert set(v_data.keys()) == set(d_data.keys())
        assert v_data["records"] == d_data["records"]
        assert v_data["source"] == d_data["source"]

    def test_extract_inventory_data(self):
        """Extract inventory data."""
        ctx = _make_context(
            handler_name="data_pipeline.step_handlers.ExtractInventoryDataHandler",
        )
        verbose_result = _run_verbose(DataPipelineExtractInventoryHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="data_pipeline_dsl.step_handlers.extract_inventory_data",
        )
        dsl_result = _run_dsl(dsl_extract_inventory, dsl_ctx)

        assert verbose_result.is_success == dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None
        v_data = verbose_result.result
        d_data = dsl_result.result
        assert set(v_data.keys()) == set(d_data.keys())
        assert v_data["records"] == d_data["records"]

    def test_extract_customer_data(self):
        """Extract customer data."""
        ctx = _make_context(
            handler_name="data_pipeline.step_handlers.ExtractCustomerDataHandler",
        )
        verbose_result = _run_verbose(DataPipelineExtractCustomerHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="data_pipeline_dsl.step_handlers.extract_customer_data",
        )
        dsl_result = _run_dsl(dsl_extract_customers, dsl_ctx)

        assert verbose_result.is_success == dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None
        v_data = verbose_result.result
        d_data = dsl_result.result
        assert set(v_data.keys()) == set(d_data.keys())
        assert v_data["records"] == d_data["records"]


# ============================================================================
# Blog Examples - Post 03: Microservices (create_user_account)
# ============================================================================


class TestBlogMicroservicesParity:
    """Parity: blog post_03 handlers (verbose) vs (DSL)."""

    def test_create_user_account_success(self):
        """Create user with valid data."""
        input_data = {
            "user_info": {
                "email": "test@example.com",
                "name": "Test User",
                "plan": "free",
            }
        }
        ctx = _make_context(
            handler_name="microservices.step_handlers.CreateUserAccountHandler",
            input_data=input_data,
        )
        verbose_result = _run_verbose(MicroservicesCreateUserAccountHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="microservices_dsl.step_handlers.create_user_account",
            input_data=input_data,
        )
        dsl_result = _run_dsl(dsl_create_user_account, dsl_ctx)

        # Both should succeed
        assert verbose_result.is_success == dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None
        # Check deterministic fields
        v_data = verbose_result.result
        d_data = dsl_result.result
        assert v_data["email"] == d_data["email"]
        assert v_data["name"] == d_data["name"]
        assert v_data["plan"] == d_data["plan"]
        assert v_data["status"] == d_data["status"]

    def test_create_user_account_missing_email(self):
        """Create user with missing email -> failure."""
        input_data = {"user_info": {"name": "Test User"}}
        ctx = _make_context(
            handler_name="microservices.step_handlers.CreateUserAccountHandler",
            input_data=input_data,
        )
        verbose_result = _run_verbose(MicroservicesCreateUserAccountHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="microservices_dsl.step_handlers.create_user_account",
            input_data=input_data,
        )
        dsl_result = _run_dsl(dsl_create_user_account, dsl_ctx)

        assert not verbose_result.is_success
        assert not dsl_result.is_success
        assert verbose_result.retryable == dsl_result.retryable

    def test_create_user_account_existing_idempotent(self):
        """Create user that already exists with matching data -> idempotent success."""
        input_data = {
            "user_info": {
                "email": "existing@example.com",
                "name": "Existing User",
                "plan": "free",
            }
        }
        ctx = _make_context(
            handler_name="microservices.step_handlers.CreateUserAccountHandler",
            input_data=input_data,
        )
        verbose_result = _run_verbose(MicroservicesCreateUserAccountHandler, ctx)

        dsl_ctx = _make_context(
            handler_name="microservices_dsl.step_handlers.create_user_account",
            input_data=input_data,
        )
        dsl_result = _run_dsl(dsl_create_user_account, dsl_ctx)

        assert verbose_result.is_success == dsl_result.is_success
        assert verbose_result.result is not None
        assert dsl_result.result is not None
        v_data = verbose_result.result
        d_data = dsl_result.result
        assert v_data["status"] == d_data["status"]
        assert v_data["status"] == "already_exists"
