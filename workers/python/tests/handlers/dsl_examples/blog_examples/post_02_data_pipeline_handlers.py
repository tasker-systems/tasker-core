"""DSL mirror of blog post_02_data_pipeline handlers.

Data pipeline: extract (3 parallel) -> transform (3) -> aggregate -> insights

Uses same sample data and transformation logic as the verbose version.
"""

from __future__ import annotations

import logging
from collections import defaultdict
from datetime import datetime, timezone
from typing import Any

from tasker_core.errors import PermanentError
from tasker_core.step_handler.functional import depends_on, step_handler

logger = logging.getLogger(__name__)

# Same sample data as verbose version
SAMPLE_SALES_DATA: list[dict[str, Any]] = [
    {"order_id": "ORD-001", "date": "2025-11-01", "product_id": "PROD-A", "quantity": 5, "amount": 499.95},
    {"order_id": "ORD-002", "date": "2025-11-05", "product_id": "PROD-B", "quantity": 3, "amount": 299.97},
    {"order_id": "ORD-003", "date": "2025-11-10", "product_id": "PROD-A", "quantity": 2, "amount": 199.98},
    {"order_id": "ORD-004", "date": "2025-11-15", "product_id": "PROD-C", "quantity": 10, "amount": 1499.90},
    {"order_id": "ORD-005", "date": "2025-11-18", "product_id": "PROD-B", "quantity": 7, "amount": 699.93},
]

SAMPLE_INVENTORY_DATA: list[dict[str, Any]] = [
    {"product_id": "PROD-A", "sku": "SKU-A-001", "warehouse": "WH-01", "quantity_on_hand": 150, "reorder_point": 50},
    {"product_id": "PROD-B", "sku": "SKU-B-002", "warehouse": "WH-01", "quantity_on_hand": 75, "reorder_point": 25},
    {"product_id": "PROD-C", "sku": "SKU-C-003", "warehouse": "WH-02", "quantity_on_hand": 200, "reorder_point": 100},
    {"product_id": "PROD-A", "sku": "SKU-A-001", "warehouse": "WH-02", "quantity_on_hand": 100, "reorder_point": 50},
    {"product_id": "PROD-B", "sku": "SKU-B-002", "warehouse": "WH-03", "quantity_on_hand": 50, "reorder_point": 25},
]

SAMPLE_CUSTOMER_DATA: list[dict[str, Any]] = [
    {"customer_id": "CUST-001", "name": "Alice Johnson", "tier": "gold", "lifetime_value": 5000.00, "join_date": "2024-01-15"},
    {"customer_id": "CUST-002", "name": "Bob Smith", "tier": "silver", "lifetime_value": 2500.00, "join_date": "2024-03-20"},
    {"customer_id": "CUST-003", "name": "Carol White", "tier": "premium", "lifetime_value": 15000.00, "join_date": "2023-11-10"},
    {"customer_id": "CUST-004", "name": "David Brown", "tier": "standard", "lifetime_value": 500.00, "join_date": "2025-01-05"},
    {"customer_id": "CUST-005", "name": "Eve Davis", "tier": "gold", "lifetime_value": 7500.00, "join_date": "2024-06-12"},
]


@step_handler("data_pipeline_dsl.step_handlers.extract_sales_data")
def extract_sales_data(context):
    """Extract sales data from simulated database."""
    dates = [r["date"] for r in SAMPLE_SALES_DATA]
    return {
        "records": SAMPLE_SALES_DATA,
        "extracted_at": datetime.now(timezone.utc).isoformat(),
        "source": "SalesDatabase",
        "total_amount": sum(r["amount"] for r in SAMPLE_SALES_DATA),
        "date_range": {"start_date": min(dates), "end_date": max(dates)},
    }


@step_handler("data_pipeline_dsl.step_handlers.extract_inventory_data")
def extract_inventory_data(context):
    """Extract inventory data from simulated warehouse system."""
    warehouses = list({r["warehouse"] for r in SAMPLE_INVENTORY_DATA})
    return {
        "records": SAMPLE_INVENTORY_DATA,
        "extracted_at": datetime.now(timezone.utc).isoformat(),
        "source": "InventorySystem",
        "total_quantity": sum(r["quantity_on_hand"] for r in SAMPLE_INVENTORY_DATA),
        "warehouses": warehouses,
        "products_tracked": len({r["product_id"] for r in SAMPLE_INVENTORY_DATA}),
    }


