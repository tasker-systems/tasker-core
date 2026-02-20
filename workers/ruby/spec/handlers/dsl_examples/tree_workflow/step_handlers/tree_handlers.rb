# frozen_string_literal: true

# DSL mirror of TreeWorkflow::StepHandlers using block DSL.
#
# Tree pattern: root -> (left, right) -> (d, e from left; f, g from right) -> final_convergence
# 8 handlers total

include TaskerCore::StepHandler::Functional # rubocop:disable Style/MixinUsage

TreeRootDslHandler = step_handler('tree_workflow_dsl.step_handlers.tree_root',
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
      branches: %w[tree_branch_left tree_branch_right]
    }
  )
end

TreeBranchLeftDslHandler = step_handler('tree_workflow_dsl.step_handlers.tree_branch_left',
                                        depends_on: { root_result: 'tree_root_dsl' }) do |root_result:, context:| # rubocop:disable Lint/UnusedBlockArgument
  raise 'Tree root result not found' unless root_result

  result = root_result * root_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'single_parent',
      input_refs: { root_result: 'sequence.tree_root.result' },
      branch: 'left_main',
      sub_branches: %w[tree_leaf_d tree_leaf_e]
    }
  )
end

TreeBranchRightDslHandler = step_handler('tree_workflow_dsl.step_handlers.tree_branch_right',
                                         depends_on: { root_result: 'tree_root_dsl' }) do |root_result:, context:| # rubocop:disable Lint/UnusedBlockArgument
  raise 'Tree root result not found' unless root_result

  result = root_result * root_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'single_parent',
      input_refs: { root_result: 'sequence.tree_root.result' },
      branch: 'right_main',
      sub_branches: %w[tree_leaf_f tree_leaf_g]
    }
  )
end

TreeLeafDDslHandler = step_handler('tree_workflow_dsl.step_handlers.tree_leaf_d',
                                   depends_on: { branch_result: 'tree_branch_left_dsl' }) do |branch_result:, context:| # rubocop:disable Lint/UnusedBlockArgument
  raise 'Tree branch left result not found' unless branch_result

  result = branch_result * branch_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square', step_type: 'single_parent',
      input_refs: { branch_result: 'sequence.tree_branch_left.result' },
      branch: 'left', leaf: 'd'
    }
  )
end

TreeLeafEDslHandler = step_handler('tree_workflow_dsl.step_handlers.tree_leaf_e',
                                   depends_on: { branch_result: 'tree_branch_left_dsl' }) do |branch_result:, context:| # rubocop:disable Lint/UnusedBlockArgument
  raise 'Tree branch left result not found' unless branch_result

  result = branch_result * branch_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square', step_type: 'single_parent',
      input_refs: { branch_result: 'sequence.tree_branch_left.result' },
      branch: 'left', leaf: 'e'
    }
  )
end

TreeLeafFDslHandler = step_handler('tree_workflow_dsl.step_handlers.tree_leaf_f',
                                   depends_on: { branch_result: 'tree_branch_right_dsl' }) do |branch_result:, context:| # rubocop:disable Lint/UnusedBlockArgument
  raise 'Tree branch right result not found' unless branch_result

  result = branch_result * branch_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square', step_type: 'single_parent',
      input_refs: { branch_result: 'sequence.tree_branch_right.result' },
      branch: 'right', leaf: 'f'
    }
  )
end

TreeLeafGDslHandler = step_handler('tree_workflow_dsl.step_handlers.tree_leaf_g',
                                   depends_on: { branch_result: 'tree_branch_right_dsl' }) do |branch_result:, context:| # rubocop:disable Lint/UnusedBlockArgument
  raise 'Tree branch right result not found' unless branch_result

  result = branch_result * branch_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square', step_type: 'single_parent',
      input_refs: { branch_result: 'sequence.tree_branch_right.result' },
      branch: 'right', leaf: 'g'
    }
  )
end

TreeFinalConvergenceDslHandler = step_handler('tree_workflow_dsl.step_handlers.tree_final_convergence',
                                              depends_on: { leaf_d_result: 'tree_leaf_d_dsl',
                                                            leaf_e_result: 'tree_leaf_e_dsl',
                                                            leaf_f_result: 'tree_leaf_f_dsl',
                                                            leaf_g_result: 'tree_leaf_g_dsl' },
                                              inputs: [:even_number]) do |leaf_d_result:, leaf_e_result:, leaf_f_result:, leaf_g_result:, even_number:, context:|
  raise 'Leaf D result not found' unless leaf_d_result
  raise 'Leaf E result not found' unless leaf_e_result
  raise 'Leaf F result not found' unless leaf_f_result
  raise 'Leaf G result not found' unless leaf_g_result

  multiplied = leaf_d_result * leaf_e_result * leaf_f_result * leaf_g_result
  result = multiplied * multiplied

  original_number = even_number || context.task.context['even_number']
  expected = original_number**32

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'multiply_all_and_square',
      step_type: 'multiple_parent_final',
      input_refs: {
        leaf_d_result: 'sequence.tree_leaf_d.result',
        leaf_e_result: 'sequence.tree_leaf_e.result',
        leaf_f_result: 'sequence.tree_leaf_f.result',
        leaf_g_result: 'sequence.tree_leaf_g.result'
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
