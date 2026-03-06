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
  -- Reuse get_step_readiness_status_batch for step-level data
  step_readiness AS (
    SELECT srs.*
    FROM unnest(input_task_uuids) AS task_uuid_list(task_uuid)
    CROSS JOIN LATERAL get_step_readiness_status(task_uuid_list.task_uuid, NULL) srs
  ),

  -- Task metadata with status, namespace, and template info
  task_info AS (
    SELECT
      t.task_uuid,
      t.named_task_uuid,
      nt.name::TEXT AS task_name,
      nt.version::TEXT AS task_version,
      tns.name::TEXT AS namespace_name,
      COALESCE(tt.to_state, 'pending')::TEXT AS task_status,
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
    LEFT JOIN task_transitions tt
      ON tt.task_uuid = t.task_uuid
      AND tt.most_recent = true
    WHERE t.task_uuid = ANY(input_task_uuids)
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

  -- Aggregated execution context (counts + derived status)
  exec_context AS (
    SELECT
      sr.task_uuid,
      COUNT(*) AS total_steps,
      COUNT(CASE WHEN sr.current_state = 'pending' THEN 1 END) AS pending_steps,
      COUNT(CASE WHEN sr.current_state = 'in_progress' THEN 1 END) AS in_progress_steps,
      COUNT(CASE WHEN sr.current_state IN ('complete', 'resolved_manually') THEN 1 END) AS completed_steps,
      COUNT(CASE WHEN sr.current_state = 'error' THEN 1 END) AS failed_steps,
      COUNT(CASE WHEN sr.current_state = 'enqueued' THEN 1 END) AS enqueued_steps,
      COUNT(CASE WHEN sr.current_state = 'enqueued_for_orchestration' THEN 1 END) AS enqueued_for_orch_steps,
      COUNT(CASE WHEN sr.current_state = 'enqueued_as_error_for_orchestration' THEN 1 END) AS enqueued_as_error_steps,
      COUNT(CASE WHEN sr.ready_for_execution = true THEN 1 END) AS ready_steps,
      COUNT(CASE WHEN sr.current_state = 'error' THEN 1 END) AS permanently_blocked_steps
    FROM step_readiness sr
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
    ti.task_uuid,
    ti.named_task_uuid,
    ti.task_name,
    ti.task_version,
    ti.namespace_name,
    ti.task_status,
    ti.created_at,
    ti.updated_at,
    ti.completed_at,
    ti.initiator,
    ti.source_system,
    ti.reason,
    ti.correlation_id,
    ti.template_configuration,

    -- Step summaries (JSONB array)
    COALESCE(ssa.step_summaries, '[]'::jsonb),

    -- Execution context (JSONB object)
    json_build_object(
      'total_steps', COALESCE(ec.total_steps, 0),
      'pending_steps', COALESCE(ec.pending_steps, 0),
      'in_progress_steps', COALESCE(ec.in_progress_steps, 0),
      'completed_steps', COALESCE(ec.completed_steps, 0),
      'failed_steps', COALESCE(ec.failed_steps, 0),
      'completion_percentage',
        CASE
          WHEN COALESCE(ec.total_steps, 0) = 0 THEN 0.0
          ELSE ROUND((COALESCE(ec.completed_steps, 0)::decimal / COALESCE(ec.total_steps, 1)::decimal) * 100, 2)
        END,
      'health_status',
        CASE
          WHEN COALESCE(ec.permanently_blocked_steps, 0) > 0 THEN 'blocked'
          WHEN COALESCE(ec.failed_steps, 0) = 0 THEN 'healthy'
          WHEN COALESCE(ec.failed_steps, 0) > 0 AND COALESCE(ec.ready_steps, 0) > 0 THEN 'recovering'
          WHEN COALESCE(ec.failed_steps, 0) > 0 AND COALESCE(ec.permanently_blocked_steps, 0) = 0 AND COALESCE(ec.ready_steps, 0) = 0 THEN 'recovering'
          ELSE 'unknown'
        END,
      'execution_status',
        CASE
          WHEN COALESCE(ec.permanently_blocked_steps, 0) > 0 THEN 'blocked_by_failures'
          WHEN COALESCE(ec.ready_steps, 0) > 0 THEN 'has_ready_steps'
          WHEN COALESCE(ec.in_progress_steps, 0) > 0
               OR COALESCE(ec.enqueued_steps, 0) > 0
               OR COALESCE(ec.enqueued_for_orch_steps, 0) > 0
               OR COALESCE(ec.enqueued_as_error_steps, 0) > 0 THEN 'processing'
          WHEN COALESCE(ec.completed_steps, 0) = COALESCE(ec.total_steps, 0) AND COALESCE(ec.total_steps, 0) > 0 THEN 'all_complete'
          ELSE 'waiting_for_dependencies'
        END,
      'recommended_action',
        CASE
          WHEN COALESCE(ec.permanently_blocked_steps, 0) > 0 THEN 'handle_failures'
          WHEN COALESCE(ec.ready_steps, 0) > 0 THEN 'execute_ready_steps'
          WHEN COALESCE(ec.in_progress_steps, 0) > 0
               OR COALESCE(ec.enqueued_steps, 0) > 0
               OR COALESCE(ec.enqueued_for_orch_steps, 0) > 0
               OR COALESCE(ec.enqueued_as_error_steps, 0) > 0 THEN 'wait_for_completion'
          WHEN COALESCE(ec.completed_steps, 0) = COALESCE(ec.total_steps, 0) AND COALESCE(ec.total_steps, 0) > 0 THEN 'finalize_task'
          ELSE 'wait_for_dependencies'
        END
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

  FROM task_info ti
  LEFT JOIN step_summaries_agg ssa ON ssa.task_uuid = ti.task_uuid
  LEFT JOIN exec_context ec ON ec.task_uuid = ti.task_uuid
  LEFT JOIN dlq_info di ON di.task_uuid = ti.task_uuid;
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
