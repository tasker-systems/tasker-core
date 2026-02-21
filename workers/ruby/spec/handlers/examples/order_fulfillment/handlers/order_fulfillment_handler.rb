# frozen_string_literal: true

# frozen_string_literal: true
#
# Vestigial task handler fixture - retained as an example of workflow-level
# validation logic. This class no longer inherits from TaskHandler::Base
# because task handlers have been removed from the architecture.
# Step orchestration is handled entirely by Rust.

module OrderFulfillment
  class OrderFulfillmentHandler
  end
end
