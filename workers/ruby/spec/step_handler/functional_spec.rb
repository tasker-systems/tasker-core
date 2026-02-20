# frozen_string_literal: true

require 'spec_helper'

RSpec.describe TaskerCore::StepHandler::Functional do
  include described_class

  # ============================================================================
  # Test Helpers
  # ============================================================================

  def make_context(dependency_results: {}, input_data: {}, step_config: {})
    task_wrapper = double('task',
                          task_uuid: 'task-456',
                          context: input_data.transform_keys(&:to_s))

    workflow_step = double('workflow_step',
                           workflow_step_uuid: 'step-789',
                           name: 'test_step',
                           inputs: input_data.transform_keys(&:to_s),
                           attempts: 0,
                           max_attempts: 3,
                           checkpoint: nil,
                           retryable: true)

    dep_wrapper = double('dependency_results')
    allow(dep_wrapper).to receive(:get_results) do |step_name|
      dep = dependency_results[step_name.to_s] || dependency_results[step_name.to_sym]
      dep&.dig(:result) || dep&.dig('result')
    end

    step_def_handler = double('handler', callable: 'test_handler', initialization: step_config.transform_keys(&:to_s))
    step_def = double('step_definition', handler: step_def_handler)

    double('step_data',
           is_a?: false,
           task: task_wrapper,
           workflow_step: workflow_step,
           dependency_results: dep_wrapper,
           step_definition: step_def)

    # Build a context that uses our doubles directly
    ctx = TaskerCore::Types::StepContext.allocate
    ctx.instance_variable_set(:@task, task_wrapper)
    ctx.instance_variable_set(:@workflow_step, workflow_step)
    ctx.instance_variable_set(:@dependency_results, dep_wrapper)
    ctx.instance_variable_set(:@step_definition, step_def)
    ctx.instance_variable_set(:@handler_name, 'test_handler')
    ctx
  end

  # ============================================================================
  # Tests: Basic step_handler
  # ============================================================================

  describe '#step_handler' do
    it 'wraps hash return as success' do
      handler_class = step_handler('my_handler') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        { processed: true }
      end

      handler = handler_class.new
      expect(handler.handler_name).to eq('my_handler')

      result = handler.call(make_context)
      expect(result.success?).to be true
      expect(result.result).to eq({ processed: true })
    end

    it 'sets custom version' do
      handler_class = step_handler('versioned', version: '2.0.0') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        {}
      end

      handler_class.new
      expect(handler_class::VERSION).to eq('2.0.0')
    end

    it 'wraps nil return as empty success' do
      handler_class = step_handler('nil_handler') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        nil
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      expect(result.result).to eq({})
    end

    it 'passes through StepHandlerCallResult without double-wrapping' do
      handler_class = step_handler('passthrough') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        TaskerCore::Types::StepHandlerCallResult.success(result: { direct: true })
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      expect(result.result).to eq({ direct: true })
    end

    it 'is a StepHandler::Base subclass' do
      handler_class = step_handler('compat') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        {}
      end

      expect(handler_class.new).to be_a(TaskerCore::StepHandler::Base)
    end

    it 'raises ArgumentError without block' do
      expect { step_handler('no_block') }.to raise_error(ArgumentError, 'block required')
    end
  end

  # ============================================================================
  # Tests: Dependency Injection
  # ============================================================================

  describe 'dependency injection' do
    it 'injects dependencies from context' do
      handler_class = step_handler('with_deps',
                                   depends_on: { cart: 'validate_cart' }) do |cart:, context:| # rubocop:disable Lint/UnusedBlockArgument
        { total: cart['total'] }
      end

      handler = handler_class.new
      ctx = make_context(dependency_results: {
                           'validate_cart' => { result: { 'total' => 99.99 } }
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({ total: 99.99 })
    end

    it 'injects nil for missing dependencies' do
      handler_class = step_handler('missing_dep',
                                   depends_on: { cart: 'validate_cart' }) do |cart:, context:| # rubocop:disable Lint/UnusedBlockArgument
        { cart_is_nil: cart.nil? }
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      expect(result.result).to eq({ cart_is_nil: true })
    end

    it 'injects multiple dependencies' do
      handler_class = step_handler('multi_deps',
                                   depends_on: { cart: 'validate_cart',
                                                 user: 'fetch_user' }) do |cart:, user:, context:| # rubocop:disable Lint/UnusedBlockArgument
        { cart: cart, user: user }
      end

      handler = handler_class.new
      ctx = make_context(dependency_results: {
                           'validate_cart' => { result: { 'total' => 50 } },
                           'fetch_user' => { result: { 'name' => 'Alice' } }
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:cart]).to eq({ 'total' => 50 })
      expect(result.result[:user]).to eq({ 'name' => 'Alice' })
    end
  end

  # ============================================================================
  # Tests: Input Injection
  # ============================================================================

  describe 'input injection' do
    it 'injects inputs from task context' do
      handler_class = step_handler('with_inputs',
                                   inputs: [:payment_info]) do |payment_info:, context:| # rubocop:disable Lint/UnusedBlockArgument
        { payment: payment_info }
      end

      handler = handler_class.new
      ctx = make_context(input_data: { payment_info: { 'card' => '1234' } })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({ payment: { 'card' => '1234' } })
    end

    it 'injects nil for missing inputs' do
      handler_class = step_handler('missing_input',
                                   inputs: [:nonexistent]) do |nonexistent:, context:| # rubocop:disable Lint/UnusedBlockArgument
        { is_nil: nonexistent.nil? }
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      expect(result.result).to eq({ is_nil: true })
    end
  end

  # ============================================================================
  # Tests: Error Classification
  # ============================================================================

  describe 'error classification' do
    it 'PermanentError -> failure(retryable=false)' do
      handler_class = step_handler('perm_err') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        raise TaskerCore::Errors::PermanentError, 'Invalid input'
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be false
      expect(result.retryable).to be false
      expect(result.message).to include('Invalid input')
    end

    it 'RetryableError -> failure(retryable=true)' do
      handler_class = step_handler('retry_err') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        raise TaskerCore::Errors::RetryableError, 'Service unavailable'
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be false
      expect(result.retryable).to be true
      expect(result.message).to include('Service unavailable')
    end

    it 'generic error -> failure(retryable=true)' do
      handler_class = step_handler('generic_err') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        raise TypeError, 'Something went wrong'
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be false
      expect(result.retryable).to be true
      expect(result.message).to include('Something went wrong')
    end
  end

  # ============================================================================
  # Tests: Decision Handler
  # ============================================================================

  describe '#decision_handler' do
    it 'Decision.route creates create_steps outcome' do
      handler_class = decision_handler('route_order') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        TaskerCore::StepHandler::Functional::Decision.route(['process_premium'], tier: 'premium')
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      outcome = result.result[:decision_point_outcome]
      expect(outcome[:type]).to eq('create_steps')
      expect(outcome[:step_names]).to eq(['process_premium'])
      expect(result.result[:routing_context]).to eq({ tier: 'premium' })
    end

    it 'Decision.skip creates no_branches outcome' do
      handler_class = decision_handler('skip_handler') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        TaskerCore::StepHandler::Functional::Decision.skip('No items to process')
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      outcome = result.result[:decision_point_outcome]
      expect(outcome[:type]).to eq('no_branches')
      expect(result.result[:reason]).to eq('No items to process')
    end

    it 'has decision capabilities via Mixins::Decision' do
      handler_class = decision_handler('test_decision') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        TaskerCore::StepHandler::Functional::Decision.route(['step_a'])
      end

      handler = handler_class.new
      expect(handler.capabilities).to include('decision_point')
      expect(handler.capabilities).to include('dynamic_workflow')
      expect(handler.capabilities).to include('step_creation')
    end

    it 'includes decision metadata' do
      handler_class = decision_handler('meta_decision') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        TaskerCore::StepHandler::Functional::Decision.route(%w[step_a step_b])
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      expect(result.metadata[:decision_point]).to be true
      expect(result.metadata[:outcome_type]).to eq('create_steps')
      expect(result.metadata[:branches_created]).to eq(2)
      expect(result.metadata[:processed_by]).to eq('meta_decision')
    end

    it 'injects dependencies in decision handler' do
      handler_class = decision_handler('route_with_deps',
                                       depends_on: { order: 'validate_order' }) do |order:, context:| # rubocop:disable Lint/UnusedBlockArgument
        if order&.dig('tier') == 'premium'
          TaskerCore::StepHandler::Functional::Decision.route(['process_premium'])
        else
          TaskerCore::StepHandler::Functional::Decision.route(['process_standard'])
        end
      end

      handler = handler_class.new
      ctx = make_context(dependency_results: {
                           'validate_order' => { result: { 'tier' => 'premium' } }
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      outcome = result.result[:decision_point_outcome]
      expect(outcome[:step_names]).to eq(['process_premium'])
    end
  end

  # ============================================================================
  # Tests: Batch Analyzer
  # ============================================================================

  describe '#batch_analyzer' do
    it 'auto-generates cursor configs from BatchConfig via Batchable mixin' do
      handler_class = batch_analyzer('analyze', worker_template: 'process_batch') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        TaskerCore::StepHandler::Functional::BatchConfig.new(total_items: 250, batch_size: 100)
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be true

      outcome = result.result['batch_processing_outcome']
      expect(outcome['type']).to eq('create_batches')
      expect(outcome['worker_template_name']).to eq('process_batch')
      expect(outcome['total_items']).to eq(250)
      expect(outcome['worker_count']).to eq(3)

      configs = outcome['cursor_configs']
      expect(configs.size).to eq(3)
      expect(configs[0]['start_cursor']).to eq(0)
      expect(configs[0]['end_cursor']).to eq(84)
      expect(configs[1]['start_cursor']).to eq(84)
      expect(configs[1]['end_cursor']).to eq(168)
      expect(configs[2]['start_cursor']).to eq(168)
      expect(configs[2]['end_cursor']).to eq(250)
    end
  end

  # ============================================================================
  # Tests: Batch Worker
  # ============================================================================

  describe '#batch_worker' do
    it 'extracts batch context as BatchWorkerContext from workflow step inputs' do
      handler_class = batch_worker('process_batch') do |batch_context:, context:| # rubocop:disable Lint/UnusedBlockArgument
        {
          start: batch_context.start_cursor,
          end_pos: batch_context.end_cursor,
          batch_id: batch_context.batch_id
        }
      end

      handler = handler_class.new
      ctx = make_context(input_data: {
                           cursor: {
                             'batch_id' => '001',
                             'start_cursor' => 100,
                             'end_cursor' => 200
                           }
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({
                                    start: 100,
                                    end_pos: 200,
                                    batch_id: '001'
                                  })
    end

    it 'returns BatchWorkerContext object with accessor methods' do
      handler_class = batch_worker('process_batch') do |batch_context:, context:| # rubocop:disable Lint/UnusedBlockArgument
        {
          is_batch_context: batch_context.is_a?(TaskerCore::BatchProcessing::BatchWorkerContext),
          no_op: batch_context.no_op?
        }
      end

      handler = handler_class.new
      ctx = make_context(input_data: {
                           cursor: {
                             'batch_id' => '002',
                             'start_cursor' => 0,
                             'end_cursor' => 50
                           }
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:is_batch_context]).to be true
      expect(result.result[:no_op]).to be false
    end
  end

  # ============================================================================
  # Tests: Combined Dependencies and Inputs
  # ============================================================================

  describe 'combined dependencies and inputs' do
    it 'works together' do
      handler_class = step_handler('combined',
                                   depends_on: { prev: 'step_1' },
                                   inputs: [:config_key]) do |prev:, config_key:, context:| # rubocop:disable Lint/UnusedBlockArgument
        { prev: prev, config: config_key }
      end

      handler = handler_class.new
      ctx = make_context(
        input_data: { config_key: 'abc' },
        dependency_results: { 'step_1' => { result: { 'count' => 5 } } }
      )
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:prev]).to eq({ 'count' => 5 })
      expect(result.result[:config]).to eq('abc')
    end
  end

  # ============================================================================
  # Tests: Context Always Available
  # ============================================================================

  describe 'context always available' do
    it 'context is always passed to handler' do
      handler_class = step_handler('ctx_check') do |context:|
        {
          has_context: !context.nil?,
          task_uuid: context.task_uuid
        }
      end

      handler = handler_class.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      expect(result.result).to eq({ has_context: true, task_uuid: 'task-456' })
    end
  end
end
