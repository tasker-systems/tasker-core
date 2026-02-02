# Skill: Ruby Development

## When to Use

Use this skill when writing, reviewing, or modifying Ruby code in `workers/ruby/`, including step handlers, FFI bindings, RSpec tests, or the Magnus-based native extension.

## Tooling

| Tool | Purpose | Command |
|------|---------|---------|
| RuboCop | Linting & formatting | `cargo make check-ruby` |
| RSpec | Testing | `cargo make test-ruby` |
| Magnus | Rust-Ruby FFI framework | `bundle exec rake compile` |
| Bundler | Dependency management | `bundle install` |

### Setup

```bash
cd workers/ruby
cargo make setup          # bundle install
bundle exec rake compile  # Build native extension
```

### Quality Checks

```bash
cargo make check-ruby     # lint + rust-check + build + test
cargo make fix-ruby       # rubocop -a + rust-fmt-fix
```

## Code Style

### Formatting & Naming

- 2-space indentation, 120-char line length
- Double quotes for interpolation, single quotes otherwise
- Classes/Modules: `PascalCase`; Methods/variables: `snake_case`; Constants: `SCREAMING_SNAKE_CASE`
- Predicates: `ready?`; Dangerous methods: `reset!`

### Module Organization

```ruby
module TaskerCore
  module StepHandler
    class MyHandler < Base
      # 1. Includes/extends (composition over inheritance)
      include Mixins::API
      include Mixins::Decision

      # 2. Constants
      DEFAULT_TIMEOUT = 30

      # 3. Class methods
      def self.handler_name; end

      # 4. Instance methods (public)
      def call(context); end

      # 5. Protected methods
      protected
      def validate_context(context); end

      # 6. Private methods
      private
      def internal_process; end
    end
  end
end
```

## Handler Pattern

### The `call(context)` Contract

Every handler inherits from `TaskerCore::StepHandler::Base` and implements `call`:

```ruby
class OrderHandler < TaskerCore::StepHandler::Base
  include TaskerCore::StepHandler::Mixins::API

  def call(context)
    input = context.input_data
    response = get("/api/orders/#{input['order_id']}")

    if response.success?
      success(result: response.body, metadata: { processed_at: Time.now.iso8601 })
    else
      failure(message: response.error, error_type: 'APIError', retryable: response.status >= 500)
    end
  rescue StandardError => e
    failure(message: e.message, error_type: 'UnexpectedError', retryable: true)
  end
end
```

### Result Factory Methods

```ruby
# Success
success(result: { id: 123 }, metadata: { duration_ms: 50 })

# Failure
failure(message: 'API failed', error_type: 'RetryableError',
        error_code: 'API_TIMEOUT', retryable: true)

# Decision (for decision handlers)
decision_success(steps: ['process_order', 'send_notification'],
                 routing_context: { decision_type: 'conditional' })

# Skip branches
skip_branches(reason: 'No items to process',
              routing_context: { skip_type: 'empty_input' })
```

## Composition Over Inheritance (TAS-112)

Handlers gain capabilities by mixing in modules, not by inheriting from specialized base classes:

```ruby
# WRONG: Inheritance hierarchy
class MyHandler < TaskerCore::APIHandler  # Don't do this

# RIGHT: Composition via mixins
class MyHandler < TaskerCore::StepHandler::Base
  include TaskerCore::StepHandler::Mixins::API       # HTTP methods
  include TaskerCore::StepHandler::Mixins::Decision   # Decision routing
  include TaskerCore::StepHandler::Mixins::Batchable  # Batch processing
end
```

| Mixin | Provides |
|-------|----------|
| `Mixins::API` | `get`, `post`, `put`, `delete` |
| `Mixins::Decision` | `decision_success`, `skip_branches`, `decision_failure` |
| `Mixins::Batchable` | `get_batch_context`, `batch_worker_complete`, `handle_no_op_worker` |

## Error Handling

### Rescue Specific Exceptions

```ruby
# BAD: Too broad
rescue Exception => e

# GOOD: Specific first, then general
rescue TaskerCore::Errors::ValidationError => e
  failure(message: e.message, error_type: 'ValidationError', retryable: false)
rescue Net::OpenTimeout, Net::ReadTimeout => e
  failure(message: e.message, error_type: 'TimeoutError', retryable: true)
rescue StandardError => e
  failure(message: e.message, error_type: 'UnexpectedError', retryable: true)
```

## FFI Considerations

```ruby
# Native extension loaded automatically
require 'tasker_core/native'

# Don't hold references to FFI objects longer than needed
def process(context)
  order_id = context.input_data['order_id']  # Extract immediately
  # @context = context  # BAD: May cause memory issues
  process_order(order_id)
end
```

- Ruby has GIL -- cannot execute handlers concurrently within a single interpreter
- `FfiDispatchChannel` uses pull-based model: Ruby polls for work when ready
- Fire-and-forget callback pattern prevents deadlocks on completion

## Testing (RSpec)

```ruby
RSpec.describe TaskerCore::StepHandler::OrderHandler do
  subject(:handler) { described_class.new }
  let(:context) { build_step_context(input_data: { 'order_id' => '12345' }) }

  describe '#call' do
    context 'when order exists' do
      before { stub_api_request(:get, '/api/orders/12345').to_return(status: 200, body: {...}.to_json) }

      it 'returns success with order data' do
        result = handler.call(context)
        expect(result).to be_success
        expect(result.result['id']).to eq('12345')
      end
    end
  end
end
```

### Integration Tests

```bash
DATABASE_URL=postgresql://tasker:tasker@localhost/tasker_rust_test \
TASKER_ENV=test bundle exec rspec spec/integration/ --format documentation
```

### Clean Rebuild

```bash
cd workers/ruby && rake clean && rake compile
```

## Documentation (YARD)

```ruby
# Processes order fulfillment.
#
# @param context [TaskerCore::StepContext] The execution context
# @return [TaskerCore::StepHandlerResult] Success or failure result
# @raise [TaskerCore::Errors::ValidationError] if order_id is missing
def call(context)
end
```

## References

- Best practices: `docs/development/best-practices-ruby.md`
- Composition: `docs/principles/composition-over-inheritance.md`
- FFI safety: `docs/development/ffi-callback-safety.md`
- Cross-language: `docs/principles/cross-language-consistency.md`
