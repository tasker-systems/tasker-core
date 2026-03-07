# frozen_string_literal: true

# DSL mirror of Ecommerce::StepHandlers using block DSL.
#
# 5 handlers: validate_cart, process_payment, update_inventory, create_order, send_confirmation
#
# NOTE: These handlers replicate the core validation and output structure
# but non-deterministic fields (random IDs, timestamps) differ between runs.
# Parity testing focuses on deterministic fields and error classification.

include TaskerCore::StepHandler::Functional # rubocop:disable Style/MixinUsage

ECOMMERCE_PRODUCTS_DSL = {
  1 => { id: 1, name: 'Widget A', price: 29.99, stock: 100, active: true },
  2 => { id: 2, name: 'Widget B', price: 49.99, stock: 50, active: true },
  3 => { id: 3, name: 'Widget C', price: 19.99, stock: 25, active: true },
  4 => { id: 4, name: 'Widget D', price: 39.99, stock: 0, active: true },
  5 => { id: 5, name: 'Widget E', price: 59.99, stock: 10, active: false }
}.freeze

EcommerceValidateCartDslHandler = step_handler(
  'ecommerce_dsl_rb.step_handlers.validate_cart',
  inputs: [:cart_items]
) do |cart_items:, context:|
  cart_items ||= context.get_input('cart_items')

  unless cart_items&.any?
    raise TaskerCore::Errors::PermanentError.new(
      'Cart items are required but were not provided',
      error_code: 'MISSING_CART_ITEMS'
    )
  end

  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  normalized = cart_items.map { |i| deep_sym.call(i) }

  normalized.each_with_index do |item, index|
    unless item[:product_id]
      raise TaskerCore::Errors::PermanentError.new(
        "Product ID is required for cart item #{index + 1}",
        error_code: 'MISSING_PRODUCT_ID'
      )
    end
    next if item[:quantity]&.positive?

    raise TaskerCore::Errors::PermanentError.new(
      "Valid quantity is required for cart item #{index + 1}",
      error_code: 'INVALID_QUANTITY'
    )
  end

  validated_items = normalized.map do |item|
    product = ECOMMERCE_PRODUCTS_DSL[item[:product_id]]

    unless product
      raise TaskerCore::Errors::PermanentError.new(
        "Product #{item[:product_id]} not found",
        error_code: 'PRODUCT_NOT_FOUND'
      )
    end

    unless product[:active]
      raise TaskerCore::Errors::PermanentError.new(
        "Product #{product[:name]} is no longer available",
        error_code: 'PRODUCT_INACTIVE'
      )
    end

    if product[:stock] < item[:quantity]
      raise TaskerCore::Errors::RetryableError.new(
        "Insufficient stock for #{product[:name]}. Available: #{product[:stock]}, Requested: #{item[:quantity]}",
        retry_after: 30,
        context: {
          product_id: product[:id],
          product_name: product[:name],
          available_stock: product[:stock],
          requested_quantity: item[:quantity]
        }
      )
    end

    {
      product_id: product[:id],
      name: product[:name],
      price: product[:price],
      quantity: item[:quantity],
      line_total: (product[:price] * item[:quantity]).round(2)
    }
  end

  subtotal = validated_items.sum { |item| item[:line_total] }
  tax_rate = 0.08
  tax = (subtotal * tax_rate).round(2)
  total_weight = validated_items.sum { |item| item[:quantity] * 0.5 }
  shipping = case total_weight
             when 0..2 then 5.99
             when 2..10 then 9.99
             else 14.99
             end
  total = subtotal + tax + shipping

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      validated_items: validated_items,
      subtotal: subtotal,
      tax: tax,
      shipping: shipping,
      total: total,
      item_count: validated_items.length,
      validated_at: Time.now.utc.iso8601
    },
    metadata: {
      operation: 'validate_cart',
      execution_hints: {
        items_validated: validated_items.length,
        total_amount: total,
        tax_rate: tax_rate,
        shipping_cost: shipping
      },
      input_refs: {
        cart_items: 'context.get_input("cart_items")'
      }
    }
  )
end

EcommerceProcessPaymentDslHandler = step_handler(
  'ecommerce_dsl_rb.step_handlers.process_payment',
  depends_on: { validate_cart: 'validate_cart_dsl' },
  inputs: [:payment_info]
) do |validate_cart:, payment_info:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  payment_info = deep_sym.call(payment_info) if payment_info
  validate_cart = deep_sym.call(validate_cart) if validate_cart

  payment_method = payment_info&.dig(:method)
  payment_token = payment_info&.dig(:token)
  amount_to_charge = validate_cart&.dig(:total)

  unless payment_method
    raise TaskerCore::Errors::PermanentError.new('Payment method is required', error_code: 'MISSING_PAYMENT_METHOD')
  end
  unless payment_token
    raise TaskerCore::Errors::PermanentError.new('Payment token is required', error_code: 'MISSING_PAYMENT_TOKEN')
  end
  unless amount_to_charge
    raise TaskerCore::Errors::PermanentError.new('Cart total is required', error_code: 'MISSING_CART_TOTAL')
  end

  payment_id = "pay_#{SecureRandom.hex(12)}"
  transaction_id = "txn_#{SecureRandom.hex(12)}"

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      payment_id: payment_id,
      amount_charged: amount_to_charge,
      currency: 'USD',
      payment_method_type: payment_method,
      transaction_id: transaction_id,
      processed_at: Time.now.utc.iso8601,
      status: 'completed'
    },
    metadata: {
      operation: 'process_payment',
      input_refs: {
        amount: 'context.get_dependency_field("validate_cart_dsl", "total")',
        payment_info: 'context.get_input("payment_info")'
      }
    }
  )
end

