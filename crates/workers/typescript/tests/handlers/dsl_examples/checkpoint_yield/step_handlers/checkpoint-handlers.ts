/**
 * Checkpoint Yield DSL Step Handlers (TAS-294).
 *
 * The analyzer uses defineBatchAnalyzer factory. The worker and aggregator
 * are class-based because they need Batchable instance methods:
 * checkpointYield(), handleNoOpWorker(), getBatchWorkerInputs(),
 * batchWorkerSuccess(), etc.
 */

import { StepHandler } from '../../../../../src/handler/base.js';
import { BatchableStepHandler } from '../../../../../src/handler/batchable.js';
import { defineBatchAnalyzer } from '../../../../../src/handler/functional.js';
import type { StepContext } from '../../../../../src/types/step-context.js';
import type { StepHandlerResult } from '../../../../../src/types/step-handler-result.js';

const DEFAULT_TOTAL_ITEMS = 100;
const DEFAULT_ITEMS_PER_CHECKPOINT = 25;

/**
 * TAS-125: Checkpoint Yield Analyzer Handler (DSL).
 */
export const CheckpointYieldAnalyzerDslHandler = defineBatchAnalyzer(
  'checkpoint_yield_dsl.step_handlers.CheckpointYieldAnalyzerDslHandler',
  {
    workerTemplate: 'checkpoint_yield_batch_dsl_ts',
  },
  async ({ context }) => {
    const totalItems = context.getInputOr(
      'total_items',
      context.getConfig<number>('total_items') ?? DEFAULT_TOTAL_ITEMS
    );

    if (totalItems <= 0) {
      return { totalItems: 0, batchSize: 1 };
    }

    // Single batch for checkpoint test
    return {
      totalItems,
      batchSize: totalItems,
      metadata: {
        test_type: 'checkpoint_yield',
        total_items: totalItems,
        analyzed_at: new Date().toISOString(),
      },
    };
  }
);

/** Configuration for checkpoint yield worker */
interface WorkerConfig {
  itemsPerCheckpoint: number;
  failAfterItems: number | undefined;
  failOnAttempt: number;
  permanentFailure: boolean;
}

/** Processing state for checkpoint yield worker */
interface ProcessingState {
  startCursor: number;
  accumulated: { running_total: number; item_ids: string[] };
  totalProcessed: number;
  currentAttempt: number;
}

/**
 * TAS-125: Checkpoint Yield Worker Handler (DSL).
 *
 * Class-based because it needs checkpointYield(), handleNoOpWorker(),
 * and getBatchWorkerInputs() from BatchableStepHandler.
 */
export class CheckpointYieldWorkerDslHandler extends BatchableStepHandler {
  static handlerName = 'checkpoint_yield_dsl.step_handlers.CheckpointYieldWorkerDslHandler';
  static handlerVersion = '1.0.0';

  async call(context: StepContext): Promise<StepHandlerResult> {
    // Check for no-op placeholder
    const noOpResult = this.handleNoOpWorker(context);
    if (noOpResult) {
      return noOpResult;
    }

    // Get batch worker inputs
    const batchInputs = this.getBatchWorkerInputs(context);
    const cursor = batchInputs?.cursor;

    if (!cursor) {
      return this.failure('No batch inputs found', 'batch_error', false);
    }

    const config = this.getWorkerConfig(context);
    const state = this.getProcessingState(context, cursor.start_cursor);
    const endCursor = cursor.end_cursor;

    return this.processItems(config, state, endCursor);
  }

  private getWorkerConfig(context: StepContext): WorkerConfig {
    return {
      itemsPerCheckpoint: context.getInputOr(
        'items_per_checkpoint',
        context.getConfig<number>('items_per_checkpoint') ?? DEFAULT_ITEMS_PER_CHECKPOINT
      ),
      failAfterItems: context.getInput<number>('fail_after_items') ?? undefined,
      failOnAttempt: context.getInputOr('fail_on_attempt', 1),
      permanentFailure: context.getInputOr('permanent_failure', false),
    };
  }

  private getProcessingState(context: StepContext, defaultStartCursor: number): ProcessingState {
    const currentAttempt = context.retryCount + 1;

    if (context.hasCheckpoint()) {
      return {
        startCursor: (context.checkpointCursor as number) ?? defaultStartCursor,
        accumulated: (context.accumulatedResults as ProcessingState['accumulated']) ?? {
          running_total: 0,
          item_ids: [],
        },
        totalProcessed: context.checkpointItemsProcessed,
        currentAttempt,
      };
    }

    return {
      startCursor: defaultStartCursor,
      accumulated: { running_total: 0, item_ids: [] },
      totalProcessed: 0,
      currentAttempt,
    };
  }

