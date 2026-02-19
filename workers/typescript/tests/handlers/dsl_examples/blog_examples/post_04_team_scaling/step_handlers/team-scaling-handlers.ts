/**
 * Team Scaling DSL Step Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based team scaling handlers.
 * Produces identical output for parity testing.
 */

import { ErrorType } from '../../../../../../src/types/error-type.js';
import { StepHandlerResult } from '../../../../../../src/types/step-handler-result.js';
import {
  PermanentError,
  RetryableError,
  defineHandler,
} from '../../../../../../src/handler/functional.js';

// =============================================================================
// Types (same as verbose)
// =============================================================================

interface ValidationResult {
  request_validated: boolean;
  ticket_id: string;
  customer_id: string;
  ticket_status: string;
  customer_tier: string;
  original_purchase_date: string;
  payment_id: string;
}

interface PolicyCheckResult {
  policy_checked: boolean;
  requires_approval: boolean;
  customer_tier: string;
}

interface ApprovalResult {
  approval_obtained: boolean;
}

interface PaymentValidationResult {
  payment_validated: boolean;
  payment_id: string;
  refund_amount: number;
}

interface RefundResult {
  refund_processed: boolean;
  refund_id: string;
  payment_id: string;
  refund_amount: number;
  gateway_transaction_id: string;
}

// =============================================================================
// Configuration (same as verbose)
// =============================================================================

const REFUND_POLICIES: Record<string, { window_days: number; requires_approval: boolean; max_amount: number }> = {
  standard: { window_days: 30, requires_approval: true, max_amount: 10_000 },
  gold: { window_days: 60, requires_approval: false, max_amount: 50_000 },
  premium: { window_days: 90, requires_approval: false, max_amount: 100_000 },
};

// =============================================================================
// Helper Functions (same as verbose)
// =============================================================================

function generateId(prefix: string): string {
  const hex = Math.random().toString(16).substring(2, 14);
  return `${prefix}_${hex}`;
}

