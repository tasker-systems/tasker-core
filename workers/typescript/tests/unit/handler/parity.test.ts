/**
 * TAS-294 Phase 2: Parity tests between verbose (class-based) and DSL (factory) handlers.
 *
 * Verifies that DSL handlers produce identical result objects to their verbose
 * counterparts when given the same inputs and dependency results.
 *
 * Test strategy:
 * - Build mock StepContext objects with appropriate inputs/dependencies
 * - Execute both verbose and DSL handlers with the same context
 * - Assert result.result matches between the two (ignoring timestamps and generated IDs)
 * - Assert result.success matches
 * - For error handlers, assert error messages and retryable flags match
 */

import { describe, expect, it } from 'bun:test';
import type { FfiStepEvent } from '../../../src/ffi/types.js';
import { StepHandler } from '../../../src/handler/base.js';
import { StepContext } from '../../../src/types/step-context.js';
import type { StepHandlerResult } from '../../../src/types/step-handler-result.js';

// Verbose handlers
import { LinearStep1Handler, LinearStep2Handler, LinearStep3Handler, LinearStep4Handler } from '../../handlers/examples/linear_workflow/index.js';
import { DiamondStartHandler, DiamondBranchBHandler, DiamondBranchCHandler, DiamondEndHandler } from '../../handlers/examples/diamond_workflow/index.js';
import { SuccessStepHandler } from '../../handlers/examples/test_scenarios/index.js';
import { SuccessHandler, PermanentErrorHandler, RetryableErrorHandler } from '../../handlers/examples/test_errors/index.js';
import { ValidateRequestHandler, AutoApproveHandler, ManagerApprovalHandler, FinanceReviewHandler, FinalizeApprovalHandler } from '../../handlers/examples/conditional_approval/index.js';
import { ValidateOrderHandler, ProcessPaymentHandler, UpdateInventoryHandler, SendNotificationHandler } from '../../handlers/examples/domain_events/index.js';
import { MultiMethodHandler, AlternateMethodHandler } from '../../handlers/examples/resolver_tests/index.js';
import { ExtractSalesDataHandler, ExtractInventoryDataHandler, ExtractCustomerDataHandler } from '../../handlers/examples/blog_examples/post_02_data_pipeline/index.js';

// DSL handlers
import { LinearStep1DslHandler, LinearStep2DslHandler, LinearStep3DslHandler, LinearStep4DslHandler } from '../../handlers/dsl_examples/linear_workflow/index.js';
import { DiamondStartDslHandler, DiamondBranchBDslHandler, DiamondBranchCDslHandler, DiamondEndDslHandler } from '../../handlers/dsl_examples/diamond_workflow/index.js';
import { SuccessStepDslHandler } from '../../handlers/dsl_examples/test_scenarios/index.js';
import { SuccessDslHandler, PermanentErrorDslHandler, RetryableErrorDslHandler } from '../../handlers/dsl_examples/test_errors/index.js';
import { ValidateRequestDslHandler, AutoApproveDslHandler, ManagerApprovalDslHandler, FinanceReviewDslHandler, FinalizeApprovalDslHandler } from '../../handlers/dsl_examples/conditional_approval/index.js';
import { ValidateOrderDslHandler, ProcessPaymentDslHandler as DomainProcessPaymentDslHandler, UpdateInventoryDslHandler as DomainUpdateInventoryDslHandler, SendNotificationDslHandler } from '../../handlers/dsl_examples/domain_events/index.js';
import { MultiMethodDslHandler, MultiMethodValidateDslHandler, MultiMethodProcessDslHandler, MultiMethodRefundDslHandler, AlternateMethodDslHandler } from '../../handlers/dsl_examples/resolver_tests/index.js';
import { ExtractSalesDataDslHandler, ExtractInventoryDataDslHandler, ExtractCustomerDataDslHandler } from '../../handlers/dsl_examples/blog_examples/post_02_data_pipeline/index.js';

// ============================================================================
// Test Helpers
// ============================================================================

