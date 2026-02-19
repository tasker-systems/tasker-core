/**
 * Domain Event Publishing DSL Step Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based domain event handlers.
 * Produces identical output for parity testing.
 */

import {
  PermanentError,
  defineHandler,
} from '../../../../../src/handler/functional.js';

/**
 * Validate order details.
 */
export const ValidateOrderDslHandler = defineHandler(
  'domain_events_dsl.step_handlers.ValidateOrderDslHandler',
  {
    inputs: {
      orderId: 'order_id',
      customerId: 'customer_id',
      amount: 'amount',
    },
  },
  async ({ orderId, customerId, amount }) => {
    const errors: string[] = [];

    if (!orderId) {
      errors.push('order_id is required');
    }

    if (!customerId) {
      errors.push('customer_id is required');
    }

    const a = amount as number | undefined | null;
    if (a === undefined || a === null || a <= 0) {
      errors.push('amount must be a positive number');
    }

    if (errors.length > 0) {
      throw new PermanentError(errors.join('; '));
    }

    return {
      order_id: orderId,
      customer_id: customerId,
      amount: a,
      validated: true,
      validated_at: new Date().toISOString(),
    };
  }
);

/**
 * Process payment for the order.
 */
export const ProcessPaymentDslHandler = defineHandler(
  'domain_events_dsl.step_handlers.ProcessPaymentDslHandler',
  { depends: { validateResult: 'domain_events_ts_validate_order' } },
  async ({ validateResult }) => {
    const result = validateResult as Record<string, unknown> | null;

    if (!result) {
      throw new PermanentError(
        'Missing dependency result from domain_events_ts_validate_order'
      );
    }

    const orderId = result.order_id as string;
    const amount = result.amount as number;

    const paymentId = `PAY-${Date.now()}-${Math.random().toString(36).substring(7)}`;

    return {
      order_id: orderId,
      payment_id: paymentId,
      amount,
      payment_status: 'completed',
      processed_at: new Date().toISOString(),
    };
  }
);

/**
 * Update inventory for order items.
 */
export const UpdateInventoryDslHandler = defineHandler(
  'domain_events_dsl.step_handlers.UpdateInventoryDslHandler',
  {
    depends: {
      validateResult: 'domain_events_ts_validate_order',
      paymentResult: 'domain_events_ts_process_payment',
    },
  },
  async ({ validateResult, paymentResult }) => {
    const valResult = validateResult as Record<string, unknown> | null;
    const payResult = paymentResult as Record<string, unknown> | null;

    if (!valResult) {
      throw new PermanentError(
        'Missing dependency result from domain_events_ts_validate_order'
      );
    }

    if (!payResult) {
      throw new PermanentError(
        'Missing dependency result from domain_events_ts_process_payment'
      );
    }

    const orderId = valResult.order_id as string;
    const itemsUpdated = Math.floor(Math.random() * 5) + 1;

    return {
      order_id: orderId,
      items_updated: itemsUpdated,
      inventory_status: 'updated',
      updated_at: new Date().toISOString(),
    };
  }
);

/**
 * Send order confirmation notification.
 */
export const SendNotificationDslHandler = defineHandler(
  'domain_events_dsl.step_handlers.SendNotificationDslHandler',
  {
    depends: {
      validateResult: 'domain_events_ts_validate_order',
      paymentResult: 'domain_events_ts_process_payment',
      inventoryResult: 'domain_events_ts_update_inventory',
    },
  },
  async ({ validateResult, paymentResult, inventoryResult }) => {
    const valResult = validateResult as Record<string, unknown> | null;
    const payResult = paymentResult as Record<string, unknown> | null;
    const invResult = inventoryResult as Record<string, unknown> | null;

    if (!valResult) {
      throw new PermanentError(
        'Missing dependency result from domain_events_ts_validate_order'
      );
    }

    if (!payResult) {
      throw new PermanentError(
        'Missing dependency result from domain_events_ts_process_payment'
      );
    }

    if (!invResult) {
      throw new PermanentError(
        'Missing dependency result from domain_events_ts_update_inventory'
      );
    }

    const orderId = valResult.order_id as string;
    const customerId = valResult.customer_id as string;
    const channels = ['email', 'sms'];

    return {
      order_id: orderId,
      customer_id: customerId,
      channels,
      notification_status: 'sent',
      sent_at: new Date().toISOString(),
    };
  }
);