  private processItems(
    config: WorkerConfig,
    state: ProcessingState,
    endCursor: number
  ): StepHandlerResult {
    let currentCursor = state.startCursor;
    let itemsInChunk = 0;
    let totalProcessed = state.totalProcessed;
    const accumulated = state.accumulated;

    while (currentCursor < endCursor) {
      // Failure injection
      if (this.shouldInjectFailure(config, totalProcessed, state.currentAttempt)) {
        return this.injectFailure(totalProcessed, currentCursor, config.permanentFailure);
      }

      // Process one item
      accumulated.running_total += currentCursor + 1;
      accumulated.item_ids.push(`item_${String(currentCursor).padStart(4, '0')}`);

      currentCursor += 1;
      itemsInChunk += 1;
      totalProcessed += 1;

      // Yield checkpoint at interval
      if (itemsInChunk >= config.itemsPerCheckpoint && currentCursor < endCursor) {
        return this.checkpointYield(currentCursor, totalProcessed, accumulated);
      }
    }

    // All items processed
    return this.success({
      items_processed: totalProcessed,
      items_succeeded: totalProcessed,
      items_failed: 0,
      batch_metadata: {
        ...accumulated,
        final_cursor: currentCursor,
        checkpoints_used: Math.floor(totalProcessed / config.itemsPerCheckpoint),
      },
    });
  }

  private shouldInjectFailure(
    config: WorkerConfig,
    totalProcessed: number,
    currentAttempt: number
  ): boolean {
    return (
      config.failAfterItems !== undefined &&
      totalProcessed >= config.failAfterItems &&
      currentAttempt === config.failOnAttempt
    );
  }

  private injectFailure(
    itemsProcessed: number,
    cursor: number,
    permanent: boolean
  ): StepHandlerResult {
    const errorType = permanent ? 'PermanentError' : 'RetryableError';
    const failureType = permanent ? 'permanent' : 'transient';
    const message = `Injected ${failureType} failure after ${itemsProcessed} items`;

    return this.failure(message, errorType, !permanent, {
      items_processed: itemsProcessed,
      cursor_at_failure: cursor,
      failure_type: failureType,
    });
  }
}

/**
 * TAS-125: Checkpoint Yield Aggregator Handler (DSL).
 */
export class CheckpointYieldAggregatorDslHandler extends StepHandler {
  static handlerName = 'checkpoint_yield_dsl.step_handlers.CheckpointYieldAggregatorDslHandler';
  static handlerVersion = '1.0.0';

  async call(context: StepContext): Promise<StepHandlerResult> {
    const batchResults = context.getAllDependencyResults(
      'checkpoint_yield_batch_dsl_ts'
    ) as Array<Record<string, unknown> | null>;

    // Handle no batches scenario
    const analyzeResult = context.getDependencyResult('analyze_items_dsl_ts') as Record<
      string,
      unknown
    > | null;
    const outcome = analyzeResult?.batch_processing_outcome as Record<string, unknown> | undefined;

    if (outcome?.type === 'no_batches') {
      return this.success({
        total_processed: 0,
        running_total: 0,
        test_passed: true,
        scenario: 'no_batches',
      });
    }

    if (!batchResults || batchResults.length === 0) {
      return this.failure('No batch worker results to aggregate', 'aggregation_error', false);
    }

    let totalProcessed = 0;
    let runningTotal = 0;
    const allItemIds: string[] = [];
    let checkpointsUsed = 0;

    for (const result of batchResults) {
      if (!result) continue;

      totalProcessed += (result.items_processed as number) ?? 0;
      const batchMetadata = result.batch_metadata as Record<string, unknown> | undefined;
      if (batchMetadata) {
        runningTotal += (batchMetadata.running_total as number) ?? 0;
        const itemIds = batchMetadata.item_ids as string[] | undefined;
        if (itemIds) {
          allItemIds.push(...itemIds);
        }
        checkpointsUsed += (batchMetadata.checkpoints_used as number) ?? 0;
      }
    }

    return this.success({
      total_processed: totalProcessed,
      running_total: runningTotal,
      item_count: allItemIds.length,
      checkpoints_used: checkpointsUsed,
      worker_count: batchResults.length,
      test_passed: true,
      scenario: 'with_batches',
    });
  }
}
