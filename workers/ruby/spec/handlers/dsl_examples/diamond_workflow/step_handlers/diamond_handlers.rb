# frozen_string_literal: true

# DSL mirror of DiamondWorkflow::StepHandlers using block DSL.
#
# Diamond pattern: start -> (branch_b, branch_c) -> end
# For input even_number=2: 2 -> 4 -> (16, 16) -> (16*16)^2 = 65536

include TaskerCore::StepHandler::Functional

DiamondStartDslHandler = step_handler('diamond_workflow_dsl.step_handlers.diamond_start',
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
      branches: %w[diamond_branch_b diamond_branch_c]
    }
  )
end

DiamondBranchBDslHandler = step_handler('diamond_workflow_dsl.step_handlers.diamond_branch_b',
                                        depends_on: { start_result: 'diamond_start' }) do |start_result:, context:|
  raise 'Diamond start result not found' unless start_result

  result = start_result * start_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'single_parent',
      input_refs: { start_result: 'sequence.diamond_start.result' },
      branch: 'left'
    }
  )
end

DiamondBranchCDslHandler = step_handler('diamond_workflow_dsl.step_handlers.diamond_branch_c',
                                        depends_on: { start_result: 'diamond_start' }) do |start_result:, context:|
  raise 'Diamond start result not found' unless start_result

  result = start_result * start_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'single_parent',
      input_refs: { start_result: 'sequence.diamond_start.result' },
      branch: 'right'
    }
  )
end

DiamondEndDslHandler = step_handler('diamond_workflow_dsl.step_handlers.diamond_end',
                                    depends_on: { branch_b_result: 'diamond_branch_b',
                                                  branch_c_result: 'diamond_branch_c' },
                                    inputs: [:even_number]) do |branch_b_result:, branch_c_result:, even_number:, context:|
  raise 'Branch B result not found' unless branch_b_result
  raise 'Branch C result not found' unless branch_c_result

  multiplied = branch_b_result * branch_c_result
  result = multiplied * multiplied

  original_number = even_number || context.task.context['even_number']
  expected = original_number**16

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'multiply_and_square',
      step_type: 'multiple_parent',
      input_refs: {
        branch_b_result: 'sequence.diamond_branch_b.result',
        branch_c_result: 'sequence.diamond_branch_c.result'
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
