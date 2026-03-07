/**
 * Batch Processing DSL Step Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based batch processing handlers.
 * Produces identical output for parity testing.
 *
 * NOTE: The verbose CsvAnalyzerHandler and CsvBatchProcessorHandler extend
 * BatchableStepHandler with complex batch-specific helpers. The DSL uses
 * defineBatchAnalyzer/defineBatchWorker for analyzer/worker, and defineHandler
 * for the aggregator.
 */

import {
  defineBatchAnalyzer,
  defineBatchWorker,
  defineHandler,
  PermanentError,
} from '../../../../../src/handler/functional.js';

const DEFAULT_BATCH_SIZE = 200;
const DEFAULT_MAX_WORKERS = 5;

/**
 * Batchable: Analyze CSV file and create batch worker configurations.
 */
export const CsvAnalyzerDslHandler = defineBatchAnalyzer(
  'batch_processing_dsl.step_handlers.CsvAnalyzerDslHandler',
  {
    inputs: {
      csvFilePath: 'csv_file_path',
      analysisMode: 'analysis_mode',
    },
    workerTemplate: 'process_csv_batch_dsl_ts',
  },
  async ({ csvFilePath, analysisMode, context }) => {
    const filePath = csvFilePath as string | undefined;
    const mode = (analysisMode as string) ?? 'inventory';

    if (!filePath) {
      throw new PermanentError('Missing required input: csv_file_path');
    }

    // Simulate CSV analysis
    const isEmptyFile = filePath.includes('empty');
    const totalRows = isEmptyFile ? 0 : 1000;

    const batchSize = (context.stepConfig?.batch_size as number) ?? DEFAULT_BATCH_SIZE;
    const maxWorkers = (context.stepConfig?.max_workers as number) ?? DEFAULT_MAX_WORKERS;

    if (totalRows === 0) {
      // Return empty batch config - the factory handles no-batch differently
      // We need to return a BatchConfig; the factory will create the batches
      return { totalItems: 0, batchSize: 1 };
    }

    const numBatches = Math.ceil(totalRows / batchSize);
    const actualWorkers = Math.min(numBatches, maxWorkers);

    return {
      totalItems: totalRows,
      batchSize: Math.ceil(totalRows / actualWorkers),
      metadata: {
        csv_file_path: filePath,
        analysis_mode: mode,
        total_rows: totalRows,
        batch_size: batchSize,
        num_batches: actualWorkers,
        analyzed_at: new Date().toISOString(),
      },
    };
  }
);

/**
 * Batch Worker: Process a batch of CSV rows.
 */
export const CsvBatchProcessorDslHandler = defineBatchWorker(
  'batch_processing_dsl.step_handlers.CsvBatchProcessorDslHandler',
  {},
  async ({ batchContext }) => {
    if (!batchContext || batchContext.isNoOp) {
      return { skipped: true, reason: 'no_op_worker' };
    }

    const rowCount = batchContext.batchSize ?? 200;
    const batchId = `batch_${String(0).padStart(3, '0')}`;

    // Simulate processing - mock results
    const validProducts = Math.floor(rowCount * 0.95);
    const invalidProducts = rowCount - validProducts;
    const lowStockItems = Math.floor(validProducts * 0.1);
    const outOfStockItems = Math.floor(validProducts * 0.02);

    return {
      batch_id: batchId,
      rows_processed: rowCount,
      cursor_start: batchContext.startCursor,
      cursor_end: batchContext.endCursor,
      valid_products: validProducts,
      invalid_products: invalidProducts,
      low_stock_items: lowStockItems,
      out_of_stock_items: outOfStockItems,
      processed_at: new Date().toISOString(),
    };
  }
);

/**
 * Deferred Convergence: Aggregate results from all batch workers.
 */
export const CsvResultsAggregatorDslHandler = defineHandler(
  'batch_processing_dsl.step_handlers.CsvResultsAggregatorDslHandler',
  {},
  async ({ context }) => {
    const batchResults = context.getAllDependencyResults(
      'process_csv_batch_dsl_ts'
    ) as Array<Record<string, unknown> | null>;

    if (!batchResults || batchResults.length === 0) {
      throw new PermanentError('No batch worker results to aggregate');
    }

    let totalProcessed = 0;
    let totalValid = 0;
    let totalInvalid = 0;
    let totalLowStock = 0;
    let totalOutOfStock = 0;
    const batchSummaries: Array<{ batch_id: string; rows: number }> = [];

    for (const result of batchResults) {
      if (result) {
        totalProcessed += (result.rows_processed as number) ?? 0;
        totalValid += (result.valid_products as number) ?? 0;
        totalInvalid += (result.invalid_products as number) ?? 0;
        totalLowStock += (result.low_stock_items as number) ?? 0;
        totalOutOfStock += (result.out_of_stock_items as number) ?? 0;
        batchSummaries.push({
          batch_id: result.batch_id as string,
          rows: (result.rows_processed as number) ?? 0,
        });
      }
    }

    return {
      total_processed: totalProcessed,
      worker_count: batchResults.length,
      valid_products: totalValid,
      invalid_products: totalInvalid,
      low_stock_items: totalLowStock,
      out_of_stock_items: totalOutOfStock,
      batch_summaries: batchSummaries,
      aggregated_at: new Date().toISOString(),
    };
  }
);