@step_handler("data_pipeline_dsl.step_handlers.extract_customer_data")
def extract_customer_data(context):
    """Extract customer data from simulated CRM."""
    tier_breakdown: dict[str, int] = {}
    for customer in SAMPLE_CUSTOMER_DATA:
        tier = customer["tier"]
        tier_breakdown[tier] = tier_breakdown.get(tier, 0) + 1

    total_ltv = sum(r["lifetime_value"] for r in SAMPLE_CUSTOMER_DATA)
    return {
        "records": SAMPLE_CUSTOMER_DATA,
        "extracted_at": datetime.now(timezone.utc).isoformat(),
        "source": "CRMSystem",
        "total_customers": len(SAMPLE_CUSTOMER_DATA),
        "total_lifetime_value": total_ltv,
        "tier_breakdown": tier_breakdown,
        "avg_lifetime_value": total_ltv / len(SAMPLE_CUSTOMER_DATA),
    }


@step_handler("data_pipeline_dsl.step_handlers.transform_sales")
@depends_on(extract_results="extract_sales_data")
def transform_sales(extract_results, context):
    """Transform extracted sales data for analytics."""
    if not extract_results:
        raise PermanentError(message="Sales extraction results not found", error_code="MISSING_EXTRACT_RESULTS")

    raw_records = extract_results.get("records", [])

    daily_groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in raw_records:
        daily_groups[record["date"]].append(record)

    daily_sales = {}
    for date, day_records in daily_groups.items():
        total = sum(r["amount"] for r in day_records)
        daily_sales[date] = {
            "total_amount": total,
            "order_count": len(day_records),
            "avg_order_value": total / len(day_records),
        }

    product_groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in raw_records:
        product_groups[record["product_id"]].append(record)

    product_sales = {}
    for product_id, product_records in product_groups.items():
        product_sales[product_id] = {
            "total_quantity": sum(r["quantity"] for r in product_records),
            "total_revenue": sum(r["amount"] for r in product_records),
            "order_count": len(product_records),
        }

    total_revenue = sum(r["amount"] for r in raw_records)

    return {
        "record_count": len(raw_records),
        "daily_sales": daily_sales,
        "product_sales": product_sales,
        "total_revenue": total_revenue,
        "transformation_type": "sales_analytics",
        "source": "extract_sales_data",
    }


@step_handler("data_pipeline_dsl.step_handlers.transform_inventory")
@depends_on(extract_results="extract_inventory_data")
def transform_inventory(extract_results, context):
    """Transform extracted inventory data for analytics."""
    if not extract_results:
        raise PermanentError(message="Inventory extraction results not found", error_code="MISSING_EXTRACT_RESULTS")

    raw_records = extract_results.get("records", [])

    warehouse_groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in raw_records:
        warehouse_groups[record["warehouse"]].append(record)

    warehouse_summary = {}
    for warehouse, wh_records in warehouse_groups.items():
        reorder_count = sum(1 for r in wh_records if r["quantity_on_hand"] <= r["reorder_point"])
        warehouse_summary[warehouse] = {
            "total_quantity": sum(r["quantity_on_hand"] for r in wh_records),
            "product_count": len({r["product_id"] for r in wh_records}),
            "reorder_alerts": reorder_count,
        }

    product_groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in raw_records:
        product_groups[record["product_id"]].append(record)

    product_inventory = {}
    for product_id, product_records in product_groups.items():
        total_qty = sum(r["quantity_on_hand"] for r in product_records)
        total_reorder = sum(r["reorder_point"] for r in product_records)
        product_inventory[product_id] = {
            "total_quantity": total_qty,
            "warehouse_count": len({r["warehouse"] for r in product_records}),
            "needs_reorder": total_qty < total_reorder,
        }

    total_on_hand = sum(r["quantity_on_hand"] for r in raw_records)
    reorder_alerts = sum(1 for data in product_inventory.values() if data["needs_reorder"])

    return {
        "record_count": len(raw_records),
        "warehouse_summary": warehouse_summary,
        "product_inventory": product_inventory,
        "total_quantity_on_hand": total_on_hand,
        "reorder_alerts": reorder_alerts,
        "transformation_type": "inventory_analytics",
        "source": "extract_inventory_data",
    }


@step_handler("data_pipeline_dsl.step_handlers.transform_customers")
@depends_on(extract_results="extract_customer_data")
def transform_customers(extract_results, context):
    """Transform extracted customer data for analytics."""
    if not extract_results:
        raise PermanentError(message="Customer extraction results not found", error_code="MISSING_EXTRACT_RESULTS")

    raw_records = extract_results.get("records", [])

    tier_groups: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for record in raw_records:
        tier_groups[record["tier"]].append(record)

    tier_analysis = {}
    for tier, tier_records in tier_groups.items():
        total_ltv = sum(r["lifetime_value"] for r in tier_records)
        tier_analysis[tier] = {
            "customer_count": len(tier_records),
            "total_lifetime_value": total_ltv,
            "avg_lifetime_value": total_ltv / len(tier_records),
        }

    value_segments = {
        "high_value": sum(1 for r in raw_records if r["lifetime_value"] >= 10000),
        "medium_value": sum(1 for r in raw_records if 1000 <= r["lifetime_value"] < 10000),
        "low_value": sum(1 for r in raw_records if r["lifetime_value"] < 1000),
    }

    total_ltv = sum(r["lifetime_value"] for r in raw_records)

    return {
        "record_count": len(raw_records),
        "tier_analysis": tier_analysis,
        "value_segments": value_segments,
        "total_lifetime_value": total_ltv,
        "avg_customer_value": total_ltv / len(raw_records) if raw_records else 0,
        "transformation_type": "customer_analytics",
        "source": "extract_customer_data",
    }