EcommerceUpdateInventoryDslHandler = step_handler(
  'ecommerce_dsl_rb.step_handlers.update_inventory',
  depends_on: { validate_cart: 'validate_cart_dsl' },
  inputs: [:customer_info]
) do |validate_cart:, customer_info:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  validate_cart = deep_sym.call(validate_cart) if validate_cart
  customer_info = deep_sym.call(customer_info) if customer_info

  validated_items = validate_cart&.dig(:validated_items)

  unless validated_items&.any?
    raise TaskerCore::Errors::PermanentError.new(
      'Validated cart items are required',
      error_code: 'MISSING_VALIDATED_ITEMS'
    )
  end

  unless customer_info
    raise TaskerCore::Errors::PermanentError.new(
      'Customer information is required',
      error_code: 'MISSING_CUSTOMER_INFO'
    )
  end

  updated_products = validated_items.map do |item|
    product = ECOMMERCE_PRODUCTS_DSL[item[:product_id]]
    stock_level = product[:stock]
    reservation_id = "rsv_#{SecureRandom.hex(8)}"

    {
      product_id: product[:id],
      name: product[:name],
      previous_stock: stock_level,
      new_stock: stock_level - item[:quantity],
      quantity_reserved: item[:quantity],
      reservation_id: reservation_id
    }
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      updated_products: updated_products,
      total_items_reserved: updated_products.sum { |p| p[:quantity_reserved] },
      inventory_log_id: "log_#{SecureRandom.hex(8)}",
      updated_at: Time.now.utc.iso8601
    },
    metadata: {
      operation: 'update_inventory',
      input_refs: {
        validated_items: 'context.get_dependency_field("validate_cart_dsl", "validated_items")',
        customer_info: 'context.get_input("customer_info")'
      }
    }
  )
end

EcommerceCreateOrderDslHandler = step_handler(
  'ecommerce_dsl_rb.step_handlers.create_order',
  depends_on: { validate_cart: 'validate_cart_dsl', process_payment: 'process_payment_dsl', update_inventory: 'update_inventory_dsl' },
  inputs: [:customer_info]
) do |validate_cart:, process_payment:, update_inventory:, customer_info:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  customer_info = deep_sym.call(customer_info) if customer_info
  validate_cart = deep_sym.call(validate_cart) if validate_cart
  process_payment = deep_sym.call(process_payment) if process_payment
  update_inventory = deep_sym.call(update_inventory) if update_inventory

  unless customer_info
    raise TaskerCore::Errors::PermanentError.new('Customer information is required', error_code: 'MISSING_CUSTOMER_INFO')
  end
  unless validate_cart&.dig(:validated_items)&.any?
    raise TaskerCore::Errors::PermanentError.new('Cart validation results are required', error_code: 'MISSING_CART_VALIDATION')
  end
  unless process_payment&.dig(:payment_id)
    raise TaskerCore::Errors::PermanentError.new('Payment results are required', error_code: 'MISSING_PAYMENT_RESULT')
  end
  unless update_inventory&.dig(:updated_products)&.any?
    raise TaskerCore::Errors::PermanentError.new('Inventory results are required', error_code: 'MISSING_INVENTORY_RESULT')
  end

  order_id = rand(1000..9999)
  order_number = "ORD-#{Date.today.strftime('%Y%m%d')}-#{SecureRandom.hex(4).upcase}"

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      order_id: order_id,
      order_number: order_number,
      status: 'confirmed',
      total_amount: validate_cart[:total],
      customer_email: customer_info[:email],
      created_at: Time.now.iso8601,
      estimated_delivery: (Time.now + (7 * 24 * 60 * 60)).strftime('%B %d, %Y')
    },
    metadata: {
      operation: 'create_order',
      input_refs: {
        customer_info: 'context.get_input("customer_info")',
        cart_validation: 'context.get_dependency_result("validate_cart_dsl")',
        payment_result: 'context.get_dependency_result("process_payment_dsl")',
        inventory_result: 'context.get_dependency_result("update_inventory_dsl")'
      }
    }
  )
end

EcommerceSendConfirmationDslHandler = step_handler(
  'ecommerce_dsl_rb.step_handlers.send_confirmation',
  depends_on: { create_order: 'create_order_dsl', validate_cart: 'validate_cart_dsl' },
  inputs: [:customer_info]
) do |create_order:, validate_cart:, customer_info:, context:| # rubocop:disable Lint/UnusedBlockArgument
  deep_sym = lambda { |obj|
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  customer_info = deep_sym.call(customer_info) if customer_info
  create_order = deep_sym.call(create_order) if create_order
  validate_cart = deep_sym.call(validate_cart) if validate_cart

  unless customer_info&.dig(:email)
    raise TaskerCore::Errors::PermanentError.new('Customer email is required', error_code: 'MISSING_CUSTOMER_EMAIL')
  end
  unless create_order&.dig(:order_id)
    raise TaskerCore::Errors::PermanentError.new('Order results are required', error_code: 'MISSING_ORDER_RESULT')
  end
  unless validate_cart&.dig(:validated_items)&.any?
    raise TaskerCore::Errors::PermanentError.new('Cart validation results are required', error_code: 'MISSING_CART_VALIDATION')
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      email_sent: true,
      recipient: customer_info[:email],
      email_type: 'order_confirmation',
      sent_at: Time.now.utc.iso8601,
      message_id: "mock_#{SecureRandom.hex(8)}"
    },
    metadata: {
      operation: 'send_confirmation_email',
      input_refs: {
        customer_info: 'context.get_input("customer_info")',
        order_result: 'context.get_dependency_result("create_order_dsl")',
        cart_validation: 'context.get_dependency_result("validate_cart_dsl")'
      }
    }
  )
end
