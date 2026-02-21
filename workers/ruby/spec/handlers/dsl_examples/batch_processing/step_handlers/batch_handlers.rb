# frozen_string_literal: true

require 'csv'

# DSL mirrors of BatchProcessing::StepHandlers using the functional API.
#
# The analyzer uses batch_analyzer DSL which auto-wraps BatchConfig into
# Batchable outcomes. The batch processor and aggregator are class-based
# because they need direct access to Batchable instance methods
# (handle_no_op_worker, get_batch_context, etc.) that aren't available
# in the functional block scope.

include TaskerCore::StepHandler::Functional # rubocop:disable Style/MixinUsage

# DSL mirror of BatchProcessing::StepHandlers::CsvAnalyzerHandler
AnalyzeCsvDslHandler = batch_analyzer 'csv_processing_dsl.step_handlers.analyze_csv',
                                      worker_template: 'process_csv_batch_dsl',
                                      inputs: [:csv_file_path] do |csv_file_path:, context: nil| # rubocop:disable Lint/UnusedBlockArgument
  raise TaskerCore::Errors::PermanentError, 'csv_file_path required in task context' if csv_file_path.nil?

  max_size_bytes = 100 * 1024 * 1024
  file_size = File.size(csv_file_path)
  if file_size > max_size_bytes
    raise TaskerCore::Errors::PermanentError,
          "CSV file too large (#{file_size} bytes, max #{max_size_bytes} bytes)"
  end

  total_rows = CSV.read(csv_file_path, headers: true).length

  if total_rows.zero?
    next TaskerCore::Types::StepHandlerCallResult.success(
      result: { 'batch_processing_outcome' => 'no_batches', 'reason' => 'dataset_too_small',
                'metadata' => { 'total_rows' => 0 } }
    )
  end

  TaskerCore::StepHandler::Functional::BatchConfig.new(
    total_items: total_rows,
    batch_size: 200,
    metadata: {
      'csv_file_path' => csv_file_path,
      'total_rows' => total_rows
    }
  )
end

# Class-based DSL mirror of BatchProcessing::StepHandlers::CsvBatchProcessorHandler
# Uses class form because it needs direct access to Batchable instance methods.
class ProcessCsvBatchDslHandler < TaskerCore::StepHandler::Batchable
  def handler_name
    'csv_processing_dsl.step_handlers.process_csv_batch'
  end

  def call(context)
    batch_context = get_batch_context(context)
    no_op_result = handle_no_op_worker(batch_context)
    return no_op_result if no_op_result

    csv_result = context.get_dependency_result('analyze_csv_dsl')
    csv_file_path = csv_result&.dig('csv_file_path') || csv_result&.dig(:csv_file_path)
    raise ArgumentError, 'csv_file_path not found in analyze_csv results' if csv_file_path.nil?

    validate_csv_path(csv_file_path)

    start_row = batch_context.start_cursor
    end_row = batch_context.end_cursor
    batch_id = batch_context.batch_id

    metrics = process_csv_range(csv_file_path, start_row, end_row)

    success(
      result: {
        'batch_id' => batch_id,
        'start_row' => start_row,
        'end_row' => end_row,
        'processed_count' => metrics[:processed_count],
        'total_inventory_value' => metrics[:total_inventory_value],
        'category_counts' => metrics[:category_counts],
        'max_price' => metrics[:max_price],
        'max_price_product' => metrics[:max_price_product],
        'average_rating' => metrics[:average_rating]
      }
    )
  end

  private

  def validate_csv_path(file_path)
    raise ArgumentError, "Path traversal detected: #{file_path}" if file_path.include?('..')
    raise ArgumentError, "CSV file not found: #{file_path}" unless File.exist?(file_path)
    raise ArgumentError, "CSV file not readable: #{file_path}" unless File.readable?(file_path)
  end

  def process_csv_range(csv_file_path, start_row, end_row)
    processed_count = 0
    total_inventory_value = 0.0
    category_counts = Hash.new(0)
    max_price = 0.0
    max_price_product = nil
    total_rating = 0.0

    CSV.foreach(csv_file_path, headers: true).with_index do |row, idx|
      next if idx < start_row
      break if idx >= end_row

      price = row['price'].to_f
      stock = row['stock'].to_i

      total_inventory_value += (price * stock)
      category_counts[row['category']] += 1

      if price > max_price
        max_price = price
        max_price_product = row['title']
      end

      total_rating += row['rating'].to_f
      processed_count += 1
    end

    avg_rating = processed_count.positive? ? (total_rating / processed_count).round(2) : 0.0

    {
      processed_count: processed_count,
      total_inventory_value: total_inventory_value.round(2),
      category_counts: category_counts,
      max_price: max_price,
      max_price_product: max_price_product,
      average_rating: avg_rating
    }
  end
end

# Class-based DSL mirror of BatchProcessing::StepHandlers::CsvResultsAggregatorHandler
class AggregateCsvResultsDslHandler < TaskerCore::StepHandler::Batchable
  def handler_name
    'csv_processing_dsl.step_handlers.aggregate_csv_results'
  end

  def call(context)
    scenario = detect_aggregation_scenario(context.dependency_results, 'analyze_csv_dsl', 'process_csv_batch_dsl_')

    aggregate_batch_worker_results(
      scenario,
      zero_metrics: {
        'total_processed' => 0,
        'total_inventory_value' => 0.0,
        'category_counts' => {},
        'max_price' => 0.0,
        'max_price_product' => nil,
        'overall_average_rating' => 0.0
      }
    ) do |batch_results|
      aggregate_batch_results(batch_results)
    end
  end

  private

  def aggregate_batch_results(batch_results)
    total_processed = 0
    total_inventory_value = 0.0
    category_counts = Hash.new(0)
    max_price = 0.0
    max_price_product = nil
    total_weighted_rating = 0.0

    batch_results.each_value do |result|
      processed = result['processed_count'] || 0
      total_processed += processed
      total_inventory_value += result['total_inventory_value'] || 0.0

      result['category_counts']&.each do |category, count|
        category_counts[category] += count
      end

      batch_max_price = result['max_price'] || 0.0
      if batch_max_price > max_price
        max_price = batch_max_price
        max_price_product = result['max_price_product']
      end

      total_weighted_rating += ((result['average_rating'] || 0.0) * processed)
    end

    avg = total_processed.positive? ? (total_weighted_rating / total_processed).round(2) : 0.0

    {
      'total_processed' => total_processed,
      'total_inventory_value' => total_inventory_value.round(2),
      'category_counts' => category_counts,
      'max_price' => max_price,
      'max_price_product' => max_price_product,
      'overall_average_rating' => avg
    }
  end
end
