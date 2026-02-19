# frozen_string_literal: true

require 'spec_helper'

# Load DSL handler files explicitly (they are NOT auto-loaded by TestEnvironment)
Dir.glob(File.expand_path('../handlers/dsl_examples/**/*.rb', __dir__)).sort.each do |f|
  load f
end

RSpec.describe 'DSL Handler Parity Tests (TAS-294 Phase 2)' do
  include TaskerCore::StepHandler::Functional

  # ============================================================================
  # Test Helpers
  # ============================================================================

  def make_context(dependency_results: {}, input_data: {}, step_config: {}, step_name: 'test_step')
    task_wrapper = double('task',
                          task_uuid: 'task-456',
                          context: input_data.transform_keys(&:to_s))

    workflow_step = double('workflow_step',
                           workflow_step_uuid: 'step-789',
                           name: step_name,
                           inputs: input_data.transform_keys(&:to_s),
                           attempts: 0,
                           max_attempts: 3,
                           results: nil,
                           checkpoint: nil,
                           retryable: true)

    dep_wrapper = double('dependency_results')
    allow(dep_wrapper).to receive(:keys).and_return(dependency_results.keys.map(&:to_s))
    allow(dep_wrapper).to receive(:get_results) do |sn|
      dep = dependency_results[sn.to_s] || dependency_results[sn.to_sym]
      dep&.dig(:result) || dep&.dig('result')
    end

    step_def_handler = double('handler', callable: 'test_handler', initialization: step_config.transform_keys(&:to_s))
    step_def = double('step_definition', handler: step_def_handler)

    ctx = TaskerCore::Types::StepContext.allocate
    ctx.instance_variable_set(:@task, task_wrapper)
    ctx.instance_variable_set(:@workflow_step, workflow_step)
    ctx.instance_variable_set(:@dependency_results, dep_wrapper)
    ctx.instance_variable_set(:@step_definition, step_def)
    ctx.instance_variable_set(:@handler_name, 'test_handler')
    ctx
  end

  # ============================================================================
  # Linear Workflow Parity
  # ============================================================================

  describe 'Linear Workflow' do
    let(:even_number) { 2 }

    it 'Step1 DSL produces same result as chained squaring' do
      handler = LinearStep1DslHandler.new
      ctx = make_context(input_data: { even_number: even_number })
      result = handler.call(ctx)

      expect(result.success?).to be true
      expect(result.result).to eq(even_number * even_number) # 4
      expect(result.metadata[:operation]).to eq('square')
    end

    it 'full linear chain produces n^8' do
      # Step1: 2^2 = 4
      step1 = LinearStep1DslHandler.new
      ctx1 = make_context(input_data: { even_number: even_number })
      r1 = step1.call(ctx1)
      expect(r1.result).to eq(4)

      # Step2: 4^2 = 16
      step2 = LinearStep2DslHandler.new
      ctx2 = make_context(dependency_results: { 'linear_step_1' => { result: r1.result } })
      r2 = step2.call(ctx2)
      expect(r2.result).to eq(16)

      # Step3: 16^2 = 256
      step3 = LinearStep3DslHandler.new
      ctx3 = make_context(dependency_results: { 'linear_step_2' => { result: r2.result } })
      r3 = step3.call(ctx3)
      expect(r3.result).to eq(256)

      # Step4: 256^2 = 65536 = 2^16 (but we square 4 times so 2^(2^4) = 2^16 = 65536)
      step4 = LinearStep4DslHandler.new
      ctx4 = make_context(
        dependency_results: { 'linear_step_3' => { result: r3.result } },
        input_data: { even_number: even_number }
      )
      r4 = step4.call(ctx4)
      expect(r4.result).to eq(65_536)
      # 4 squarings: n^(2^4) = n^16 (the verbose handler's verification uses n^8, which is a math error)
      expect(r4.result).to eq(even_number**16)
      # Verification matches is false because the handler's expected uses n^8 (same bug as verbose handler)
      expect(r4.metadata[:verification][:matches]).to be false
    end
  end

  # ============================================================================
  # Diamond Workflow Parity
  # ============================================================================

  describe 'Diamond Workflow' do
    let(:even_number) { 2 }

    it 'full diamond produces n^16' do
      start = DiamondStartDslHandler.new
      ctx = make_context(input_data: { even_number: even_number })
      rs = start.call(ctx)
      expect(rs.result).to eq(4) # 2^2

      branch_b = DiamondBranchBDslHandler.new
      rb = branch_b.call(make_context(dependency_results: { 'diamond_start' => { result: rs.result } }))
      expect(rb.result).to eq(16) # 4^2

      branch_c = DiamondBranchCDslHandler.new
      rc = branch_c.call(make_context(dependency_results: { 'diamond_start' => { result: rs.result } }))
      expect(rc.result).to eq(16) # 4^2

      diamond_end = DiamondEndDslHandler.new
      re = diamond_end.call(make_context(
                              dependency_results: {
                                'diamond_branch_b' => { result: rb.result },
                                'diamond_branch_c' => { result: rc.result }
                              },
                              input_data: { even_number: even_number }
                            ))
      expect(re.result).to eq(even_number**16)
      expect(re.metadata[:verification][:matches]).to be true
    end
  end

  # ============================================================================
  # Tree Workflow Parity
  # ============================================================================

  describe 'Tree Workflow' do
    let(:even_number) { 2 }

    it 'full tree produces n^32' do
      root = TreeRootDslHandler.new
      rr = root.call(make_context(input_data: { even_number: even_number }))
      expect(rr.result).to eq(4)

      left = TreeBranchLeftDslHandler.new
      rl = left.call(make_context(dependency_results: { 'tree_root' => { result: rr.result } }))
      expect(rl.result).to eq(16)

      right = TreeBranchRightDslHandler.new
      rri = right.call(make_context(dependency_results: { 'tree_root' => { result: rr.result } }))
      expect(rri.result).to eq(16)

      leaf_d = TreeLeafDDslHandler.new
      rd = leaf_d.call(make_context(dependency_results: { 'tree_branch_left' => { result: rl.result } }))
      expect(rd.result).to eq(256)

      leaf_e = TreeLeafEDslHandler.new
      re = leaf_e.call(make_context(dependency_results: { 'tree_branch_left' => { result: rl.result } }))
      expect(re.result).to eq(256)

      leaf_f = TreeLeafFDslHandler.new
      rf = leaf_f.call(make_context(dependency_results: { 'tree_branch_right' => { result: rri.result } }))
      expect(rf.result).to eq(256)

      leaf_g = TreeLeafGDslHandler.new
      rg = leaf_g.call(make_context(dependency_results: { 'tree_branch_right' => { result: rri.result } }))
      expect(rg.result).to eq(256)

      final = TreeFinalConvergenceDslHandler.new
      rfinal = final.call(make_context(
                            dependency_results: {
                              'tree_leaf_d' => { result: rd.result },
                              'tree_leaf_e' => { result: re.result },
                              'tree_leaf_f' => { result: rf.result },
                              'tree_leaf_g' => { result: rg.result }
                            },
                            input_data: { even_number: even_number }
                          ))
      # Actual: (256^4)^2 = (2^32)^2 = 2^64 (handler verification uses n^32, same math discrepancy as verbose handler)
      expect(rfinal.result).to eq(even_number**64)
      expect(rfinal.metadata[:verification][:matches]).to be false
    end
  end

  # ============================================================================
  # Mixed DAG Workflow Parity
  # ============================================================================

  describe 'Mixed DAG Workflow' do
    let(:even_number) { 2 }

    it 'full DAG produces n^64' do
      init = DagInitDslHandler.new
      ri = init.call(make_context(input_data: { even_number: even_number }))
      expect(ri.result).to eq(4)

      left = DagProcessLeftDslHandler.new
      rl = left.call(make_context(dependency_results: { 'dag_init' => { result: ri.result } }))
      expect(rl.result).to eq(16)

      right = DagProcessRightDslHandler.new
      rr = right.call(make_context(dependency_results: { 'dag_init' => { result: ri.result } }))
      expect(rr.result).to eq(16)

      validate = DagValidateDslHandler.new
      rv = validate.call(make_context(dependency_results: {
                                        'dag_process_left' => { result: rl.result },
                                        'dag_process_right' => { result: rr.result }
                                      }))
      expect(rv.result).to eq((16 * 16)**2)

      transform = DagTransformDslHandler.new
      rt = transform.call(make_context(dependency_results: { 'dag_process_left' => { result: rl.result } }))
      expect(rt.result).to eq(16**2)

      analyze = DagAnalyzeDslHandler.new
      ra = analyze.call(make_context(dependency_results: { 'dag_process_right' => { result: rr.result } }))
      expect(ra.result).to eq(16**2)

      finalize = DagFinalizeDslHandler.new
      rf = finalize.call(make_context(
                           dependency_results: {
                             'dag_validate' => { result: rv.result },
                             'dag_transform' => { result: rt.result },
                             'dag_analyze' => { result: ra.result }
                           },
                           input_data: { even_number: even_number }
                         ))
      expect(rf.result).to eq(even_number**64)
      expect(rf.metadata[:verification][:matches]).to be true
    end
  end

  # ============================================================================
  # Error Scenarios Parity
  # ============================================================================

  describe 'Error Scenarios' do
    it 'success handler returns success result' do
      handler = ErrorSuccessDslHandler.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      expect(result.result[:status]).to eq('success')
      expect(result.result[:message]).to eq('Step completed successfully')
    end

    it 'permanent error handler returns non-retryable error' do
      handler = ErrorPermanentDslHandler.new
      result = handler.call(make_context)
      expect(result.success?).to be false
      expect(result.retryable).to be false
      expect(result.message).to include('Invalid payment method')
    end

    it 'retryable error handler returns retryable error' do
      handler = ErrorRetryableDslHandler.new
      result = handler.call(make_context)
      expect(result.success?).to be false
      expect(result.retryable).to be true
      expect(result.message).to include('Payment service timeout')
    end
  end

  # ============================================================================
  # Conditional Approval (Decision Handler) Parity
  # ============================================================================

  describe 'Conditional Approval' do
    it 'validates request with correct fields' do
      handler = ApprovalValidateRequestDslHandler.new
      ctx = make_context(input_data: { amount: 500, requester: 'Alice', purpose: 'supplies' })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:amount]).to eq(500)
      expect(result.result[:requester]).to eq('Alice')
    end

    it 'routes small amounts to auto_approve' do
      handler = ApprovalRoutingDecisionDslHandler.new
      ctx = make_context(input_data: { amount: 500 })
      result = handler.call(ctx)
      expect(result.success?).to be true
      outcome = result.result[:decision_point_outcome]
      expect(outcome[:type]).to eq('create_steps')
      expect(outcome[:step_names]).to eq(['auto_approve'])
    end

    it 'routes medium amounts to manager_approval' do
      handler = ApprovalRoutingDecisionDslHandler.new
      ctx = make_context(input_data: { amount: 2500 })
      result = handler.call(ctx)
      expect(result.success?).to be true
      outcome = result.result[:decision_point_outcome]
      expect(outcome[:step_names]).to eq(['manager_approval'])
    end

    it 'routes large amounts to dual approval' do
      handler = ApprovalRoutingDecisionDslHandler.new
      ctx = make_context(input_data: { amount: 10_000 })
      result = handler.call(ctx)
      expect(result.success?).to be true
      outcome = result.result[:decision_point_outcome]
      expect(outcome[:step_names]).to eq(%w[manager_approval finance_review])
    end

    it 'auto_approve returns correct structure' do
      handler = ApprovalAutoApproveDslHandler.new
      ctx = make_context(input_data: { amount: 500, requester: 'Alice' })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:approved]).to be true
      expect(result.result[:approval_type]).to eq('automatic')
      expect(result.result[:approved_by]).to eq('system')
    end

    it 'manager approves amounts <= 10000' do
      handler = ApprovalManagerApprovalDslHandler.new
      ctx = make_context(input_data: { amount: 5000, requester: 'Alice', purpose: 'equipment' })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:approved]).to be true
      expect(result.result[:approval_type]).to eq('manager')
    end

    it 'manager rejects amounts > 10000' do
      handler = ApprovalManagerApprovalDslHandler.new
      ctx = make_context(input_data: { amount: 15_000, requester: 'Alice', purpose: 'equipment' })
      result = handler.call(ctx)
      expect(result.success?).to be false
      expect(result.retryable).to be false
    end
  end

  # ============================================================================
  # Order Fulfillment Parity
  # ============================================================================

  describe 'Order Fulfillment' do
    let(:task_context) do
      {
        customer_info: { id: 'CUST-001', email: 'test@example.com' },
        order_items: [
          { product_id: 101, quantity: 2, price: 29.99 },
          { product_id: 102, quantity: 1, price: 49.99 }
        ],
        payment_info: { method: 'credit_card', token: 'tok_test' },
        shipping_info: { address: '123 Main St' }
      }
    end

    it 'validate_order validates items and calculates total' do
      handler = OrderValidateOrderDslHandler.new
      ctx = make_context(input_data: task_context)
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:customer_validated]).to be true
      expect(result.result[:validated_items].length).to eq(2)
      expect(result.result[:order_total]).to eq(29.99 * 2 + 49.99)
    end

    it 'validate_order rejects missing customer' do
      handler = OrderValidateOrderDslHandler.new
      ctx = make_context(input_data: { customer_info: {}, order_items: task_context[:order_items] })
      result = handler.call(ctx)
      expect(result.success?).to be false
      expect(result.retryable).to be false
    end

    it 'reserve_inventory reserves validated items' do
      # First validate
      v_handler = OrderValidateOrderDslHandler.new
      v_ctx = make_context(input_data: task_context)
      v_result = v_handler.call(v_ctx)

      # Then reserve
      handler = OrderReserveInventoryDslHandler.new
      ctx = make_context(dependency_results: { 'validate_order' => { result: v_result.result } })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:reservation_status]).to eq('confirmed')
      expect(result.result[:items_reserved]).to eq(2)
    end
  end

  # ============================================================================
  # Domain Events Parity
  # ============================================================================

  describe 'Domain Events' do
    it 'validate_order returns validated result' do
      handler = DomainEventValidateOrderDslHandler.new
      ctx = make_context(input_data: { order_id: 'ORD-001', customer_id: 'C-001', amount: 100 })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:validated]).to be true
      expect(result.result[:validation_checks]).to include('amount_positive')
    end

    it 'process_payment handles simulate_failure' do
      handler = DomainEventProcessPaymentDslHandler.new
      ctx = make_context(input_data: { simulate_failure: true })
      result = handler.call(ctx)
      expect(result.success?).to be false
      expect(result.retryable).to be true
    end

    it 'update_inventory returns items' do
      handler = DomainEventUpdateInventoryDslHandler.new
      ctx = make_context(input_data: { order_id: 'ORD-001' })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:items].length).to eq(2)
      expect(result.result[:success]).to be true
    end
  end

  # ============================================================================
  # Multi-Method Handler Parity
  # ============================================================================

  describe 'Multi-Method Handlers' do
    it 'call method returns correct invoked_method' do
      handler = MultiMethodCallDslHandler.new
      result = handler.call(make_context(input_data: { data: { 'key' => 'value' } }))
      expect(result.success?).to be true
      expect(result.result[:invoked_method]).to eq('call')
    end

    it 'validate method checks for amount field' do
      handler = MultiMethodValidateDslHandler.new
      result = handler.call(make_context(input_data: { data: {} }))
      expect(result.success?).to be false # Missing amount
    end

    it 'validate method succeeds with amount' do
      handler = MultiMethodValidateDslHandler.new
      result = handler.call(make_context(input_data: { data: { 'amount' => 100 } }))
      expect(result.success?).to be true
      expect(result.result[:validated]).to be true
    end

    it 'process method applies 10% fee' do
      handler = MultiMethodProcessDslHandler.new
      result = handler.call(make_context(input_data: { data: { 'amount' => 100 } }))
      expect(result.success?).to be true
      expect(result.result[:processed_amount]).to be_within(0.01).of(110.0)
      expect(result.result[:processing_fee]).to be_within(0.01).of(10.0)
    end

    it 'refund method returns refund data' do
      handler = MultiMethodRefundDslHandler.new
      result = handler.call(make_context(input_data: { data: { 'amount' => 50, 'reason' => 'damaged' } }))
      expect(result.success?).to be true
      expect(result.result[:refund_amount]).to eq(50)
      expect(result.result[:refund_reason]).to eq('damaged')
    end
  end

  # ============================================================================
  # Blog Post 01: Ecommerce Parity
  # ============================================================================

  describe 'Blog Post 01: Ecommerce' do
    let(:cart_items) do
      [
        { product_id: 1, quantity: 2 },
        { product_id: 2, quantity: 1 }
      ]
    end

    it 'validate_cart validates items and calculates totals' do
      handler = EcommerceValidateCartDslHandler.new
      ctx = make_context(input_data: { cart_items: cart_items })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:item_count]).to eq(2)
      expect(result.result[:subtotal]).to eq(29.99 * 2 + 49.99)
      expect(result.result[:tax]).to be > 0
      expect(result.result[:total]).to be > result.result[:subtotal]
    end

    it 'validate_cart rejects empty cart' do
      handler = EcommerceValidateCartDslHandler.new
      ctx = make_context(input_data: { cart_items: [] })
      result = handler.call(ctx)
      expect(result.success?).to be false
      expect(result.retryable).to be false
    end

    it 'validate_cart rejects unknown products' do
      handler = EcommerceValidateCartDslHandler.new
      ctx = make_context(input_data: { cart_items: [{ product_id: 999, quantity: 1 }] })
      result = handler.call(ctx)
      expect(result.success?).to be false
    end
  end

  # ============================================================================
  # Blog Post 02: Data Pipeline Parity
  # ============================================================================

  describe 'Blog Post 02: Data Pipeline' do
    it 'extract_sales returns sample data with correct record count' do
      handler = PipelineExtractSalesDslHandler.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      expect(result.result[:records].length).to eq(5)
      expect(result.result[:source]).to eq('SalesDatabase')
      expect(result.result[:total_amount]).to be > 0
    end

    it 'extract_customers returns tier breakdown' do
      handler = PipelineExtractCustomersDslHandler.new
      result = handler.call(make_context)
      expect(result.success?).to be true
      expect(result.result[:records].length).to eq(5)
      expect(result.result[:tier_breakdown]).to be_a(Hash)
    end

    it 'transform_sales produces daily and product groupings' do
      # Extract first
      extract = PipelineExtractSalesDslHandler.new
      extract_result = extract.call(make_context)

      handler = PipelineTransformSalesDslHandler.new
      ctx = make_context(dependency_results: {
                           'extract_sales_data' => { result: extract_result.result }
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:record_count]).to eq(5)
      expect(result.result[:daily_sales]).to be_a(Hash)
      expect(result.result[:product_sales]).to be_a(Hash)
    end
  end

  # ============================================================================
  # Blog Post 03: Microservices Parity
  # ============================================================================

  describe 'Blog Post 03: Microservices' do
    let(:user_info) do
      { email: 'alice@example.com', name: 'Alice Johnson', plan: 'pro' }
    end

    it 'create_user_account validates and creates user' do
      handler = MicroCreateUserAccountDslHandler.new
      ctx = make_context(input_data: { user_info: user_info })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:email]).to eq('alice@example.com')
      expect(result.result[:status]).to eq('created')
    end

    it 'create_user_account rejects missing email' do
      handler = MicroCreateUserAccountDslHandler.new
      ctx = make_context(input_data: { user_info: { name: 'No Email' } })
      result = handler.call(ctx)
      expect(result.success?).to be false
      expect(result.retryable).to be false
    end

    it 'setup_billing skips for free plan' do
      user_result = { user_id: 'user_abc', email: 'test@example.com', plan: 'free' }
      handler = MicroSetupBillingDslHandler.new
      ctx = make_context(dependency_results: { 'create_user_account' => { result: user_result } })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:status]).to eq('skipped_free_plan')
    end

    it 'setup_billing creates profile for pro plan' do
      user_result = { user_id: 'user_abc', email: 'test@example.com', plan: 'pro' }
      handler = MicroSetupBillingDslHandler.new
      ctx = make_context(dependency_results: { 'create_user_account' => { result: user_result } })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:status]).to eq('active')
      expect(result.result[:price]).to eq(29.99)
    end
  end

  # ============================================================================
  # Blog Post 04: Payments Parity
  # ============================================================================

  describe 'Blog Post 04: Payments' do
    it 'validate_payment_eligibility validates payment ID format' do
      handler = PaymentsValidateEligibilityDslHandler.new
      ctx = make_context(input_data: { payment_id: 'pay_test123', refund_amount: 5000 })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:payment_validated]).to be true
      expect(result.result[:eligibility_status]).to eq('eligible')
    end

    it 'validate_payment rejects invalid payment ID' do
      handler = PaymentsValidateEligibilityDslHandler.new
      ctx = make_context(input_data: { payment_id: 'invalid', refund_amount: 5000 })
      result = handler.call(ctx)
      expect(result.success?).to be false
      expect(result.retryable).to be false
    end

    it 'process_gateway_refund creates refund' do
      validation = {
        payment_validated: true, payment_id: 'pay_abc123',
        refund_amount: 5000, original_amount: 6000
      }
      handler = PaymentsProcessGatewayRefundDslHandler.new
      ctx = make_context(
        dependency_results: { 'validate_payment_eligibility' => { result: validation } },
        input_data: { refund_reason: 'customer_request' }
      )
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:refund_processed]).to be true
      expect(result.result[:refund_status]).to eq('processed')
    end

    it 'notify_customer sends confirmation' do
      refund_result = {
        refund_processed: true, refund_id: 'rfnd_abc', payment_id: 'pay_abc',
        refund_amount: 5000, estimated_arrival: Time.now.utc.iso8601
      }
      handler = PaymentsNotifyCustomerDslHandler.new
      ctx = make_context(
        dependency_results: { 'process_gateway_refund' => { result: refund_result } },
        input_data: { customer_email: 'alice@example.com', refund_reason: 'customer_request' }
      )
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:notification_sent]).to be true
      expect(result.result[:customer_email]).to eq('alice@example.com')
    end
  end

  # ============================================================================
  # Blog Post 04: Customer Success Parity
  # ============================================================================

  describe 'Blog Post 04: Customer Success' do
    it 'validate_refund_request validates required fields' do
      handler = CsValidateRefundRequestDslHandler.new
      ctx = make_context(input_data: {
                           ticket_id: 'TK-001', customer_id: 'CUST-001',
                           refund_amount: 5000, refund_reason: 'defective'
                         })
      result = handler.call(ctx)
      expect(result.success?).to be true
      expect(result.result[:request_validated]).to be true
      expect(result.result[:ticket_status]).to eq('open')
    end

    it 'validate_refund_request rejects closed tickets' do
      handler = CsValidateRefundRequestDslHandler.new
      ctx = make_context(input_data: {
                           ticket_id: 'ticket_closed_001', customer_id: 'CUST-001',
                           refund_amount: 5000
                         })
      result = handler.call(ctx)
      expect(result.success?).to be false
      expect(result.retryable).to be false
    end

    it 'validate_refund_request rejects missing fields' do
      handler = CsValidateRefundRequestDslHandler.new
      ctx = make_context(input_data: { ticket_id: 'TK-001' })
      result = handler.call(ctx)
      expect(result.success?).to be false
    end
  end

  # ============================================================================
  # Handler Class Properties
  # ============================================================================

  describe 'Handler Class Properties' do
    it 'all DSL handlers are StepHandler::Base subclasses' do
      [
        LinearStep1DslHandler, LinearStep2DslHandler,
        DiamondStartDslHandler, DiamondEndDslHandler,
        TreeRootDslHandler, TreeFinalConvergenceDslHandler,
        DagInitDslHandler, DagFinalizeDslHandler,
        ErrorSuccessDslHandler, ErrorPermanentDslHandler,
        OrderValidateOrderDslHandler,
        DomainEventValidateOrderDslHandler,
        MultiMethodCallDslHandler,
        EcommerceValidateCartDslHandler,
        PipelineExtractSalesDslHandler,
        MicroCreateUserAccountDslHandler,
        PaymentsValidateEligibilityDslHandler,
        CsValidateRefundRequestDslHandler
      ].each do |handler_class|
        expect(handler_class.new).to be_a(TaskerCore::StepHandler::Base),
                                     "#{handler_class} should be a StepHandler::Base subclass"
      end
    end

    it 'decision handlers have decision capabilities' do
      handler = ApprovalRoutingDecisionDslHandler.new
      expect(handler.capabilities).to include('decision_point')
    end
  end
end
