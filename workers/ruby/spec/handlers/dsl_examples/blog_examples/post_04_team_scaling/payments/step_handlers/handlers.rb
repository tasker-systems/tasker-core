# frozen_string_literal: true

# DSL mirror of Payments::StepHandlers using block DSL.
#
# 4 handlers: validate_payment_eligibility, process_gateway_refund,
#             update_payment_records, notify_customer
#
# NOTE: Non-deterministic fields differ between runs.

include TaskerCore::StepHandler::Functional # rubocop:disable Style/MixinUsage

PaymentsValidateEligibilityDslHandler = step_handler(
  'payments_dsl.step_handlers.validate_payment_eligibility',
  inputs: %i[payment_id refund_amount refund_reason]
) do |payment_id:, refund_amount:, refund_reason:, context:| # rubocop:disable Lint/UnusedBlockArgument
  payment_id ||= context.get_input('payment_id')
  refund_amount ||= context.get_input('refund_amount')

  missing = []
  missing << 'payment_id' unless payment_id && !payment_id.to_s.empty?
  missing << 'refund_amount' unless refund_amount && !refund_amount.to_s.empty?

  if missing.any?
    raise TaskerCore::Errors::PermanentError.new(
      "Missing required fields for payment validation: #{missing.join(', ')}",
      error_code: 'MISSING_REQUIRED_FIELDS'
    )
  end

  if refund_amount <= 0
    raise TaskerCore::Errors::PermanentError.new(
      "Refund amount must be positive, got: #{refund_amount}",
      error_code: 'INVALID_REFUND_AMOUNT'
    )
  end

  unless payment_id.match?(/^pay_[a-zA-Z0-9_]+$/)
    raise TaskerCore::Errors::PermanentError.new(
      "Invalid payment ID format: #{payment_id}",
      error_code: 'INVALID_PAYMENT_ID'
    )
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      payment_validated: true,
      payment_id: payment_id,
      original_amount: refund_amount + 1000,
      refund_amount: refund_amount,
      payment_method: 'credit_card',
      gateway_provider: 'MockPaymentGateway',
      eligibility_status: 'eligible',
      validation_timestamp: Time.now.utc.iso8601,
      namespace: 'payments'
    },
    metadata: { operation: 'validate_payment_eligibility' }
  )
end

PaymentsProcessGatewayRefundDslHandler = step_handler(
  'payments_dsl.step_handlers.process_gateway_refund',
  depends_on: { validation: 'validate_payment_eligibility_dsl' },
  inputs: %i[refund_reason]
) do |validation:, refund_reason:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  validation = deep_sym.call(validation) if validation

  unless validation&.dig(:payment_validated)
    raise TaskerCore::Errors::PermanentError.new('Payment validation must be completed', error_code: 'MISSING_VALIDATION')
  end

  payment_id = validation[:payment_id]
  refund_amount = validation[:refund_amount]
  refund_id = "rfnd_#{SecureRandom.hex(12)}"
  gateway_txn_id = "gtx_#{SecureRandom.hex(10)}"

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      refund_processed: true,
      refund_id: refund_id,
      payment_id: payment_id,
      refund_amount: refund_amount,
      refund_status: 'processed',
      gateway_transaction_id: gateway_txn_id,
      gateway_provider: 'MockPaymentGateway',
      processed_at: Time.now.utc.iso8601,
      estimated_arrival: (Time.now + (5 * 24 * 3600)).utc.iso8601,
      namespace: 'payments'
    },
    metadata: { operation: 'process_gateway_refund' }
  )
end

PaymentsUpdateRecordsDslHandler = step_handler(
  'payments_dsl.step_handlers.update_payment_records',
  depends_on: { refund: 'process_gateway_refund_dsl', validation: 'validate_payment_eligibility_dsl' },
  inputs: [:refund_reason]
) do |refund:, validation:, refund_reason:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  refund = deep_sym.call(refund) if refund

  unless refund&.dig(:refund_processed)
    raise TaskerCore::Errors::PermanentError.new('Gateway refund must be completed', error_code: 'MISSING_REFUND')
  end

  payment_id = refund[:payment_id]
  refund_id = refund[:refund_id]
  record_id = "rec_#{SecureRandom.hex(8)}"

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      records_updated: true,
      payment_id: payment_id,
      refund_id: refund_id,
      record_id: record_id,
      payment_status: 'refunded',
      refund_status: 'completed',
      history_entries_created: 2,
      updated_at: Time.now.utc.iso8601,
      namespace: 'payments'
    },
    metadata: { operation: 'update_payment_records' }
  )
end

PaymentsNotifyCustomerDslHandler = step_handler(
  'payments_dsl.step_handlers.notify_customer',
  depends_on: { refund: 'process_gateway_refund_dsl' },
  inputs: %i[customer_email refund_reason]
) do |refund:, customer_email:, refund_reason:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  refund = deep_sym.call(refund) if refund

  unless refund&.dig(:refund_processed)
    raise TaskerCore::Errors::PermanentError.new('Refund must be processed first', error_code: 'MISSING_REFUND')
  end

  customer_email ||= context.get_input('customer_email')
  unless customer_email
    raise TaskerCore::Errors::PermanentError.new('Customer email is required', error_code: 'MISSING_CUSTOMER_EMAIL')
  end

  unless customer_email.match?(/\A[^@\s]+@[^@\s]+\z/)
    raise TaskerCore::Errors::PermanentError.new("Invalid email format: #{customer_email}", error_code: 'INVALID_EMAIL_FORMAT')
  end

  message_id = "msg_#{SecureRandom.hex(12)}"

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      notification_sent: true,
      customer_email: customer_email,
      message_id: message_id,
      notification_type: 'refund_confirmation',
      sent_at: Time.now.utc.iso8601,
      delivery_status: 'delivered',
      refund_id: refund[:refund_id],
      refund_amount: refund[:refund_amount],
      namespace: 'payments'
    },
    metadata: { operation: 'notify_customer' }
  )
end
