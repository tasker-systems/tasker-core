"""DSL mirror of batch_processing_handlers using @batch_analyzer, @batch_worker, @step_handler.

Simplified batch processing that tests the decorator API without CSV file I/O.
The analyzer returns a BatchConfig, the worker processes items, and the aggregator
collects results -- matching the verbose handler output structure.
"""

from __future__ import annotations

from typing import Any

from tasker_core.step_handler.functional import (
    BatchConfig,
    batch_analyzer,
    batch_worker,
    depends_on,
    inputs,
    step_handler,
)
from tasker_core.types import StepHandlerResult


@batch_analyzer(
    "batch_processing_dsl.step_handlers.csv_analyzer",
    worker_template="process_csv_batch_py",
)
@inputs("csv_file_path")
def csv_analyzer(csv_file_path, context):
    """Analyze CSV and create batch configurations.

    For DSL parity testing, we use a simulated row count from step_config
    rather than reading an actual file.
    """
    if not csv_file_path:
        return StepHandlerResult.failure(
            message="csv_file_path is required",
            error_type="validation_error",
            retryable=False,
        )

    # Get configuration from step_config
    step_config = context.step_config or {}
    batch_size = step_config.get("batch_size", 200)
    # For parity testing, use simulated_row_count from step_config
    row_count = step_config.get("simulated_row_count", 0)

    if row_count == 0:
        return BatchConfig(total_items=0, batch_size=batch_size)

    return BatchConfig(
        total_items=row_count,
        batch_size=batch_size,
        metadata={
            "csv_file_path": csv_file_path,
            "batch_size": batch_size,
            "analysis_mode": context.input_data.get("analysis_mode", "default"),
        },
    )


@batch_worker("batch_processing_dsl.step_handlers.csv_batch_processor")
def csv_batch_processor(batch_context, context):
    """Process a batch of items based on cursor range."""
    if batch_context is None:
        return {"no_batch": True}

    return {
        "items_processed": batch_context.end_cursor - batch_context.start_cursor,
        "items_succeeded": batch_context.end_cursor - batch_context.start_cursor,
        "items_failed": 0,
        "batch_id": batch_context.batch_id,
    }


@step_handler("batch_processing_dsl.step_handlers.csv_results_aggregator")
def csv_results_aggregator(context):
    """Aggregate results from all batch workers."""
    worker_results: list[dict[str, Any]] = []

    for dep_name in context.dependency_results:
        if dep_name.startswith("process_csv_batch_"):
            unwrapped = context.get_dependency_result(dep_name)
            if unwrapped is not None and isinstance(unwrapped, dict):
                worker_results.append(unwrapped)

    total_processed = sum(r.get("items_processed", 0) for r in worker_results)
    total_succeeded = sum(r.get("items_succeeded", 0) for r in worker_results)
    total_failed = sum(r.get("items_failed", 0) for r in worker_results)
    worker_count = len(worker_results)

    total_inventory_value = 0.0
    for worker_result in worker_results:
        for item_result in worker_result.get("results", []):
            total_inventory_value += item_result.get("inventory_value", 0.0)

    return {
        "total_processed": total_processed,
        "total_succeeded": total_succeeded,
        "total_failed": total_failed,
        "worker_count": worker_count,
        "total_inventory_value": total_inventory_value,
        "result": {
            "total_processed": total_processed,
            "total_inventory_value": total_inventory_value,
            "worker_count": worker_count,
        },
    }


__all__ = [
    "csv_analyzer",
    "csv_batch_processor",
    "csv_results_aggregator",
]
