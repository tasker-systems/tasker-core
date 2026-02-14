# Ruby Guide

This guide covers using Tasker with Ruby step handlers via the `tasker-rb` gem.

## Quick Start

```bash
# Add to Gemfile
gem 'tasker-rb'

# Install
bundle install
```

## Writing a Step Handler

Ruby step handlers inherit from `TaskerCore::StepHandler::Base`:

```ruby path=null start=null
class MyHandler < TaskerCore::StepHandler::Base
  def call(context)
    # Your handler logic here
    TaskerCore::Types::StepHandlerCallResult.success(
      result: { processed: true }
    )
  end
end
```

### Minimal Handler Example

```ruby path=null start=null
class LinearStep1Handler < TaskerCore::StepHandler::Base
  def call(context)
    # Access task context data using get_input()
    even_number = context.get_input('even_number')
    raise 'Task context must contain an even number' unless even_number&.even?

    # Perform business logic
    result = even_number * even_number

    logger.info "Linear Step 1: #{even_number}² = #{result}"

    # Return standardized result
    TaskerCore::Types::StepHandlerCallResult.success(
      result: result,
      metadata: {
        operation: 'square',
        step_type: 'initial'
      }
    )
  end
end
```

### Accessing Task Context

Use `get_input()` for task context access (cross-language standard API):

```ruby path=null start=null
# Get value from task context
customer_id = context.get_input('customer_id')

# Access nested task context
task_context = context.task.context
items = task_context['items']
```

### Accessing Dependency Results

Access results from upstream steps using `context.sequence`:

```ruby path=null start=null
# Get result from a specific upstream step
previous_result = context.sequence.get('previous_step_name')

# Access the computed result value
value = previous_result['result']

# Or use the dependency_results hash directly
all_results = context.dependency_results
```

## Complete Example: Order Validation Handler

This example shows a real-world e-commerce handler:

```ruby path=null start=null
module OrderFulfillment
  module StepHandlers
    class ValidateOrderHandler < TaskerCore::StepHandler::Base
      def call(context)
        logger.info "Starting order validation - task_uuid=#{context.task_uuid}"

        # Extract and validate inputs
        order_inputs = extract_and_validate_inputs(context)

        # Validate order data
        validation_results = validate_order_data(order_inputs)

        # Return success result
        TaskerCore::Types::StepHandlerCallResult.success(
          result: {
            customer_validated: true,
            customer_id: validation_results[:customer_id],
            validated_items: validation_results[:items],
            order_total: validation_results[:total],
            validation_status: 'complete',
            validated_at: Time.now.iso8601
          },
          metadata: {
            operation: 'validate_order',
            item_count: validation_results[:items]&.length || 0
          }
        )
      end

      private

      def extract_and_validate_inputs(context)
        task_context = deep_symbolize_keys(context.task.context)

        customer_info = task_context[:customer_info]
        order_items = task_context[:order_items]

        unless customer_info&.dig(:id)
          raise TaskerCore::Errors::PermanentError.new(
            'Customer ID is required',
            error_code: 'MISSING_CUSTOMER_ID'
          )
        end

        unless order_items&.any?
          raise TaskerCore::Errors::PermanentError.new(
            'Order items are required',
            error_code: 'MISSING_ORDER_ITEMS'
          )
        end

        {
          customer_id: customer_info[:id],
          customer_email: customer_info[:email],
          order_items: order_items
        }
      end

      def validate_order_data(inputs)
        validated_items = inputs[:order_items].map.with_index do |item, index|
          unless item[:product_id] && item[:quantity] && item[:price]
            raise TaskerCore::Errors::PermanentError.new(
              "Invalid order item at position #{index + 1}",
              error_code: 'INVALID_ORDER_ITEM'
            )
          end

          {
            product_id: item[:product_id],
            quantity: item[:quantity],
            unit_price: item[:price],
            line_total: item[:price] * item[:quantity]
          }
        end

        total_amount = validated_items.sum { |item| item[:line_total] }

        {
          items: validated_items,
          total: total_amount,
          customer_id: inputs[:customer_id]
        }
      end

      def deep_symbolize_keys(obj)
        case obj
        when Hash
          obj.each_with_object({}) do |(key, value), result|
            result[key.to_sym] = deep_symbolize_keys(value)
          end
        when Array
          obj.map { |item| deep_symbolize_keys(item) }
        else
          obj
        end
      end
    end
  end
end
```

## Error Handling

Use typed errors to control retry behavior:

```ruby path=null start=null
# Permanent error - will NOT be retried
raise TaskerCore::Errors::PermanentError.new(
  'Invalid order data',
  error_code: 'VALIDATION_ERROR',
  context: { field: 'customer_id' }
)

# Retryable error - will be retried up to max_attempts
raise TaskerCore::Errors::RetryableError.new(
  'Payment gateway timeout',
  retry_after: 30,  # seconds
  context: { gateway: 'stripe' }
)
```

## Task Template Configuration

Define workflows in YAML:

```yaml path=null start=null
name: order_fulfillment
namespace_name: ecommerce
version: 1.0.0
description: "E-commerce order processing workflow"

steps:
  - name: validate_order
    handler:
      callable: OrderFulfillment::StepHandlers::ValidateOrderHandler
    dependencies: []

  - name: reserve_inventory
    handler:
      callable: OrderFulfillment::StepHandlers::ReserveInventoryHandler
    dependencies:
      - validate_order

  - name: process_payment
    handler:
      callable: OrderFulfillment::StepHandlers::ProcessPaymentHandler
    dependencies:
      - validate_order
      - reserve_inventory

  - name: ship_order
    handler:
      callable: OrderFulfillment::StepHandlers::ShipOrderHandler
    dependencies:
      - validate_order
      - reserve_inventory
      - process_payment
```

