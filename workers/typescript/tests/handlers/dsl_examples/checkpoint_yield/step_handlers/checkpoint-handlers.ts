/**
 * Checkpoint Yield DSL Step Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based checkpoint yield handlers.
 *
 * NOTE: The verbose CheckpointYieldWorkerHandler uses complex checkpoint
 * yielding with this.checkpointYield(), this.handleNoOpWorker(), etc.
 * The DSL batch worker has batchContext but not checkpoint-specific helpers.
 * For full checkpoint parity, we use defineHandler and manually implement
 * the checkpoint logic.
 */

import {
  PermanentError,
  defineBatchAnalyzer,
  defineHandler,
} from '../../../../../src/handler/functional.js';

const DEFAULT_TOTAL_ITEMS = 100;

/**
 * TAS-125: Checkpoint Yield Analyzer Handler (DSL).
 */
export const CheckpointYieldAnalyzerDslHandler = defineBatchAnalyzer(
  'checkpoint_yield_dsl.step_handlers.CheckpointYieldAnalyzerDslHandler',
  {
    workerTemplate: 'checkpoint_yield_batch_ts',
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

/**
 * TAS-125: Checkpoint Yield Worker Handler (DSL).
 *
 * NOTE: This uses defineHandler rather than defineBatchWorker because
 * the verbose handler uses checkpoint-specific helpers (checkpointYield,
 * handleNoOpWorker) that are only available on BatchableStepHandler.
 * For parity testing, we focus on the final result output.
 */
export const CheckpointYieldWorkerDslHandler = defineHandler(
  'checkpoint_yield_dsl.step_handlers.CheckpointYieldWorkerDslHandler',
  {},
  async ({ context }) => {
    // Simplified version that produces the same final output structure
    const totalItems = context.getInputOr('total_items', DEFAULT_TOTAL_ITEMS) as number;
    const itemsPerCheckpoint = context.getInputOr(
      'items_per_checkpoint',
      context.getConfig<number>('items_per_checkpoint') ?? 25
    ) as number;

    // Process all items (simplified - no actual checkpoint yielding in DSL)
    const accumulated = { running_total: 0, item_ids: [] as string[] };

    for (let cursor = 0; cursor < totalItems; cursor++) {
      accumulated.running_total += cursor + 1;
      accumulated.item_ids.push(`item_${String(cursor).padStart(4, '0')}`);
    }

    return {
      items_processed: totalItems,
      items_succeeded: totalItems,
      items_failed: 0,
      batch_metadata: {
        ...accumulated,
        final_cursor: totalItems,
        checkpoints_used: Math.floor(totalItems / itemsPerCheckpoint),
      },
    };
  }
);

/**
 * TAS-125: Checkpoint Yield Aggregator Handler (DSL).
 */
export const CheckpointYieldAggregatorDslHandler = defineHandler(
  'checkpoint_yield_dsl.step_handlers.CheckpointYieldAggregatorDslHandler',
  {},
  async ({ context }) => {
    const batchResults = context.getAllDependencyResults(
      'checkpoint_yield_batch_ts'
    ) as Array<Record<string, unknown> | null>;

    // Handle no batches scenario
    const analyzeResult = context.getDependencyResult('analyze_items_ts') as Record<
      string,
      unknown
    > | null;
    const outcome = analyzeResult?.batch_processing_outcome as Record<string, unknown> | undefined;

    if (outcome?.type === 'no_batches') {
      return {
        total_processed: 0,
        running_total: 0,
        test_passed: true,
        scenario: 'no_batches',
      };
    }

    if (!batchResults || batchResults.length === 0) {
      throw new PermanentError('No batch worker results to aggregate');
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

    return {
      total_processed: totalProcessed,
      running_total: runningTotal,
      item_count: allItemIds.length,
      checkpoints_used: checkpointsUsed,
      worker_count: batchResults.length,
      test_passed: true,
      scenario: 'with_batches',
    };
  }
);