function createFfiEvent(): FfiStepEvent {
  return {
    event_id: 'event-123',
    task_uuid: 'task-456',
    step_uuid: 'step-789',
    correlation_id: 'corr-001',
    trace_id: null,
    span_id: null,
    task_correlation_id: 'task-corr-001',
    parent_correlation_id: null,
    task: {
      task_uuid: 'task-456',
      named_task_uuid: 'named-task-001',
      name: 'TestTask',
      namespace: 'test',
      version: '1.0.0',
      context: null,
      correlation_id: 'corr-001',
      parent_correlation_id: null,
      complete: false,
      priority: 0,
      initiator: null,
      source_system: null,
      reason: null,
      tags: null,
      identity_hash: 'hash-123',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      requested_at: new Date().toISOString(),
    },
    workflow_step: {
      workflow_step_uuid: 'ws-001',
      task_uuid: 'task-456',
      named_step_uuid: 'ns-001',
      retries: 0,
      attempts: 0,
      max_attempts: 3,
      in_process: false,
      processed: false,
      inputs: null,
      results: null,
      checkpoint: null,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    },
    step_definition: {
      named_step_uuid: 'ns-001',
      named_task_uuid: 'nt-001',
      name: 'test_step',
      handler_class: 'TestHandler',
      handler_callable: 'test_handler',
      handler_initialization: null,
      default_retries: 3,
      skippable: false,
      is_decision_step: false,
      is_batchable_step: false,
      depends_on_step_names: null,
    },
    dependency_results: null,
  } as unknown as FfiStepEvent;
}

function makeContext(overrides: {
  inputData?: Record<string, unknown>;
  dependencyResults?: Record<string, unknown>;
  stepConfig?: Record<string, unknown>;
  stepInputs?: Record<string, unknown>;
  retryCount?: number;
} = {}): StepContext {
  const event = createFfiEvent();
  return new StepContext({
    event,
    taskUuid: 'task-123',
    stepUuid: 'step-456',
    correlationId: 'corr-789',
    handlerName: 'test_handler',
    inputData: overrides.inputData ?? {},
    dependencyResults: overrides.dependencyResults ?? {},
    stepConfig: overrides.stepConfig ?? {},
    stepInputs: overrides.stepInputs ?? {},
    retryCount: overrides.retryCount ?? 0,
    maxRetries: 3,
  });
}

/**
 * Strip timestamp-like and random-generated fields from a result
 * so we can compare the stable parts of the output.
 */
function stripVolatileFields(obj: unknown): unknown {
  if (obj === null || obj === undefined) return obj;
  if (typeof obj !== 'object') return obj;
  if (Array.isArray(obj)) return obj.map(stripVolatileFields);

  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(obj as Record<string, unknown>)) {
    // Skip fields that contain timestamps or generated IDs
    if (
      key.endsWith('_at') ||
      key === 'timestamp' ||
      key === 'audit_id' ||
      key === 'payment_id' ||
      key === 'refund_id' ||
      key === 'email_id' ||
      key === 'order_id' ||
      key === 'order_number' ||
      key === 'transaction_id' ||
      key === 'reservation_id' ||
      key === 'inventory_log_id' ||
      key === 'billing_id' ||
      key === 'preferences_id' ||
      key === 'user_id' ||
      key === 'welcome_sequence_id' ||
      key === 'approval_id' ||
      key === 'manager_id' ||
      key === 'record_id' ||
      key === 'message_id' ||
      key === 'delegated_task_id' ||
      key === 'correlation_id' ||
      key === 'gateway_transaction_id' ||
      key === 'next_billing_date'
    ) {
      continue;
    }
    result[key] = stripVolatileFields(value);
  }
  return result;
}

async function runHandler(
  HandlerClass: typeof StepHandler & { new (): StepHandler },
  context: StepContext
): Promise<StepHandlerResult> {
  const handler = new HandlerClass();
  return handler.call(context);
}

// ============================================================================
// Parity Tests
// ============================================================================

