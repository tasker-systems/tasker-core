# frozen_string_literal: true

# DSL mirror of DataPipeline::StepHandlers using block DSL.
#
# 8 handlers: extract_sales, extract_customers, extract_inventory,
#             transform_sales, transform_customers, transform_inventory,
#             aggregate_metrics, generate_insights

include TaskerCore::StepHandler::Functional

SAMPLE_SALES_DSL = [
  { order_id: 'ORD-001', date: '2025-11-01', product_id: 'PROD-A', quantity: 5, amount: 499.95 },
  { order_id: 'ORD-002', date: '2025-11-05', product_id: 'PROD-B', quantity: 3, amount: 299.97 },
  { order_id: 'ORD-003', date: '2025-11-10', product_id: 'PROD-A', quantity: 2, amount: 199.98 },
  { order_id: 'ORD-004', date: '2025-11-15', product_id: 'PROD-C', quantity: 10, amount: 1499.90 },
  { order_id: 'ORD-005', date: '2025-11-18', product_id: 'PROD-B', quantity: 7, amount: 699.93 }
].freeze

SAMPLE_CUSTOMERS_DSL = [
  { customer_id: 'CUST-001', name: 'Alice Johnson', tier: 'gold', lifetime_value: 5000.00, join_date: '2024-01-15' },
  { customer_id: 'CUST-002', name: 'Bob Smith', tier: 'silver', lifetime_value: 2500.00, join_date: '2024-03-20' },
  { customer_id: 'CUST-003', name: 'Carol White', tier: 'premium', lifetime_value: 15_000.00, join_date: '2023-11-10' },
  { customer_id: 'CUST-004', name: 'David Brown', tier: 'standard', lifetime_value: 500.00, join_date: '2025-01-05' },
  { customer_id: 'CUST-005', name: 'Eve Davis', tier: 'gold', lifetime_value: 7500.00, join_date: '2024-06-12' }
].freeze

SAMPLE_INVENTORY_DSL = [
  { product_id: 'PROD-A', sku: 'SKU-A-001', warehouse: 'WH-01', quantity_on_hand: 150, reorder_point: 50 },
  { product_id: 'PROD-B', sku: 'SKU-B-002', warehouse: 'WH-01', quantity_on_hand: 75, reorder_point: 25 },
  { product_id: 'PROD-C', sku: 'SKU-C-003', warehouse: 'WH-02', quantity_on_hand: 200, reorder_point: 100 },
  { product_id: 'PROD-A', sku: 'SKU-A-001', warehouse: 'WH-02', quantity_on_hand: 100, reorder_point: 50 },
  { product_id: 'PROD-B', sku: 'SKU-B-002', warehouse: 'WH-03', quantity_on_hand: 50, reorder_point: 25 }
].freeze

PipelineExtractSalesDslHandler = step_handler(
  'data_pipeline_dsl.step_handlers.extract_sales_data'
) do |context:|
  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      records: SAMPLE_SALES_DSL,
      extracted_at: Time.now.utc.iso8601,
      source: 'SalesDatabase',
      total_amount: SAMPLE_SALES_DSL.sum { |r| r[:amount] },
      date_range: {
        start_date: SAMPLE_SALES_DSL.map { |r| r[:date] }.min,
        end_date: SAMPLE_SALES_DSL.map { |r| r[:date] }.max
      }
    },
    metadata: {
      operation: 'extract_sales',
      source: 'SalesDatabase',
      records_extracted: SAMPLE_SALES_DSL.count
    }
  )
end

PipelineExtractCustomersDslHandler = step_handler(
  'data_pipeline_dsl.step_handlers.extract_customer_data'
) do |context:|
  tier_breakdown = SAMPLE_CUSTOMERS_DSL.group_by { |c| c[:tier] }.transform_values(&:count)

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      records: SAMPLE_CUSTOMERS_DSL,
      extracted_at: Time.now.utc.iso8601,
      source: 'CRMSystem',
      total_customers: SAMPLE_CUSTOMERS_DSL.count,
      total_lifetime_value: SAMPLE_CUSTOMERS_DSL.sum { |r| r[:lifetime_value] },
      tier_breakdown: tier_breakdown,
      avg_lifetime_value: SAMPLE_CUSTOMERS_DSL.sum { |r| r[:lifetime_value] } / SAMPLE_CUSTOMERS_DSL.count.to_f
    },
    metadata: {
      operation: 'extract_customers',
      source: 'CRMSystem',
      records_extracted: SAMPLE_CUSTOMERS_DSL.count,
      customer_tiers: tier_breakdown.keys
    }
  )
end

PipelineExtractInventoryDslHandler = step_handler(
  'data_pipeline_dsl.step_handlers.extract_inventory_data'
) do |context:|
  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      records: SAMPLE_INVENTORY_DSL,
      extracted_at: Time.now.utc.iso8601,
      source: 'InventorySystem',
      total_quantity: SAMPLE_INVENTORY_DSL.sum { |r| r[:quantity_on_hand] },
      warehouses: SAMPLE_INVENTORY_DSL.map { |r| r[:warehouse] }.uniq,
      products_tracked: SAMPLE_INVENTORY_DSL.map { |r| r[:product_id] }.uniq.count
    },
    metadata: {
      operation: 'extract_inventory',
      source: 'InventorySystem',
      records_extracted: SAMPLE_INVENTORY_DSL.count,
      warehouses: SAMPLE_INVENTORY_DSL.map { |r| r[:warehouse] }.uniq
    }
  )
end

