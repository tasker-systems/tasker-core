"""DSL mirror of checkpoint_yield_handlers.

The analyzer uses @batch_analyzer DSL. The worker and aggregator are
class-based because they need direct access to Batchable instance methods
(checkpoint_yield, batch_worker_success, handle_no_op_worker,
detect_aggregation_scenario, etc.).
"""

from __future__ import annotations

from typing import TYPE_CHECKING, cast

from tasker_core.batch_processing import Batchable
from tasker_core.step_handler import StepHandler
from tasker_core.step_handler.functional import (
    BatchConfig,
    batch_analyzer,
)
from tasker_core.types import ErrorType, StepHandlerResult

if TYPE_CHECKING:
    from tasker_core.types import StepContext


@batch_analyzer(
    "checkpoint_yield_dsl_py.step_handlers.checkpoint_yield_analyzer",
    worker_template="checkpoint_yield_batch_dsl_py",
)
def checkpoint_yield_analyzer(context):
    """Analyze and create batch configuration."""
    total_items = context.get_input_or("total_items", 100)

    if total_items <= 0:
        return BatchConfig(total_items=0, batch_size=1)

    return BatchConfig(
        total_items=total_items,
        batch_size=total_items,
        metadata={
            "test_type": "checkpoint_yield",
            "items_per_batch": total_items,
        },
    )


class CheckpointYieldWorkerDslHandler(StepHandler, Batchable):
    """DSL mirror of CheckpointYieldWorkerHandler.

    Class-based because checkpoint_yield(), handle_no_op_worker(), and
    batch_worker_success() require Batchable instance methods.
    """

    handler_name = "checkpoint_yield_dsl_py.step_handlers.checkpoint_yield_worker"

    def call(self, context: StepContext) -> StepHandlerResult:
        """Process batch items with checkpoint yielding."""
        # Check for no-op placeholder
        no_op_result = self.handle_no_op_worker(context)
        if no_op_result:
            return no_op_result

        # Get batch worker inputs (cursor range)
        batch_inputs = self.get_batch_worker_inputs(context)
        if batch_inputs is None:
            return self.failure(
                message="No batch inputs found",
                error_type=ErrorType.VALIDATION_ERROR,
                retryable=False,
            )

        # Configuration from task context
        items_per_checkpoint = context.get_input_or(
            "items_per_checkpoint",
            context.get_config("items_per_checkpoint") or 25,
        )
        fail_after_items = context.get_input("fail_after_items")
        fail_on_attempt = context.get_input_or("fail_on_attempt", 1)
        permanent_failure = context.get_input_or("permanent_failure", False)

        # Checkpoint resume logic
        if context.has_checkpoint():
            start_cursor = context.checkpoint_cursor or batch_inputs.cursor.start_cursor
            accumulated = context.accumulated_results or {
                "running_total": 0,
                "item_ids": [],
            }
            total_processed = context.checkpoint_items_processed
        else:
            start_cursor = batch_inputs.cursor.start_cursor
            accumulated = {"running_total": 0, "item_ids": []}
            total_processed = 0

        end_cursor = batch_inputs.cursor.end_cursor
        current_attempt = context.retry_count + 1

        # Process items in chunks
        current_cursor = start_cursor
        items_in_chunk = 0

        while current_cursor < end_cursor:
            # Failure injection
            if (
                fail_after_items is not None
                and total_processed >= fail_after_items
                and current_attempt == fail_on_attempt
            ):
                return self._inject_failure(total_processed, current_cursor, permanent_failure)

            # Process one item
            item_id = f"item_{current_cursor:04d}"
            value = current_cursor + 1
            accumulated["running_total"] += value
            cast(list[str], accumulated["item_ids"]).append(item_id)

            current_cursor += 1
            items_in_chunk += 1
            total_processed += 1

            # Yield checkpoint at interval
            if items_in_chunk >= items_per_checkpoint and current_cursor < end_cursor:
                return self.checkpoint_yield(
                    cursor=current_cursor,
                    items_processed=total_processed,
                    accumulated_results=accumulated,
                )

        # All items processed
        return self.batch_worker_success(
            items_processed=total_processed,
            items_succeeded=total_processed,
            items_failed=0,
            batch_metadata={
                **accumulated,
                "final_cursor": current_cursor,
                "checkpoints_used": total_processed // items_per_checkpoint,
            },
        )

    def _inject_failure(
        self, items_processed: int, cursor: int, permanent: bool
    ) -> StepHandlerResult:
        """Inject a failure for testing retry behavior."""
        if permanent:
            return self.failure(
                message=f"Injected permanent failure after {items_processed} items",
                error_type=ErrorType.PERMANENT_ERROR,
                retryable=False,
                metadata={
                    "items_processed": items_processed,
                    "cursor_at_failure": cursor,
                    "failure_type": "permanent",
                },
            )
        return self.failure(
            message=f"Injected transient failure after {items_processed} items",
            error_type=ErrorType.RETRYABLE_ERROR,
            retryable=True,
            metadata={
                "items_processed": items_processed,
                "cursor_at_failure": cursor,
                "failure_type": "transient",
            },
        )


class CheckpointYieldAggregatorDslHandler(StepHandler, Batchable):
    """DSL mirror of CheckpointYieldAggregatorHandler.

    Class-based for detect_aggregation_scenario() and no_batches_aggregation_result().
    """

    handler_name = "checkpoint_yield_dsl_py.step_handlers.checkpoint_yield_aggregator"

    def call(self, context: StepContext) -> StepHandlerResult:
        """Aggregate batch worker results."""
        scenario = self.detect_aggregation_scenario(
            context.dependency_results,
            "analyze_items_dsl_py",
            "checkpoint_yield_batch_dsl_py_",
        )

        if scenario.is_no_batches:
            return self.no_batches_aggregation_result(
                {
                    "total_processed": 0,
                    "running_total": 0,
                    "test_passed": True,
                    "scenario": "no_batches",
                }
            )

        total_processed = 0
        running_total = 0
        all_item_ids: list[str] = []
        checkpoints_used = 0

        for _worker_name, result in scenario.batch_results.items():
            if result:
                total_processed += result.get("items_processed", 0)
                batch_metadata = result.get("batch_metadata", {})
                running_total += batch_metadata.get("running_total", 0)
                all_item_ids.extend(batch_metadata.get("item_ids", []))
                checkpoints_used += batch_metadata.get("checkpoints_used", 0)

        return self.success(
            {
                "total_processed": total_processed,
                "running_total": running_total,
                "item_count": len(all_item_ids),
                "checkpoints_used": checkpoints_used,
                "worker_count": scenario.worker_count,
                "test_passed": True,
                "scenario": "with_batches",
            }
        )


__all__ = [
    "checkpoint_yield_analyzer",
    "CheckpointYieldWorkerDslHandler",
    "CheckpointYieldAggregatorDslHandler",
]
