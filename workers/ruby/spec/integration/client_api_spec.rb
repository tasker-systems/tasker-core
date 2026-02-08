# frozen_string_literal: true

# Client API FFI Integration Tests (TAS-231)
#
# Tests the client FFI functions against a running orchestration server.
# Verifies full round-trip: Ruby -> Magnus FFI -> Rust -> REST API -> PostgreSQL -> response.
#
# Prerequisites:
# - Ruby FFI extension compiled: bundle exec rake compile
# - DATABASE_URL set and database accessible
# - Orchestration server running (default: http://localhost:8080)
# - FFI_CLIENT_TESTS=true environment variable
#
# Run: FFI_CLIENT_TESTS=true DATABASE_URL=... bundle exec rspec spec/integration/client_api_spec.rb

require 'spec_helper'

RSpec.describe 'Client API FFI Integration', :client_integration do # rubocop:disable RSpec/DescribeClass
  # Bootstrap the worker once for all tests in this file
  before(:all) do
    result = TaskerCore::FFI.bootstrap_worker
    raise "Bootstrap failed: #{result}" unless result.is_a?(Hash) && result['status'] == 'started'
  end

  after(:all) do
    TaskerCore::FFI.stop_worker
  end

  describe 'health check' do
    it 'returns healthy response from orchestration API' do
      result = TaskerCore::FFI.client_health_check
      expect(result).to be_a(Hash)
      expect(result).to have_key('healthy')
    end
  end

  describe 'task lifecycle' do
    let!(:created_task) do
      request = {
        'name' => 'success_only',
        'namespace' => 'test_errors',
        'version' => '1.0.0',
        'context' => { 'test_run' => 'client_api_integration', 'run_id' => SecureRandom.uuid },
        'initiator' => 'ruby-client-test',
        'source_system' => 'integration-test',
        'reason' => 'TAS-231 client API integration test'
      }
      TaskerCore::FFI.client_create_task(request)
    end

    it 'creates a task via the orchestration API' do
      expect(created_task).to be_a(Hash)
      expect(created_task).to have_key('task_uuid')
      expect(created_task['name']).to eq('success_only')
      expect(created_task['namespace']).to eq('test_errors')
    end

    it 'gets the created task by UUID' do
      task_uuid = created_task['task_uuid']
      result = TaskerCore::FFI.client_get_task(task_uuid)

      expect(result).to be_a(Hash)
      expect(result['task_uuid']).to eq(task_uuid)
      expect(result['name']).to eq('success_only')
      expect(result['namespace']).to eq('test_errors')
      expect(result['version']).to eq('1.0.0')
      expect(result).to have_key('created_at')
      expect(result).to have_key('updated_at')
      expect(result).to have_key('correlation_id')
      expect(result['total_steps']).to be_a(Integer)
    end

    it 'lists tasks with pagination' do
      result = TaskerCore::FFI.client_list_tasks(50, 0, nil, nil)

      expect(result).to be_a(Hash)
      expect(result).to have_key('tasks')
      expect(result['tasks']).to be_an(Array)
      expect(result).to have_key('pagination')
      expect(result['pagination']['total_count']).to be >= 1
    end

    it 'lists task steps' do
      task_uuid = created_task['task_uuid']
      result = TaskerCore::FFI.client_list_task_steps(task_uuid)

      expect(result).to be_an(Array)

      next if result.empty?

      step = result.first
      expect(step).to have_key('step_uuid')
      expect(step['task_uuid']).to eq(task_uuid)
      expect(step).to have_key('name')
    end

    it 'gets a specific step' do
      task_uuid = created_task['task_uuid']
      steps = TaskerCore::FFI.client_list_task_steps(task_uuid)
      skip 'No steps available' if steps.empty?

      step_uuid = steps.first['step_uuid']
      result = TaskerCore::FFI.client_get_step(task_uuid, step_uuid)

      expect(result).to be_a(Hash)
      expect(result['step_uuid']).to eq(step_uuid)
      expect(result['task_uuid']).to eq(task_uuid)
      expect(result).to have_key('name')
      expect(result).to have_key('current_state')
      expect(result['attempts']).to be_a(Integer)
      expect(result['max_attempts']).to be_a(Integer)
    end

    it 'gets step audit history' do
      task_uuid = created_task['task_uuid']
      steps = TaskerCore::FFI.client_list_task_steps(task_uuid)
      skip 'No steps available' if steps.empty?

      step_uuid = steps.first['step_uuid']
      result = TaskerCore::FFI.client_get_step_audit_history(task_uuid, step_uuid)

      expect(result).to be_an(Array)
      # May be empty for newly created tasks
    end

    it 'cancels the task' do
      task_uuid = created_task['task_uuid']
      result = TaskerCore::FFI.client_cancel_task(task_uuid)

      expect(result).to be_a(Hash)
    end
  end

  describe 'error handling' do
    it 'handles non-existent task gracefully' do
      # Should either raise an exception or return an error response
      result = TaskerCore::FFI.client_get_task('00000000-0000-0000-0000-000000000000')
      expect(result).to be_a(Hash)
    rescue RuntimeError
      # Expected - server returns 404/error
    end
  end
end
