# frozen_string_literal: true

# DSL mirror of ErrorScenarios step handlers using block DSL.
#
# 3 handlers: success, permanent_error, retryable_error

include TaskerCore::StepHandler::Functional # rubocop:disable Style/MixinUsage

ErrorSuccessDslHandler = step_handler('error_scenarios_dsl.step_handlers.success') do |context:|
  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      status: 'success',
      message: 'Step completed successfully',
      timestamp: Time.now.utc.iso8601
    },
    metadata: {
      step_name: context.workflow_step.name,
      handler: 'ErrorSuccessDslHandler'
    }
  )
end

ErrorPermanentDslHandler = step_handler('error_scenarios_dsl.step_handlers.permanent_error') do |context:| # rubocop:disable Lint/UnusedBlockArgument
  raise TaskerCore::Errors::PermanentError.new(
    'Invalid payment method: test_invalid_card',
    context: {
      error_code: 'INVALID_PAYMENT_METHOD',
      reason: 'Test failure scenario',
      card_type: 'test_invalid_card',
      expected_behavior: 'No retries, immediate failure'
    }
  )
end

ErrorRetryableDslHandler = step_handler('error_scenarios_dsl.step_handlers.retryable_error') do |context:|
  retry_count = context.workflow_step.results&.dig('retry_count') || 0

  raise TaskerCore::Errors::RetryableError.new(
    "Payment service timeout after 30s (attempt #{retry_count + 1})",
    retry_after: 5,
    context: {
      error_code: 'SERVICE_TIMEOUT',
      reason: 'Test failure scenario',
      service: 'payment_gateway',
      timeout_seconds: 30,
      retry_count: retry_count,
      expected_behavior: 'Retry with exponential backoff'
    }
  )
end
