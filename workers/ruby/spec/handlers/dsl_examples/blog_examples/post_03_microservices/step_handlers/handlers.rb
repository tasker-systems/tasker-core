# frozen_string_literal: true

# DSL mirror of Microservices::StepHandlers using block DSL.
#
# 5 handlers: create_user_account, setup_billing_profile,
#             initialize_preferences, send_welcome_sequence, update_user_status
#
# NOTE: Non-deterministic fields (random IDs, timestamps) differ between runs.
# Parity testing focuses on deterministic fields, structure, and error classification.

include TaskerCore::StepHandler::Functional

EXISTING_USERS_DSL = {
  'existing@example.com' => {
    id: 'user_existing_001',
    email: 'existing@example.com',
    name: 'Existing User',
    plan: 'free',
    created_at: '2025-01-01T00:00:00Z'
  }
}.freeze

BILLING_TIERS_DSL = {
  'free' => { price: 0, features: ['basic_features'], billing_required: false },
  'pro' => { price: 29.99, features: %w[basic_features advanced_analytics], billing_required: true },
  'enterprise' => { price: 299.99,
                    features: %w[basic_features advanced_analytics priority_support custom_integrations], billing_required: true }
}.freeze

DEFAULT_PREFERENCES_DSL = {
  'free' => {
    email_notifications: true, marketing_emails: false, product_updates: true,
    weekly_digest: false, theme: 'light', language: 'en', timezone: 'UTC'
  },
  'pro' => {
    email_notifications: true, marketing_emails: true, product_updates: true,
    weekly_digest: true, theme: 'dark', language: 'en', timezone: 'UTC', api_notifications: true
  },
  'enterprise' => {
    email_notifications: true, marketing_emails: true, product_updates: true,
    weekly_digest: true, theme: 'dark', language: 'en', timezone: 'UTC',
    api_notifications: true, audit_logs: true, advanced_reports: true
  }
}.freeze

MicroCreateUserAccountDslHandler = step_handler(
  'microservices_dsl.step_handlers.create_user_account',
  inputs: [:user_info]
) do |user_info:, context:|
  deep_sym = ->(obj) {
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  user_info = deep_sym.call(user_info || context.task.context['user_info'] || {})

  raise TaskerCore::Errors::PermanentError.new('Email is required', error_code: 'MISSING_EMAIL') unless user_info[:email]
  raise TaskerCore::Errors::PermanentError.new('Name is required', error_code: 'MISSING_NAME') unless user_info[:name]
  unless user_info[:email].match?(/\A[\w+\-.]+@[a-z\d-]+(\.[a-z\d-]+)*\.[a-z]+\z/i)
    raise TaskerCore::Errors::PermanentError.new("Invalid email format: #{user_info[:email]}", error_code: 'INVALID_EMAIL_FORMAT')
  end

  validated = { email: user_info[:email], name: user_info[:name],
                plan: user_info[:plan] || 'free', source: user_info[:source] || 'web' }.compact

  if EXISTING_USERS_DSL.key?(validated[:email])
    existing = EXISTING_USERS_DSL[validated[:email]]
    if existing[:email] == validated[:email] && existing[:name] == validated[:name] && existing[:plan] == validated[:plan]
      result = { user_id: existing[:id], email: existing[:email], plan: existing[:plan],
                 status: 'already_exists', created_at: existing[:created_at] }
    else
      raise TaskerCore::Errors::PermanentError.new(
        "User with email #{validated[:email]} already exists with different data",
        error_code: 'USER_CONFLICT'
      )
    end
  else
    result = { user_id: "user_#{SecureRandom.hex(6)}", email: validated[:email],
               name: validated[:name], plan: validated[:plan], status: 'created',
               created_at: Time.now.utc.iso8601 }
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: { operation: 'create_user', service: 'user_service',
                idempotent: result[:status] == 'already_exists' }
  )
end

MicroSetupBillingDslHandler = step_handler(
  'microservices_dsl.step_handlers.setup_billing_profile',
  depends_on: { user_data: 'create_user_account' }
) do |user_data:, context:|
  raise TaskerCore::Errors::PermanentError.new('User data not found', error_code: 'MISSING_USER_DATA') unless user_data

  user_id = user_data['user_id'] || user_data[:user_id]
  plan = user_data['plan'] || user_data[:plan] || 'free'

  tier_config = BILLING_TIERS_DSL[plan] || BILLING_TIERS_DSL['free']

  result = if tier_config[:billing_required]
             { billing_id: "billing_#{SecureRandom.hex(6)}", user_id: user_id, plan: plan,
               price: tier_config[:price], currency: 'USD', billing_cycle: 'monthly',
               features: tier_config[:features], status: 'active',
               next_billing_date: (Time.now.utc + (30 * 24 * 3600)).iso8601,
               created_at: Time.now.utc.iso8601 }
           else
             { user_id: user_id, plan: plan, billing_required: false,
               status: 'skipped_free_plan', message: 'Free plan users do not require billing setup' }
           end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: { operation: 'setup_billing', service: 'billing_service', plan: plan,
                billing_required: tier_config[:billing_required] }
  )
