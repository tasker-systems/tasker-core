/**
 * Error Testing DSL Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based error testing handlers.
 * Produces identical output for parity testing.
 *
 * NOTE: The verbose PermanentErrorHandler and RetryableErrorHandler use
 * this.failure() which returns a StepHandlerResult directly. In the DSL,
 * we return StepHandlerResult directly to match their exact output format.
 */

import { defineHandler } from '../../../../../src/handler/functional.js';
import { ErrorType } from '../../../../../src/types/error-type.js';
import { StepHandlerResult } from '../../../../../src/types/step-handler-result.js';

/**
 * Handler that always succeeds.
 */
export const SuccessDslHandler = defineHandler(
  'test_errors_dsl.step_handlers.SuccessDslHandler',
  { inputs: { message: 'message' } },
  async ({ message }) => {
    const msg = (message as string) ?? 'Success!';
    const timestamp = new Date().toISOString();

    return {
      message: msg,
      timestamp,
      scenario: 'success',
      handler: 'SuccessHandler',
    };
  }
);

/**
 * Handler that always fails with a permanent (non-retryable) error.
 */
export const PermanentErrorDslHandler = defineHandler(
  'test_errors_dsl.step_handlers.PermanentErrorDslHandler',
  { inputs: { message: 'message' } },
  async ({ message }) => {
    const msg = (message as string) ?? 'Permanent error - no retry allowed';

    return StepHandlerResult.failure(msg, ErrorType.PERMANENT_ERROR, false, {
      scenario: 'permanent_error',
      handler: 'PermanentErrorHandler',
      timestamp: new Date().toISOString(),
    });
  }
);

/**
 * Handler that always fails with a retryable error.
 */
export const RetryableErrorDslHandler = defineHandler(
  'test_errors_dsl.step_handlers.RetryableErrorDslHandler',
  { inputs: { message: 'message' } },
  async ({ message, context }) => {
    const msg = (message as string) ?? 'Retryable error - will be retried';

    return StepHandlerResult.failure(msg, ErrorType.RETRYABLE_ERROR, true, {
      scenario: 'retryable_error',
      handler: 'RetryableErrorHandler',
      timestamp: new Date().toISOString(),
      attempt: context.retryCount,
    });
  }
);