function generateUuid(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

function determineCustomerTier(customerId: string): string {
  if (customerId.toLowerCase().includes('vip') || customerId.toLowerCase().includes('premium')) return 'premium';
  else if (customerId.toLowerCase().includes('gold')) return 'gold';
  return 'standard';
}

// =============================================================================
// Customer Success Namespace Handlers
// =============================================================================

export const ValidateRefundRequestDslHandler = defineHandler(
  'TeamScalingDsl.CustomerSuccess.StepHandlers.ValidateRefundRequestDslHandler',
  {
    inputs: {
      ticketId: 'ticket_id',
      customerId: 'customer_id',
      refundAmount: 'refund_amount',
      refundReason: 'refund_reason',
    },
  },
  async ({ ticketId, customerId, refundAmount }) => {
    const tId = ticketId as string | undefined;
    const cId = customerId as string | undefined;
    const rAmount = refundAmount as number | undefined;

    const missingFields: string[] = [];
    if (!tId) missingFields.push('ticket_id');
    if (!cId) missingFields.push('customer_id');
    if (!rAmount) missingFields.push('refund_amount');

    if (missingFields.length > 0) {
      return StepHandlerResult.failure(
        `Missing required fields for refund validation: ${missingFields.join(', ')}`,
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    const validTicketId = tId as string;
    const validCustomerId = cId as string;

    // Simulate customer service system validation
    if (validTicketId.includes('ticket_closed')) {
      return StepHandlerResult.failure('Cannot process refund for closed ticket', ErrorType.PERMANENT_ERROR, false);
    }
    if (validTicketId.includes('ticket_cancelled')) {
      return StepHandlerResult.failure('Cannot process refund for cancelled ticket', ErrorType.PERMANENT_ERROR, false);
    }
    if (validTicketId.includes('ticket_duplicate')) {
      return StepHandlerResult.failure('Cannot process refund for duplicate ticket', ErrorType.PERMANENT_ERROR, false);
    }

    const purchaseDate = new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString();
    const now = new Date().toISOString();

    return StepHandlerResult.success(
      {
        request_validated: true,
        ticket_id: validTicketId,
        customer_id: validCustomerId,
        ticket_status: 'open',
        customer_tier: determineCustomerTier(validCustomerId),
        original_purchase_date: purchaseDate,
        payment_id: generateId('pay'),
        validation_timestamp: now,
        namespace: 'customer_success',
      },
      {
        operation: 'validate_refund_request',
        service: 'customer_service_platform',
        ticket_id: validTicketId,
        customer_tier: determineCustomerTier(validCustomerId),
      }
    );
  }
);

export const CheckRefundPolicyDslHandler = defineHandler(
  'TeamScalingDsl.CustomerSuccess.StepHandlers.CheckRefundPolicyDslHandler',
  {
    depends: { validationResult: 'validate_refund_request' },
    inputs: { refundAmount: 'refund_amount' },
  },
  async ({ validationResult, context }) => {
    const validation = validationResult as ValidationResult | null;

    if (!validation?.request_validated) {
      return StepHandlerResult.failure('Request validation must be completed before policy check', ErrorType.PERMANENT_ERROR, false);
    }

    const customerTier = (context.getDependencyField('validate_refund_request', 'customer_tier') as string) || 'standard';
    const purchaseDateStr = context.getDependencyField('validate_refund_request', 'original_purchase_date') as string;
    const refundAmount = context.getInput('refund_amount') as number;

    const policy = REFUND_POLICIES[customerTier] || REFUND_POLICIES.standard;
    const purchaseDate = new Date(purchaseDateStr);
    const daysSincePurchase = Math.floor((Date.now() - purchaseDate.getTime()) / (24 * 60 * 60 * 1000));
    const withinWindow = daysSincePurchase <= policy.window_days;
    const withinAmountLimit = refundAmount <= policy.max_amount;

    if (!withinWindow) {
      return StepHandlerResult.failure(
        `Refund request outside policy window: ${daysSincePurchase} days (max: ${policy.window_days} days)`,
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    if (!withinAmountLimit) {
      return StepHandlerResult.failure(
        `Refund amount exceeds policy limit: $${(refundAmount / 100).toFixed(2)} (max: $${(policy.max_amount / 100).toFixed(2)})`,
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    const now = new Date().toISOString();

    return StepHandlerResult.success(
      {
        policy_checked: true,
        policy_compliant: true,
        customer_tier: customerTier,
        refund_window_days: policy.window_days,
        days_since_purchase: daysSincePurchase,
        within_refund_window: withinWindow,
        requires_approval: policy.requires_approval,
        max_allowed_amount: policy.max_amount,
        policy_checked_at: now,
        namespace: 'customer_success',
      },
      { operation: 'check_refund_policy', service: 'policy_engine', customer_tier: customerTier, requires_approval: policy.requires_approval }
    );
  }
);

export const GetManagerApprovalDslHandler = defineHandler(
  'TeamScalingDsl.CustomerSuccess.StepHandlers.GetManagerApprovalDslHandler',
  {
    depends: {
      policyResult: 'check_refund_policy',
      validationResult: 'validate_refund_request',
    },
  },
  async ({ policyResult, context }) => {
    const policy = policyResult as PolicyCheckResult | null;

    if (!policy?.policy_checked) {
      return StepHandlerResult.failure('Policy check must be completed before approval', ErrorType.PERMANENT_ERROR, false);
    }

    const requiresApproval = context.getDependencyField('check_refund_policy', 'requires_approval') as boolean;
    const customerTier = context.getDependencyField('check_refund_policy', 'customer_tier') as string;
    const ticketId = context.getDependencyField('validate_refund_request', 'ticket_id') as string;
    const customerId = context.getDependencyField('validate_refund_request', 'customer_id') as string;
    const now = new Date().toISOString();

    if (requiresApproval) {
      if (ticketId.includes('ticket_denied')) {
        return StepHandlerResult.failure('Manager denied refund request', ErrorType.PERMANENT_ERROR, false);
      }
      if (ticketId.includes('ticket_pending')) {
        return StepHandlerResult.failure('Waiting for manager approval', ErrorType.RETRYABLE_ERROR, true);
      }

      return StepHandlerResult.success(
        {
          approval_obtained: true, approval_required: true, auto_approved: false,
          approval_id: generateId('appr'), manager_id: `mgr_${Math.floor(Math.random() * 5) + 1}`,
          manager_notes: `Approved refund request for customer ${customerId}`,
          approved_at: now, namespace: 'customer_success',
        },
        { operation: 'get_manager_approval', service: 'approval_portal', approval_required: true }
      );
    } else {
      return StepHandlerResult.success(
        {
          approval_obtained: true, approval_required: false, auto_approved: true,
          approval_id: null, manager_id: null,
          manager_notes: `Auto-approved for customer tier ${customerTier}`,
          approved_at: now, namespace: 'customer_success',
        },
        { operation: 'get_manager_approval', service: 'approval_portal', approval_required: false, auto_approved: true }
      );
    }
  }
);

export const ExecuteRefundWorkflowDslHandler = defineHandler(
  'TeamScalingDsl.CustomerSuccess.StepHandlers.ExecuteRefundWorkflowDslHandler',
  {
    depends: {
      approvalResult: 'get_manager_approval',
      validationResult: 'validate_refund_request',
    },
  },
  async ({ approvalResult, context }) => {
    const approval = approvalResult as ApprovalResult | null;

    if (!approval?.approval_obtained) {
      return StepHandlerResult.failure('Manager approval must be obtained before executing refund', ErrorType.PERMANENT_ERROR, false);
    }

    const paymentId = context.getDependencyField('validate_refund_request', 'payment_id') as string;
    if (!paymentId) {
      return StepHandlerResult.failure('Payment ID not found in validation results', ErrorType.PERMANENT_ERROR, false);
    }

    const correlationId = (context.getInput('correlation_id') as string) || `cs-${generateId('corr')}`;
    const taskId = `task_${generateUuid()}`;
    const now = new Date().toISOString();

    return StepHandlerResult.success(
      {
        task_delegated: true, target_namespace: 'payments', target_workflow: 'process_refund',
        delegated_task_id: taskId, delegated_task_status: 'created',
        delegation_timestamp: now, correlation_id: correlationId, namespace: 'customer_success',
      },
      {
        operation: 'execute_refund_workflow', service: 'task_delegation',
        target_namespace: 'payments', target_workflow: 'process_refund', delegated_task_id: taskId,
      }
    );
  }
);

export const UpdateTicketStatusDslHandler = defineHandler(
  'TeamScalingDsl.CustomerSuccess.StepHandlers.UpdateTicketStatusDslHandler',
  {
    depends: {
      delegationResult: 'execute_refund_workflow',
      validationResult: 'validate_refund_request',
    },
    inputs: { refundAmount: 'refund_amount' },
  },
  async ({ delegationResult, context }) => {
    const delegation = delegationResult as { task_delegated: boolean } | null;

    if (!delegation?.task_delegated) {
      return StepHandlerResult.failure('Refund workflow must be executed before updating ticket', ErrorType.PERMANENT_ERROR, false);
    }

    const ticketId = context.getDependencyField('validate_refund_request', 'ticket_id') as string;
    const delegatedTaskId = context.getDependencyField('execute_refund_workflow', 'delegated_task_id') as string;
    const correlationId = context.getDependencyField('execute_refund_workflow', 'correlation_id') as string;
    const refundAmount = context.getInput('refund_amount') as number;

    if (ticketId.includes('ticket_locked')) {
      return StepHandlerResult.failure('Ticket locked by another agent, will retry', ErrorType.RETRYABLE_ERROR, true);
    }
    if (ticketId.includes('ticket_update_error')) {
      return StepHandlerResult.failure('System error updating ticket', ErrorType.RETRYABLE_ERROR, true);
    }

    const now = new Date().toISOString();

    return StepHandlerResult.success(
      {
        ticket_updated: true, ticket_id: ticketId, previous_status: 'in_progress', new_status: 'resolved',
        resolution_note: `Refund of $${(refundAmount / 100).toFixed(2)} processed successfully. Delegated task ID: ${delegatedTaskId}. Correlation ID: ${correlationId}`,
        updated_at: now, refund_completed: true, delegated_task_id: delegatedTaskId, namespace: 'customer_success',
      },
      { operation: 'update_ticket_status', service: 'customer_service_platform', ticket_id: ticketId, new_status: 'resolved' }
    );
  }
);

// =============================================================================
// Payments Namespace Handlers
// =============================================================================

export const ValidatePaymentEligibilityDslHandler = defineHandler(
  'TeamScalingDsl.Payments.StepHandlers.ValidatePaymentEligibilityDslHandler',
  {
    inputs: {
      paymentId: 'payment_id',
      refundAmount: 'refund_amount',
      partialRefund: 'partial_refund',
    },
  },
  async ({ paymentId, refundAmount }) => {
    const pId = paymentId as string | undefined;
    const rAmount = refundAmount as number | undefined;

    const missingFields: string[] = [];
    if (!pId) missingFields.push('payment_id');
    if (!rAmount) missingFields.push('refund_amount');

    if (missingFields.length > 0) {
      return StepHandlerResult.failure(
        `Missing required fields for payment validation: ${missingFields.join(', ')}`,
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    const validPaymentId = pId as string;
    const validRefundAmount = rAmount as number;

    if (validRefundAmount <= 0) {
      return StepHandlerResult.failure(`Refund amount must be positive, got: ${validRefundAmount}`, ErrorType.PERMANENT_ERROR, false);
    }

    if (!/^pay_[a-zA-Z0-9_]+$/.test(validPaymentId)) {
      return StepHandlerResult.failure(`Invalid payment ID format: ${validPaymentId}`, ErrorType.PERMANENT_ERROR, false);
    }

    if (validPaymentId.includes('pay_test_insufficient')) {
      return StepHandlerResult.failure('Insufficient funds available for refund', ErrorType.PERMANENT_ERROR, false);
    }
    if (validPaymentId.includes('pay_test_processing')) {
      return StepHandlerResult.failure('Payment is still processing, cannot refund yet', ErrorType.RETRYABLE_ERROR, true);
    }
    if (validPaymentId.includes('pay_test_ineligible')) {
      return StepHandlerResult.failure('Payment is not eligible for refund: past refund window', ErrorType.PERMANENT_ERROR, false);
    }

    const now = new Date().toISOString();

    return StepHandlerResult.success(
      {
        payment_validated: true, payment_id: validPaymentId, original_amount: validRefundAmount + 1000,
        refund_amount: validRefundAmount, payment_method: 'credit_card', gateway_provider: 'MockPaymentGateway',
        eligibility_status: 'eligible', validation_timestamp: now, namespace: 'payments',
      },
      { operation: 'validate_payment_eligibility', service: 'payment_gateway', payment_id: validPaymentId, gateway_provider: 'MockPaymentGateway' }
    );
  }
);

export const ProcessGatewayRefundDslHandler = defineHandler(
  'TeamScalingDsl.Payments.StepHandlers.ProcessGatewayRefundDslHandler',
  { depends: { validationResult: 'validate_payment_eligibility' } },
  async ({ validationResult, context }) => {
    const validation = validationResult as PaymentValidationResult | null;

    if (!validation?.payment_validated) {
      return StepHandlerResult.failure('Payment validation must be completed before processing refund', ErrorType.PERMANENT_ERROR, false);
    }

    const paymentId = context.getDependencyField('validate_payment_eligibility', 'payment_id') as string;
    const refundAmount = context.getDependencyField('validate_payment_eligibility', 'refund_amount') as number;

    if (paymentId.includes('pay_test_gateway_timeout')) {
      return StepHandlerResult.failure('Gateway timeout, will retry', ErrorType.RETRYABLE_ERROR, true);
    }
    if (paymentId.includes('pay_test_gateway_error')) {
      return StepHandlerResult.failure('Gateway refund failed: Gateway error', ErrorType.PERMANENT_ERROR, false);
    }
    if (paymentId.includes('pay_test_rate_limit')) {
      return StepHandlerResult.failure('Gateway rate limited, will retry', ErrorType.RETRYABLE_ERROR, true);
    }

    const now = new Date();
    const estimatedArrival = new Date(now.getTime() + 5 * 24 * 60 * 60 * 1000).toISOString();

    return StepHandlerResult.success(
      {
        refund_processed: true, refund_id: generateId('rfnd'), payment_id: paymentId,
        refund_amount: refundAmount, refund_status: 'processed', gateway_transaction_id: generateId('gtx'),
        gateway_provider: 'MockPaymentGateway', processed_at: now.toISOString(),
        estimated_arrival: estimatedArrival, namespace: 'payments',
      },
      { operation: 'process_gateway_refund', service: 'payment_gateway', gateway_provider: 'MockPaymentGateway' }
    );
  }
);

export const UpdatePaymentRecordsDslHandler = defineHandler(
  'TeamScalingDsl.Payments.StepHandlers.UpdatePaymentRecordsDslHandler',
  { depends: { refundResult: 'process_gateway_refund' } },
  async ({ refundResult, context }) => {
    const refund = refundResult as RefundResult | null;

    if (!refund?.refund_processed) {
      return StepHandlerResult.failure('Gateway refund must be completed before updating records', ErrorType.PERMANENT_ERROR, false);
    }

    const paymentId = context.getDependencyField('process_gateway_refund', 'payment_id') as string;
    const refundId = context.getDependencyField('process_gateway_refund', 'refund_id') as string;

    if (paymentId.includes('pay_test_record_lock')) {
      return StepHandlerResult.failure('Payment record locked, will retry', ErrorType.RETRYABLE_ERROR, true);
    }
    if (paymentId.includes('pay_test_record_error')) {
      return StepHandlerResult.failure('Record update failed: Database error', ErrorType.RETRYABLE_ERROR, true);
    }

    const now = new Date().toISOString();

    return StepHandlerResult.success(
      {
        records_updated: true, payment_id: paymentId, refund_id: refundId, record_id: generateId('rec'),
        payment_status: 'refunded', refund_status: 'completed', history_entries_created: 2,
        updated_at: now, namespace: 'payments',
      },
      { operation: 'update_payment_records', service: 'payment_record_system', payment_id: paymentId }
    );
  }
);

export const NotifyCustomerDslHandler = defineHandler(
  'TeamScalingDsl.Payments.StepHandlers.NotifyCustomerDslHandler',
  {
    depends: { refundResult: 'process_gateway_refund' },
    inputs: { customerEmail: 'customer_email' },
  },
  async ({ refundResult, customerEmail, context }) => {
    const refund = refundResult as RefundResult | null;

    if (!refund?.refund_processed) {
      return StepHandlerResult.failure('Refund must be processed before sending notification', ErrorType.PERMANENT_ERROR, false);
    }

    const email = customerEmail as string | undefined;
    if (!email) {
      return StepHandlerResult.failure('Customer email is required for notification', ErrorType.PERMANENT_ERROR, false);
    }

    if (!/^[^@\s]+@[^@\s]+$/.test(email)) {
      return StepHandlerResult.failure(`Invalid customer email format: ${email}`, ErrorType.PERMANENT_ERROR, false);
    }

    const refundId = context.getDependencyField('process_gateway_refund', 'refund_id') as string;
    const refundAmount = context.getDependencyField('process_gateway_refund', 'refund_amount') as number;

    if (email.includes('@test_bounce')) {
      return StepHandlerResult.failure('Customer email bounced', ErrorType.PERMANENT_ERROR, false);
    }
    if (email.includes('@test_invalid')) {
      return StepHandlerResult.failure('Invalid customer email address', ErrorType.PERMANENT_ERROR, false);
    }
    if (email.includes('@test_rate_limit')) {
      return StepHandlerResult.failure('Email service rate limited, will retry', ErrorType.RETRYABLE_ERROR, true);
    }

    const now = new Date().toISOString();

    return StepHandlerResult.success(
      {
        notification_sent: true, customer_email: email, message_id: generateId('msg'),
        notification_type: 'refund_confirmation', sent_at: now, delivery_status: 'delivered',
        refund_id: refundId, refund_amount: refundAmount, namespace: 'payments',
      },
      { operation: 'notify_customer', service: 'email_service', customer_email: email }
    );
  }
);
