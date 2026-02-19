/**
 * Data Pipeline Analytics DSL Step Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based data pipeline handlers.
 * Produces identical output for parity testing.
 */

import { ErrorType } from '../../../../../../src/types/error-type.js';
import { StepHandlerResult } from '../../../../../../src/types/step-handler-result.js';
import {
  PermanentError,
  defineHandler,
} from '../../../../../../src/handler/functional.js';

// =============================================================================
// Types (same as verbose)
// =============================================================================

interface SalesRecord {
  order_id: string;
  date: string;
  product_id: string;
  quantity: number;
  amount: number;
}

interface InventoryRecord {
  product_id: string;
  sku: string;
  warehouse: string;
  quantity_on_hand: number;
  reorder_point: number;
}

interface CustomerRecord {
  customer_id: string;
  name: string;
  tier: string;
  lifetime_value: number;
  join_date: string;
}

interface DailySales {
  total_amount: number;
  order_count: number;
  avg_order_value: number;
}

interface ProductSales {
  total_quantity: number;
  total_revenue: number;
  order_count: number;
}

interface WarehouseSummary {
  total_quantity: number;
  product_count: number;
  reorder_alerts: number;
}

interface ProductInventory {
  total_quantity: number;
  warehouse_count: number;
  needs_reorder: boolean;
}

interface TierAnalysis {
  customer_count: number;
  total_lifetime_value: number;
  avg_lifetime_value: number;
}

interface ValueSegments {
  high_value: number;
  medium_value: number;
  low_value: number;
}

interface Insight {
  category: string;
  finding: string;
  metric: number;
  recommendation: string;
}

interface HealthScore {
  score: number;
  max_score: number;
  rating: string;
}

// =============================================================================
// Sample Data (same as verbose)
// =============================================================================

const SAMPLE_SALES_DATA: SalesRecord[] = [
  { order_id: 'ORD-001', date: '2025-11-01', product_id: 'PROD-A', quantity: 5, amount: 499.95 },
  { order_id: 'ORD-002', date: '2025-11-05', product_id: 'PROD-B', quantity: 3, amount: 299.97 },
  { order_id: 'ORD-003', date: '2025-11-10', product_id: 'PROD-A', quantity: 2, amount: 199.98 },
  { order_id: 'ORD-004', date: '2025-11-15', product_id: 'PROD-C', quantity: 10, amount: 1499.9 },
  { order_id: 'ORD-005', date: '2025-11-18', product_id: 'PROD-B', quantity: 7, amount: 699.93 },
];

const SAMPLE_INVENTORY_DATA: InventoryRecord[] = [
  { product_id: 'PROD-A', sku: 'SKU-A-001', warehouse: 'WH-01', quantity_on_hand: 150, reorder_point: 50 },
  { product_id: 'PROD-B', sku: 'SKU-B-002', warehouse: 'WH-01', quantity_on_hand: 75, reorder_point: 25 },
  { product_id: 'PROD-C', sku: 'SKU-C-003', warehouse: 'WH-02', quantity_on_hand: 200, reorder_point: 100 },
  { product_id: 'PROD-A', sku: 'SKU-A-001', warehouse: 'WH-02', quantity_on_hand: 100, reorder_point: 50 },
  { product_id: 'PROD-B', sku: 'SKU-B-002', warehouse: 'WH-03', quantity_on_hand: 50, reorder_point: 25 },
];

const SAMPLE_CUSTOMER_DATA: CustomerRecord[] = [
  { customer_id: 'CUST-001', name: 'Alice Johnson', tier: 'gold', lifetime_value: 5000.0, join_date: '2024-01-15' },
  { customer_id: 'CUST-002', name: 'Bob Smith', tier: 'silver', lifetime_value: 2500.0, join_date: '2024-03-20' },
  { customer_id: 'CUST-003', name: 'Carol White', tier: 'premium', lifetime_value: 15000.0, join_date: '2023-11-10' },
  { customer_id: 'CUST-004', name: 'David Brown', tier: 'standard', lifetime_value: 500.0, join_date: '2025-01-05' },
  { customer_id: 'CUST-005', name: 'Eve Davis', tier: 'gold', lifetime_value: 7500.0, join_date: '2024-06-12' },
];

// =============================================================================
// Extract Handlers
// =============================================================================

