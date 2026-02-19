/**
 * Microservices Coordination DSL Step Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based microservices handlers.
 * Produces identical output for parity testing.
 */

import { ErrorType } from '../../../../../../src/types/error-type.js';
import { StepHandlerResult } from '../../../../../../src/types/step-handler-result.js';
import {
  PermanentError,
  defineHandler,
} from '../../../../../../src/handler/functional.js';

// =============================================================================
// Types (same as verbose)
// =============================================================================

interface UserInfo {
  email: string;
  name: string;
  plan?: string;
  phone?: string;
  source?: string;
  preferences?: Record<string, unknown>;
}

interface UserData {
  user_id: string;
  email: string;
  name?: string;
  plan: string;
  status: string;
  created_at: string;
}

interface BillingData {
  billing_id?: string;
  user_id: string;
  plan: string;
  price?: number;
  billing_required: boolean;
  status: string;
  next_billing_date?: string;
}

interface PreferencesData {
  preferences_id: string;
  user_id: string;
  preferences: Record<string, unknown>;
}

interface WelcomeData {
  user_id: string;
  channels_used: string[];
  messages_sent: number;
  status: string;
}

// =============================================================================
// Simulated Data (same as verbose)
// =============================================================================

const EXISTING_USERS: Record<string, UserData> = {
  'existing@example.com': {
    user_id: 'user_existing_001',
    email: 'existing@example.com',
    name: 'Existing User',
    plan: 'free',
    status: 'active',
    created_at: '2025-01-01T00:00:00Z',
  },
};

const BILLING_TIERS: Record<string, { price: number; features: string[]; billing_required: boolean }> = {
  free: { price: 0, features: ['basic_features'], billing_required: false },
  pro: { price: 29.99, features: ['basic_features', 'advanced_analytics'], billing_required: true },
  enterprise: {
    price: 299.99,
    features: ['basic_features', 'advanced_analytics', 'priority_support', 'custom_integrations'],
    billing_required: true,
  },
};

const DEFAULT_PREFERENCES: Record<string, Record<string, unknown>> = {
  free: { email_notifications: true, marketing_emails: false, product_updates: true, weekly_digest: false, theme: 'light', language: 'en', timezone: 'UTC' },
  pro: { email_notifications: true, marketing_emails: true, product_updates: true, weekly_digest: true, theme: 'dark', language: 'en', timezone: 'UTC', api_notifications: true },
  enterprise: { email_notifications: true, marketing_emails: true, product_updates: true, weekly_digest: true, theme: 'dark', language: 'en', timezone: 'UTC', api_notifications: true, audit_logs: true, advanced_reports: true },
};

const WELCOME_TEMPLATES: Record<string, { subject: string; greeting: string; highlights: string[]; upgrade_prompt: string | null }> = {
  free: { subject: 'Welcome to Our Platform!', greeting: 'Thanks for joining us', highlights: ['Get started with basic features', 'Explore your dashboard', 'Join our community'], upgrade_prompt: 'Upgrade to Pro for advanced features' },
  pro: { subject: 'Welcome to Pro!', greeting: 'Thanks for upgrading to Pro', highlights: ['Access advanced analytics', 'Priority support', 'API access', 'Custom integrations'], upgrade_prompt: 'Consider Enterprise for dedicated support' },
  enterprise: { subject: 'Welcome to Enterprise!', greeting: 'Welcome to your Enterprise account', highlights: ['Dedicated account manager', 'Custom SLA', 'Advanced security features', 'Priority support 24/7'], upgrade_prompt: null },
};

// =============================================================================
// Helper Functions (same as verbose)
// =============================================================================

function generateId(prefix: string): string {
  const hex = Math.random().toString(16).substring(2, 14);
  return `${prefix}_${hex}`;
}

function isValidEmail(email: string): boolean {
  const pattern = /^[\w+\-.]+@[a-z\d-]+(\.[a-z\d-]+)*\.[a-z]+$/i;
  return pattern.test(email);
}

// =============================================================================
// Handlers
// =============================================================================

