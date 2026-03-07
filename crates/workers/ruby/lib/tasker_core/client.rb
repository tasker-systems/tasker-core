# frozen_string_literal: true

module TaskerCore
  # High-level client wrapper around the TaskerCore FFI client methods.
  #
  # The raw FFI exposes `TaskerCore::FFI.client_create_task(hash)` and similar
  # methods that require callers to construct complete request hashes with all
  # required fields (initiator, source_system, reason, etc.) and return plain
  # hashes. This module provides keyword-argument methods with sensible defaults
  # and wraps responses into typed `ClientTypes::*` Dry::Struct objects.
  #
  # @example Creating a task with defaults
  #   response = TaskerCore::Client.create_task(
  #     name: 'process_order',
  #     namespace: 'ecommerce',
  #     context: { order_id: 123 }
  #   )
  #   response.task_uuid  # => "550e8400-..."
  #   response.status     # => "pending"
  #
  # @example Getting a task
  #   task = TaskerCore::Client.get_task("550e8400-...")
  #   task.name       # => "process_order"
  #   task.namespace  # => "ecommerce"
  #
  # @example Listing tasks with filters
  #   list = TaskerCore::Client.list_tasks(namespace: 'ecommerce', limit: 10)
  #   list.tasks.size  # => 3
  #   list.pagination  # => { "total_count" => 42, ... }
  module Client
    module_function

    # Create a task via the orchestration API.
    #
    # @param name [String] Named task template name
    # @param namespace [String] Task namespace (default: "default")
    # @param context [Hash] Workflow context passed to step handlers
    # @param version [String] Template version (default: "1.0.0")
    # @param initiator [String] Who initiated the request
    # @param source_system [String] Originating system
    # @param reason [String] Reason for creating the task
    # @param options [Hash] Additional TaskRequest fields
    # @return [ClientTypes::TaskResponse, Hash] Typed response or raw hash on schema mismatch
    def create_task(name:, namespace: 'default', context: {}, version: '1.0.0',
                    initiator: 'tasker-core-ruby', source_system: 'tasker-core',
                    reason: 'Task requested', **options)
      request = {
        'name' => name,
        'namespace' => namespace,
        'version' => version,
        'context' => deep_to_hash(context),
        'initiator' => initiator,
        'source_system' => source_system,
        'reason' => reason
      }
      options.each { |k, v| request[k.to_s] = v }

      response = TaskerCore::FFI.client_create_task(request)
      wrap_response(response, Types::ClientTaskResponse)
    end

    # Get a task by UUID.
    #
    # @param task_uuid [String] The task UUID
    # @return [ClientTypes::TaskResponse, Hash] Typed response or raw hash
    def get_task(task_uuid)
      response = TaskerCore::FFI.client_get_task(task_uuid.to_s)
      wrap_response(response, Types::ClientTaskResponse)
    end

    # List tasks with optional filtering and pagination.
    #
    # @param limit [Integer] Maximum number of results (default: 50)
    # @param offset [Integer] Pagination offset (default: 0)
    # @param namespace [String, nil] Filter by namespace
    # @param status [String, nil] Filter by status
    # @return [ClientTypes::TaskListResponse, Hash] Typed response or raw hash
    def list_tasks(limit: 50, offset: 0, namespace: nil, status: nil)
      response = TaskerCore::FFI.client_list_tasks(limit, offset, namespace, status)
      wrap_response(response, Types::ClientTaskListResponse)
    end

    # Cancel a task by UUID.
    #
    # @param task_uuid [String] The task UUID
    # @return [Hash] Cancellation result
    def cancel_task(task_uuid)
      TaskerCore::FFI.client_cancel_task(task_uuid.to_s)
    end

    # List workflow steps for a task.
    #
    # @param task_uuid [String] The task UUID
    # @return [Array<ClientTypes::StepResponse>, Array<Hash>] Typed steps or raw hashes
    def list_task_steps(task_uuid)
      response = TaskerCore::FFI.client_list_task_steps(task_uuid.to_s)
      return response unless response.is_a?(Array)

      response.map { |step| wrap_response(step, Types::ClientStepResponse) }
    end

    # Get a specific workflow step.
    #
    # @param task_uuid [String] The task UUID
    # @param step_uuid [String] The step UUID
    # @return [ClientTypes::StepResponse, Hash] Typed response or raw hash
    def get_step(task_uuid, step_uuid)
      response = TaskerCore::FFI.client_get_step(task_uuid.to_s, step_uuid.to_s)
      wrap_response(response, Types::ClientStepResponse)
    end

    # Get audit history for a workflow step.
    #
    # @param task_uuid [String] The task UUID
    # @param step_uuid [String] The step UUID
    # @return [Array<ClientTypes::StepAuditResponse>, Array<Hash>] Typed audit entries or raw hashes
    def get_step_audit_history(task_uuid, step_uuid)
      response = TaskerCore::FFI.client_get_step_audit_history(task_uuid.to_s, step_uuid.to_s)
      return response unless response.is_a?(Array)

      response.map { |entry| wrap_response(entry, Types::ClientStepAuditResponse) }
    end

    # Check orchestration API health.
    #
    # @return [ClientTypes::HealthResponse, Hash] Typed response or raw hash
    def health_check
      response = TaskerCore::FFI.client_health_check
      wrap_response(response, Types::ClientHealthResponse)
    end

    # @api private
    # Wrap a raw FFI hash response into a Dry::Struct type.
    # Falls back to the raw hash if the schema doesn't match,
    # providing forward-compatibility when the API adds new fields.
    def wrap_response(raw, type_class)
      return raw unless raw.is_a?(Hash)

      # Dry::Struct requires symbolized keys; raw FFI returns string keys
      type_class.new(raw.transform_keys(&:to_sym))
    rescue Dry::Struct::Error, KeyError, Dry::Types::ConstraintError
      raw
    end

    # @api private
    # Recursively convert ActionController::Parameters (and similar) to plain hashes/arrays.
    def deep_to_hash(obj)
      case obj
      when Hash
        obj.transform_values { |v| deep_to_hash(v) }
      when Array
        obj.map { |v| deep_to_hash(v) }
      else
        # Handle ActionController::Parameters if available (Rails contexts)
        if obj.respond_to?(:to_unsafe_h)
          obj.to_unsafe_h.transform_values { |v| deep_to_hash(v) }
        else
          obj
        end
      end
    end

    private_class_method :wrap_response, :deep_to_hash
  end
end
