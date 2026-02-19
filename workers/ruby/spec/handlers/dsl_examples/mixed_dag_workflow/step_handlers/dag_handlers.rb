# frozen_string_literal: true

# DSL mirror of MixedDagWorkflow::StepHandlers using block DSL.
#
# Mixed DAG: A -> (B, C) -> D(B+C), E(B), F(C) -> G(D+E+F)
# 7 handlers total

include TaskerCore::StepHandler::Functional

DagInitDslHandler = step_handler('mixed_dag_workflow_dsl.step_handlers.dag_init',
                                 inputs: [:even_number]) do |even_number:, context:|
  even_number ||= context.task.context['even_number']
  raise 'Task context must contain an even number' unless even_number&.even?

  result = even_number * even_number

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'initial',
      input_refs: { even_number: 'context.task.context.even_number' },
      branches: %w[dag_process_left dag_process_right]
    }
  )
end

DagProcessLeftDslHandler = step_handler('mixed_dag_workflow_dsl.step_handlers.dag_process_left',
                                        depends_on: { init_result: 'dag_init' }) do |init_result:, context:|
  raise 'Init result not found' unless init_result

  result = init_result * init_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'single_parent',
      input_refs: { init_result: 'sequence.dag_init.result' },
      branch: 'left',
      feeds_to: %w[dag_validate dag_transform]
    }
  )
end

DagProcessRightDslHandler = step_handler('mixed_dag_workflow_dsl.step_handlers.dag_process_right',
                                         depends_on: { init_result: 'dag_init' }) do |init_result:, context:|
  raise 'Init result not found' unless init_result

  result = init_result * init_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'single_parent',
      input_refs: { init_result: 'sequence.dag_init.result' },
      branch: 'right',
      feeds_to: %w[dag_validate dag_analyze]
    }
  )
end

DagValidateDslHandler = step_handler('mixed_dag_workflow_dsl.step_handlers.dag_validate',
                                     depends_on: { left_result: 'dag_process_left',
                                                   right_result: 'dag_process_right' }) do |left_result:, right_result:, context:|
  raise 'Process left result not found' unless left_result
  raise 'Process right result not found' unless right_result

  multiplied = left_result * right_result
  result = multiplied * multiplied

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'multiply_and_square',
      step_type: 'multiple_parent',
      input_refs: {
        left_result: 'sequence.dag_process_left.result',
        right_result: 'sequence.dag_process_right.result'
      },
      multiplied: multiplied,
      convergence_type: 'dual_branch'
    }
  )
end

DagTransformDslHandler = step_handler('mixed_dag_workflow_dsl.step_handlers.dag_transform',
                                      depends_on: { left_result: 'dag_process_left' }) do |left_result:, context:|
  raise 'Process left result not found' unless left_result

  result = left_result * left_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'single_parent',
      input_refs: { left_result: 'sequence.dag_process_left.result' },
      transform_type: 'left_branch_square'
    }
  )
end

DagAnalyzeDslHandler = step_handler('mixed_dag_workflow_dsl.step_handlers.dag_analyze',
                                    depends_on: { right_result: 'dag_process_right' }) do |right_result:, context:|
  raise 'Process right result not found' unless right_result

  result = right_result * right_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'single_parent',
      input_refs: { right_result: 'sequence.dag_process_right.result' },
      analysis_type: 'right_branch_square'
    }
  )
end

DagFinalizeDslHandler = step_handler('mixed_dag_workflow_dsl.step_handlers.dag_finalize',
                                     depends_on: { validate_result: 'dag_validate',
                                                   transform_result: 'dag_transform',
                                                   analyze_result: 'dag_analyze' },
                                     inputs: [:even_number]) do |validate_result:, transform_result:, analyze_result:, even_number:, context:|
  raise 'Validate result (D) not found' unless validate_result
  raise 'Transform result (E) not found' unless transform_result
  raise 'Analyze result (F) not found' unless analyze_result

  multiplied = validate_result * transform_result * analyze_result
  result = multiplied * multiplied

  original_number = even_number || context.task.context['even_number']
  expected = original_number**64

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'multiply_three_and_square',
      step_type: 'multiple_parent_final',
      input_refs: {
        validate_result: 'sequence.dag_validate.result',
        transform_result: 'sequence.dag_transform.result',
        analyze_result: 'sequence.dag_analyze.result'
      },
      multiplied: multiplied,
      verification: {
        original_number: original_number,
        expected_result: expected,
        actual_result: result,
        matches: result == expected
      }
    }
  )
end
