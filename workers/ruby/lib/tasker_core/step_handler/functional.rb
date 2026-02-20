# frozen_string_literal: true

# Functional/block DSL for step handlers (TAS-294).
#
# This module provides block-based alternatives to the class-based handler API.
# It reduces boilerplate for common handler patterns while preserving full access
# to the underlying StepContext for advanced use cases.
#
# The DSL methods auto-wrap return values and classify exceptions:
# - Hash return -> StepHandlerCallResult.success(result: hash)
# - StepHandlerCallResult return -> pass through unchanged
# - PermanentError raised -> StepHandlerCallResult.error(retryable: false)
# - RetryableError raised -> StepHandlerCallResult.error(retryable: true)
# - Other errors -> StepHandlerCallResult.error(retryable: true)
#
# @example Basic handler
#   include TaskerCore::StepHandler::Functional
#
#   step_handler "process_payment",
#     depends_on: { cart: "validate_cart" },
#     inputs: [:payment_info] do |cart:, payment_info:, context:|
#
#     raise TaskerCore::Errors::PermanentError, "Payment info required" unless payment_info
#     result = charge_card(payment_info, cart["total"])
#     { payment_id: result["id"], amount: cart["total"] }
#   end

module TaskerCore
  module StepHandler
    module Functional
      # ========================================================================
      # Helper: Decision
      # ========================================================================

      # Helper for decision handler return values.
      #
      # @example
      #   decision_handler "route_order",
      #     depends_on: { order: "validate_order" } do |order:, context:|
      #     if order["tier"] == "premium"
      #       Decision.route(["process_premium"], tier: "premium")
      #     else
      #       Decision.route(["process_standard"])
      #     end
      #   end
      class Decision
        attr_reader :type, :steps, :reason, :routing_context

        def initialize(type, steps, reason, routing_context)
          @type = type
          @steps = steps
          @reason = reason
          @routing_context = routing_context
          freeze
        end

        # Route to the specified steps.
        #
        # @param steps [Array<String>] Step names to route to
        # @param routing_context [Hash] Additional routing context
        # @return [Decision]
        def self.route(steps, **routing_context)
          new('create_steps', Array(steps), nil, routing_context)
        end

        # Skip all branches.
        #
        # @param reason [String] Reason for skipping
        # @param routing_context [Hash] Additional routing context
        # @return [Decision]
        def self.skip(reason, **routing_context)
          new('no_branches', [], reason, routing_context)
        end
      end

      # ========================================================================
      # Helper: BatchConfig
      # ========================================================================

      # Configuration returned by batch analyzer handlers.
      #
      # @example
      #   batch_analyzer "analyze_orders", worker_template: "process_batch" do |context:|
      #     BatchConfig.new(total_items: 250, batch_size: 100)
      #   end
      class BatchConfig
        attr_reader :total_items, :batch_size, :metadata

        # @param total_items [Integer] Total number of items to process
        # @param batch_size [Integer] Size of each batch
        # @param metadata [Hash] Optional metadata
        def initialize(total_items:, batch_size:, metadata: {})
          @total_items = total_items
          @batch_size = batch_size
          @metadata = metadata
          freeze
        end
      end

      # ========================================================================
      # Internal: Auto-wrapping
      # ========================================================================

      module_function

      # @api private
      def _wrap_result(result)
        case result
        when Types::StepHandlerCallResult::Success,
             Types::StepHandlerCallResult::Error,
             Types::StepHandlerCallResult::CheckpointYield
          result
        when Hash
          Types::StepHandlerCallResult.success(result: result)
        when nil
          Types::StepHandlerCallResult.success(result: {})
        else
          Types::StepHandlerCallResult.success(result: result.respond_to?(:to_h) ? result.to_h : {})
        end
      end

      # @api private
      def _wrap_exception(exception)
        Types::StepHandlerCallResult.from_exception(exception)
      end

      # @api private
      def _inject_args(context, depends_on, inputs)
        args = { context: context }

        depends_on.each do |param_name, step_name|
          args[param_name.to_sym] = context.get_dependency_result(step_name.to_s)
        end

        inputs.each do |input_key|
          args[input_key.to_sym] = context.get_input(input_key.to_s)
        end

        args
      end

      # ========================================================================
      # Public DSL Methods
      # ========================================================================

      # Define a step handler from a block.
      #
      # Returns a StepHandler::Base subclass that can be registered with HandlerRegistry.
      # The block receives injected dependencies, inputs, and context as keyword arguments.
      # Return values are auto-wrapped as success results, and exceptions are
      # auto-classified as failure results.
      #
      # @param name [String] Handler name (must match step definition)
      # @param depends_on [Hash] Mapping of parameter names to dependency step names
      # @param inputs [Array<Symbol, String>] Task context input keys to inject
      # @param version [String] Handler version (default: "1.0.0")
      # @yield [**args] Block receiving dependencies, inputs, and context as keyword args
      # @return [Class] StepHandler::Base subclass
      #
      # @example
      #   ProcessPayment = step_handler "process_payment",
      #     depends_on: { cart: "validate_cart" },
      #     inputs: [:payment_info] do |cart:, payment_info:, context:|
      #
      #     raise TaskerCore::Errors::PermanentError, "Payment info required" unless payment_info
      #     result = charge_card(payment_info, cart["total"])
      #     { payment_id: result["id"], amount: cart["total"] }
      #   end
      def step_handler(name, depends_on: {}, inputs: [], version: '1.0.0', &block)
        raise ArgumentError, 'block required' unless block

        handler_depends = depends_on
        handler_inputs = inputs
        handler_block = block

        Class.new(Base) do
          const_set(:VERSION, version)

          define_method(:handler_name) { name }

          define_method(:call) do |context|
            args = Functional._inject_args(context, handler_depends, handler_inputs)
            result = handler_block.call(**args)
            Functional._wrap_result(result)
          rescue StandardError => e
            Functional._wrap_exception(e)
          end
        end
      end

      # Define a decision handler from a block.
      #
      # The block should return a `Decision.route(...)` or `Decision.skip(...)`.
      #
      # @param name [String] Handler name
      # @param depends_on [Hash] Mapping of parameter names to dependency step names
      # @param inputs [Array<Symbol, String>] Task context input keys to inject
      # @param version [String] Handler version (default: "1.0.0")
      # @yield [**args] Block returning a Decision
      # @return [Class] StepHandler::Base subclass with decision capabilities
      #
      # @example
      #   RouteOrder = decision_handler "route_order",
      #     depends_on: { order: "validate_order" } do |order:, context:|
      #     if order["tier"] == "premium"
      #       Decision.route(["process_premium"], tier: "premium")
      #     else
      #       Decision.route(["process_standard"])
      #     end
      #   end
      def decision_handler(name, depends_on: {}, inputs: [], version: '1.0.0', &block)
        raise ArgumentError, 'block required' unless block

        handler_depends = depends_on
        handler_inputs = inputs
        handler_block = block

        Class.new(Base) do
          include Mixins::Decision

          const_set(:VERSION, version)

          define_method(:handler_name) { name }

          define_method(:call) do |context|
            args = Functional._inject_args(context, handler_depends, handler_inputs)
            raw_result = handler_block.call(**args)

            case raw_result
            when Types::StepHandlerCallResult::Success,
                 Types::StepHandlerCallResult::Error
              raw_result
            when Decision
              result_data = {}
              if raw_result.type == 'create_steps'
                result_data[:routing_context] = raw_result.routing_context unless raw_result.routing_context.empty?
                decision_success(steps: raw_result.steps, result_data: result_data)
              else
                result_data[:reason] = raw_result.reason if raw_result.reason
                result_data[:routing_context] = raw_result.routing_context unless raw_result.routing_context.empty?
                decision_no_branches(result_data: result_data)
              end
            else
              Functional._wrap_result(raw_result)
            end
          rescue StandardError => e
            Functional._wrap_exception(e)
          end
        end
      end

      # Define a batch analyzer handler.
      #
      # The block should return a `BatchConfig` with `total_items` and `batch_size`.
      # Cursor configs are generated automatically.
      #
      # @param name [String] Handler name
      # @param worker_template [String] Name of the worker template step
      # @param depends_on [Hash] Mapping of parameter names to dependency step names
      # @param inputs [Array<Symbol, String>] Task context input keys to inject
      # @param version [String] Handler version (default: "1.0.0")
      # @yield [**args] Block returning a BatchConfig
      # @return [Class] StepHandler::Base subclass
      #
      # @example
      #   AnalyzeOrders = batch_analyzer "analyze_orders",
      #     worker_template: "process_batch" do |context:|
      #     BatchConfig.new(total_items: 250, batch_size: 100)
      #   end
      def batch_analyzer(name, worker_template:, depends_on: {}, inputs: [], version: '1.0.0', &block)
        raise ArgumentError, 'block required' unless block

        handler_depends = depends_on
        handler_inputs = inputs
        handler_block = block
        handler_worker_template = worker_template

        Class.new(Base) do
          include Mixins::Batchable

          const_set(:VERSION, version)

          define_method(:handler_name) { name }

          define_method(:call) do |context|
            args = Functional._inject_args(context, handler_depends, handler_inputs)
            raw_result = handler_block.call(**args)

            case raw_result
            when Types::StepHandlerCallResult::Success,
                 Types::StepHandlerCallResult::Error
              raw_result
            when BatchConfig
              total_items = raw_result.total_items
              batch_size = raw_result.batch_size
              worker_count = (total_items.to_f / batch_size).ceil

              cursor_configs = create_cursor_configs(total_items, worker_count)

              create_batches_outcome(
                worker_template_name: handler_worker_template,
                cursor_configs: cursor_configs,
                total_items: total_items,
                metadata: raw_result.metadata
              )
            else
              Functional._wrap_result(raw_result)
            end
          rescue StandardError => e
            Functional._wrap_exception(e)
          end
        end
      end

      # Define a batch worker handler.
      #
      # The block receives a `batch_context` keyword argument extracted from the
      # step context, containing cursor configuration for this worker's partition.
      #
      # @param name [String] Handler name
      # @param depends_on [Hash] Mapping of parameter names to dependency step names
      # @param inputs [Array<Symbol, String>] Task context input keys to inject
      # @param version [String] Handler version (default: "1.0.0")
      # @yield [**args] Block receiving batch_context and other injected args
      # @return [Class] StepHandler::Base subclass
      #
      # @example
      #   ProcessBatch = batch_worker "process_batch" do |batch_context:, context:|
      #     cursor = batch_context&.dig(:cursor_config)
      #     # process items from cursor[:start_cursor] to cursor[:end_cursor]
      #     { processed: true }
      #   end
      def batch_worker(name, depends_on: {}, inputs: [], version: '1.0.0', &block)
        raise ArgumentError, 'block required' unless block

        handler_depends = depends_on
        handler_inputs = inputs
        handler_block = block

        Class.new(Base) do
          include Mixins::Batchable

          const_set(:VERSION, version)

          define_method(:handler_name) { name }

          define_method(:call) do |context|
            args = Functional._inject_args(context, handler_depends, handler_inputs)

            # Delegate batch context extraction to Batchable mixin
            args[:batch_context] = get_batch_context(context)

            result = handler_block.call(**args)
            Functional._wrap_result(result)
          rescue StandardError => e
            Functional._wrap_exception(e)
          end
        end
      end
    end
  end
end
