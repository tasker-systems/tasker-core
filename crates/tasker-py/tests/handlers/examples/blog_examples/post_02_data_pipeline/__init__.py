"""Blog Post 02: Data Pipeline Analytics - DAG Workflow Example.

This module demonstrates a data pipeline workflow with:
- Parallel extract handlers (3 sources)
- Sequential transform handlers
- DAG convergence (aggregate metrics)
- Final insights generation

TAS-137 Best Practices:
- get_dependency_result() for upstream step results
- DAG convergence pattern with multiple dependencies
- Proper error handling and validation
"""

from .aggregate_handler import AggregateMetricsHandler
from .extract_handlers import (
    ExtractCustomerDataHandler,
    ExtractInventoryDataHandler,
    ExtractSalesDataHandler,
)
from .generate_insights_handler import GenerateInsightsHandler
from .transform_handlers import (
    TransformCustomersHandler,
    TransformInventoryHandler,
    TransformSalesHandler,
)

__all__ = [
    "AggregateMetricsHandler",
    "ExtractCustomerDataHandler",
    "ExtractInventoryDataHandler",
    "ExtractSalesDataHandler",
    "GenerateInsightsHandler",
    "TransformCustomersHandler",
    "TransformInventoryHandler",
    "TransformSalesHandler",
]
