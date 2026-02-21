# frozen_string_literal: true

# DSL mirror of CustomerSuccess::StepHandlers using block DSL.
#
# 5 handlers: validate_refund_request, check_refund_policy,
#             get_manager_approval, execute_refund_workflow, update_ticket_status
#
# NOTE: These handlers replicate the core validation and output structure.
# Non-deterministic fields differ between runs.

include TaskerCore::StepHandler::Functional # rubocop:disable Style/MixinUsage

CS_REFUND_POLICIES_DSL = {
  standard: { window_days: 30, requires_approval: true, max_amount: 10_000 },
  gold: { window_days: 60, requires_approval: false, max_amount: 50_000 },
  premium: { window_days: 90, requires_approval: false, max_amount: 100_000 }
}.freeze

CsValidateRefundRequestDslHandler = step_handler(
  'customer_success_dsl.step_handlers.validate_refund_request',
  inputs: %i[ticket_id customer_id refund_amount refund_reason]
) do |ticket_id:, customer_id:, refund_amount:, refund_reason:, context:| # rubocop:disable Lint/UnusedBlockArgument
  ticket_id ||= context.get_input('ticket_id')
  customer_id ||= context.get_input('customer_id')
  refund_amount ||= context.get_input('refund_amount')

  missing = []
  missing << 'ticket_id' unless ticket_id && !ticket_id.to_s.empty?
  missing << 'customer_id' unless customer_id && !customer_id.to_s.empty?
  missing << 'refund_amount' unless refund_amount && !refund_amount.to_s.empty?

  if missing.any?
    raise TaskerCore::Errors::PermanentError.new(
      "Missing required fields for refund validation: #{missing.join(', ')}",
      error_code: 'MISSING_REQUIRED_FIELDS'
    )
  end

  # Ticket status simulation
  case ticket_id.to_s
  when /ticket_closed/
    raise TaskerCore::Errors::PermanentError.new('Cannot process refund for closed ticket', error_code: 'TICKET_CLOSED')
  when /ticket_cancelled/
    raise TaskerCore::Errors::PermanentError.new('Cannot process refund for cancelled ticket', error_code: 'TICKET_CANCELLED')
  when /ticket_duplicate/
    raise TaskerCore::Errors::PermanentError.new('Cannot process refund for duplicate ticket', error_code: 'TICKET_DUPLICATE')
  end

  customer_tier = case customer_id.to_s
                  when /vip/, /premium/ then 'premium'
                  when /gold/ then 'gold'
                  else 'standard'
                  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      request_validated: true,
      ticket_id: ticket_id,
      customer_id: customer_id,
      ticket_status: 'open',
      customer_tier: customer_tier,
      original_purchase_date: (Time.now - (30 * 24 * 3600)).utc.iso8601,
      payment_id: "pay_#{SecureRandom.hex(8)}",
      validation_timestamp: Time.now.utc.iso8601,
      namespace: 'customer_success'
    },
    metadata: {
      operation: 'validate_refund_request',
      input_refs: {
        ticket_id: 'context.get_input("ticket_id")',
        customer_id: 'context.get_input("customer_id")',
        refund_amount: 'context.get_input("refund_amount")',
        refund_reason: 'context.get_input("refund_reason")'
      }
    }
  )
end

CsCheckRefundPolicyDslHandler = step_handler(
  'customer_success_dsl.step_handlers.check_refund_policy',
  depends_on: { validation: 'validate_refund_request_dsl' },
  inputs: %i[refund_amount refund_reason]
) do |validation:, refund_amount:, refund_reason:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  validation = deep_sym.call(validation) if validation

  unless validation&.dig(:request_validated)
    raise TaskerCore::Errors::PermanentError.new('Request validation must be completed', error_code: 'MISSING_VALIDATION')
  end

  customer_tier = (validation[:customer_tier] || 'standard').to_sym
  policy = CS_REFUND_POLICIES_DSL[customer_tier] || CS_REFUND_POLICIES_DSL[:standard]

  purchase_date = Time.parse(validation[:original_purchase_date])
  days_since_purchase = ((Time.now - purchase_date) / 86_400).to_i
  within_window = days_since_purchase <= policy[:window_days]

  refund_amount ||= context.get_input('refund_amount')
  within_amount_limit = refund_amount <= policy[:max_amount]

  unless within_window
    raise TaskerCore::Errors::PermanentError.new(
      "Refund request outside policy window: #{days_since_purchase} days (max: #{policy[:window_days]} days)",
      error_code: 'OUTSIDE_REFUND_WINDOW'
    )
  end

  unless within_amount_limit
    raise TaskerCore::Errors::PermanentError.new(
      'Refund amount exceeds policy limit',
      error_code: 'EXCEEDS_AMOUNT_LIMIT'
    )
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      policy_checked: true,
      policy_compliant: true,
      customer_tier: customer_tier.to_s,
      refund_window_days: policy[:window_days],
      days_since_purchase: days_since_purchase,
      within_refund_window: within_window,
      requires_approval: policy[:requires_approval],
      max_allowed_amount: policy[:max_amount],
      policy_checked_at: Time.now.utc.iso8601,
      namespace: 'customer_success'
    },
    metadata: { operation: 'check_refund_policy' }
  )
