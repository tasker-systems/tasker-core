/**
 * Blog Example DSL Handlers.
 *
 * Exports all DSL handlers from blog post examples.
 */

// Post 01: E-commerce Order Processing
export {
  CreateOrderDslHandler,
  ProcessPaymentDslHandler,
  SendConfirmationDslHandler,
  UpdateInventoryDslHandler,
  ValidateCartDslHandler,
} from './post_01_ecommerce/index.js';

// Post 02: Data Pipeline Analytics
export {
  AggregateMetricsDslHandler,
  ExtractCustomerDataDslHandler,
  ExtractInventoryDataDslHandler,
  ExtractSalesDataDslHandler,
  GenerateInsightsDslHandler,
  TransformCustomersDslHandler,
  TransformInventoryDslHandler,
  TransformSalesDslHandler,
} from './post_02_data_pipeline/index.js';

// Post 03: Microservices Coordination
export {
  CreateUserAccountDslHandler,
  InitializePreferencesDslHandler,
  SendWelcomeSequenceDslHandler,
  SetupBillingProfileDslHandler,
  UpdateUserStatusDslHandler,
} from './post_03_microservices/index.js';

// Post 04: Team Scaling
export {
  CheckRefundPolicyDslHandler,
  ExecuteRefundWorkflowDslHandler,
  GetManagerApprovalDslHandler,
  NotifyCustomerDslHandler,
  ProcessGatewayRefundDslHandler,
  UpdatePaymentRecordsDslHandler,
  UpdateTicketStatusDslHandler,
  ValidatePaymentEligibilityDslHandler,
  ValidateRefundRequestDslHandler,
} from './post_04_team_scaling/index.js';