@step_handler("data_pipeline_dsl.step_handlers.aggregate_metrics")
@depends_on(
    sales_data="transform_sales",
    inventory_data="transform_inventory",
    customer_data="transform_customers",
)
def aggregate_metrics(sales_data, inventory_data, customer_data, context):
    """Aggregate metrics from all transform steps."""
    missing = []
    if not sales_data:
        missing.append("transform_sales")
    if not inventory_data:
        missing.append("transform_inventory")
    if not customer_data:
        missing.append("transform_customers")

    if missing:
        raise PermanentError(
            message=f"Missing transform results: {', '.join(missing)}",
            error_code="MISSING_TRANSFORM_RESULTS",
        )

    total_revenue = sales_data.get("total_revenue", 0)
    sales_record_count = sales_data.get("record_count", 0)
    total_inventory = inventory_data.get("total_quantity_on_hand", 0)
    reorder_alerts = inventory_data.get("reorder_alerts", 0)
    total_customers = customer_data.get("record_count", 0)
    total_ltv = customer_data.get("total_lifetime_value", 0)

    revenue_per_customer = total_revenue / total_customers if total_customers > 0 else 0
    inventory_turnover = total_revenue / total_inventory if total_inventory > 0 else 0

    return {
        "total_revenue": total_revenue,
        "total_inventory_quantity": total_inventory,
        "total_customers": total_customers,
        "total_customer_lifetime_value": total_ltv,
        "sales_transactions": sales_record_count,
        "inventory_reorder_alerts": reorder_alerts,
        "revenue_per_customer": round(revenue_per_customer, 2),
        "inventory_turnover_indicator": round(inventory_turnover, 4),
        "aggregation_complete": True,
        "sources_included": 3,
    }


@step_handler("data_pipeline_dsl.step_handlers.generate_insights")
@depends_on(metrics="aggregate_metrics")
def generate_insights(metrics, context):
    """Generate business insights from aggregated metrics."""
    if not metrics:
        raise PermanentError(message="Aggregated metrics not found", error_code="MISSING_AGGREGATE_RESULTS")

    insights = []
    revenue = metrics.get("total_revenue", 0)
    customers = metrics.get("total_customers", 0)
    revenue_per_customer = metrics.get("revenue_per_customer", 0)

    if revenue > 0:
        recommendation = "Consider upselling strategies" if revenue_per_customer < 500 else "Customer spend is healthy"
        insights.append({
            "category": "Revenue",
            "finding": f"Total revenue of ${revenue} with {customers} customers",
            "metric": revenue_per_customer,
            "recommendation": recommendation,
        })

    inventory_alerts = metrics.get("inventory_reorder_alerts", 0)
    if inventory_alerts > 0:
        insights.append({
            "category": "Inventory",
            "finding": f"{inventory_alerts} products need reordering",
            "metric": inventory_alerts,
            "recommendation": "Review reorder points and place purchase orders",
        })
    else:
        insights.append({
            "category": "Inventory",
            "finding": "All products above reorder points",
            "metric": 0,
            "recommendation": "Inventory levels are healthy",
        })

    total_ltv = metrics.get("total_customer_lifetime_value", 0)
    avg_ltv = total_ltv / customers if customers > 0 else 0
    recommendation = "Focus on retention programs" if avg_ltv > 3000 else "Increase customer engagement"
    insights.append({
        "category": "Customer Value",
        "finding": f"Average customer lifetime value: ${avg_ltv:.2f}",
        "metric": avg_ltv,
        "recommendation": recommendation,
    })

    # Health score
    score = 0
    if revenue_per_customer > 500:
        score += 40
    if inventory_alerts == 0:
        score += 30
    if avg_ltv > 3000:
        score += 30

    if score >= 80:
        rating = "Excellent"
    elif score >= 60:
        rating = "Good"
    elif score >= 40:
        rating = "Fair"
    else:
        rating = "Needs Improvement"

    health_score = {"score": score, "max_score": 100, "rating": rating}

    return {
        "insights": insights,
        "health_score": health_score,
        "total_metrics_analyzed": len(metrics.keys()),
        "pipeline_complete": True,
        "generated_at": datetime.now(timezone.utc).isoformat(),
    }


__all__ = [
    "extract_sales_data",
    "extract_inventory_data",
    "extract_customer_data",
    "transform_sales",
    "transform_inventory",
    "transform_customers",
    "aggregate_metrics",
    "generate_insights",
]
