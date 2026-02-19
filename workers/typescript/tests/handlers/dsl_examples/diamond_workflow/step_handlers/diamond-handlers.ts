/**
 * Diamond Workflow DSL Step Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based diamond workflow handlers.
 * Produces identical output for parity testing.
 */

import {
  PermanentError,
  defineHandler,
} from '../../../../../src/handler/functional.js';

/**
 * Diamond Start: Square the initial even number.
 */
export const DiamondStartDslHandler = defineHandler(
  'diamond_workflow_dsl.step_handlers.DiamondStartDslHandler',
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
 * Diamond Branch B: Add constant to squared result.
 */
export const DiamondBranchBDslHandler = defineHandler(
  'diamond_workflow_dsl.step_handlers.DiamondBranchBDslHandler',
  { depends: { startResult: 'diamond_start_ts' } },
  async ({ startResult }) => {
    const result = startResult as Record<string, unknown> | null;

    if (!result) {
      throw new PermanentError('Missing dependency result from diamond_start_ts');
    }

    const squaredValue = result.squared_value as number;
    const constant = 25;
    const branchBValue = squaredValue + constant;

    return {
      branch_b_value: branchBValue,
      operation: 'add',
      constant,
      input: squaredValue,
    };
  }
);

/**
 * Diamond Branch C: Multiply squared result by factor.
 */
export const DiamondBranchCDslHandler = defineHandler(
  'diamond_workflow_dsl.step_handlers.DiamondBranchCDslHandler',
  { depends: { startResult: 'diamond_start_ts' } },
  async ({ startResult }) => {
    const result = startResult as Record<string, unknown> | null;

    if (!result) {
      throw new PermanentError('Missing dependency result from diamond_start_ts');
    }

    const squaredValue = result.squared_value as number;
    const factor = 2;
    const branchCValue = squaredValue * factor;

    return {
      branch_c_value: branchCValue,
      operation: 'multiply',
      factor,
      input: squaredValue,
    };
  }
);

/**
 * Diamond End: Average results from both branches.
 */
export const DiamondEndDslHandler = defineHandler(
  'diamond_workflow_dsl.step_handlers.DiamondEndDslHandler',
  {
    depends: {
      branchBResult: 'diamond_branch_b_ts',
      branchCResult: 'diamond_branch_c_ts',
    },
  },
  async ({ branchBResult, branchCResult }) => {
    const bResult = branchBResult as Record<string, unknown> | null;
    const cResult = branchCResult as Record<string, unknown> | null;

    if (!bResult) {
      throw new PermanentError('Missing dependency result from diamond_branch_b_ts');
    }

    if (!cResult) {
      throw new PermanentError('Missing dependency result from diamond_branch_c_ts');
    }

    const branchBValue = bResult.branch_b_value as number;
    const branchCValue = cResult.branch_c_value as number;
    const finalValue = (branchBValue + branchCValue) / 2;

    return {
      final_value: finalValue,
      operation: 'average',
      branch_b_value: branchBValue,
      branch_c_value: branchCValue,
    };
  }
);