## Handler Registration

Handlers are discovered automatically via the resolver chain from template `callable` fields. For explicit registration:

```ruby path=null start=null
# Register handlers by their callable name
registry = TaskerCore::Registry::HandlerRegistry.instance
registry.register_handler(
  'OrderFulfillment::StepHandlers::ValidateOrderHandler',
  OrderFulfillment::StepHandlers::ValidateOrderHandler
)
```

## Testing

Write RSpec tests for your handlers:

```ruby path=null start=null
require 'spec_helper'

RSpec.describe OrderFulfillment::StepHandlers::ValidateOrderHandler do
  let(:handler) { described_class.new }
  let(:context) do
    build_test_context(
      task_context: {
        customer_info: { id: 123, email: 'test@example.com' },
        order_items: [
          { product_id: 1, quantity: 2, price: 29.99 }
        ]
      }
    )
  end

  describe '#call' do
    it 'validates order and returns success' do
      result = handler.call(context)

      expect(result).to be_success
      expect(result.result[:order_total]).to eq(59.98)
      expect(result.result[:validated_items].length).to eq(1)
    end

    context 'with missing customer ID' do
      let(:context) do
        build_test_context(
          task_context: {
            customer_info: { email: 'test@example.com' },
            order_items: []
          }
        )
      end

      it 'raises permanent error' do
        expect {
          handler.call(context)
        }.to raise_error(TaskerCore::Errors::PermanentError)
      end
    end
  end
end
```

Run tests:

```bash
bundle exec rspec spec/handlers/
```

## Common Patterns

### Type-Safe Context Access

```ruby path=null start=null
# TAS-137: Cross-language standard API
value = context.get_input('field_name')

# Access task context directly
task_context = context.task.context
```

### Dependency Result Access

```ruby path=null start=null
# Get result from upstream step
previous_result = context.sequence.get('step_name')

# Extract computed value
value = previous_result['result']
```

### Metadata for Observability

```ruby path=null start=null
TaskerCore::Types::StepHandlerCallResult.success(
  result: data,
  metadata: {
    operation: 'my_operation',
    input_refs: {
      field: 'context.get_input("field")'
    }
  }
)
```

### Handler with Dependencies

```ruby path=null start=null
class ProcessPaymentHandler < TaskerCore::StepHandler::Base
  def call(context)
    # Get results from upstream steps
    order_result = context.sequence.get('validate_order')
    inventory_result = context.sequence.get('reserve_inventory')

    amount = order_result['result']['order_total']
    reservation_id = inventory_result['result']['reservation_id']

    # Process payment...
    payment_id = process_payment(amount)

    TaskerCore::Types::StepHandlerCallResult.success(
      result: {
        payment_id: payment_id,
        amount_charged: amount,
        reservation_id: reservation_id
      }
    )
  end
end
```

## Submitting Tasks via Client SDK

The `tasker-rb` gem includes a `TaskerCore::Client` module that provides keyword-argument methods with sensible defaults and wraps responses into typed `Dry::Struct` objects:

```ruby path=null start=null
require 'tasker_core'

# Create a task (defaults: initiator="tasker-core-ruby", source_system="tasker-core")
response = TaskerCore::Client.create_task(
  name: 'order_fulfillment',
  namespace: 'ecommerce',
  context: {
    customer: { id: 123, email: 'customer@example.com' },
    items: [
      { product_id: 1, quantity: 2, price: 29.99 }
    ]
  },
  initiator: 'my-service',
  source_system: 'my-api',
  reason: 'New order received'
)
puts "Task created: #{response.task_uuid}"
puts "Status: #{response.status}"

# Get task status
task = TaskerCore::Client.get_task(response.task_uuid)
puts "Task status: #{task.status}"

# List tasks with filters
task_list = TaskerCore::Client.list_tasks(namespace: 'ecommerce', limit: 10)
task_list.tasks.each do |t|
  puts "  #{t.task_uuid}: #{t.status}"
end
puts "Total: #{task_list.pagination.total_count}"

# List task steps
steps = TaskerCore::Client.list_task_steps(response.task_uuid)
steps.each do |step|
  puts "Step #{step.name}: #{step.current_state}"
end

# Check health
health = TaskerCore::Client.health_check
puts "Status: #{health.status}"

# Cancel a task
TaskerCore::Client.cancel_task(response.task_uuid)
```

### Available Client Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `create_task(name:, namespace:, context:, version:, **opts)` | `ClientTypes::TaskResponse` | Create a new task |
| `get_task(task_uuid)` | `ClientTypes::TaskResponse` | Get task by UUID |
| `list_tasks(limit:, offset:, namespace:, status:)` | `ClientTypes::TaskListResponse` | List tasks with filters |
| `cancel_task(task_uuid)` | `Hash` | Cancel a task |
| `list_task_steps(task_uuid)` | `Array<ClientTypes::StepResponse>` | List workflow steps |
| `get_step(task_uuid, step_uuid)` | `ClientTypes::StepResponse` | Get specific step |
| `get_step_audit_history(task_uuid, step_uuid)` | `Array<ClientTypes::StepAuditResponse>` | Get step audit trail |
| `health_check` | `ClientTypes::HealthResponse` | Check API health |

Response types are `Dry::Struct` objects with typed attributes (e.g., `response.task_uuid`, `response.status`, `task_list.pagination.total_count`).

## Next Steps

- See [Architecture](../architecture/README.md) for system design
- See [Workers Reference](../workers/README.md) for advanced patterns
- See the [tasker-core workers/ruby](https://github.com/tasker-systems/tasker-core/tree/main/workers/ruby) for complete examples
