# frozen_string_literal: true

require 'dry-struct'
require 'dry-types'

module TaskerCore
  module Types
    # TAS-231: Client API response types for orchestration client FFI.
    #
    # These types match the JSON responses from the orchestration API,
    # as returned by the client FFI functions (client_create_task, etc.).
    module ClientTypes
      module Types
        include Dry.Types()
      end

      # Step readiness status within a task response
      class StepReadiness < Dry::Struct
        attribute :workflow_step_uuid, Types::String
        attribute :task_uuid, Types::String
        attribute :named_step_uuid, Types::String
        attribute :name, Types::String
        attribute :current_state, Types::String
        attribute :dependencies_satisfied, Types::Bool
        attribute :retry_eligible, Types::Bool
        attribute :ready_for_execution, Types::Bool
        attribute? :last_failure_at, Types::String.optional
        attribute? :next_retry_at, Types::String.optional
        attribute :total_parents, Types::Integer
        attribute :completed_parents, Types::Integer
        attribute :attempts, Types::Integer
        attribute :max_attempts, Types::Integer
        attribute? :backoff_request_seconds, Types::Integer.optional
        attribute? :last_attempted_at, Types::String.optional
      end

      # Task response from the orchestration API.
      # Returned by client_create_task and client_get_task.
      class TaskResponse < Dry::Struct
        attribute :task_uuid, Types::String
        attribute :name, Types::String
        attribute :namespace, Types::String
        attribute :version, Types::String
        attribute :status, Types::String
        attribute :created_at, Types::String
        attribute :updated_at, Types::String
        attribute? :completed_at, Types::String.optional
        attribute :context, Types::Hash
        attribute :initiator, Types::String
        attribute :source_system, Types::String
        attribute :reason, Types::String
        attribute? :priority, Types::Integer.optional
        attribute? :tags, Types::Array.of(Types::String).optional
        attribute :correlation_id, Types::String
        attribute? :parent_correlation_id, Types::String.optional

        # Execution context
        attribute :total_steps, Types::Integer
        attribute :pending_steps, Types::Integer
        attribute :in_progress_steps, Types::Integer
        attribute :completed_steps, Types::Integer
        attribute :failed_steps, Types::Integer
        attribute :ready_steps, Types::Integer
        attribute :execution_status, Types::String
        attribute :recommended_action, Types::String
        attribute :completion_percentage, Types::Float
        attribute :health_status, Types::String

        # Step readiness
        attribute :steps, Types::Array.default([].freeze)

        def to_s
          "#<ClientTaskResponse #{namespace}/#{name}:#{version} status=#{status}>"
        end
      end

      # Pagination information in list responses
      class PaginationInfo < Dry::Struct
        attribute :page, Types::Integer
        attribute :per_page, Types::Integer
        attribute :total_count, Types::Integer
        attribute :total_pages, Types::Integer
        attribute :has_next, Types::Bool
        attribute :has_previous, Types::Bool
      end

      # Task list response with pagination.
      # Returned by client_list_tasks.
      class TaskListResponse < Dry::Struct
        attribute :tasks, Types::Array.default([].freeze)
        attribute :pagination, Types::Hash
      end

      # Step response from the orchestration API.
      # Returned by client_get_step.
      class StepResponse < Dry::Struct
        attribute :step_uuid, Types::String
        attribute :task_uuid, Types::String
        attribute :name, Types::String
        attribute :created_at, Types::String
        attribute :updated_at, Types::String
        attribute? :completed_at, Types::String.optional
        attribute? :results, Types::Hash.optional

        # Readiness fields
        attribute :current_state, Types::String
        attribute :dependencies_satisfied, Types::Bool
        attribute :retry_eligible, Types::Bool
        attribute :ready_for_execution, Types::Bool
        attribute :total_parents, Types::Integer
        attribute :completed_parents, Types::Integer
        attribute :attempts, Types::Integer
        attribute :max_attempts, Types::Integer
        attribute? :last_failure_at, Types::String.optional
        attribute? :next_retry_at, Types::String.optional
        attribute? :last_attempted_at, Types::String.optional
      end

      # Step audit history response (SOC2 compliance).
      # Returned by client_get_step_audit_history.
      class StepAuditResponse < Dry::Struct
        attribute :audit_uuid, Types::String
        attribute :workflow_step_uuid, Types::String
        attribute :transition_uuid, Types::String
        attribute :task_uuid, Types::String
        attribute :recorded_at, Types::String
        attribute? :worker_uuid, Types::String.optional
        attribute? :correlation_id, Types::String.optional
        attribute :success, Types::Bool
        attribute? :execution_time_ms, Types::Integer.optional
        attribute? :result, Types::Hash.optional
        attribute :step_name, Types::String
        attribute? :from_state, Types::String.optional
        attribute :to_state, Types::String
      end

      # Health check response from the orchestration API.
      # Returned by client_health_check.
      class HealthResponse < Dry::Struct
        attribute :status, Types::String
        attribute :timestamp, Types::String
      end
    end
  end
end
