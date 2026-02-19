# frozen_string_literal: true

# DSL mirror of DomainEvents::StepHandlers using block DSL.
#
# 4 handlers: validate_order, process_payment, update_inventory, send_notification
#
# NOTE: Domain event handlers use config[] for handler configuration.
# The DSL handlers replicate the same output structure but domain event
# publishing (which is handled by the worker's post-execution callback
# system) is outside the handler's concern.

include TaskerCore::StepHandler::Functional

DomainEventValidateOrderDslHandler = step_handler(
  'domain_events_dsl.step_handlers.validate_order'
) do |context:|
  ctx = context.task.context || {}
  order_id = ctx['order_id'] || SecureRandom.uuid
  customer_id = ctx['customer_id'] || 'unknown'
  amount = ctx['amount'] || 0

  validation_checks = %w[order_id_present customer_id_present]
  validation_checks << 'amount_positive' if amount.positive?

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      order_id: order_id,
      validation_timestamp: Time.now.iso8601,
      validation_checks: validation_checks,
      validated: true
    },
    metadata: {
      validation_mode: 'standard'
    }
  )
end

DomainEventProcessPaymentDslHandler = step_handler(
  'domain_events_dsl.step_handlers.process_payment'
) do |context:|
  ctx = context.task.context || {}
  order_id = ctx['order_id'] || 'unknown'
  amount = ctx['amount'] || 0
  simulate_failure = ctx['simulate_failure'] || false

  if simulate_failure
    TaskerCore::Types::StepHandlerCallResult.failure(
      error_message: 'Simulated payment failure',
      error_code: 'PAYMENT_DECLINED',
      error_type: 'PaymentError',
      retryable: true,
      metadata: {
        order_id: order_id,
        failed_at: Time.now.iso8601
      }
    )
  else
    transaction_id = "TXN-#{SecureRandom.uuid}"
    TaskerCore::Types::StepHandlerCallResult.success(
      result: {
        transaction_id: transaction_id,
        amount: amount,
        payment_method: 'credit_card',
        processed_at: Time.now.iso8601,
        status: 'success'
      },
      metadata: {
        payment_provider: 'mock'
      }
    )
  end
end

DomainEventUpdateInventoryDslHandler = step_handler(
  'domain_events_dsl.step_handlers.update_inventory'
) do |context:|
  ctx = context.task.context || {}
  order_id = ctx['order_id'] || 'unknown'

  items = [
    { sku: 'ITEM-001', quantity: 1 },
    { sku: 'ITEM-002', quantity: 2 }
  ]

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      order_id: order_id,
      items: items,
      success: true,
      updated_at: Time.now.iso8601
    },
    metadata: {
      inventory_source: 'mock'
    }
  )
end

DomainEventSendNotificationDslHandler = step_handler(
  'domain_events_dsl.step_handlers.send_notification'
) do |context:|
  ctx = context.task.context || {}
  customer_id = ctx['customer_id'] || 'unknown'

  notification_id = "NOTIF-#{SecureRandom.uuid}"

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      notification_id: notification_id,
      channel: 'email',
      recipient: customer_id,
      sent_at: Time.now.iso8601,
      status: 'delivered'
    },
    metadata: {
      notification_type: 'email'
    }
  )
end
