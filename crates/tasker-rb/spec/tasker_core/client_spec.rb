# frozen_string_literal: true

require 'spec_helper'

RSpec.describe TaskerCore::Client do
  # Mock FFI module to avoid requiring compiled Rust extension
  let(:mock_task_response) do
    {
      'task_uuid' => 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      'name' => 'test_task',
      'namespace' => 'test',
      'version' => '1.0.0',
      'status' => 'pending',
      'created_at' => '2026-01-01T00:00:00Z',
      'updated_at' => '2026-01-01T00:00:00Z',
      'context' => { 'key' => 'value' },
      'initiator' => 'tasker-core-ruby',
      'source_system' => 'tasker-core',
      'reason' => 'Task requested',
      'correlation_id' => 'corr-id-123',
      'total_steps' => 3,
      'pending_steps' => 3,
      'in_progress_steps' => 0,
      'completed_steps' => 0,
      'failed_steps' => 0,
      'ready_steps' => 1,
      'execution_status' => 'pending',
      'recommended_action' => 'wait',
      'completion_percentage' => 0.0,
      'health_status' => 'healthy',
      'steps' => []
    }
  end

  let(:mock_step_response) do
    {
      'step_uuid' => '11111111-2222-3333-4444-555555555555',
      'task_uuid' => 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      'name' => 'validate_input',
      'created_at' => '2026-01-01T00:00:00Z',
      'updated_at' => '2026-01-01T00:00:00Z',
      'current_state' => 'pending',
      'dependencies_satisfied' => true,
      'retry_eligible' => false,
      'ready_for_execution' => true,
      'total_parents' => 0,
      'completed_parents' => 0,
      'attempts' => 0,
      'max_attempts' => 3
    }
  end

  let(:mock_audit_response) do
    {
      'audit_uuid' => 'audit-uuid-1',
      'workflow_step_uuid' => '11111111-2222-3333-4444-555555555555',
      'transition_uuid' => 'trans-uuid-1',
      'task_uuid' => 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      'recorded_at' => '2026-01-01T00:00:01Z',
      'success' => true,
      'step_name' => 'validate_input',
      'to_state' => 'complete'
    }
  end

  let(:mock_health_response) do
    {
      'status' => 'ok',
      'timestamp' => '2026-01-01T00:00:00Z'
    }
  end

  let(:mock_list_response) do
    {
      'tasks' => [mock_task_response],
      'pagination' => {
        'page' => 1,
        'per_page' => 50,
        'total_count' => 1,
        'total_pages' => 1,
        'has_next' => false,
        'has_previous' => false
      }
    }
  end

  describe '.create_task' do
    before do
      allow(TaskerCore::FFI).to receive(:client_create_task).and_return(mock_task_response)
    end

    it 'creates a task with required arguments' do
      result = described_class.create_task(name: 'test_task', namespace: 'test', context: { key: 'value' })

      expect(TaskerCore::FFI).to have_received(:client_create_task).with(
        hash_including(
          'name' => 'test_task',
          'namespace' => 'test',
          'context' => { key: 'value' }
        )
      )
      expect(result).to be_a(TaskerCore::Types::ClientTypes::TaskResponse)
      expect(result.task_uuid).to eq('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')
      expect(result.name).to eq('test_task')
    end

    it 'provides sensible defaults for optional fields' do
      described_class.create_task(name: 'test_task')

      expect(TaskerCore::FFI).to have_received(:client_create_task).with(
        hash_including(
          'namespace' => 'default',
          'version' => '1.0.0',
          'initiator' => 'tasker-core-ruby',
          'source_system' => 'tasker-core',
          'reason' => 'Task requested'
        )
      )
    end

    it 'allows overriding defaults' do
      described_class.create_task(
        name: 'test_task',
        initiator: 'my-app',
        source_system: 'custom-system',
        reason: 'Custom reason'
      )

      expect(TaskerCore::FFI).to have_received(:client_create_task).with(
        hash_including(
          'initiator' => 'my-app',
          'source_system' => 'custom-system',
          'reason' => 'Custom reason'
        )
      )
    end

    it 'passes additional options through' do
      described_class.create_task(name: 'test_task', tags: %w[a b])

      expect(TaskerCore::FFI).to have_received(:client_create_task).with(
        hash_including('tags' => %w[a b])
      )
    end

    it 'converts ActionController::Parameters-like objects via deep_to_hash' do
      # Simulate an object that responds to to_unsafe_h (like ActionController::Parameters)
      params_like = double('Params', to_unsafe_h: { 'nested' => 'value' })

      described_class.create_task(name: 'test_task', context: params_like)

      expect(TaskerCore::FFI).to have_received(:client_create_task).with(
        hash_including('context' => { 'nested' => 'value' })
      )
    end

    it 'falls back to raw hash when response does not match schema' do
      allow(TaskerCore::FFI).to receive(:client_create_task).and_return({ 'unexpected' => 'data' })

      result = described_class.create_task(name: 'test_task')

      expect(result).to be_a(Hash)
      expect(result['unexpected']).to eq('data')
    end
  end

  describe '.get_task' do
    before do
      allow(TaskerCore::FFI).to receive(:client_get_task).and_return(mock_task_response)
    end

    it 'gets a task by UUID and wraps the response' do
      result = described_class.get_task('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')

      expect(TaskerCore::FFI).to have_received(:client_get_task).with('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')
      expect(result).to be_a(TaskerCore::Types::ClientTypes::TaskResponse)
      expect(result.task_uuid).to eq('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')
    end

    it 'converts non-string UUIDs to strings' do
      allow(TaskerCore::FFI).to receive(:client_get_task).and_return(mock_task_response)

      uuid_obj = double('UUID', to_s: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')
      described_class.get_task(uuid_obj)

      expect(TaskerCore::FFI).to have_received(:client_get_task).with('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')
    end
  end

  describe '.list_tasks' do
    before do
      allow(TaskerCore::FFI).to receive(:client_list_tasks).and_return(mock_list_response)
    end

    it 'lists tasks with default pagination' do
      result = described_class.list_tasks

      expect(TaskerCore::FFI).to have_received(:client_list_tasks).with(50, 0, nil, nil)
      expect(result).to be_a(TaskerCore::Types::ClientTypes::TaskListResponse)
    end

    it 'passes pagination and filter arguments' do
      described_class.list_tasks(limit: 10, offset: 5, namespace: 'test', status: 'pending')

      expect(TaskerCore::FFI).to have_received(:client_list_tasks).with(10, 5, 'test', 'pending')
    end
  end

  describe '.cancel_task' do
    before do
      allow(TaskerCore::FFI).to receive(:client_cancel_task).and_return({ 'cancelled' => true })
    end

    it 'cancels a task and returns the raw hash' do
      result = described_class.cancel_task('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')

      expect(TaskerCore::FFI).to have_received(:client_cancel_task).with('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')
      expect(result).to be_a(Hash)
      expect(result['cancelled']).to be(true)
    end
  end

  describe '.list_task_steps' do
    before do
      allow(TaskerCore::FFI).to receive(:client_list_task_steps).and_return([mock_step_response])
    end

    it 'lists steps and wraps each into StepResponse' do
      result = described_class.list_task_steps('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')

      expect(result).to be_an(Array)
      expect(result.size).to eq(1)
      expect(result.first).to be_a(TaskerCore::Types::ClientTypes::StepResponse)
      expect(result.first.step_uuid).to eq('11111111-2222-3333-4444-555555555555')
    end

    it 'returns empty array when no steps' do
      allow(TaskerCore::FFI).to receive(:client_list_task_steps).and_return([])

      result = described_class.list_task_steps('aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee')

      expect(result).to eq([])
    end
  end

  describe '.get_step' do
    before do
      allow(TaskerCore::FFI).to receive(:client_get_step).and_return(mock_step_response)
    end

    it 'gets a step and wraps the response' do
      result = described_class.get_step('task-uuid', 'step-uuid')

      expect(TaskerCore::FFI).to have_received(:client_get_step).with('task-uuid', 'step-uuid')
      expect(result).to be_a(TaskerCore::Types::ClientTypes::StepResponse)
      expect(result.name).to eq('validate_input')
    end
  end

  describe '.get_step_audit_history' do
    before do
      allow(TaskerCore::FFI).to receive(:client_get_step_audit_history).and_return([mock_audit_response])
    end

    it 'gets audit history and wraps each entry' do
      result = described_class.get_step_audit_history('task-uuid', 'step-uuid')

      expect(TaskerCore::FFI).to have_received(:client_get_step_audit_history).with('task-uuid', 'step-uuid')
      expect(result).to be_an(Array)
      expect(result.first).to be_a(TaskerCore::Types::ClientTypes::StepAuditResponse)
      expect(result.first.step_name).to eq('validate_input')
    end

    it 'returns empty array when no audit entries' do
      allow(TaskerCore::FFI).to receive(:client_get_step_audit_history).and_return([])

      result = described_class.get_step_audit_history('task-uuid', 'step-uuid')

      expect(result).to eq([])
    end
  end

  describe '.health_check' do
    before do
      allow(TaskerCore::FFI).to receive(:client_health_check).and_return(mock_health_response)
    end

    it 'checks health and wraps the response' do
      result = described_class.health_check

      expect(TaskerCore::FFI).to have_received(:client_health_check)
      expect(result).to be_a(TaskerCore::Types::ClientTypes::HealthResponse)
      expect(result.status).to eq('ok')
    end

    it 'falls back to raw hash for unexpected response shape' do
      allow(TaskerCore::FFI).to receive(:client_health_check).and_return({ 'healthy' => true })

      result = described_class.health_check

      # HealthResponse requires status and timestamp, so this falls back to raw hash
      expect(result).to be_a(Hash)
      expect(result['healthy']).to be(true)
    end
  end
end
