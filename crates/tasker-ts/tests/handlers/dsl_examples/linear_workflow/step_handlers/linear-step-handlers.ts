/**
 * Linear Workflow DSL Step Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based linear workflow handlers.
 * Produces identical output for parity testing.
 */

import { defineHandler, PermanentError } from '../../../../../src/handler/functional.js';

/**
 * Step 1: Square the initial even number.
 */
export const LinearStep1DslHandler = defineHandler(
  'LinearWorkflowDsl.StepHandlers.LinearStep1DslHandler',
  { inputs: { evenNumber: 'even_number' } },
  async ({ evenNumber }) => {
    const n = evenNumber as number | undefined | null;

    if (n === undefined || n === null) {
      throw new PermanentError('Missing required input: even_number');
    }

    if (n % 2 !== 0) {
      throw new PermanentError(`Input must be an even number, got: ${n}`);
    }

    const squaredValue = n * n;

    return {
      squared_value: squaredValue,
      operation: 'square',
      input: n,
    };
  }
);

/**
 * Step 2: Add constant to squared result.
 */
export const LinearStep2DslHandler = defineHandler(
  'LinearWorkflowDsl.StepHandlers.LinearStep2DslHandler',
  { depends: { step1Result: 'linear_step_1_dsl_ts' } },
  async ({ step1Result: _step1Result, context }) => {
    const squaredValue = context.getDependencyField('linear_step_1_dsl_ts', 'squared_value') as
      | number
      | null;

    if (squaredValue === null || squaredValue === undefined) {
      throw new PermanentError('Missing dependency result from linear_step_1_dsl_ts');
    }

    const constant = 10;
    const addedValue = squaredValue + constant;

    return {
      added_value: addedValue,
      operation: 'add',
      constant,
      input: squaredValue,
    };
  }
);

/**
 * Step 3: Multiply by factor.
 */
export const LinearStep3DslHandler = defineHandler(
  'LinearWorkflowDsl.StepHandlers.LinearStep3DslHandler',
  { depends: { step2Result: 'linear_step_2_dsl_ts' } },
  async ({ context }) => {
    const addedValue = context.getDependencyField('linear_step_2_dsl_ts', 'added_value') as
      | number
      | null;

    if (addedValue === null || addedValue === undefined) {
      throw new PermanentError('Missing dependency result from linear_step_2_dsl_ts');
    }

    const factor = 3;
    const multipliedValue = addedValue * factor;

    return {
      multiplied_value: multipliedValue,
      operation: 'multiply',
      factor,
      input: addedValue,
    };
  }
);

/**
 * Step 4: Divide for final result.
 */
export const LinearStep4DslHandler = defineHandler(
  'LinearWorkflowDsl.StepHandlers.LinearStep4DslHandler',
  { depends: { step3Result: 'linear_step_3_dsl_ts' } },
  async ({ context }) => {
    const multipliedValue = context.getDependencyField(
      'linear_step_3_dsl_ts',
      'multiplied_value'
    ) as number | null;

    if (multipliedValue === null || multipliedValue === undefined) {
      throw new PermanentError('Missing dependency result from linear_step_3_dsl_ts');
    }

    const divisor = 2;
    const finalValue = multipliedValue / divisor;

    return {
      final_value: finalValue,
      operation: 'divide',
      divisor,
      input: multipliedValue,
    };
  }
);