describe('TAS-294 Verbose vs DSL Parity', () => {
  describe('Linear Workflow', () => {
    it('Step1: square even number', async () => {
      const ctx = makeContext({ inputData: { even_number: 4 } });
      const verbose = await runHandler(LinearStep1Handler, ctx);
      const dsl = await runHandler(LinearStep1DslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });

    it('Step1: rejects odd number', async () => {
      const ctx = makeContext({ inputData: { even_number: 3 } });
      const verbose = await runHandler(LinearStep1Handler, ctx);
      const dsl = await runHandler(LinearStep1DslHandler, ctx);

      expect(dsl.success).toBe(false);
      expect(verbose.success).toBe(false);
    });

    it('Step1: rejects missing input', async () => {
      const ctx = makeContext({});
      const verbose = await runHandler(LinearStep1Handler, ctx);
      const dsl = await runHandler(LinearStep1DslHandler, ctx);

      expect(dsl.success).toBe(false);
      expect(verbose.success).toBe(false);
    });

    it('Step2: adds constant', async () => {
      const ctx = makeContext({
        dependencyResults: {
          linear_step_1: { result: { squared_value: 16, operation: 'square', input: 4 } },
        },
      });
      const verbose = await runHandler(LinearStep2Handler, ctx);
      const dsl = await runHandler(LinearStep2DslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });

    it('Step3: multiplies by factor', async () => {
      const ctx = makeContext({
        dependencyResults: {
          linear_step_2: { result: { added_value: 26, operation: 'add', constant: 10, input: 16 } },
        },
      });
      const verbose = await runHandler(LinearStep3Handler, ctx);
      const dsl = await runHandler(LinearStep3DslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });

    it('Step4: divides for final result', async () => {
      const ctx = makeContext({
        dependencyResults: {
          linear_step_3: { result: { multiplied_value: 78, operation: 'multiply', factor: 3, input: 26 } },
        },
      });
      const verbose = await runHandler(LinearStep4Handler, ctx);
      const dsl = await runHandler(LinearStep4DslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });
  });

  describe('Diamond Workflow', () => {
    const squaredResult = { result: { squared_value: 36, operation: 'square', input: 6 } };

    it('Start: squares even number', async () => {
      const ctx = makeContext({ inputData: { even_number: 6 } });
      const verbose = await runHandler(DiamondStartHandler, ctx);
      const dsl = await runHandler(DiamondStartDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });

    it('BranchB: adds constant', async () => {
      const ctx = makeContext({ dependencyResults: { diamond_start_ts: squaredResult } });
      const verbose = await runHandler(DiamondBranchBHandler, ctx);
      const dsl = await runHandler(DiamondBranchBDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });

    it('BranchC: multiplies by factor', async () => {
      const ctx = makeContext({ dependencyResults: { diamond_start_ts: squaredResult } });
      const verbose = await runHandler(DiamondBranchCHandler, ctx);
      const dsl = await runHandler(DiamondBranchCDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });

    it('End: averages branches', async () => {
      const ctx = makeContext({
        dependencyResults: {
          diamond_branch_b_ts: { result: { branch_b_value: 61, operation: 'add', constant: 25, input: 36 } },
          diamond_branch_c_ts: { result: { branch_c_value: 72, operation: 'multiply', factor: 2, input: 36 } },
        },
      });
      const verbose = await runHandler(DiamondEndHandler, ctx);
      const dsl = await runHandler(DiamondEndDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });
  });

  describe('Test Scenarios', () => {
    it('SuccessStepHandler matches', async () => {
      const ctx = makeContext({ inputData: { message: 'Test message' } });
      const verbose = await runHandler(SuccessStepHandler, ctx);
      const dsl = await runHandler(SuccessStepDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      // Strip timestamps for comparison
      const vResult = stripVolatileFields(verbose.result);
      const dResult = stripVolatileFields(dsl.result);
      expect(dResult).toEqual(vResult);
    });
  });

  describe('Error Handlers', () => {
    it('SuccessHandler matches', async () => {
      const ctx = makeContext({ inputData: { message: 'Custom success' } });
      const verbose = await runHandler(SuccessHandler, ctx);
      const dsl = await runHandler(SuccessDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      const vResult = stripVolatileFields(verbose.result);
      const dResult = stripVolatileFields(dsl.result);
      expect(dResult).toEqual(vResult);
    });

    it('PermanentErrorHandler matches', async () => {
      const ctx = makeContext({ inputData: { message: 'Test permanent error' } });
      const verbose = await runHandler(PermanentErrorHandler, ctx);
      const dsl = await runHandler(PermanentErrorDslHandler, ctx);

      expect(dsl.success).toBe(false);
      expect(verbose.success).toBe(false);
      expect(dsl.retryable).toBe(verbose.retryable);
      expect(dsl.errorMessage).toBe(verbose.errorMessage);
      expect(dsl.errorType).toBe(verbose.errorType);
      // Metadata comparison (strip timestamps)
      const vMeta = stripVolatileFields(dsl.metadata);
      const dMeta = stripVolatileFields(verbose.metadata);
      expect(vMeta).toEqual(dMeta);
    });

    it('RetryableErrorHandler matches', async () => {
      const ctx = makeContext({ inputData: { message: 'Test retryable error' } });
      const verbose = await runHandler(RetryableErrorHandler, ctx);
      const dsl = await runHandler(RetryableErrorDslHandler, ctx);

      expect(dsl.success).toBe(false);
      expect(verbose.success).toBe(false);
      expect(dsl.retryable).toBe(verbose.retryable);
      expect(dsl.errorMessage).toBe(verbose.errorMessage);
      expect(dsl.errorType).toBe(verbose.errorType);
    });
  });

  describe('Conditional Approval', () => {
    it('ValidateRequest matches', async () => {
      const ctx = makeContext({
        inputData: { amount: 500, requester: 'test@example.com', purpose: 'Testing' },
      });
      const verbose = await runHandler(ValidateRequestHandler, ctx);
      const dsl = await runHandler(ValidateRequestDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      const vResult = stripVolatileFields(verbose.result);
      const dResult = stripVolatileFields(dsl.result);
      expect(dResult).toEqual(vResult);
    });

    it('ValidateRequest rejects missing fields', async () => {
      const ctx = makeContext({ inputData: {} });
      const verbose = await runHandler(ValidateRequestHandler, ctx);
      const dsl = await runHandler(ValidateRequestDslHandler, ctx);

      expect(dsl.success).toBe(false);
      expect(verbose.success).toBe(false);
    });

    it('AutoApprove matches', async () => {
      const ctx = makeContext({
        dependencyResults: {
          validate_request_ts: {
            result: { validated: true, amount: 500, requester: 'test@example.com', purpose: 'Test' },
          },
        },
      });
      const verbose = await runHandler(AutoApproveHandler, ctx);
      const dsl = await runHandler(AutoApproveDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      const vResult = stripVolatileFields(verbose.result);
      const dResult = stripVolatileFields(dsl.result);
      expect(dResult).toEqual(vResult);
    });

    it('ManagerApproval matches', async () => {
      const ctx = makeContext({
        dependencyResults: {
          validate_request_ts: {
            result: { validated: true, amount: 2000, requester: 'test@example.com', purpose: 'Test' },
          },
        },
      });
      const verbose = await runHandler(ManagerApprovalHandler, ctx);
      const dsl = await runHandler(ManagerApprovalDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      const vResult = stripVolatileFields(verbose.result);
      const dResult = stripVolatileFields(dsl.result);
      expect(dResult).toEqual(vResult);
    });

    it('FinanceReview matches', async () => {
      const ctx = makeContext({
        dependencyResults: {
          validate_request_ts: {
            result: { validated: true, amount: 6000, requester: 'test@example.com', purpose: 'Test' },
          },
        },
      });
      const verbose = await runHandler(FinanceReviewHandler, ctx);
      const dsl = await runHandler(FinanceReviewDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      const vResult = stripVolatileFields(verbose.result);
      const dResult = stripVolatileFields(dsl.result);
      expect(dResult).toEqual(vResult);
    });

    it('FinalizeApproval matches with auto-approve', async () => {
      const ctx = makeContext({
        dependencyResults: {
          auto_approve_ts: { result: { approved: true, approval_type: 'automatic' } },
          manager_approval_ts: null,
          finance_review_ts: null,
        },
      });
      const verbose = await runHandler(FinalizeApprovalHandler, ctx);
      const dsl = await runHandler(FinalizeApprovalDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      const vResult = stripVolatileFields(verbose.result);
      const dResult = stripVolatileFields(dsl.result);
      expect(dResult).toEqual(vResult);
    });
  });

  describe('Domain Events', () => {
    it('ValidateOrder matches', async () => {
      const ctx = makeContext({
        inputData: { order_id: 'ORD-001', customer_id: 'CUST-001', amount: 100 },
      });
      const verbose = await runHandler(ValidateOrderHandler, ctx);
      const dsl = await runHandler(ValidateOrderDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      const vResult = stripVolatileFields(verbose.result);
      const dResult = stripVolatileFields(dsl.result);
      expect(dResult).toEqual(vResult);
    });

    it('ValidateOrder rejects missing fields', async () => {
      const ctx = makeContext({ inputData: {} });
      const verbose = await runHandler(ValidateOrderHandler, ctx);
      const dsl = await runHandler(ValidateOrderDslHandler, ctx);

      expect(dsl.success).toBe(false);
      expect(verbose.success).toBe(false);
    });
  });

  describe('Resolver Tests (Multi-Method)', () => {
    it('MultiMethod call matches', async () => {
      const ctx = makeContext({ inputData: { data: { amount: 100 } } });
      const verbose = await runHandler(MultiMethodHandler, ctx);
      const dsl = await runHandler(MultiMethodDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });

    it('MultiMethod validate matches (with valid data)', async () => {
      const ctx = makeContext({ inputData: { data: { amount: 100 } } });
      const verboseHandler = new MultiMethodHandler();
      const verbose = await verboseHandler.validate(ctx);
      const dsl = await runHandler(MultiMethodValidateDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });

    it('MultiMethod validate matches (missing amount)', async () => {
      const ctx = makeContext({ inputData: { data: {} } });
      const verboseHandler = new MultiMethodHandler();
      const verbose = await verboseHandler.validate(ctx);
      const dsl = await runHandler(MultiMethodValidateDslHandler, ctx);

      expect(dsl.success).toBe(false);
      expect(verbose.success).toBe(false);
    });

    it('MultiMethod process matches', async () => {
      const ctx = makeContext({ inputData: { data: { amount: 100 } } });
      const verboseHandler = new MultiMethodHandler();
      const verbose = await verboseHandler.process(ctx);
      const dsl = await runHandler(MultiMethodProcessDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });

    it('MultiMethod refund matches (stable fields)', async () => {
      const ctx = makeContext({ inputData: { data: { amount: 50, reason: 'defective' } } });
      const verboseHandler = new MultiMethodHandler();
      const verbose = await verboseHandler.refund(ctx);
      const dsl = await runHandler(MultiMethodRefundDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      // refund_id contains Date.now() so strip it
      const vResult = stripVolatileFields(verbose.result);
      const dResult = stripVolatileFields(dsl.result);
      expect(dResult).toEqual(vResult);
    });

    it('AlternateMethod call matches', async () => {
      const ctx = makeContext({});
      const verbose = await runHandler(AlternateMethodHandler, ctx);
      const dsl = await runHandler(AlternateMethodDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect(dsl.result).toEqual(verbose.result);
    });
  });

  describe('Data Pipeline (Extract Handlers - no deps)', () => {
    it('ExtractSalesData matches', async () => {
      const ctx = makeContext({});
      const verbose = await runHandler(ExtractSalesDataHandler, ctx);
      const dsl = await runHandler(ExtractSalesDataDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      // Records are deterministic sample data
      expect((dsl.result as Record<string, unknown>).records).toEqual(
        (verbose.result as Record<string, unknown>).records
      );
      expect((dsl.result as Record<string, unknown>).source).toEqual(
        (verbose.result as Record<string, unknown>).source
      );
      expect((dsl.result as Record<string, unknown>).total_amount).toEqual(
        (verbose.result as Record<string, unknown>).total_amount
      );
    });

    it('ExtractInventoryData matches', async () => {
      const ctx = makeContext({});
      const verbose = await runHandler(ExtractInventoryDataHandler, ctx);
      const dsl = await runHandler(ExtractInventoryDataDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect((dsl.result as Record<string, unknown>).records).toEqual(
        (verbose.result as Record<string, unknown>).records
      );
      expect((dsl.result as Record<string, unknown>).total_quantity).toEqual(
        (verbose.result as Record<string, unknown>).total_quantity
      );
    });

    it('ExtractCustomerData matches', async () => {
      const ctx = makeContext({});
      const verbose = await runHandler(ExtractCustomerDataHandler, ctx);
      const dsl = await runHandler(ExtractCustomerDataDslHandler, ctx);

      expect(dsl.success).toBe(verbose.success);
      expect((dsl.result as Record<string, unknown>).records).toEqual(
        (verbose.result as Record<string, unknown>).records
      );
      expect((dsl.result as Record<string, unknown>).total_customers).toEqual(
        (verbose.result as Record<string, unknown>).total_customers
      );
      expect((dsl.result as Record<string, unknown>).tier_breakdown).toEqual(
        (verbose.result as Record<string, unknown>).tier_breakdown
      );
    });
  });

  describe('Handler Registration', () => {
    it('ALL_DSL_HANDLERS has expected count', async () => {
      const { ALL_DSL_HANDLERS } = await import('../../handlers/dsl_examples/index.js');
      // All handlers should be loadable
      expect(ALL_DSL_HANDLERS.length).toBeGreaterThanOrEqual(50);

      // All should extend StepHandler
      for (const HandlerClass of ALL_DSL_HANDLERS) {
        const handler = new HandlerClass();
        expect(handler).toBeInstanceOf(StepHandler);
        expect(handler.name).toBeTruthy();
        expect(handler.version).toBeTruthy();
      }
    });

    it('DSL handlers have unique names', async () => {
      const { ALL_DSL_HANDLERS } = await import('../../handlers/dsl_examples/index.js');
      const names = ALL_DSL_HANDLERS.map((H: typeof StepHandler) => H.handlerName);
      const uniqueNames = new Set(names);
      expect(uniqueNames.size).toBe(names.length);
    });
  });
});