end

CsGetManagerApprovalDslHandler = step_handler(
  'customer_success_dsl.step_handlers.get_manager_approval',
  depends_on: { policy: 'check_refund_policy_dsl', validation: 'validate_refund_request_dsl' },
  inputs: %i[refund_amount refund_reason]
) do |policy:, validation:, refund_amount:, refund_reason:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  policy = deep_sym.call(policy) if policy
  validation = deep_sym.call(validation) if validation

  unless policy&.dig(:policy_checked)
    raise TaskerCore::Errors::PermanentError.new('Policy check must be completed', error_code: 'MISSING_POLICY_CHECK')
  end

  requires_approval = policy[:requires_approval]

  if requires_approval
    approval_id = "appr_#{SecureRandom.hex(8)}"
    manager_id = "mgr_#{rand(1..5)}"

    result = {
      approval_obtained: true,
      approval_required: true,
      auto_approved: false,
      approval_id: approval_id,
      manager_id: manager_id,
      manager_notes: "Approved refund request for customer #{validation&.dig(:customer_id)}",
      approved_at: Time.now.utc.iso8601,
      namespace: 'customer_success'
    }
  else
    result = {
      approval_obtained: true,
      approval_required: false,
      auto_approved: true,
      approval_id: nil,
      manager_id: nil,
      manager_notes: nil,
      approved_at: Time.now.utc.iso8601,
      namespace: 'customer_success'
    }
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: { operation: 'get_manager_approval' }
  )
end

CsExecuteRefundWorkflowDslHandler = step_handler(
  'customer_success_dsl.step_handlers.execute_refund_workflow',
  depends_on: { approval: 'get_manager_approval_dsl', validation: 'validate_refund_request_dsl' },
  inputs: %i[refund_amount refund_reason customer_email ticket_id correlation_id]
) do |approval:, validation:, refund_amount:, refund_reason:, customer_email:, ticket_id:, correlation_id:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  approval = deep_sym.call(approval) if approval
  validation = deep_sym.call(validation) if validation

  unless approval&.dig(:approval_obtained)
    raise TaskerCore::Errors::PermanentError.new('Manager approval must be obtained', error_code: 'MISSING_APPROVAL')
  end

  payment_id = validation&.dig(:payment_id)
  unless payment_id
    raise TaskerCore::Errors::PermanentError.new('Payment ID not found', error_code: 'MISSING_PAYMENT_ID')
  end

  correlation_id ||= "cs-#{SecureRandom.hex(8)}"
  task_id = "task_#{SecureRandom.uuid}"

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      task_delegated: true,
      target_namespace: 'payments',
      target_workflow: 'process_refund',
      delegated_task_id: task_id,
      delegated_task_status: 'created',
      delegation_timestamp: Time.now.utc.iso8601,
      correlation_id: correlation_id,
      namespace: 'customer_success'
    },
    metadata: { operation: 'execute_refund_workflow' }
  )
end

CsUpdateTicketStatusDslHandler = step_handler(
  'customer_success_dsl.step_handlers.update_ticket_status',
  depends_on: { delegation: 'execute_refund_workflow_dsl', validation: 'validate_refund_request_dsl' },
  inputs: %i[refund_amount refund_reason]
) do |delegation:, validation:, refund_amount:, refund_reason:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  delegation = deep_sym.call(delegation) if delegation
  validation = deep_sym.call(validation) if validation

  unless delegation&.dig(:task_delegated)
    raise TaskerCore::Errors::PermanentError.new('Refund workflow must be executed first', error_code: 'MISSING_DELEGATION')
  end

  ticket_id = validation&.dig(:ticket_id)
  delegated_task_id = delegation&.dig(:delegated_task_id)
  correlation_id = delegation&.dig(:correlation_id)
  refund_amount ||= context.get_input('refund_amount')

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      ticket_updated: true,
      ticket_id: ticket_id,
      previous_status: 'in_progress',
      new_status: 'resolved',
      resolution_note: "Refund of $#{format('%.2f', (refund_amount || 0) / 100.0)} processed successfully. " \
                       "Delegated task ID: #{delegated_task_id}. " \
                       "Correlation ID: #{correlation_id}",
      updated_at: Time.now.utc.iso8601,
      refund_completed: true,
      delegated_task_id: delegated_task_id,
      namespace: 'customer_success'
    },
    metadata: { operation: 'update_ticket_status' }
  )
end
