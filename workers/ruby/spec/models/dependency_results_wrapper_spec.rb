# frozen_string_literal: true

require 'spec_helper'

RSpec.describe TaskerCore::Models::DependencyResultsWrapper do
  # ============================================================================
  # StepExecutionResult: Top-level string keys accessible via symbols
  # ============================================================================

  describe 'StepExecutionResult with top-level string keys' do
    it 'allows access to step results via both string and symbol keys' do
      # serde_magnus serializes Rust struct field names as Ruby symbols,
      # so the outer keys (step names) arrive as symbols from Rust FFI.
      results_data = {
        step_one: { result: 42, success: true },
        step_two: { result: 'hello', success: true }
      }

      wrapper = described_class.new(results_data)

      # Symbol access (native key type)
      expect(wrapper.get_result(:step_one)).to be_a(ActiveSupport::HashWithIndifferentAccess)
      expect(wrapper.get_result(:step_one)[:result]).to eq(42)

      # String access (indifferent access)
      expect(wrapper.get_result('step_one')).to be_a(ActiveSupport::HashWithIndifferentAccess)
      expect(wrapper.get_result('step_one')['result']).to eq(42)

      # Mixed access patterns
      expect(wrapper['step_two'][:success]).to be true
      expect(wrapper[:step_two]['success']).to be true
    end

    it 'supports key? with both string and symbol keys' do
      results_data = { step_one: { result: 1, success: true } }
      wrapper = described_class.new(results_data)

      expect(wrapper.key?(:step_one)).to be true
      expect(wrapper.key?('step_one')).to be true
      expect(wrapper.key?(:nonexistent)).to be false
    end
  end

  # ============================================================================
  # StepExecutionResult: Nested hash values with indifferent access
  # ============================================================================

  describe 'StepExecutionResult with nested hash values' do
    it 'converts inner StepExecutionResult hashes to indifferent access' do
      results_data = {
        step_one: {
          result: { 'amount' => 99.99, 'currency' => 'USD' },
          success: true,
          metadata: { 'processed_at' => '2025-01-01' }
        }
      }

      wrapper = described_class.new(results_data)
      step_result = wrapper.get_result(:step_one)

      # Inner hash fields accessible via both string and symbol keys
      expect(step_result[:result]).to eq({ 'amount' => 99.99, 'currency' => 'USD' })
      expect(step_result['result']).to eq({ 'amount' => 99.99, 'currency' => 'USD' })
      expect(step_result[:success]).to be true
      expect(step_result['success']).to be true
      expect(step_result[:metadata]).to eq({ 'processed_at' => '2025-01-01' })
      expect(step_result['metadata']).to eq({ 'processed_at' => '2025-01-01' })
    end

    it 'extracts result value via get_results with indifferent key access' do
      results_data = {
        step_one: { result: { 'total' => 42 }, success: true }
      }

      wrapper = described_class.new(results_data)

      # get_results extracts the 'result' field value
      expect(wrapper.get_results(:step_one)).to eq({ 'total' => 42 })
      expect(wrapper.get_results('step_one')).to eq({ 'total' => 42 })
    end
  end

  # ============================================================================
  # StepExecutionResult: nil results_data during initialization
  # ============================================================================

  describe 'StepExecutionResult with nil results_data' do
    it 'handles nil results_data gracefully' do
      wrapper = described_class.new(nil)

      expect(wrapper.keys).to eq([])
      expect(wrapper.get_result(:anything)).to be_nil
      expect(wrapper.get_results(:anything)).to be_nil
      expect(wrapper[:anything]).to be_nil
      expect(wrapper.key?(:anything)).to be false
    end

    it 'handles empty hash results_data' do
      wrapper = described_class.new({})

      expect(wrapper.keys).to eq([])
      expect(wrapper.get_result(:step_one)).to be_nil
      expect(wrapper.get_results(:step_one)).to be_nil
    end
  end
end
