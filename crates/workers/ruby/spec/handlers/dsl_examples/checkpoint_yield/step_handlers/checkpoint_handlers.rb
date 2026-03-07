# frozen_string_literal: true

# DSL mirrors of Examples::CheckpointYield handlers.
#
# The analyzer uses batch_analyzer DSL. The worker and aggregator are
# class-based because they need direct access to Batchable instance
# methods (checkpoint_yield, batch_worker_success, handle_no_op_worker,
# detect_aggregation_scenario, etc.).

include TaskerCore::StepHandler::Functional # rubocop:disable Style/MixinUsage

# DSL mirror of Examples::CheckpointYield::AnalyzerHandler
AnalyzeItemsDslHandler = batch_analyzer 'checkpoint_yield_dsl.step_handlers.analyze_items',
                                        worker_template: 'checkpoint_yield_batch_dsl',
                                        inputs: [:total_items] do |total_items:, context: nil| # rubocop:disable Lint/UnusedBlockArgument
  total = total_items || 100

  if total <= 0
    next TaskerCore::Types::StepHandlerCallResult.success(
      result: { 'batch_processing_outcome' => 'no_batches', 'reason' => 'no_items_to_process' }
    )
  end

  TaskerCore::StepHandler::Functional::BatchConfig.new(
    total_items: total,
    batch_size: total,
    metadata: {
      'test_type' => 'checkpoint_yield',
      'items_per_batch' => total
    }
  )
end

# Class-based DSL mirror of Examples::CheckpointYield::WorkerHandler
class CheckpointYieldBatchDslHandler < TaskerCore::StepHandler::Batchable
  def handler_name
    'checkpoint_yield_dsl.step_handlers.checkpoint_yield_batch'
  end

  def call(context)
    batch_ctx = get_batch_context(context)
    return batch_worker_failure('No batch inputs found') unless batch_ctx

    no_op_result = handle_no_op_worker(batch_ctx)
    return no_op_result if no_op_result

    items_per_checkpoint = context.get_input_or(
      'items_per_checkpoint', context.get_config('items_per_checkpoint') || 25
    )
    fail_after_items = context.get_input('fail_after_items')
    fail_on_attempt = context.get_input_or('fail_on_attempt', 1)
    permanent_failure = context.get_input_or('permanent_failure', false)

    if batch_ctx.has_checkpoint?
      start_cursor = batch_ctx.checkpoint_cursor
      accumulated = batch_ctx.accumulated_results || { 'running_total' => 0, 'item_ids' => [] }
      total_processed = batch_ctx.checkpoint_items_processed
    else
      start_cursor = batch_ctx.start_cursor
      accumulated = { 'running_total' => 0, 'item_ids' => [] }
      total_processed = 0
    end

    end_cursor = batch_ctx.end_cursor
    current_attempt = context.retry_count + 1
    current_cursor = start_cursor
    items_in_chunk = 0

    while current_cursor < end_cursor
      if fail_after_items && total_processed >= fail_after_items && current_attempt == fail_on_attempt
        return inject_failure(total_processed, current_cursor, permanent_failure)
      end

      accumulated['running_total'] += (current_cursor + 1)
      accumulated['item_ids'] << "item_#{current_cursor.to_s.rjust(4, '0')}"

      current_cursor += 1
      items_in_chunk += 1
      total_processed += 1

      next unless items_in_chunk >= items_per_checkpoint && current_cursor < end_cursor

      return checkpoint_yield(
        cursor: current_cursor,
        items_processed: total_processed,
        accumulated_results: accumulated
      )
    end

    batch_worker_success(
      items_processed: total_processed,
      items_succeeded: total_processed,
      items_failed: 0,
      results: accumulated.merge(
        'final_cursor' => current_cursor,
        'checkpoints_used' => total_processed / items_per_checkpoint
      )
    )
  end

  private

  def inject_failure(items_processed, cursor, permanent)
    error_type = permanent ? 'PermanentError' : 'RetryableError'
    failure_type = permanent ? 'permanent' : 'transient'

    TaskerCore::Types::StepHandlerCallResult.error(
      message: "Injected #{failure_type} failure after #{items_processed} items",
      error_type: error_type,
      retryable: !permanent,
      metadata: {
        items_processed: items_processed,
        cursor_at_failure: cursor,
        failure_type: failure_type
      }
    )
  end
end

# Class-based DSL mirror of Examples::CheckpointYield::AggregatorHandler
class AggregateResultsDslHandler < TaskerCore::StepHandler::Batchable
  def handler_name
    'checkpoint_yield_dsl.step_handlers.aggregate_results'
  end

  def call(context)
    scenario = detect_aggregation_scenario(
      context.dependency_results,
      'analyze_items_dsl',
      'checkpoint_yield_batch_dsl_'
    )

    if scenario.no_batches?
      return no_batches_aggregation_result(
        'total_processed' => 0,
        'running_total' => 0,
        'test_passed' => true,
        'scenario' => 'no_batches'
      )
    end

    total_processed = 0
    running_total = 0
    all_item_ids = []
    checkpoints_used = 0

    scenario.batch_results.each_value do |result|
      next unless result

      total_processed += result['items_processed'] || 0
      batch_data = result['results'] || {}
      running_total += batch_data['running_total'] || 0
      all_item_ids.concat(batch_data['item_ids'] || [])
      checkpoints_used += batch_data['checkpoints_used'] || 0
    end

    TaskerCore::Types::StepHandlerCallResult.success(
      result: {
        'total_processed' => total_processed,
        'running_total' => running_total,
        'item_count' => all_item_ids.length,
        'checkpoints_used' => checkpoints_used,
        'worker_count' => scenario.worker_count,
        'test_passed' => true,
        'scenario' => 'with_batches'
      }
    )
  end
end