export const CreateUserAccountDslHandler = defineHandler(
  'MicroservicesDsl.StepHandlers.CreateUserAccountDslHandler',
  { inputs: { userInfo: 'user_info' } },
  async ({ userInfo }) => {
    const info = ((userInfo as UserInfo) || {}) as UserInfo;

    if (!info.email) {
      return StepHandlerResult.failure('Email is required but was not provided', ErrorType.PERMANENT_ERROR, false);
    }
    if (!info.name) {
      return StepHandlerResult.failure('Name is required but was not provided', ErrorType.PERMANENT_ERROR, false);
    }
    if (!isValidEmail(info.email)) {
      return StepHandlerResult.failure(`Invalid email format: ${info.email}`, ErrorType.PERMANENT_ERROR, false);
    }

    const plan = info.plan || 'free';
    const source = info.source || 'web';

    if (EXISTING_USERS[info.email]) {
      const existing = EXISTING_USERS[info.email];
      if (existing.name === info.name && existing.plan === plan) {
        return StepHandlerResult.success(
          { user_id: existing.user_id, email: existing.email, plan: existing.plan, status: 'already_exists', created_at: existing.created_at },
          { operation: 'create_user', service: 'user_service', idempotent: true }
        );
      } else {
        return StepHandlerResult.failure(
          `User with email ${info.email} already exists with different data`,
          ErrorType.PERMANENT_ERROR,
          false
        );
      }
    }

    const now = new Date().toISOString();
    const userId = generateId('user');

    return StepHandlerResult.success(
      { user_id: userId, email: info.email, name: info.name, plan, phone: info.phone, source, status: 'created', created_at: now },
      { operation: 'create_user', service: 'user_service', idempotent: false }
    );
  }
);

export const SetupBillingProfileDslHandler = defineHandler(
  'MicroservicesDsl.StepHandlers.SetupBillingProfileDslHandler',
  { depends: { userData: 'create_user_account' } },
  async ({ userData }) => {
    const user = userData as UserData | null;

    if (!user) {
      return StepHandlerResult.failure('User data not found from create_user_account step', ErrorType.PERMANENT_ERROR, false);
    }

    const userId = user.user_id;
    const plan = user.plan || 'free';
    const tierConfig = BILLING_TIERS[plan] || BILLING_TIERS.free;

    if (tierConfig.billing_required) {
      const now = new Date();
      const nextBilling = new Date(now.getTime() + 30 * 24 * 60 * 60 * 1000);

      return StepHandlerResult.success(
        {
          billing_id: generateId('billing'), user_id: userId, plan, price: tierConfig.price,
          currency: 'USD', billing_cycle: 'monthly', features: tierConfig.features,
          status: 'active', next_billing_date: nextBilling.toISOString(), created_at: now.toISOString(),
        },
        { operation: 'setup_billing', service: 'billing_service', plan, billing_required: true }
      );
    } else {
      return StepHandlerResult.success(
        { user_id: userId, plan, billing_required: false, status: 'skipped_free_plan', message: 'Free plan users do not require billing setup' },
        { operation: 'setup_billing', service: 'billing_service', plan, billing_required: false }
      );
    }
  }
);

export const InitializePreferencesDslHandler = defineHandler(
  'MicroservicesDsl.StepHandlers.InitializePreferencesDslHandler',
  {
    depends: { userData: 'create_user_account' },
    inputs: { userInfo: 'user_info' },
  },
  async ({ userData, userInfo }) => {
    const user = userData as UserData | null;

    if (!user) {
      return StepHandlerResult.failure('User data not found from create_user_account step', ErrorType.PERMANENT_ERROR, false);
    }

    const userId = user.user_id;
    const plan = user.plan || 'free';
    const info = ((userInfo as UserInfo) || {}) as UserInfo;
    const customPrefs = info.preferences || {};
    const defaultPrefs = DEFAULT_PREFERENCES[plan] || DEFAULT_PREFERENCES.free;
    const finalPrefs = { ...defaultPrefs, ...customPrefs };
    const now = new Date().toISOString();

    return StepHandlerResult.success(
      {
        preferences_id: generateId('prefs'), user_id: userId, plan, preferences: finalPrefs,
        defaults_applied: Object.keys(defaultPrefs).length, customizations: Object.keys(customPrefs).length,
        status: 'active', created_at: now, updated_at: now,
      },
      { operation: 'initialize_preferences', service: 'preferences_service', plan, custom_preferences_count: Object.keys(customPrefs).length }
    );
  }
);

