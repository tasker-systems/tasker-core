# frozen_string_literal: true

require 'spec_helper'

RSpec.describe 'TaskerCore::Worker::EventBridge FFI safe-failure pattern' do
  let(:bridge) { TaskerCore::Worker::EventBridge.instance }

  describe '#build_ffi_safe_failure' do
    let(:original_data) do
      {
        event_id: 'evt-123',
        task_uuid: 'task-456',
        step_uuid: 'step-789',
        success: true
      }
    end
    let(:error) { StandardError.new('test serialization error') }
    let(:result) { bridge.send(:build_ffi_safe_failure, original_data, error) }

    it 'includes task_uuid in the fallback hash' do
      expect(result).to have_key('task_uuid')
      expect(result['task_uuid']).to eq('task-456')
    end

    it 'includes step_uuid in the fallback hash' do
      expect(result).to have_key('step_uuid')
      expect(result['step_uuid']).to eq('step-789')
    end

    it 'sets success to false' do
      expect(result['success']).to be false
    end

    it 'sets status to error' do
      expect(result['status']).to eq('error')
    end

    it 'includes an empty result hash' do
      expect(result['result']).to eq({})
    end

    describe 'metadata' do
      it 'sets execution_time_ms to 0' do
        expect(result['metadata']['execution_time_ms']).to eq(0)
      end

      it 'sets retryable to false' do
        expect(result['metadata']['retryable']).to be false
      end

      it 'includes a completed_at timestamp' do
        completed_at = result['metadata']['completed_at']
        expect(completed_at).not_to be_nil
        # Should be parseable as ISO 8601
        expect { Time.iso8601(completed_at) }.not_to raise_error
      end

      it 'includes worker_id' do
        expect(result['metadata']['worker_id']).to eq('ruby_worker')
      end

      it 'includes custom metadata with ffi_serialization_error' do
        custom = result['metadata']['custom']
        expect(custom).to have_key('ffi_serialization_error')
        expect(custom['ffi_serialization_error']).to include('test serialization error')
      end

      it 'includes custom metadata with original_success' do
        custom = result['metadata']['custom']
        expect(custom['original_success']).to eq('true')
      end
    end

    describe 'error' do
      it 'sets error_type to FFI_SERIALIZATION_ERROR' do
        expect(result['error']['error_type']).to eq('FFI_SERIALIZATION_ERROR')
      end

      it 'includes the original error message in error.message' do
        expect(result['error']['message']).to include('test serialization error')
      end

      it 'sets retryable to false' do
        expect(result['error']['retryable']).to be false
      end

      it 'truncates error.message to 500 characters' do
        long_error = StandardError.new('x' * 1000)
        long_result = bridge.send(:build_ffi_safe_failure, original_data, long_error)

        expect(long_result['error']['message'].length).to be <= 500
      end

      it 'truncates ffi_serialization_error in custom to 500 characters' do
        long_error = StandardError.new('y' * 1000)
        long_result = bridge.send(:build_ffi_safe_failure, original_data, long_error)

        custom_error = long_result['metadata']['custom']['ffi_serialization_error']
        expect(custom_error.length).to be <= 500
      end
    end

    describe 'key types' do
      it 'uses string keys at the top level (not symbols)' do
        expect(result.keys).to all(be_a(String))
      end

      it 'uses string keys in metadata' do
        expect(result['metadata'].keys).to all(be_a(String))
      end

      it 'uses string keys in metadata.custom' do
        expect(result['metadata']['custom'].keys).to all(be_a(String))
      end

      it 'uses string keys in error' do
        expect(result['error'].keys).to all(be_a(String))
      end
    end
  end

  describe '#publish_step_completion FFI fallback path' do
    let(:completion_data) do
      {
        event_id: 'evt-123',
        task_uuid: 'task-456',
        step_uuid: 'step-789',
        success: true,
        metadata: {}
      }
    end

    context 'when primary FFI call fails' do
      before do
        # First call raises, second call (fallback) succeeds
        call_count = 0
        allow(TaskerCore::FFI).to receive(:complete_step_event) do |_event_id, _data|
          call_count += 1
          raise StandardError, 'primary FFI serialization error' if call_count == 1

          true
        end
      end

      it 'submits the fallback failure result' do
        expect { bridge.publish_step_completion(completion_data) }.not_to raise_error
      end

      it 'calls complete_step_event twice (primary + fallback)' do
        expect(TaskerCore::FFI).to receive(:complete_step_event).twice
        bridge.publish_step_completion(completion_data)
      end
    end

    context 'when both primary and fallback FFI calls fail' do
      before do
        allow(TaskerCore::FFI).to receive(:complete_step_event)
          .and_raise(StandardError, 'FFI always fails')
      end

      it 'raises the fallback error' do
        expect { bridge.publish_step_completion(completion_data) }
          .to raise_error(StandardError, /FFI always fails/)
      end
    end

    context 'when primary FFI succeeds' do
      before do
        allow(TaskerCore::FFI).to receive(:complete_step_event).and_return(true)
      end

      it 'publishes the step.completion.sent monitoring event' do
        published = false
        bridge.subscribe('step.completion.sent') { published = true }
        bridge.publish_step_completion(completion_data)
        expect(published).to be true
      end

      it 'only calls complete_step_event once' do
        expect(TaskerCore::FFI).to receive(:complete_step_event).once
        bridge.publish_step_completion(completion_data)
      end
    end
  end
end
