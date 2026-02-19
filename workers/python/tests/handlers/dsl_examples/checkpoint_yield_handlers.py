"""DSL mirror of checkpoint_yield_handlers.

Note: The checkpoint yield handlers use Batchable mixin methods extensively
(create_cursor_ranges, batch_analyzer_success, batch_worker_success,
checkpoint_yield, etc.) which require class-based handlers. The DSL decorators
(@batch_analyzer, @batch_worker) handle this via the Batchable mixin on the
generated class.

For the worker handler, checkpoint_yield is too stateful for the simple
decorator pattern, so we use @step_handler with direct context access.
"""

from __future__ import annotations

from tasker_core.step_handler.functional import (
    BatchConfig,
    batch_analyzer,
    step_handler,
)
from tasker_core.types import StepHandlerResult


@batch_analyzer(
    "CheckpointYieldDsl.StepHandlers.checkpoint_yield_analyzer",
    worker_template="checkpoint_yield_batch",
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


@step_handler("CheckpointYieldDsl.StepHandlers.checkpoint_yield_worker")
def checkpoint_yield_worker(context):
    """Process batch items - simplified version without checkpoint yielding.

    The full checkpoint yield behavior requires the Batchable mixin's
    checkpoint_yield() method which is only available on the class-based handler.
    This DSL version processes all items in one pass for output parity testing.
    """
    # Get batch inputs from step_inputs
    step_inputs = context.step_inputs or {}
    cursor = step_inputs.get("cursor", {})
    start_cursor = cursor.get("start_cursor", 0)
    end_cursor = cursor.get("end_cursor", 0)

    total_items = context.get_input_or("total_items", 100)
    items_per_checkpoint = context.get_input_or("items_per_checkpoint", 25)

    # Process all items
    running_total = 0
    item_ids = []

    for cursor_pos in range(start_cursor, end_cursor):
        item_id = f"item_{cursor_pos:04d}"
        value = cursor_pos + 1
        running_total += value
        item_ids.append(item_id)

    total_processed = end_cursor - start_cursor

    return StepHandlerResult.success(
        {
            "items_processed": total_processed,
            "items_succeeded": total_processed,
            "items_failed": 0,
            "batch_metadata": {
                "running_total": running_total,
                "item_ids": item_ids,
                "final_cursor": end_cursor,
                "checkpoints_used": total_processed // items_per_checkpoint,
            },
        }
    )


@step_handler("CheckpointYieldDsl.StepHandlers.checkpoint_yield_aggregator")
def checkpoint_yield_aggregator(context):
    """Aggregate batch worker results."""
    total_processed = 0
    running_total = 0
    all_item_ids: list[str] = []
    checkpoints_used = 0
    worker_count = 0

    for dep_name in context.dependency_results:
        if dep_name.startswith("checkpoint_yield_batch_"):
            result = context.get_dependency_result(dep_name)
            if result:
                worker_count += 1
                total_processed += result.get("items_processed", 0)
                batch_metadata = result.get("batch_metadata", {})
                running_total += batch_metadata.get("running_total", 0)
                all_item_ids.extend(batch_metadata.get("item_ids", []))
                checkpoints_used += batch_metadata.get("checkpoints_used", 0)

    return {
        "total_processed": total_processed,
        "running_total": running_total,
        "item_count": len(all_item_ids),
        "checkpoints_used": checkpoints_used,
        "worker_count": worker_count,
        "test_passed": True,
        "scenario": "with_batches" if worker_count > 0 else "no_batches",
    }


__all__ = [
    "checkpoint_yield_analyzer",
    "checkpoint_yield_worker",
    "checkpoint_yield_aggregator",
]