export const SendWelcomeSequenceDslHandler = defineHandler(
  'MicroservicesDsl.StepHandlers.SendWelcomeSequenceDslHandler',
  {
    depends: {
      userData: 'create_user_account',
      billingData: 'setup_billing_profile',
      preferencesData: 'initialize_preferences',
    },
  },
  async ({ userData, billingData, preferencesData }) => {
    const user = userData as UserData | null;
    const billing = billingData as BillingData | null;
    const preferences = preferencesData as PreferencesData | null;

    const missing: string[] = [];
    if (!user) missing.push('create_user_account');
    if (!billing) missing.push('setup_billing_profile');
    if (!preferences) missing.push('initialize_preferences');

    if (missing.length > 0) {
      return StepHandlerResult.failure(`Missing results from steps: ${missing.join(', ')}`, ErrorType.PERMANENT_ERROR, false);
    }

    const validUser = user as UserData;
    const validBilling = billing as BillingData;
    const validPreferences = preferences as PreferencesData;

    const userId = validUser.user_id;
    const email = validUser.email;
    const plan = validUser.plan || 'free';
    const prefs = validPreferences.preferences || {};
    const template = WELCOME_TEMPLATES[plan] || WELCOME_TEMPLATES.free;
    const channelsUsed: string[] = [];
    const messagesSent: Array<Record<string, unknown>> = [];
    const now = new Date().toISOString();

    if (prefs.email_notifications !== false) {
      channelsUsed.push('email');
      // Build email body (simplified version matching verbose)
      const parts: string[] = [template.greeting, '', 'Here are your account highlights:'];
      for (const highlight of template.highlights) parts.push(`- ${highlight}`);
      if (plan !== 'free' && validBilling.billing_id) {
        parts.push('', `Billing ID: ${validBilling.billing_id}`);
        parts.push(`Next billing date: ${validBilling.next_billing_date}`);
      }
      if (template.upgrade_prompt) parts.push('', template.upgrade_prompt);
      parts.push('', `User ID: ${userId}`);

      messagesSent.push({ channel: 'email', to: email, subject: template.subject, body: parts.join('\n'), sent_at: now });
    }

    channelsUsed.push('in_app');
    messagesSent.push({ channel: 'in_app', user_id: userId, title: template.subject, message: template.greeting, sent_at: now });

    if (plan === 'enterprise') {
      channelsUsed.push('sms');
      messagesSent.push({ channel: 'sms', to: '+1-555-ENTERPRISE', message: 'Welcome to Enterprise! Your account manager will contact you soon.', sent_at: now });
    }

    return StepHandlerResult.success(
      { user_id: userId, plan, channels_used: channelsUsed, messages_sent: messagesSent.length, welcome_sequence_id: generateId('welcome'), status: 'sent', sent_at: now },
      { operation: 'send_welcome_sequence', service: 'notification_service', plan, channels_used: channelsUsed.length }
    );
  }
);

export const UpdateUserStatusDslHandler = defineHandler(
  'MicroservicesDsl.StepHandlers.UpdateUserStatusDslHandler',
  {
    depends: {
      userData: 'create_user_account',
      billingData: 'setup_billing_profile',
      preferencesData: 'initialize_preferences',
      welcomeData: 'send_welcome_sequence',
    },
  },
  async ({ userData, billingData, preferencesData, welcomeData }) => {
    const user = userData as UserData | null;
    const billing = billingData as BillingData | null;
    const preferences = preferencesData as PreferencesData | null;
    const welcome = welcomeData as WelcomeData | null;

    const missing: string[] = [];
    if (!user) missing.push('create_user_account');
    if (!billing) missing.push('setup_billing_profile');
    if (!preferences) missing.push('initialize_preferences');
    if (!welcome) missing.push('send_welcome_sequence');

    if (missing.length > 0) {
      return StepHandlerResult.failure(
        `Cannot complete registration: missing results from steps: ${missing.join(', ')}`,
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    const validUser = user as UserData;
    const validBilling = billing as BillingData;
    const validPreferences = preferences as PreferencesData;
    const validWelcome = welcome as WelcomeData;

    const userId = validUser.user_id;
    const email = validUser.email;
    const plan = validUser.plan || 'free';

    // Build registration summary
    const summary: Record<string, unknown> = {
      user_id: userId,
      email,
      plan,
      registration_status: 'complete',
    };

    if (plan !== 'free' && validBilling.billing_id) {
      summary.billing_id = validBilling.billing_id;
      summary.next_billing_date = validBilling.next_billing_date;
    }

    const prefs = validPreferences.preferences || {};
    summary.preferences_count = Object.keys(prefs).length;
    summary.welcome_sent = true;
    summary.notification_channels = validWelcome.channels_used;
    summary.user_created_at = validUser.created_at;
    summary.registration_completed_at = new Date().toISOString();

    const now = new Date().toISOString();

    return StepHandlerResult.success(
      {
        user_id: userId,
        status: 'active',
        plan,
        registration_summary: summary,
        activation_timestamp: now,
        all_services_coordinated: true,
        services_completed: ['user_service', 'billing_service', 'preferences_service', 'notification_service'],
      },
      { operation: 'update_user_status', service: 'user_service', plan, workflow_complete: true }
    );
  }
);
