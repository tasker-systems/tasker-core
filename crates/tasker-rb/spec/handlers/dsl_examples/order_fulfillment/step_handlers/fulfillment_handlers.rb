# frozen_string_literal: true

# DSL mirror of OrderFulfillment::StepHandlers using block DSL.
#
# 4 handlers: validate_order -> reserve_inventory -> process_payment -> ship_order
#
# NOTE: The verbose handlers have complex simulation logic with random IDs.
# DSL handlers reproduce the same validation and core output structure
# but deterministic fields only are tested for parity.

include TaskerCore::StepHandler::Functional # rubocop:disable Style/MixinUsage

OrderValidateOrderDslHandler = step_handler('order_fulfillment_dsl.step_handlers.validate_order') do |context:|
  task_context = context.task.context
  customer_info = task_context['customer_info']
  order_items = task_context['order_items']

  # Deep symbolize keys helper
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  customer_info = deep_sym.call(customer_info) if customer_info
  order_items = deep_sym.call(order_items) if order_items

  raise TaskerCore::Errors::PermanentError, 'Customer ID is required' unless customer_info&.dig(:id)
  raise TaskerCore::Errors::PermanentError, 'Customer email is required' unless customer_info&.dig(:email)
  raise TaskerCore::Errors::PermanentError, 'Order items are required' unless order_items&.any?

  products = {
    101 => { name: 'Premium Widget A', stock: 100, category: 'widgets' },
    102 => { name: 'Deluxe Widget B', stock: 50, category: 'widgets' },
    103 => { name: 'Standard Gadget C', stock: 200, category: 'gadgets' }
  }

  validated_items = order_items.map.with_index do |item, index|
    unless item[:product_id] && item[:quantity] && item[:price]
      raise TaskerCore::Errors::PermanentError, "Invalid order item at position #{index + 1}: missing required fields"
    end
    raise TaskerCore::Errors::PermanentError, 'Invalid quantity' if item[:quantity] <= 0
    raise TaskerCore::Errors::PermanentError, 'Invalid price' if item[:price] < 0

    product = products[item[:product_id]]
    raise TaskerCore::Errors::PermanentError, "Product #{item[:product_id]} not found" unless product

    {
      product_id: item[:product_id], product_name: product[:name],
      quantity: item[:quantity], unit_price: item[:price],
      line_total: item[:price] * item[:quantity],
      available_stock: product[:stock], category: product[:category]
    }
  end

  total_amount = validated_items.sum { |i| i[:line_total] }
  raise TaskerCore::Errors::PermanentError, 'Order total exceeds maximum' if total_amount > 50_000.00

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      customer_validated: true,
      customer_id: customer_info[:id],
      validated_items: validated_items,
      order_total: total_amount,
      validation_status: 'complete',
      validated_at: Time.now.iso8601
    },
    metadata: {
      operation: 'validate_order',
      item_count: validated_items.length
    }
  )
end

OrderReserveInventoryDslHandler = step_handler('order_fulfillment_dsl.step_handlers.reserve_inventory',
                                               depends_on: { validate_order: 'validate_order_dsl' }) do |validate_order:, context:| # rubocop:disable Lint/UnusedBlockArgument
  raise TaskerCore::Errors::PermanentError, 'validate_order step results not found' unless validate_order

  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  validated_items = validate_order[:validated_items] || validate_order['validated_items']
  raise TaskerCore::Errors::PermanentError, 'No validated items found' unless validated_items&.any?

  validated_items = validated_items.map { |i| deep_sym.call(i) }
  reservation_id = "RES-#{Time.now.to_i}-#{SecureRandom.hex(4).upcase}"
  expires_at = (Time.now + (15 * 60)).iso8601

  locations = { 101 => 'WH-EAST-A1', 102 => 'WH-WEST-B2', 103 => 'WH-CENTRAL-C3' }

  reservations = validated_items.map do |item|
    {
      product_id: item[:product_id], product_name: item[:product_name],
      quantity_requested: item[:quantity], quantity_reserved: item[:quantity],
      unit_price: item[:unit_price], line_total: item[:line_total],
      stock_location: locations[item[:product_id]] || 'WH-DEFAULT',
      reservation_reference: "#{reservation_id}-#{item[:product_id]}"
    }
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      reservation_id: reservation_id,
      items_reserved: reservations.length,
      reservation_status: 'confirmed',
      total_reserved_value: reservations.sum { |r| r[:line_total] },
      expires_at: expires_at,
      reserved_at: Time.now.iso8601,
      reservation_details: reservations
    },
    metadata: { operation: 'reserve_inventory', items_count: reservations.length }
  )
end

OrderProcessPaymentDslHandler = step_handler('order_fulfillment_dsl.step_handlers.process_payment',
                                             depends_on: { validate_order: 'validate_order_dsl',
                                                           reserve_inventory: 'reserve_inventory_dsl' }) do |validate_order:, reserve_inventory:, context:|
  raise TaskerCore::Errors::PermanentError, 'validate_order results not found' unless validate_order
  raise TaskerCore::Errors::PermanentError, 'reserve_inventory results not found' unless reserve_inventory

  task_ctx = context.task.context
  payment_info = task_ctx['payment_info']
  raise TaskerCore::Errors::PermanentError, 'Payment information is required' unless payment_info
  unless payment_info['method'] || payment_info[:method]
    raise TaskerCore::Errors::PermanentError, 'Payment method is required'
  end
  unless payment_info['token'] || payment_info[:token]
    raise TaskerCore::Errors::PermanentError, 'Payment token is required'
  end

  order_total = validate_order[:order_total] || validate_order['order_total']
  payment_id = "PAY-#{Time.now.to_i}-#{SecureRandom.hex(6).upcase}"
  txn_id = "TXN-#{SecureRandom.hex(8).upcase}"

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      payment_processed: true,
      payment_id: payment_id,
      transaction_id: txn_id,
      amount_charged: order_total,
      payment_method_used: payment_info['method'] || payment_info[:method],
      gateway_response: { status: 'succeeded', transaction_id: txn_id, amount: order_total },
      processed_at: Time.now.iso8601,
      payment_status: 'completed'
    },
    metadata: { operation: 'process_payment' }
  )
end

OrderShipOrderDslHandler = step_handler('order_fulfillment_dsl.step_handlers.ship_order',
                                        depends_on: { validate_order: 'validate_order_dsl',
                                                      reserve_inventory: 'reserve_inventory_dsl',
                                                      process_payment: 'process_payment_dsl' }) do |validate_order:, reserve_inventory:, process_payment:, context:|
  raise TaskerCore::Errors::PermanentError, 'validate_order results not found' unless validate_order
  raise TaskerCore::Errors::PermanentError, 'reserve_inventory results not found' unless reserve_inventory
  raise TaskerCore::Errors::PermanentError, 'process_payment results not found' unless process_payment

  task_ctx = context.task.context
  shipping_info = task_ctx['shipping_info']
  raise TaskerCore::Errors::PermanentError, 'Shipping information is required' unless shipping_info

  shipment_id = "SHIP-#{Time.now.to_i}-#{SecureRandom.hex(4).upcase}"
  tracking = "UPS#{SecureRandom.hex(6).upcase}"

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      shipment_id: shipment_id,
      tracking_number: tracking,
      shipping_status: 'label_created',
      estimated_delivery: (Time.now.to_date + 5).iso8601,
      shipping_cost: 8.99,
      carrier: 'UPS',
      service_type: 'UPS Ground',
      label_url: "https://labels.ups.com/#{shipment_id}.pdf",
      processed_at: Time.now.iso8601
    },
    metadata: { operation: 'ship_order' }
  )
end
