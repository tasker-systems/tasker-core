# frozen_string_literal: true

require 'spec_helper'

RSpec.describe TaskerCore::StepHandler::Functional, '#api_handler' do
  include described_class

  # ============================================================================
  # Test Helpers
  # ============================================================================

  def make_context(input_data: {})
    task_wrapper = double('task',
                          task_uuid: 'task-456',
                          context: input_data.transform_keys(&:to_s))

    workflow_step = double('workflow_step',
                           workflow_step_uuid: 'step-789',
                           name: 'test_step',
                           inputs: input_data.transform_keys(&:to_s),
                           attempts: 0,
                           max_attempts: 3,
                           checkpoint: nil,
                           retryable: true)

    dep_wrapper = double('dependency_results')
    allow(dep_wrapper).to receive(:get_results).and_return(nil)

    step_def_handler = double('handler', callable: 'test_handler', initialization: {})
    step_def = double('step_definition', handler: step_def_handler)

    ctx = TaskerCore::Types::StepContext.allocate
    ctx.instance_variable_set(:@task, task_wrapper)
    ctx.instance_variable_set(:@workflow_step, workflow_step)
    ctx.instance_variable_set(:@dependency_results, dep_wrapper)
    ctx.instance_variable_set(:@step_definition, step_def)
    ctx.instance_variable_set(:@handler_name, 'test_api')
    ctx
  end

  def mock_faraday_response(status:, body: '', headers: {})
    double('faraday_response',
           status: status,
           body: body,
           headers: Faraday::Utils::Headers.new(headers),
           success?: (200..299).cover?(status))
  end

  # ============================================================================
  # Tests: Handler Composition
  # ============================================================================

  describe 'handler composition' do
    it 'produces a class that includes the API mixin' do
      handler_class = api_handler('fetch_data', base_url: 'https://api.example.com') do |**_args|
        { ok: true }
      end

      expect(handler_class.ancestors).to include(TaskerCore::StepHandler::Mixins::API)
    end

    it 'sets handler name and version' do
      handler_class = api_handler('fetch_data', base_url: 'https://api.example.com', version: '2.0.0') do |**_args|
        { ok: true }
      end

      handler = handler_class.new
      expect(handler.handler_name).to eq('fetch_data')
      expect(handler_class::VERSION).to eq('2.0.0')
    end

    it 'configures base_url via config method' do
      handler_class = api_handler('fetch_data', base_url: 'https://api.example.com') do |**_args|
        { ok: true }
      end

      handler = handler_class.new
      expect(handler.config).to include(url: 'https://api.example.com')
    end

    it 'configures timeout and headers via config method' do
      handler_class = api_handler(
        'fetch_data',
        base_url: 'https://api.example.com',
        timeout: 60,
        headers: { 'Authorization' => 'Bearer token123' }
      ) do |**_args|
        { ok: true }
      end

      handler = handler_class.new
      expect(handler.config).to include(timeout: 60)
      expect(handler.config).to include(headers: { 'Authorization' => 'Bearer token123' })
    end
  end

  # ============================================================================
  # Tests: HTTP Methods
  # ============================================================================

  describe 'HTTP methods via api parameter' do
    it 'GET returns success via api.get' do
      handler_class = api_handler('fetch_user', base_url: 'https://api.example.com') do |api:, **_args|
        response = api.get('/users/1')
        api.api_success(data: response.body, status: response.status)
      end

      handler = handler_class.new
      mock_response = mock_faraday_response(status: 200, body: { 'id' => 1, 'name' => 'Alice' })
      mock_conn = double('connection')
      allow(mock_conn).to receive(:get).and_return(mock_response)
      allow(handler).to receive(:connection).and_return(mock_conn)
      # Stub process_response so it doesn't raise on the mock
      allow(handler).to receive(:process_response)

      result = handler.call(make_context)

      expect(result.success?).to be true
      expect(result.result['data']).to eq({ 'id' => 1, 'name' => 'Alice' })
    end

    it 'POST returns success via api.post' do
      handler_class = api_handler('create_user', base_url: 'https://api.example.com') do |api:, **_args|
        response = api.post('/users', data: { name: 'Bob' })
        api.api_success(data: response.body, status: response.status)
      end

      handler = handler_class.new
      mock_response = mock_faraday_response(status: 201, body: { 'id' => 42 })
      mock_conn = double('connection')
      allow(mock_conn).to receive(:post).and_return(mock_response)
      allow(handler).to receive(:connection).and_return(mock_conn)
      allow(handler).to receive(:process_response)

      result = handler.call(make_context)

      expect(result.success?).to be true
      expect(result.result['data']).to eq({ 'id' => 42 })
    end

    it 'DELETE returns success via api.delete' do
      handler_class = api_handler('remove_user', base_url: 'https://api.example.com') do |api:, **_args|
        response = api.delete('/users/1')
        { deleted: true, status: response.status }
      end

      handler = handler_class.new
      mock_response = mock_faraday_response(status: 204)
      mock_conn = double('connection')
      allow(mock_conn).to receive(:delete).and_return(mock_response)
      allow(handler).to receive(:connection).and_return(mock_conn)
      allow(handler).to receive(:process_response)

      result = handler.call(make_context)

      expect(result.success?).to be true
      expect(result.result[:deleted]).to be true
    end
  end

  # ============================================================================
  # Tests: Error Classification
  # ============================================================================

  describe 'error classification via api_failure' do
    it '404 is not retryable' do
      handler_class = api_handler('fetch_missing', base_url: 'https://api.example.com') do |api:, **_args|
        api.api_failure(message: 'Not found', status: 404)
      end

      handler = handler_class.new
      result = handler.call(make_context)

      expect(result.success?).to be false
      expect(result.retryable).to be false
    end

    it '503 is retryable' do
      handler_class = api_handler('fetch_unavailable', base_url: 'https://api.example.com') do |api:, **_args|
        api.api_failure(message: 'Service unavailable', status: 503)
      end

      handler = handler_class.new
      result = handler.call(make_context)

      expect(result.success?).to be false
      expect(result.retryable).to be true
    end

    it '429 is retryable' do
      handler_class = api_handler('fetch_throttled', base_url: 'https://api.example.com') do |api:, **_args|
        api.api_failure(message: 'Rate limited', status: 429)
      end

      handler = handler_class.new
      result = handler.call(make_context)

      expect(result.success?).to be false
      expect(result.retryable).to be true
    end
  end

  # ============================================================================
  # Tests: api parameter is self
  # ============================================================================

  describe 'api parameter identity' do
    it 'api is the handler instance itself' do
      captured_api = nil

      handler_class = api_handler('check_self', base_url: 'https://api.example.com') do |api:, **_args|
        captured_api = api
        { ok: true }
      end

      handler = handler_class.new
      handler.call(make_context)

      expect(captured_api).to be(handler)
    end

    it 'api has HTTP methods' do
      handler_class = api_handler('check_methods', base_url: 'https://api.example.com') do |**_args|
        { ok: true }
      end

      handler = handler_class.new
      expect(handler).to respond_to(:get)
      expect(handler).to respond_to(:post)
      expect(handler).to respond_to(:put)
      expect(handler).to respond_to(:delete)
    end

    it 'api has result helpers' do
      handler_class = api_handler('check_helpers', base_url: 'https://api.example.com') do |**_args|
        { ok: true }
      end

      handler = handler_class.new
      expect(handler).to respond_to(:api_success)
      expect(handler).to respond_to(:api_failure)
    end
  end
end