export const ExtractSalesDataDslHandler = defineHandler(
  'DataPipelineDsl.StepHandlers.ExtractSalesDataDslHandler',
  {},
  async () => {
    const dates = SAMPLE_SALES_DATA.map((r) => r.date);
    const totalAmount = SAMPLE_SALES_DATA.reduce((sum, r) => sum + r.amount, 0);

    return {
      records: SAMPLE_SALES_DATA,
      extracted_at: new Date().toISOString(),
      source: 'SalesDatabase',
      total_amount: totalAmount,
      date_range: {
        start_date: dates.reduce((a, b) => (a < b ? a : b)),
        end_date: dates.reduce((a, b) => (a > b ? a : b)),
      },
    };
  }
);

export const ExtractInventoryDataDslHandler = defineHandler(
  'DataPipelineDsl.StepHandlers.ExtractInventoryDataDslHandler',
  {},
  async () => {
    const warehouses = [...new Set(SAMPLE_INVENTORY_DATA.map((r) => r.warehouse))];
    const totalQuantity = SAMPLE_INVENTORY_DATA.reduce((sum, r) => sum + r.quantity_on_hand, 0);
    const productsTracked = new Set(SAMPLE_INVENTORY_DATA.map((r) => r.product_id)).size;

    return {
      records: SAMPLE_INVENTORY_DATA,
      extracted_at: new Date().toISOString(),
      source: 'InventorySystem',
      total_quantity: totalQuantity,
      warehouses,
      products_tracked: productsTracked,
    };
  }
);

export const ExtractCustomerDataDslHandler = defineHandler(
  'DataPipelineDsl.StepHandlers.ExtractCustomerDataDslHandler',
  {},
  async () => {
    const tierBreakdown: Record<string, number> = {};
    for (const customer of SAMPLE_CUSTOMER_DATA) {
      tierBreakdown[customer.tier] = (tierBreakdown[customer.tier] || 0) + 1;
    }

    const totalLtv = SAMPLE_CUSTOMER_DATA.reduce((sum, r) => sum + r.lifetime_value, 0);

    return {
      records: SAMPLE_CUSTOMER_DATA,
      extracted_at: new Date().toISOString(),
      source: 'CRMSystem',
      total_customers: SAMPLE_CUSTOMER_DATA.length,
      total_lifetime_value: totalLtv,
      tier_breakdown: tierBreakdown,
      avg_lifetime_value: totalLtv / SAMPLE_CUSTOMER_DATA.length,
    };
  }
);

// =============================================================================
// Transform Handlers
// =============================================================================

export const TransformSalesDslHandler = defineHandler(
  'DataPipelineDsl.StepHandlers.TransformSalesDslHandler',
  { depends: { extractResults: 'extract_sales_data' } },
  async ({ extractResults }) => {
    const results = extractResults as Record<string, unknown> | null;

    if (!results) {
      return StepHandlerResult.failure('Sales extraction results not found', ErrorType.PERMANENT_ERROR, false);
    }

    const rawRecords = (results.records || []) as SalesRecord[];

    const dailyGroups = new Map<string, SalesRecord[]>();
    for (const record of rawRecords) {
      const existing = dailyGroups.get(record.date) || [];
      existing.push(record);
      dailyGroups.set(record.date, existing);
    }

    const dailySales: Record<string, DailySales> = {};
    for (const [date, dayRecords] of dailyGroups) {
      const total = dayRecords.reduce((sum, r) => sum + r.amount, 0);
      dailySales[date] = {
        total_amount: total,
        order_count: dayRecords.length,
        avg_order_value: total / dayRecords.length,
      };
    }

    const productGroups = new Map<string, SalesRecord[]>();
    for (const record of rawRecords) {
      const existing = productGroups.get(record.product_id) || [];
      existing.push(record);
      productGroups.set(record.product_id, existing);
    }

    const productSales: Record<string, ProductSales> = {};
    for (const [productId, productRecords] of productGroups) {
      productSales[productId] = {
        total_quantity: productRecords.reduce((sum, r) => sum + r.quantity, 0),
        total_revenue: productRecords.reduce((sum, r) => sum + r.amount, 0),
        order_count: productRecords.length,
      };
    }

    const totalRevenue = rawRecords.reduce((sum, r) => sum + r.amount, 0);

    return {
      record_count: rawRecords.length,
      daily_sales: dailySales,
      product_sales: productSales,
      total_revenue: totalRevenue,
      transformation_type: 'sales_analytics',
      source: 'extract_sales_data',
    };
  }
);

