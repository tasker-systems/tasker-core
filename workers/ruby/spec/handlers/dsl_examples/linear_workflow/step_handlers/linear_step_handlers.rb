# frozen_string_literal: true

# DSL mirror of LinearWorkflow::StepHandlers using block DSL.
#
# Same mathematical operations as the verbose version:
#   Step1: square the even_number
#   Step2: square step1 result
#   Step3: square step2 result
#   Step4: square step3 result
#
# For input even_number=2: 2 -> 4 -> 16 -> 256 -> 65536

include TaskerCore::StepHandler::Functional

LinearStep1DslHandler = step_handler('linear_workflow_dsl.step_handlers.linear_step_1',
                                     inputs: [:even_number]) do |even_number:, context:|
  raise 'Task context must contain an even number' unless even_number&.even?

  result = even_number * even_number

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'initial',
      input_refs: {
        even_number: 'context.get_input("even_number")'
      }
    }
  )
end

LinearStep2DslHandler = step_handler('linear_workflow_dsl.step_handlers.linear_step_2',
                                     depends_on: { previous_result: 'linear_step_1' }) do |previous_result:, context:|
  raise 'Previous step result not found' unless previous_result

  result = previous_result * previous_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'intermediate',
      input_refs: {
        previous_result: 'sequence.linear_step_1.result'
      }
    }
  )
end

LinearStep3DslHandler = step_handler('linear_workflow_dsl.step_handlers.linear_step_3',
                                     depends_on: { previous_result: 'linear_step_2' }) do |previous_result:, context:|
  raise 'Previous step result not found' unless previous_result

  result = previous_result * previous_result

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'single_parent',
      input_refs: {
        previous_result: 'sequence.linear_step_2.result'
      }
    }
  )
end

LinearStep4DslHandler = step_handler('linear_workflow_dsl.step_handlers.linear_step_4',
                                     depends_on: { previous_result: 'linear_step_3' },
                                     inputs: [:even_number]) do |previous_result:, even_number:, context:|
  raise 'Previous step result not found' unless previous_result

  result = previous_result * previous_result

  original_number = even_number || context.task.context['even_number']
  expected = original_number**8

  TaskerCore::Types::StepHandlerCallResult.success(
    result: result,
    metadata: {
      operation: 'square',
      step_type: 'final',
      input_refs: {
        previous_result: 'sequence.linear_step_3.result'
      },
      verification: {
        original_number: original_number,
        expected_result: expected,
        actual_result: result,
        matches: result == expected
      }
    }
  )
end
