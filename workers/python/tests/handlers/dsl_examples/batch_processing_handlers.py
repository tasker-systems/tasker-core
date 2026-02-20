"""DSL mirror of batch_processing_handlers using class-based Batchable handlers.

The analyzer uses @batch_analyzer DSL. The batch processor and aggregator are
class-based because they need direct access to Batchable instance methods
(handle_no_op_worker, get_batch_worker_inputs, batch_worker_success,
aggregate_worker_results, etc.) and must perform real CSV file I/O.
"""

from __future__ import annotations

import csv
import os
from typing import TYPE_CHECKING, Any

from tasker_core.batch_processing import Batchable
from tasker_core.step_handler import StepHandler
from tasker_core.step_handler.functional import (
    BatchConfig,
    batch_analyzer,
    inputs,
)
from tasker_core.types import StepHandlerResult

if TYPE_CHECKING:
    from tasker_core.types import StepContext


@batch_analyzer(
    "batch_processing_dsl_py.step_handlers.csv_analyzer",
    worker_template="process_csv_batch_dsl_py",
)
@inputs("csv_file_path")
def csv_analyzer(csv_file_path, context):
    """Analyze CSV file and create batch configurations."""
    if not csv_file_path:
        return StepHandlerResult.failure(
            message="csv_file_path is required",
            error_type="validation_error",
            retryable=False,
        )

    step_config = context.step_config or {}
    batch_size = step_config.get("batch_size", 200)
    max_workers = step_config.get("max_workers", 5)

    # Check if file exists
    if not os.path.exists(csv_file_path):
        return BatchConfig(total_items=0, batch_size=batch_size)

    # Count CSV rows (excluding header)
    try:
        with open(csv_file_path, encoding="utf-8") as f:
            reader = csv.reader(f)
            next(reader)  # Skip header
            row_count = sum(1 for _ in reader)
    except Exception:
        return BatchConfig(total_items=0, batch_size=batch_size)

    if row_count == 0:
        return BatchConfig(total_items=0, batch_size=batch_size)

    # Calculate actual workers needed
    num_batches = (row_count + batch_size - 1) // batch_size
    actual_workers = min(num_batches, max_workers)
    effective_batch_size = (row_count + actual_workers - 1) // actual_workers

    return BatchConfig(
        total_items=row_count,
        batch_size=effective_batch_size,
        metadata={
            "csv_file_path": csv_file_path,
            "batch_size": batch_size,
            "analysis_mode": context.input_data.get("analysis_mode", "default"),
        },
    )


class CsvBatchProcessorDslHandler(StepHandler, Batchable):
    """DSL mirror of CsvBatchProcessorHandler.

    Class-based because it needs handle_no_op_worker(), get_batch_worker_inputs(),
    and batch_worker_success() from Batchable, plus real CSV file I/O.
    """

    handler_name = "batch_processing_dsl_py.step_handlers.csv_batch_processor"

    def call(self, context: StepContext) -> StepHandlerResult:
        """Process a batch of CSV rows."""
        step_inputs = context.step_inputs or {}

        # Check for no-op placeholder
        if step_inputs.get("is_no_op"):
            return self.batch_worker_success(
                items_processed=0,
                items_succeeded=0,
                items_failed=0,
                results=[],
                errors=[],
                last_cursor=0,
                metadata={"no_op": True},
            )

        cursor = step_inputs.get("cursor", {})
        start_cursor = cursor.get("start_cursor", 0)
        end_cursor = cursor.get("end_cursor", 0)
        batch_id = cursor.get("batch_id", "unknown")

        # Get CSV file path from analyzer dependency result
        analyzer_result = context.get_dependency_result("analyze_csv_dsl_py")
        if analyzer_result is None:
            return StepHandlerResult.failure(
                message="Missing analyze_csv_dsl_py result",
                error_type="dependency_error",
                retryable=True,
            )

        batch_metadata = analyzer_result.get("batch_metadata", {})
        csv_file_path = batch_metadata.get("csv_file_path")

        if not csv_file_path:
            return StepHandlerResult.failure(
                message="csv_file_path not found in batch metadata",
                error_type="dependency_error",
                retryable=False,
            )

        # Process the batch
        results = self._process_csv_batch(csv_file_path, start_cursor, end_cursor)

        return self.batch_worker_success(
            items_processed=results["items_processed"],
            items_succeeded=results["items_succeeded"],
            items_failed=results["items_failed"],
            results=results["results"],
            errors=results.get("errors", []),
            last_cursor=end_cursor,
            metadata={"batch_id": batch_id},
        )

    def _process_csv_batch(
        self, csv_file_path: str, start_cursor: int, end_cursor: int
    ) -> dict[str, Any]:
        """Process rows from start_cursor to end_cursor."""
        results: list[dict[str, Any]] = []
        errors: list[dict[str, Any]] = []
        items_processed = 0
        items_succeeded = 0
        items_failed = 0

        with open(csv_file_path, encoding="utf-8") as f:
            reader = csv.DictReader(f)

            for row_num, row in enumerate(reader, start=1):
                if row_num <= start_cursor:
                    continue
                if row_num > end_cursor:
                    break

                items_processed += 1

                try:
                    price = float(row.get("price", 0))
                    stock = int(row.get("stock", 0))
                    inventory_value = price * stock

                    results.append(
                        {
                            "row_number": row_num,
                            "product_id": row.get("id", f"PROD-{row_num}"),
                            "inventory_value": inventory_value,
                            "price": price,
                            "stock": stock,
                        }
                    )
                    items_succeeded += 1
                except (ValueError, KeyError) as e:
                    items_failed += 1
                    errors.append({"row_number": row_num, "error": str(e)})

        return {
            "items_processed": items_processed,
            "items_succeeded": items_succeeded,
            "items_failed": items_failed,
            "results": results,
            "errors": errors,
        }


class CsvResultsAggregatorDslHandler(StepHandler, Batchable):
    """DSL mirror of CsvResultsAggregatorHandler.

    Class-based for aggregate_worker_results() from Batchable.
    """

    handler_name = (
        "batch_processing_dsl_py.step_handlers.csv_results_aggregator"
    )

    def call(self, context: StepContext) -> StepHandlerResult:
        """Aggregate results from all batch workers."""
        worker_results: list[dict[str, Any]] = []

        for dep_name in context.dependency_results:
            if dep_name.startswith("process_csv_batch_"):
                unwrapped = context.get_dependency_result(dep_name)
                if unwrapped is not None and isinstance(unwrapped, dict):
                    worker_results.append(unwrapped)

        aggregated = self.aggregate_worker_results(worker_results)

        # Calculate inventory metrics
        total_inventory_value = 0.0
        for worker_result in worker_results:
            for item_result in worker_result.get("results", []):
                total_inventory_value += item_result.get("inventory_value", 0.0)

        worker_count = aggregated.get("batch_count", len(worker_results))

        return StepHandlerResult.success(
            {
                "total_processed": aggregated.get("total_processed", 0),
                "total_succeeded": aggregated.get("total_succeeded", 0),
                "total_failed": aggregated.get("total_failed", 0),
                "worker_count": worker_count,
                "total_inventory_value": total_inventory_value,
                "result": {
                    "total_processed": aggregated.get("total_processed", 0),
                    "total_inventory_value": total_inventory_value,
                    "worker_count": worker_count,
                },
            }
        )


__all__ = [
    "csv_analyzer",
    "CsvBatchProcessorDslHandler",
    "CsvResultsAggregatorDslHandler",
]