export const TransformInventoryDslHandler = defineHandler(
  'DataPipelineDsl.StepHandlers.TransformInventoryDslHandler',
  { depends: { extractResults: 'extract_inventory_data' } },
  async ({ extractResults }) => {
    const results = extractResults as Record<string, unknown> | null;

    if (!results) {
      return StepHandlerResult.failure('Inventory extraction results not found', ErrorType.PERMANENT_ERROR, false);
    }

    const rawRecords = (results.records || []) as InventoryRecord[];

    const warehouseGroups = new Map<string, InventoryRecord[]>();
    for (const record of rawRecords) {
      const existing = warehouseGroups.get(record.warehouse) || [];
      existing.push(record);
      warehouseGroups.set(record.warehouse, existing);
    }

    const warehouseSummary: Record<string, WarehouseSummary> = {};
    for (const [warehouse, whRecords] of warehouseGroups) {
      const reorderCount = whRecords.filter((r) => r.quantity_on_hand <= r.reorder_point).length;
      warehouseSummary[warehouse] = {
        total_quantity: whRecords.reduce((sum, r) => sum + r.quantity_on_hand, 0),
        product_count: new Set(whRecords.map((r) => r.product_id)).size,
        reorder_alerts: reorderCount,
      };
    }

    const productGroups = new Map<string, InventoryRecord[]>();
    for (const record of rawRecords) {
      const existing = productGroups.get(record.product_id) || [];
      existing.push(record);
      productGroups.set(record.product_id, existing);
    }

    const productInventory: Record<string, ProductInventory> = {};
    for (const [productId, productRecords] of productGroups) {
      const totalQty = productRecords.reduce((sum, r) => sum + r.quantity_on_hand, 0);
      const totalReorder = productRecords.reduce((sum, r) => sum + r.reorder_point, 0);
      productInventory[productId] = {
        total_quantity: totalQty,
        warehouse_count: new Set(productRecords.map((r) => r.warehouse)).size,
        needs_reorder: totalQty < totalReorder,
      };
    }

    const totalOnHand = rawRecords.reduce((sum, r) => sum + r.quantity_on_hand, 0);
    const reorderAlerts = Object.values(productInventory).filter((data) => data.needs_reorder).length;

    return {
      record_count: rawRecords.length,
      warehouse_summary: warehouseSummary,
      product_inventory: productInventory,
      total_quantity_on_hand: totalOnHand,
      reorder_alerts: reorderAlerts,
      transformation_type: 'inventory_analytics',
      source: 'extract_inventory_data',
    };
  }
);

export const TransformCustomersDslHandler = defineHandler(
  'DataPipelineDsl.StepHandlers.TransformCustomersDslHandler',
  { depends: { extractResults: 'extract_customer_data' } },
  async ({ extractResults }) => {
    const results = extractResults as Record<string, unknown> | null;

    if (!results) {
      return StepHandlerResult.failure('Customer extraction results not found', ErrorType.PERMANENT_ERROR, false);
    }

    const rawRecords = (results.records || []) as CustomerRecord[];

    const tierGroups = new Map<string, CustomerRecord[]>();
    for (const record of rawRecords) {
      const existing = tierGroups.get(record.tier) || [];
      existing.push(record);
      tierGroups.set(record.tier, existing);
    }

    const tierAnalysis: Record<string, TierAnalysis> = {};
    for (const [tier, tierRecords] of tierGroups) {
      const totalLtv = tierRecords.reduce((sum, r) => sum + r.lifetime_value, 0);
      tierAnalysis[tier] = {
        customer_count: tierRecords.length,
        total_lifetime_value: totalLtv,
        avg_lifetime_value: totalLtv / tierRecords.length,
      };
    }

    const valueSegments: ValueSegments = {
      high_value: rawRecords.filter((r) => r.lifetime_value >= 10000).length,
      medium_value: rawRecords.filter((r) => r.lifetime_value >= 1000 && r.lifetime_value < 10000).length,
      low_value: rawRecords.filter((r) => r.lifetime_value < 1000).length,
    };

    const totalLtv = rawRecords.reduce((sum, r) => sum + r.lifetime_value, 0);

    return {
      record_count: rawRecords.length,
      tier_analysis: tierAnalysis,
      value_segments: valueSegments,
      total_lifetime_value: totalLtv,
      avg_customer_value: rawRecords.length > 0 ? totalLtv / rawRecords.length : 0,
      transformation_type: 'customer_analytics',
      source: 'extract_customer_data',
    };
  }
);

// =============================================================================
// Aggregate Handler
// =============================================================================