PipelineTransformSalesDslHandler = step_handler(
  'data_pipeline_dsl.step_handlers.transform_sales',
  depends_on: { extract_results: 'extract_sales_data' }
) do |extract_results:, context:|
  raise TaskerCore::Errors::PermanentError.new('Sales extraction results not found', error_code: 'MISSING_EXTRACT_RESULTS') unless extract_results

  raw_records = extract_results['records'] || extract_results[:records]

  daily_sales = raw_records.group_by { |r| r[:date] || r['date'] }
                           .transform_values do |day_records|
    {
      total_amount: day_records.sum { |r| r[:amount] || r['amount'] || 0 },
      order_count: day_records.count,
      avg_order_value: day_records.sum { |r| r[:amount] || r['amount'] || 0 } / day_records.count.to_f
    }
  end

  product_sales = raw_records.group_by { |r| r[:product_id] || r['product_id'] }
                             .transform_values do |product_records|
    {
      total_quantity: product_records.sum { |r| r[:quantity] || r['quantity'] || 0 },
      total_revenue: product_records.sum { |r| r[:amount] || r['amount'] || 0 },
      order_count: product_records.count
    }
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      record_count: raw_records.count,
      daily_sales: daily_sales,
      product_sales: product_sales,
      total_revenue: raw_records.sum { |r| r[:amount] || r['amount'] || 0 },
      transformation_type: 'sales_analytics',
      source: 'extract_sales_data'
    },
    metadata: {
      operation: 'transform_sales',
      source_step: 'extract_sales_data',
      transformation_applied: true,
      record_count: raw_records.count
    }
  )
end

PipelineTransformCustomersDslHandler = step_handler(
  'data_pipeline_dsl.step_handlers.transform_customers',
  depends_on: { extract_results: 'extract_customer_data' }
) do |extract_results:, context:|
  raise TaskerCore::Errors::PermanentError.new('Customer extraction results not found', error_code: 'MISSING_EXTRACT_RESULTS') unless extract_results

  raw_records = extract_results['records'] || extract_results[:records]

  tier_analysis = raw_records.group_by { |r| r[:tier] || r['tier'] }
                             .transform_values do |tier_records|
    {
      count: tier_records.count,
      total_value: tier_records.sum { |r| r[:lifetime_value] || r['lifetime_value'] || 0 },
      avg_value: tier_records.sum { |r| r[:lifetime_value] || r['lifetime_value'] || 0 } / tier_records.count.to_f
    }
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      record_count: raw_records.count,
      tier_analysis: tier_analysis,
      transformation_type: 'customer_analytics',
      source: 'extract_customer_data'
    },
    metadata: {
      operation: 'transform_customers',
      source_step: 'extract_customer_data',
      transformation_applied: true,
      record_count: raw_records.count
    }
  )
end

PipelineTransformInventoryDslHandler = step_handler(
  'data_pipeline_dsl.step_handlers.transform_inventory',
  depends_on: { extract_results: 'extract_inventory_data' }
) do |extract_results:, context:|
  raise TaskerCore::Errors::PermanentError.new('Inventory extraction results not found', error_code: 'MISSING_EXTRACT_RESULTS') unless extract_results

  raw_records = extract_results['records'] || extract_results[:records]

  warehouse_analysis = raw_records.group_by { |r| r[:warehouse] || r['warehouse'] }
                                  .transform_values do |wh_records|
    {
      total_quantity: wh_records.sum { |r| r[:quantity_on_hand] || r['quantity_on_hand'] || 0 },
      product_count: wh_records.count,
      below_reorder: wh_records.count { |r| (r[:quantity_on_hand] || r['quantity_on_hand'] || 0) < (r[:reorder_point] || r['reorder_point'] || 0) }
    }
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      record_count: raw_records.count,
      warehouse_analysis: warehouse_analysis,
      transformation_type: 'inventory_analytics',
      source: 'extract_inventory_data'
    },
    metadata: {
      operation: 'transform_inventory',
      source_step: 'extract_inventory_data',
      transformation_applied: true,
      record_count: raw_records.count
    }
  )
end

PipelineAggregateMetricsDslHandler = step_handler(
  'data_pipeline_dsl.step_handlers.aggregate_metrics',
  depends_on: {
    sales_data: 'transform_sales',
    customer_data: 'transform_customers',
    inventory_data: 'transform_inventory'
  }
) do |sales_data:, customer_data:, inventory_data:, context:|
  raise TaskerCore::Errors::PermanentError, 'Sales transform results not found' unless sales_data
  raise TaskerCore::Errors::PermanentError, 'Customer transform results not found' unless customer_data
  raise TaskerCore::Errors::PermanentError, 'Inventory transform results not found' unless inventory_data

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      sales_summary: {
        total_revenue: sales_data['total_revenue'] || sales_data[:total_revenue],
        record_count: sales_data['record_count'] || sales_data[:record_count]
      },
      customer_summary: {
        record_count: customer_data['record_count'] || customer_data[:record_count]
      },
      inventory_summary: {
        record_count: inventory_data['record_count'] || inventory_data[:record_count]
      },
      aggregated_at: Time.now.utc.iso8601
    },
    metadata: {
      operation: 'aggregate_metrics',
      sources: %w[transform_sales transform_customers transform_inventory]
    }
  )
end

PipelineGenerateInsightsDslHandler = step_handler(
  'data_pipeline_dsl.step_handlers.generate_insights',
  depends_on: { metrics: 'aggregate_metrics' }
) do |metrics:, context:|
  raise TaskerCore::Errors::PermanentError, 'Aggregated metrics not found' unless metrics

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      insights_generated: true,
      report_type: 'automated',
      generated_at: Time.now.utc.iso8601,
      metrics_snapshot: metrics
    },
    metadata: {
      operation: 'generate_insights',
      source_step: 'aggregate_metrics'
    }
  )
end
