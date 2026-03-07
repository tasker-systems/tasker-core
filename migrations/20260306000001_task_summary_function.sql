-- Migration: get_task_summaries / get_task_summary SQL Functions
-- TAS-317: Task Visualization - Live task execution state summary
--
-- Provides a rich, single-query summary of task(s) including:
-- - Task metadata (name, namespace, version, status, timestamps)
-- - Step summaries as JSONB array (state, attempts, errors, readiness)
-- - Execution context (counts, percentages, health/status classification)
-- - DLQ status (pending investigations)
--
-- Follows batch-first pattern from get_task_execution_contexts_batch.

SET search_path TO tasker, public;

-- ============================================================================
-- get_task_summaries: Batch task summary for visualization
-- ============================================================================
CREATE FUNCTION tasker.get_task_summaries(input_task_uuids uuid[])
RETURNS TABLE(
    task_uuid uuid,
    named_task_uuid uuid,
    task_name text,
    task_version text,
    namespace_name text,
    task_status text,
    created_at timestamptz,
    updated_at timestamptz,
    completed_at timestamptz,
    initiator text,
    source_system text,
    reason text,
    correlation_id uuid,
    template_configuration jsonb,
    step_summaries jsonb,
    execution_context jsonb,
    dlq_status jsonb
)
LANGUAGE plpgsql STABLE
AS $$
BEGIN
  RETURN QUERY
  WITH
  -- Reuse get_task_execution_contexts_batch for execution context
  exec_context AS (
    SELECT ctx.*
    FROM get_task_execution_contexts_batch(input_task_uuids) ctx
  ),

  -- Task metadata not covered by execution context: namespace, template config, timestamps
  task_metadata AS (
    SELECT
      t.task_uuid,
      nt.name::TEXT AS task_name,
      nt.version::TEXT AS task_version,
      tns.name::TEXT AS namespace_name,
      t.created_at::timestamptz AS created_at,
      t.updated_at::timestamptz AS updated_at,
      t.completed_at::timestamptz AS completed_at,
      t.initiator::TEXT AS initiator,
      t.source_system::TEXT AS source_system,
      t.reason::TEXT AS reason,
      t.correlation_id,
      nt.configuration AS template_configuration
    FROM tasks t
    JOIN named_tasks nt ON nt.named_task_uuid = t.named_task_uuid
    JOIN task_namespaces tns ON tns.task_namespace_uuid = nt.task_namespace_uuid
    WHERE t.task_uuid = ANY(input_task_uuids)
  ),

  -- Reuse get_step_readiness_status for per-step detail (step summaries need raw step data)
  step_readiness AS (
    SELECT srs.*
    FROM unnest(input_task_uuids) AS task_uuid_list(task_uuid)
    CROSS JOIN LATERAL get_step_readiness_status(task_uuid_list.task_uuid, NULL) srs
  ),

  -- Build per-step JSONB summaries with error info from results
  step_summaries_agg AS (
    SELECT
      sr.task_uuid,
      COALESCE(
        json_agg(
          json_build_object(
            'step_uuid', sr.workflow_step_uuid,
            'name', sr.name,
            'current_state', sr.current_state,
            'created_at', ws.created_at,
            'completed_at', ws.processed_at,
            'last_attempted_at', ws.last_attempted_at,
            'attempts', sr.attempts,
            'max_attempts', sr.max_attempts,
            'dependencies_satisfied', sr.dependencies_satisfied,
            'retry_eligible', sr.retry_eligible,
            'error_type', ws.results -> 'error' ->> 'type',
            'error_retryable', (ws.results -> 'error' ->> 'retryable')::boolean,
            'error_status_code', (ws.results -> 'error' ->> 'status_code')::integer
          ) ORDER BY ws.created_at
        )::jsonb,
        '[]'::jsonb
      ) AS step_summaries
    FROM step_readiness sr
    JOIN workflow_steps ws ON ws.workflow_step_uuid = sr.workflow_step_uuid
    GROUP BY sr.task_uuid
  ),

  -- DLQ status (pending investigations only)
  dlq_info AS (
    SELECT
      dlq.task_uuid,
      json_build_object(
        'in_dlq', true,
        'dlq_reason', dlq.dlq_reason::text,
        'resolution_status', dlq.resolution_status::text
      )::jsonb AS dlq_status
    FROM tasks_dlq dlq
    WHERE dlq.task_uuid = ANY(input_task_uuids)
      AND dlq.resolution_status = 'pending'
  )

  SELECT
    ec.task_uuid,
    ec.named_task_uuid,
    tm.task_name,
    tm.task_version,
    tm.namespace_name,
    ec.status AS task_status,
    tm.created_at,
    tm.updated_at,
    tm.completed_at,
    tm.initiator,
    tm.source_system,
    tm.reason,
    tm.correlation_id,
    tm.template_configuration,

    -- Step summaries (JSONB array)
    COALESCE(ssa.step_summaries, '[]'::jsonb),

    -- Execution context (JSONB object) - reuse computed values from get_task_execution_contexts_batch
    json_build_object(
      'total_steps', ec.total_steps,
      'pending_steps', ec.pending_steps,
      'in_progress_steps', ec.in_progress_steps,
      'completed_steps', ec.completed_steps,
      'failed_steps', ec.failed_steps,
      'completion_percentage', ec.completion_percentage,
      'health_status', ec.health_status,
      'execution_status', ec.execution_status,
      'recommended_action', ec.recommended_action
    )::jsonb AS execution_context,

    -- DLQ status (JSONB object)
    COALESCE(
      di.dlq_status,
      json_build_object(
        'in_dlq', false,
        'dlq_reason', NULL,
        'resolution_status', NULL
      )::jsonb
    ) AS dlq_status

  FROM exec_context ec
  JOIN task_metadata tm ON tm.task_uuid = ec.task_uuid
  LEFT JOIN step_summaries_agg ssa ON ssa.task_uuid = ec.task_uuid
  LEFT JOIN dlq_info di ON di.task_uuid = ec.task_uuid;
END;
$$;

COMMENT ON FUNCTION tasker.get_task_summaries(uuid[]) IS 'TAS-317: Batch task summary for visualization.

Returns rich task summaries including metadata, step details, execution context,
and DLQ status. Designed for task visualization rendering (Mermaid/SVG).

Performance: Reuses get_step_readiness_status for step-level data, single pass
aggregation for execution context, LEFT JOIN for optional DLQ data.';


-- ============================================================================
-- get_task_summary: Single-task convenience wrapper
-- ============================================================================
CREATE FUNCTION tasker.get_task_summary(input_task_uuid uuid)
RETURNS TABLE(
    task_uuid uuid,
    named_task_uuid uuid,
    task_name text,
    task_version text,
    namespace_name text,
    task_status text,
    created_at timestamptz,
    updated_at timestamptz,
    completed_at timestamptz,
    initiator text,
    source_system text,
    reason text,
    correlation_id uuid,
    template_configuration jsonb,
    step_summaries jsonb,
    execution_context jsonb,
    dlq_status jsonb
)
LANGUAGE sql STABLE
AS $$
  SELECT * FROM tasker.get_task_summaries(ARRAY[input_task_uuid]::uuid[]);
$$;

COMMENT ON FUNCTION tasker.get_task_summary(uuid) IS 'TAS-317: Single-task summary convenience wrapper. Delegates to get_task_summaries batch function.';
