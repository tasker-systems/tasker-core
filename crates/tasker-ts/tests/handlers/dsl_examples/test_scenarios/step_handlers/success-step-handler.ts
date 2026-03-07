/**
 * Success Step Handler DSL (TAS-294).
 *
 * Factory API equivalent of the class-based SuccessStepHandler.
 */

import { defineHandler } from '../../../../../src/handler/functional.js';

/**
 * Simple step handler that always succeeds.
 */
export const SuccessStepDslHandler = defineHandler(
  'TestScenariosDsl.StepHandlers.SuccessStepDslHandler',
  { inputs: { message: 'message' } },
  async ({ message }) => {
    const msg = (message as string) ?? 'Hello from TypeScript!';
    const timestamp = new Date().toISOString();

    return {
      message: msg,
      timestamp,
      handler: 'SuccessStepHandler',
      language: 'typescript',
    };
  }
);