export const AggregateMetricsDslHandler = defineHandler(
  'DataPipelineDsl.StepHandlers.AggregateMetricsDslHandler',
  {
    depends: {
      salesData: 'transform_sales',
      inventoryData: 'transform_inventory',
      customerData: 'transform_customers',
    },
  },
  async ({ salesData, inventoryData, customerData }) => {
    const sales = salesData as Record<string, unknown> | null;
    const inventory = inventoryData as Record<string, unknown> | null;
    const customers = customerData as Record<string, unknown> | null;

    const missing: string[] = [];
    if (!sales) missing.push('transform_sales');
    if (!inventory) missing.push('transform_inventory');
    if (!customers) missing.push('transform_customers');

    if (missing.length > 0) {
      return StepHandlerResult.failure(
        `Missing transform results: ${missing.join(', ')}`,
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    const totalRevenue = (sales?.total_revenue as number) || 0;
    const salesRecordCount = (sales?.record_count as number) || 0;
    const totalInventory = (inventory?.total_quantity_on_hand as number) || 0;
    const reorderAlerts = (inventory?.reorder_alerts as number) || 0;
    const totalCustomers = (customers?.record_count as number) || 0;
    const totalLtv = (customers?.total_lifetime_value as number) || 0;

    const revenuePerCustomer = totalCustomers > 0 ? totalRevenue / totalCustomers : 0;
    const inventoryTurnover = totalInventory > 0 ? totalRevenue / totalInventory : 0;

    return {
      total_revenue: totalRevenue,
      total_inventory_quantity: totalInventory,
      total_customers: totalCustomers,
      total_customer_lifetime_value: totalLtv,
      sales_transactions: salesRecordCount,
      inventory_reorder_alerts: reorderAlerts,
      revenue_per_customer: Math.round(revenuePerCustomer * 100) / 100,
      inventory_turnover_indicator: Math.round(inventoryTurnover * 10000) / 10000,
      aggregation_complete: true,
      sources_included: 3,
    };
  }
);

// =============================================================================
// Generate Insights Handler
// =============================================================================

function calculateHealthScore(
  revenuePerCustomer: number,
  inventoryAlerts: number,
  avgLtv: number
): HealthScore {
  let score = 0;
  if (revenuePerCustomer > 500) score += 40;
  if (inventoryAlerts === 0) score += 30;
  if (avgLtv > 3000) score += 30;

  let rating: string;
  if (score >= 80) rating = 'Excellent';
  else if (score >= 60) rating = 'Good';
  else if (score >= 40) rating = 'Fair';
  else rating = 'Needs Improvement';

  return { score, max_score: 100, rating };
}

export const GenerateInsightsDslHandler = defineHandler(
  'DataPipelineDsl.StepHandlers.GenerateInsightsDslHandler',
  { depends: { metrics: 'aggregate_metrics' } },
  async ({ metrics: metricsInput }) => {
    const metrics = metricsInput as Record<string, unknown> | null;

    if (!metrics) {
      return StepHandlerResult.failure('Aggregated metrics not found', ErrorType.PERMANENT_ERROR, false);
    }

    const insights: Insight[] = [];

    const revenue = (metrics.total_revenue as number) || 0;
    const customers = (metrics.total_customers as number) || 0;
    const revenuePerCustomer = (metrics.revenue_per_customer as number) || 0;

    if (revenue > 0) {
      insights.push({
        category: 'Revenue',
        finding: `Total revenue of $${revenue} with ${customers} customers`,
        metric: revenuePerCustomer,
        recommendation:
          revenuePerCustomer < 500 ? 'Consider upselling strategies' : 'Customer spend is healthy',
      });
    }

    const inventoryAlerts = (metrics.inventory_reorder_alerts as number) || 0;
    if (inventoryAlerts > 0) {
      insights.push({
        category: 'Inventory',
        finding: `${inventoryAlerts} products need reordering`,
        metric: inventoryAlerts,
        recommendation: 'Review reorder points and place purchase orders',
      });
    } else {
      insights.push({
        category: 'Inventory',
        finding: 'All products above reorder points',
        metric: 0,
        recommendation: 'Inventory levels are healthy',
      });
    }

    const totalLtv = (metrics.total_customer_lifetime_value as number) || 0;
    const avgLtv = customers > 0 ? totalLtv / customers : 0;

    insights.push({
      category: 'Customer Value',
      finding: `Average customer lifetime value: $${avgLtv.toFixed(2)}`,
      metric: avgLtv,
      recommendation:
        avgLtv > 3000 ? 'Focus on retention programs' : 'Increase customer engagement',
    });

    const healthScore = calculateHealthScore(revenuePerCustomer, inventoryAlerts, avgLtv);

    return {
      insights,
      health_score: healthScore,
      total_metrics_analyzed: Object.keys(metrics).length,
      pipeline_complete: true,
      generated_at: new Date().toISOString(),
    };
  }
);
