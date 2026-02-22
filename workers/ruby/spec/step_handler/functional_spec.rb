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

  # ============================================================================
  # Test Structs for Model-Based Injection
  # ============================================================================

  module TestTypes
    include Dry.Types()

    class RefundInput < Dry::Struct
      attribute :ticket_id, TestTypes::String
      attribute :customer_id, TestTypes::String
      attribute :refund_amount, TestTypes::Float
    end

    class ApprovalResult < Dry::Struct
      attribute :approved, TestTypes::Bool
      attribute :approval_id, TestTypes::String
    end
  end

  # ============================================================================
  # Tests: Struct-Based inputs:
  # ============================================================================

  describe 'struct-based inputs:' do
    it 'injects a single typed inputs param from struct class' do
      handler_class = step_handler('struct_inputs',
                                   inputs: TestTypes::RefundInput) do |inputs:, context:| # rubocop:disable Lint/UnusedBlockArgument
        {
          ticket: inputs.ticket_id,
          customer: inputs.customer_id,
          amount: inputs.refund_amount
        }
      end

      handler = handler_class.new
      ctx = make_context(input_data: {
                           ticket_id: 'TKT-001',
                           customer_id: 'CUST-42',
                           refund_amount: 99.99
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({
                                    ticket: 'TKT-001',
                                    customer: 'CUST-42',
                                    amount: 99.99
                                  })
    end

    it 'symbol array inputs: still works (backward compatible)' do
      handler_class = step_handler('symbol_inputs',
                                   inputs: %i[ticket_id customer_id]) do |ticket_id:, customer_id:, context:| # rubocop:disable Lint/UnusedBlockArgument
        { ticket: ticket_id, customer: customer_id }
      end

      handler = handler_class.new
      ctx = make_context(input_data: { ticket_id: 'TKT-002', customer_id: 'CUST-43' })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({ ticket: 'TKT-002', customer: 'CUST-43' })
    end
  end

  # ============================================================================
  # Tests: Array-Pair depends_on:
  # ============================================================================

  describe 'array-pair depends_on:' do
    it 'injects typed model from [step_name, StructClass] pair' do
      handler_class = step_handler('struct_dep',
                                   depends_on: {
                                     approval: ['get_approval', TestTypes::ApprovalResult]
                                   }) do |approval:, context:| # rubocop:disable Lint/UnusedBlockArgument
        {
          approved: approval.approved,
          id: approval.approval_id
        }
      end

      handler = handler_class.new
      ctx = make_context(dependency_results: {
                           'get_approval' => { result: { 'approved' => true, 'approval_id' => 'APR-001' } }
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({ approved: true, id: 'APR-001' })
    end

    it 'mixes typed and untyped deps' do
      handler_class = step_handler('mixed_deps',
                                   depends_on: {
                                     approval: ['get_approval', TestTypes::ApprovalResult],
                                     validation: 'validate_request'
                                   }) do |approval:, validation:, context:| # rubocop:disable Lint/UnusedBlockArgument
        {
          approved: approval.approved,
          valid: validation&.dig('is_valid')
        }
      end

      handler = handler_class.new
      ctx = make_context(dependency_results: {
                           'get_approval' => { result: { 'approved' => true, 'approval_id' => 'APR-002' } },
                           'validate_request' => { result: { 'is_valid' => true } }
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({ approved: true, valid: true })
    end

    it 'plain string depends_on: still works (backward compatible)' do
      handler_class = step_handler('string_dep',
                                   depends_on: { cart: 'validate_cart' }) do |cart:, context:| # rubocop:disable Lint/UnusedBlockArgument
        { total: cart['total'] }
      end

      handler = handler_class.new
      ctx = make_context(dependency_results: {
                           'validate_cart' => { result: { 'total' => 42.0 } }
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({ total: 42.0 })
    end
  end

  # ============================================================================
  # Tests: Mixed Mode (Struct Inputs + Struct Deps)
  # ============================================================================

  # ============================================================================
  # Test Structs with validate! for Model-Level Validation
  # ============================================================================

  module TestTypes
    class ValidatedRefundInput < Dry::Struct
      transform_types do |type|
        if type.default?
          type
        else
          type.optional.meta(omittable: true)
        end
      end

      attribute :ticket_id, TestTypes::String
      attribute :customer_id, TestTypes::String
      attribute :refund_amount, TestTypes::Float

      def validate!
        missing = []
        missing << 'ticket_id' if ticket_id.blank?
        missing << 'customer_id' if customer_id.blank?
        missing << 'refund_amount' if refund_amount.nil?
        return if missing.empty?

        raise TaskerCore::Errors::PermanentError.new(
          "Missing required fields: #{missing.join(', ')}",
          error_code: 'MISSING_FIELDS'
        )
      end
    end
  end

  # ============================================================================
  # Tests: Model-Level Input Validation
  # ============================================================================

  describe 'model-level input validation via validate!' do
    it 'raises PermanentError on missing required fields' do
      handler_class = step_handler('validated_handler',
                                   inputs: TestTypes::ValidatedRefundInput) do |inputs:, context:| # rubocop:disable Lint/UnusedBlockArgument
        { ticket: inputs.ticket_id }
      end

      handler = handler_class.new
      ctx = make_context(input_data: { ticket_id: 'TKT-001' }) # missing customer_id, refund_amount
      result = handler.call(ctx)
      expect(result.success?).to be false
      expect(result.retryable).to be false
      expect(result.message).to include('customer_id')
      expect(result.message).to include('refund_amount')
    end

    it 'passes when all required fields are present' do
      handler_class = step_handler('validated_ok',
                                   inputs: TestTypes::ValidatedRefundInput) do |inputs:, context:| # rubocop:disable Lint/UnusedBlockArgument
        {
          ticket: inputs.ticket_id,
          customer: inputs.customer_id,
          amount: inputs.refund_amount
        }
      end

      handler = handler_class.new
      ctx = make_context(input_data: {
                           ticket_id: 'TKT-001',
                           customer_id: 'CUST-42',
                           refund_amount: 99.99
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({
                                    ticket: 'TKT-001',
                                    customer: 'CUST-42',
                                    amount: 99.99
                                  })
    end

    it 'skips validation for structs without validate!' do
      handler_class = step_handler('no_validate',
                                   inputs: TestTypes::RefundInput) do |inputs:, context:| # rubocop:disable Lint/UnusedBlockArgument
        { ticket: inputs.ticket_id }
      end

      handler = handler_class.new
      ctx = make_context(input_data: {
                           ticket_id: 'TKT-001',
                           customer_id: 'CUST-42',
                           refund_amount: 99.99
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
    end
  end

  # ============================================================================
  # Tests: _inject_args HWIA and plain hash handling
  # ============================================================================

  describe 'attribute parsing with HashWithIndifferentAccess' do
    it 'converts HWIA to plain hash before symbolizing keys for Dry::Struct' do
      # ActiveSupport::HashWithIndifferentAccess stores keys as strings internally,
      # so transform_keys(&:to_sym) on HWIA doesn't actually produce symbol keys.
      # _inject_args must call .to_h first to get a real plain Hash.
      hwia_result = ActiveSupport::HashWithIndifferentAccess.new(
        'approved' => true,
        'approval_id' => 'APR-HWIA'
      )

      handler_class = step_handler('hwia_dep',
                                   depends_on: {
                                     approval: ['get_approval', TestTypes::ApprovalResult]
                                   }) do |approval:, context:| # rubocop:disable Lint/UnusedBlockArgument
        {
          approved: approval.approved,
          id: approval.approval_id
        }
      end

      dep_wrapper = double('dependency_results')
      allow(dep_wrapper).to receive(:get_results).with('get_approval').and_return(hwia_result)

      ctx = TaskerCore::Types::StepContext.allocate
      ctx.instance_variable_set(:@task, double('task', task_uuid: 'task-456', context: {}))
      ctx.instance_variable_set(:@workflow_step, double('workflow_step',
                                                        workflow_step_uuid: 'step-789',
                                                        name: 'test_step',
                                                        inputs: {},
                                                        attempts: 0,
                                                        max_attempts: 3,
                                                        checkpoint: nil,
                                                        retryable: true))
      ctx.instance_variable_set(:@dependency_results, dep_wrapper)
      ctx.instance_variable_set(:@step_definition, double('step_definition',
                                                          handler: double('handler', callable: 'test',
                                                                                     initialization: {})))
      ctx.instance_variable_set(:@handler_name, 'test')

      handler = handler_class.new
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({ approved: true, id: 'APR-HWIA' })
    end

    it 'handles plain hashes correctly when symbolizing keys for Dry::Struct' do
      plain_hash_result = { 'approved' => false, 'approval_id' => 'APR-PLAIN' }

      handler_class = step_handler('plain_dep',
                                   depends_on: {
                                     approval: ['get_approval', TestTypes::ApprovalResult]
                                   }) do |approval:, context:| # rubocop:disable Lint/UnusedBlockArgument
        {
          approved: approval.approved,
          id: approval.approval_id
        }
      end

      dep_wrapper = double('dependency_results')
      allow(dep_wrapper).to receive(:get_results).with('get_approval').and_return(plain_hash_result)

      ctx = TaskerCore::Types::StepContext.allocate
      ctx.instance_variable_set(:@task, double('task', task_uuid: 'task-456', context: {}))
      ctx.instance_variable_set(:@workflow_step, double('workflow_step',
                                                        workflow_step_uuid: 'step-789',
                                                        name: 'test_step',
                                                        inputs: {},
                                                        attempts: 0,
                                                        max_attempts: 3,
                                                        checkpoint: nil,
                                                        retryable: true))
      ctx.instance_variable_set(:@dependency_results, dep_wrapper)
      ctx.instance_variable_set(:@step_definition, double('step_definition',
                                                          handler: double('handler', callable: 'test',
                                                                                     initialization: {})))
      ctx.instance_variable_set(:@handler_name, 'test')

      handler = handler_class.new
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({ approved: false, id: 'APR-PLAIN' })
    end
  end

  # ============================================================================
  # Tests: Mixed Struct Inputs + Struct Deps
  # ============================================================================

  describe 'mixed struct inputs and deps' do
    it 'struct inputs: and array-pair depends_on: work together' do
      handler_class = step_handler('full_struct',
                                   depends_on: { approval: ['get_approval', TestTypes::ApprovalResult] },
                                   inputs: TestTypes::RefundInput) do |approval:, inputs:, context:| # rubocop:disable Lint/UnusedBlockArgument
        {
          ticket: inputs.ticket_id,
          approved: approval.approved
        }
      end

      handler = handler_class.new
      ctx = make_context(
        input_data: {
          ticket_id: 'TKT-100',
          customer_id: 'CUST-99',
          refund_amount: 200.0
        },
        dependency_results: {
          'get_approval' => { result: { 'approved' => true, 'approval_id' => 'APR-100' } }
        }
      )
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({ ticket: 'TKT-100', approved: true })
    end
  end

  # ============================================================================
  # Tests: Dry::Struct Result Serialization
  # ============================================================================

  module TestTypes
    class RefundResultStruct < Dry::Struct
      attribute :request_validated, TestTypes::Bool
      attribute :ticket_id, TestTypes::String
      attribute :customer_id, TestTypes::String
      attribute :amount, TestTypes::Float
    end
  end

  describe 'Dry::Struct result serialization via _wrap_result' do
    it 'serializes a Dry::Struct return value to a hash' do
      handler_class = step_handler('struct_result') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        TestTypes::RefundResultStruct.new(
          request_validated: true,
          ticket_id: 'TKT-001',
          customer_id: 'CUST-42',
          amount: 99.99
        )
      end

      handler = handler_class.new
      ctx = make_context
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({
                                    request_validated: true,
                                    ticket_id: 'TKT-001',
                                    customer_id: 'CUST-42',
                                    amount: 99.99
                                  })
    end

    it 'plain hash return continues to work' do
      handler_class = step_handler('hash_result') do |context:| # rubocop:disable Lint/UnusedBlockArgument
        { ticket_id: 'TKT-001', amount: 50.0 }
      end

      handler = handler_class.new
      ctx = make_context
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result).to eq({ ticket_id: 'TKT-001', amount: 50.0 })
    end
  end
end
