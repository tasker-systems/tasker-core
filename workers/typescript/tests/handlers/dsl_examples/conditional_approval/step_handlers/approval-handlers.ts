/**
 * Conditional Approval DSL Step Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based conditional approval handlers.
 * Produces identical output for parity testing.
 */

import {
  Decision,
  PermanentError,
  defineDecisionHandler,
  defineHandler,
} from '../../../../../src/handler/functional.js';

/**
 * Validate the approval request.
 */
export const ValidateRequestDslHandler = defineHandler(
  'conditional_approval_dsl.step_handlers.ValidateRequestDslHandler',
  {
    inputs: {
      amount: 'amount',
      requester: 'requester',
      purpose: 'purpose',
    },
  },
  async ({ amount, requester, purpose }) => {
    const errors: string[] = [];

    const a = amount as number | undefined | null;
    const r = requester as string | undefined | null;
    const p = purpose as string | undefined | null;

    if (a === undefined || a === null || a <= 0) {
      errors.push('Amount must be a positive number');
    }

    if (!r || r.trim().length === 0) {
      errors.push('Requester is required');
    }

    if (!p || p.trim().length === 0) {
      errors.push('Purpose is required');
    }

    if (errors.length > 0) {
      throw new PermanentError(errors.join('; '));
    }

    return {
      validated: true,
      amount: a,
      requester: r,
      purpose: p,
      validated_at: new Date().toISOString(),
    };
  }
);

const SMALL_THRESHOLD = 1000;
const LARGE_THRESHOLD = 5000;

/**
 * Decision point: Route based on amount thresholds.
 */
export const RoutingDecisionDslHandler = defineDecisionHandler(
  'conditional_approval_dsl.step_handlers.RoutingDecisionDslHandler',
  { depends: { validateResult: 'validate_request_ts' } },
  async ({ validateResult }) => {
    const result = validateResult as Record<string, unknown> | null;

    if (!result) {
      throw new PermanentError('Missing dependency result from validate_request_ts');
    }

    const amount = result.amount as number;
    const stepsToCreate: string[] = [];
    let routingPath: string;

    if (amount < SMALL_THRESHOLD) {
      stepsToCreate.push('auto_approve_ts');
      routingPath = 'auto_approve';
    } else if (amount < LARGE_THRESHOLD) {
      stepsToCreate.push('manager_approval_ts');
      routingPath = 'manager_approval';
    } else {
      stepsToCreate.push('manager_approval_ts');
      stepsToCreate.push('finance_review_ts');
      routingPath = 'dual_approval';
    }

    return Decision.route(stepsToCreate, {
      amount,
      routing_path: routingPath,
      steps_created: stepsToCreate,
      thresholds: {
        small: SMALL_THRESHOLD,
        large: LARGE_THRESHOLD,
      },
      decided_at: new Date().toISOString(),
    });
  }
);

/**
 * Automatic approval for small amounts.
 */
export const AutoApproveDslHandler = defineHandler(
  'conditional_approval_dsl.step_handlers.AutoApproveDslHandler',
  { depends: { validateResult: 'validate_request_ts' } },
  async ({ validateResult }) => {
    const result = validateResult as Record<string, unknown> | null;

    if (!result) {
      throw new PermanentError('Missing dependency result from validate_request_ts');
    }

    const amount = result.amount as number;
    const requester = result.requester as string;

    return {
      approved: true,
      approval_type: 'automatic',
      amount,
      requester,
      reason: 'Amount below automatic approval threshold',
      approved_at: new Date().toISOString(),
    };
  }
);

/**
 * Manager approval for medium/large amounts.
 */
export const ManagerApprovalDslHandler = defineHandler(
  'conditional_approval_dsl.step_handlers.ManagerApprovalDslHandler',
  { depends: { validateResult: 'validate_request_ts' } },
  async ({ validateResult }) => {
    const result = validateResult as Record<string, unknown> | null;

    if (!result) {
      throw new PermanentError('Missing dependency result from validate_request_ts');
    }

    const amount = result.amount as number;
    const requester = result.requester as string;
    const purpose = result.purpose as string;

    return {
      approved: true,
      approval_type: 'manager',
      amount,
      requester,
      purpose,
      approver: 'test_manager@example.com',
      approved_at: new Date().toISOString(),
    };
  }
);

/**
 * Finance review for large amounts.
 */
export const FinanceReviewDslHandler = defineHandler(
  'conditional_approval_dsl.step_handlers.FinanceReviewDslHandler',
  { depends: { validateResult: 'validate_request_ts' } },
  async ({ validateResult }) => {
    const result = validateResult as Record<string, unknown> | null;

    if (!result) {
      throw new PermanentError('Missing dependency result from validate_request_ts');
    }

    const amount = result.amount as number;
    const requester = result.requester as string;
    const purpose = result.purpose as string;

    return {
      approved: true,
      approval_type: 'finance',
      amount,
      requester,
      purpose,
      reviewer: 'finance_team@example.com',
      audit_id: `AUDIT-${Date.now()}`,
      reviewed_at: new Date().toISOString(),
    };
  }
);

/**
 * Convergence point: Finalize approval after all required approvals.
 */
export const FinalizeApprovalDslHandler = defineHandler(
  'conditional_approval_dsl.step_handlers.FinalizeApprovalDslHandler',
  {
    depends: {
      autoApproveResult: 'auto_approve_ts',
      managerApprovalResult: 'manager_approval_ts',
      financeReviewResult: 'finance_review_ts',
    },
  },
  async ({ autoApproveResult, managerApprovalResult, financeReviewResult }) => {
    const autoApprove = autoApproveResult as Record<string, unknown> | null;
    const managerApproval = managerApprovalResult as Record<string, unknown> | null;
    const financeReview = financeReviewResult as Record<string, unknown> | null;

    const approvals: Array<{ type: string; approved: boolean; approver?: string }> = [];

    if (autoApprove) {
      approvals.push({
        type: 'automatic',
        approved: autoApprove.approved as boolean,
      });
    }

    if (managerApproval) {
      approvals.push({
        type: 'manager',
        approved: managerApproval.approved as boolean,
        approver: managerApproval.approver as string,
      });
    }

    if (financeReview) {
      approvals.push({
        type: 'finance',
        approved: financeReview.approved as boolean,
        approver: financeReview.reviewer as string,
      });
    }

    const allApproved = approvals.every((a) => a.approved);

    return {
      final_approved: allApproved,
      approval_count: approvals.length,
      approvals,
      finalized_at: new Date().toISOString(),
    };
  }
);
