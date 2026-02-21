# frozen_string_literal: true

# DSL mirror of ConditionalApproval::StepHandlers using block DSL.
#
# 6 handlers: validate_request, routing_decision, auto_approve,
#             manager_approval, finance_review, finalize_approval
#
# NOTE: routing_decision uses decision_handler DSL.
# finalize_approval accesses dependency_results directly for dynamic convergence.

include TaskerCore::StepHandler::Functional # rubocop:disable Style/MixinUsage

ApprovalValidateRequestDslHandler = step_handler(
  'conditional_approval_dsl_rb.step_handlers.validate_request',
  inputs: %i[amount requester purpose]
) do |amount:, requester:, purpose:, context:|
  amount ||= context.task.context['amount']
  requester ||= context.task.context['requester']
  purpose ||= context.task.context['purpose']

  raise 'Task context must contain amount' unless amount
  raise 'Amount must be positive' unless amount.positive?
  raise 'Task context must contain requester' unless requester && !requester.empty?
  raise 'Task context must contain purpose' unless purpose && !purpose.empty?

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      amount: amount,
      requester: requester,
      purpose: purpose,
      validated_at: Time.now.iso8601
    },
    metadata: {
      operation: 'validate',
      step_type: 'initial',
      validation_checks: %w[amount_positive requester_present purpose_present]
    }
  )
end

SMALL_AMOUNT_THRESHOLD_DSL = 1_000
LARGE_AMOUNT_THRESHOLD_DSL = 5_000

ApprovalRoutingDecisionDslHandler = decision_handler(
  'conditional_approval_dsl_rb.step_handlers.routing_decision',
  inputs: [:amount]
) do |amount:, context:|
  amount ||= context.task.context['amount']
  raise 'Amount is required for routing decision' unless amount

  if amount < SMALL_AMOUNT_THRESHOLD_DSL
    Decision.route(['auto_approve_dsl'],
                   route_type: 'auto_approval',
                   reasoning: "Amount $#{amount} below $#{SMALL_AMOUNT_THRESHOLD_DSL} threshold - auto-approval")
  elsif amount < LARGE_AMOUNT_THRESHOLD_DSL
    Decision.route(['manager_approval_dsl'],
                   route_type: 'manager_only',
                   reasoning: "Amount $#{amount} requires manager approval (between $#{SMALL_AMOUNT_THRESHOLD_DSL} and $#{LARGE_AMOUNT_THRESHOLD_DSL})")
  else
    Decision.route(%w[manager_approval_dsl finance_review_dsl],
                   route_type: 'dual_approval',
                   reasoning: "Amount $#{amount} >= $#{LARGE_AMOUNT_THRESHOLD_DSL} - requires both manager and finance approval")
  end
end

ApprovalAutoApproveDslHandler = step_handler(
  'conditional_approval_dsl_rb.step_handlers.auto_approve',
  inputs: %i[amount requester]
) do |amount:, requester:, context:| # rubocop:disable Lint/UnusedBlockArgument
  amount ||= context.task.context['amount']

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      approved: true,
      approval_type: 'automatic',
      approved_amount: amount,
      approved_by: 'system',
      approved_at: Time.now.iso8601,
      notes: 'Automatically approved - below manual review threshold'
    },
    metadata: {
      operation: 'auto_approve',
      step_type: 'dynamic_branch',
      approval_method: 'automated'
    }
  )
end

ApprovalManagerApprovalDslHandler = step_handler(
  'conditional_approval_dsl_rb.step_handlers.manager_approval',
  inputs: %i[amount requester purpose]
) do |amount:, requester:, purpose:, context:| # rubocop:disable Lint/UnusedBlockArgument
  amount ||= context.task.context['amount']

  approved = amount <= 10_000

  if approved
    TaskerCore::Types::StepHandlerCallResult.success(
      result: {
        approved: true,
        approval_type: 'manager',
        approved_amount: amount,
        approved_by: 'manager_system',
        approved_at: Time.now.iso8601,
        notes: 'Approved by manager review'
      },
      metadata: {
        operation: 'manager_approval',
        step_type: 'dynamic_branch',
        approval_method: 'manual_review',
        reviewer_role: 'manager'
      }
    )
  else
    raise TaskerCore::Errors::PermanentError.new(
      "Amount $#{amount} exceeds manager approval limit of $10,000",
      error_code: 'AMOUNT_EXCEEDS_MANAGER_LIMIT',
      context: { amount: amount, limit: 10_000 }
    )
  end
end

ApprovalFinanceReviewDslHandler = step_handler(
  'conditional_approval_dsl_rb.step_handlers.finance_review',
  inputs: %i[amount requester purpose]
) do |amount:, requester:, purpose:, context:| # rubocop:disable Lint/UnusedBlockArgument
  amount ||= context.task.context['amount']
  purpose ||= context.task.context['purpose']

  budget_available = amount <= 25_000

  if budget_available
    budget_code = "BUDGET-#{purpose.upcase.gsub(/\s+/, '_')}-#{Time.now.to_i}"
    TaskerCore::Types::StepHandlerCallResult.success(
      result: {
        approved: true,
        approval_type: 'finance',
        approved_amount: amount,
        approved_by: 'finance_system',
        approved_at: Time.now.iso8601,
        notes: 'Approved by finance review - budget confirmed',
        budget_code: budget_code
      },
      metadata: {
        operation: 'finance_review',
        step_type: 'dynamic_branch',
        approval_method: 'financial_review',
        reviewer_role: 'finance',
        compliance_checks: %w[budget_available compliance_verified]
      }
    )
  else
    raise TaskerCore::Errors::PermanentError.new(
      "Insufficient budget for $#{amount} request",
      error_code: 'INSUFFICIENT_BUDGET',
      context: { amount: amount, purpose: purpose }
    )
  end
end

ApprovalFinalizeApprovalDslHandler = step_handler(
  'conditional_approval_dsl_rb.step_handlers.finalize_approval',
  inputs: %i[amount requester purpose]
) do |amount:, requester:, purpose:, context:|
  amount ||= context.task.context['amount']
  requester ||= context.task.context['requester']
  purpose ||= context.task.context['purpose']

  # Collect approval results from dynamic dependencies
  approval_steps = %w[auto_approve_dsl manager_approval_dsl finance_review_dsl]
  approvals = approval_steps.filter_map do |step_name|
    result_data = context.get_dependency_result(step_name)
    next unless result_data.is_a?(Hash) && (result_data['approved'] == true || result_data[:approved] == true)

    result_data
  end

  unless approvals.all? { |a| a['approved'] || a[:approved] }
    raise TaskerCore::Errors::PermanentError.new(
      'Not all required approvals were granted',
      error_code: 'APPROVAL_DENIED',
      context: { approvals: approvals }
    )
  end

  # Determine approval path
  types = approvals.map { |a| a[:approval_type] || a['approval_type'] }.sort
  approval_path = case types
                  when ['automatic'] then 'auto'
                  when ['manager'] then 'manager_only'
                  when %w[finance manager] then 'dual_approval'
                  else 'unknown'
                  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      approved: true,
      final_amount: amount,
      requester: requester,
      purpose: purpose,
      approval_chain: approvals.map { |a| a[:approval_type] || a['approval_type'] },
      approved_by: approvals.map { |a| a[:approved_by] || a['approved_by'] },
      finalized_at: Time.now.iso8601,
      approval_path: approval_path
    },
    metadata: {
      operation: 'finalize_approval',
      step_type: 'convergence',
      approval_count: approvals.size
    }
  )
end
