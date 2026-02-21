/**
 * Multi-Method Handler DSL (TAS-294).
 *
 * The verbose MultiMethodHandler has call(), validate(), process(), refund()
 * methods. The DSL can't replicate multi-method dispatch directly, so we
 * use separate defineHandler for each method variant.
 *
 * For parity testing, each DSL handler produces the same output as the
 * corresponding method on the verbose MultiMethodHandler.
 */

import { defineHandler, PermanentError } from '../../../../../src/handler/functional.js';

/**
 * Multi-method handler: default call method.
 */
export const MultiMethodDslHandler = defineHandler(
  'ResolverTestsDsl.StepHandlers.MultiMethodDslHandler',
  { inputs: { data: 'data' } },
  async ({ data, context }) => {
    const input = (data as Record<string, unknown>) ?? {};

    return {
      invoked_method: 'call',
      handler: 'MultiMethodHandler',
      message: 'Default call method invoked',
      input_received: input,
      step_uuid: context.stepUuid,
    };
  }
);

/**
 * Multi-method handler: validate method variant.
 */
export const MultiMethodValidateDslHandler = defineHandler(
  'ResolverTestsDsl.StepHandlers.MultiMethodValidateDslHandler',
  { inputs: { data: 'data' } },
  async ({ data, context }) => {
    const input = (data as Record<string, unknown>) ?? {};
    const hasRequiredFields = input.amount !== undefined;

    if (!hasRequiredFields) {
      throw new PermanentError('Validation failed: missing required field "amount"');
    }

    return {
      invoked_method: 'validate',
      handler: 'MultiMethodHandler',
      message: 'Validation completed successfully',
      validated: true,
      input_validated: input,
      step_uuid: context.stepUuid,
    };
  }
);

/**
 * Multi-method handler: process method variant.
 */
export const MultiMethodProcessDslHandler = defineHandler(
  'ResolverTestsDsl.StepHandlers.MultiMethodProcessDslHandler',
  { inputs: { data: 'data' } },
  async ({ data, context }) => {
    const input = (data as Record<string, unknown>) ?? {};
    const amount = (input.amount as number) ?? 0;
    const processedAmount = amount * 1.1;

    return {
      invoked_method: 'process',
      handler: 'MultiMethodHandler',
      message: 'Processing completed',
      original_amount: amount,
      processed_amount: processedAmount,
      processing_fee: processedAmount - amount,
      step_uuid: context.stepUuid,
    };
  }
);

/**
 * Multi-method handler: refund method variant.
 */
export const MultiMethodRefundDslHandler = defineHandler(
  'ResolverTestsDsl.StepHandlers.MultiMethodRefundDslHandler',
  { inputs: { data: 'data' } },
  async ({ data, context }) => {
    const input = (data as Record<string, unknown>) ?? {};
    const amount = (input.amount as number) ?? 0;
    const reason = (input.reason as string) ?? 'not_specified';

    return {
      invoked_method: 'refund',
      handler: 'MultiMethodHandler',
      message: 'Refund processed',
      refund_amount: amount,
      refund_reason: reason,
      refund_id: `refund_${Date.now()}`,
      step_uuid: context.stepUuid,
    };
  }
);

/**
 * Alternate handler: default call method.
 */
export const AlternateMethodDslHandler = defineHandler(
  'ResolverTestsDsl.StepHandlers.AlternateMethodDslHandler',
  {},
  async ({ context }) => {
    return {
      invoked_method: 'call',
      handler: 'AlternateMethodHandler',
      message: 'Alternate handler default method',
      step_uuid: context.stepUuid,
    };
  }
);

/**
 * Alternate handler: execute_action method variant.
 */
export const AlternateMethodExecuteActionDslHandler = defineHandler(
  'ResolverTestsDsl.StepHandlers.AlternateMethodExecuteActionDslHandler',
  { inputs: { actionType: 'action_type' } },
  async ({ actionType, context }) => {
    const action = (actionType as string) ?? 'default_action';

    return {
      invoked_method: 'execute_action',
      handler: 'AlternateMethodHandler',
      message: 'Custom action executed',
      action_type: action,
      step_uuid: context.stepUuid,
    };
  }
);