end

MicroInitializePreferencesDslHandler = step_handler(
  'microservices_dsl.step_handlers.initialize_preferences',
  depends_on: { user_data: 'create_user_account' },
  inputs: [:user_info]
) do |user_data:, user_info:, context:|
  raise TaskerCore::Errors::PermanentError.new('User data not found', error_code: 'MISSING_USER_DATA') unless user_data

  deep_sym = ->(obj) {
    case obj
    when Hash then obj.each_with_object({}) { |(k, v), h| h[k.to_sym] = deep_sym.call(v) }
    when Array then obj.map { |i| deep_sym.call(i) }
    else obj
    end
  }

  user_id = user_data['user_id'] || user_data[:user_id]
  plan = user_data['plan'] || user_data[:plan] || 'free'

  user_info = deep_sym.call(user_info || context.task.context['user_info'] || {})
  custom_prefs = user_info[:preferences] || {}

  default_prefs = DEFAULT_PREFERENCES_DSL[plan] || DEFAULT_PREFERENCES_DSL['free']
  final_prefs = default_prefs.merge(custom_prefs)

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      preferences_id: "prefs_#{SecureRandom.hex(6)}",
      user_id: user_id, plan: plan, preferences: final_prefs,
      defaults_applied: default_prefs.keys.count,
      customizations: custom_prefs.keys.count,
      status: 'active', created_at: Time.now.utc.iso8601,
      updated_at: Time.now.utc.iso8601
    },
    metadata: { operation: 'initialize_preferences', service: 'preferences_service',
                plan: plan, custom_preferences_count: custom_prefs.keys.count }
  )
end

MicroSendWelcomeSequenceDslHandler = step_handler(
  'microservices_dsl.step_handlers.send_welcome_sequence',
  depends_on: { user_data: 'create_user_account',
                billing_data: 'setup_billing_profile',
                preferences_data: 'initialize_preferences' }
) do |user_data:, billing_data:, preferences_data:, context:|
  raise TaskerCore::Errors::PermanentError.new('User data not found', error_code: 'MISSING_USER_DATA') unless user_data
  raise TaskerCore::Errors::PermanentError.new('Billing data not found', error_code: 'MISSING_BILLING_DATA') unless billing_data
  raise TaskerCore::Errors::PermanentError.new('Preferences data not found', error_code: 'MISSING_PREFERENCES_DATA') unless preferences_data

  user_id = user_data['user_id'] || user_data[:user_id]
  email = user_data['email'] || user_data[:email]
  plan = user_data['plan'] || user_data[:plan] || 'free'

  prefs = preferences_data['preferences'] || preferences_data[:preferences] || {}

  channels_used = []
  messages_sent = 0

  if prefs[:email_notifications] != false && prefs['email_notifications'] != false
    channels_used << 'email'
    messages_sent += 1
  end

  channels_used << 'in_app'
  messages_sent += 1

  if plan == 'enterprise'
    channels_used << 'sms'
    messages_sent += 1
  end

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      user_id: user_id, plan: plan, channels_used: channels_used,
      messages_sent: messages_sent,
      welcome_sequence_id: "welcome_#{SecureRandom.hex(6)}",
      status: 'sent', sent_at: Time.now.utc.iso8601
    },
    metadata: { operation: 'send_welcome_sequence', service: 'notification_service',
                plan: plan, channels_used: channels_used.count }
  )
end

MicroUpdateUserStatusDslHandler = step_handler(
  'microservices_dsl.step_handlers.update_user_status',
  depends_on: { user_data: 'create_user_account',
                billing_data: 'setup_billing_profile',
                preferences_data: 'initialize_preferences',
                welcome_data: 'send_welcome_sequence' }
) do |user_data:, billing_data:, preferences_data:, welcome_data:, context:|
  missing = []
  missing << 'create_user_account' unless user_data
  missing << 'setup_billing_profile' unless billing_data
  missing << 'initialize_preferences' unless preferences_data
  missing << 'send_welcome_sequence' unless welcome_data

  unless missing.empty?
    raise TaskerCore::Errors::PermanentError.new(
      "Cannot complete registration: missing results from steps: #{missing.join(', ')}",
      error_code: 'INCOMPLETE_WORKFLOW'
    )
  end

  user_id = user_data['user_id'] || user_data[:user_id]
  plan = user_data['plan'] || user_data[:plan] || 'free'

  TaskerCore::Types::StepHandlerCallResult.success(
    result: {
      user_id: user_id, status: 'active', plan: plan,
      activation_timestamp: Time.now.utc.iso8601,
      all_services_coordinated: true,
      services_completed: %w[user_service billing_service preferences_service notification_service]
    },
    metadata: { operation: 'update_user_status', service: 'user_service',
                plan: plan, workflow_complete: true }
  )
end
