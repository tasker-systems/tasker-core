# frozen_string_literal: true

# DSL mirror of ResolverTests::StepHandlers::MultiMethodHandler using block DSL.
#
# The verbose handler has multiple methods: call, validate, process, refund.
# In the functional DSL, each method becomes its own step_handler.
#
# Also includes AlternateMethodHandler mirrors: call, execute_action.

include TaskerCore::StepHandler::Functional # rubocop:disable Style/MixinUsage

MultiMethodCallDslHandler = step_handler(
  'resolver_tests_dsl_rb.step_handlers.multi_method_call'
) do |context:|
  input = context.task.context['data'] || {}

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      invoked_method: 'call',
      handler: 'MultiMethodCallDslHandler',
      message: 'Default call method invoked',
      input_received: input,
      step_name: context.step_name
    }
  )
end

MultiMethodValidateDslHandler = step_handler(
  'resolver_tests_dsl_rb.step_handlers.multi_method_validate'
) do |context:|
  input = context.task.context['data'] || {}

  has_required_fields = input.key?('amount')

  unless has_required_fields
    next TaskerCore::Types::StepHandlerCallResult.failure(
      message: 'Validation failed: missing required field "amount"',
      error_type: 'validation_error',
      retryable: false
    )
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      invoked_method: 'validate',
      handler: 'MultiMethodValidateDslHandler',
      message: 'Validation completed successfully',
      validated: true,
      input_validated: input,
      step_name: context.step_name
    }
  )
end

MultiMethodProcessDslHandler = step_handler(
  'resolver_tests_dsl_rb.step_handlers.multi_method_process'
) do |context:|
  input = context.task.context['data'] || {}
  amount = input['amount'] || 0

  processed_amount = amount * 1.1

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      invoked_method: 'process',
      handler: 'MultiMethodProcessDslHandler',
      message: 'Processing completed',
      original_amount: amount,
      processed_amount: processed_amount,
      processing_fee: processed_amount - amount,
      step_name: context.step_name
    }
  )
end

MultiMethodRefundDslHandler = step_handler(
  'resolver_tests_dsl_rb.step_handlers.multi_method_refund'
) do |context:|
  input = context.task.context['data'] || {}
  amount = input['amount'] || 0
  reason = input['reason'] || 'not_specified'

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      invoked_method: 'refund',
      handler: 'MultiMethodRefundDslHandler',
      message: 'Refund processed',
      refund_amount: amount,
      refund_reason: reason,
      refund_id: "refund_#{Time.now.to_i}",
      step_name: context.step_name
    }
  )
end

AlternateMethodCallDslHandler = step_handler(
  'resolver_tests_dsl_rb.step_handlers.alternate_method_call'
) do |context:|
  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      invoked_method: 'call',
      handler: 'AlternateMethodCallDslHandler',
      message: 'Alternate handler default method',
      step_name: context.step_name
    }
  )
end

AlternateMethodExecuteActionDslHandler = step_handler(
  'resolver_tests_dsl_rb.step_handlers.alternate_method_execute_action'
) do |context:|
  action = context.task.context['action_type'] || 'default_action'

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      invoked_method: 'execute_action',
      handler: 'AlternateMethodExecuteActionDslHandler',
      message: 'Custom action executed',
      action_type: action,
      step_name: context.step_name
    }
  )
end
