/**
 * DSL Example Handlers for E2E Testing (TAS-294).
 *
 * Factory API equivalents of all class-based example handlers.
 * Each DSL handler produces identical output to its verbose counterpart.
 */

// Batch Processing DSL Handlers
export {
  CsvAnalyzerDslHandler,
  CsvBatchProcessorDslHandler,
  CsvResultsAggregatorDslHandler,
} from './batch_processing/index.js';

// Blog Examples DSL Handlers
export {
  AggregateMetricsDslHandler as DataPipelineAggregateMetricsDslHandler,
  CheckRefundPolicyDslHandler as CustomerSuccessCheckRefundPolicyDslHandler,
  CreateOrderDslHandler as EcommerceCreateOrderDslHandler,
  CreateUserAccountDslHandler as MicroservicesCreateUserAccountDslHandler,
  ExecuteRefundWorkflowDslHandler as CustomerSuccessExecuteRefundWorkflowDslHandler,
  ExtractCustomerDataDslHandler as DataPipelineExtractCustomerDataDslHandler,
  ExtractInventoryDataDslHandler as DataPipelineExtractInventoryDataDslHandler,
  ExtractSalesDataDslHandler as DataPipelineExtractSalesDataDslHandler,
  GenerateInsightsDslHandler as DataPipelineGenerateInsightsDslHandler,
  GetManagerApprovalDslHandler as CustomerSuccessGetManagerApprovalDslHandler,
  InitializePreferencesDslHandler as MicroservicesInitializePreferencesDslHandler,
  NotifyCustomerDslHandler as PaymentsNotifyCustomerDslHandler,
  ProcessGatewayRefundDslHandler as PaymentsProcessGatewayRefundDslHandler,
  ProcessPaymentDslHandler as EcommerceProcessPaymentDslHandler,
  SendConfirmationDslHandler as EcommerceSendConfirmationDslHandler,
  SendWelcomeSequenceDslHandler as MicroservicesSendWelcomeSequenceDslHandler,
  SetupBillingProfileDslHandler as MicroservicesSetupBillingProfileDslHandler,
  TransformCustomersDslHandler as DataPipelineTransformCustomersDslHandler,
  TransformInventoryDslHandler as DataPipelineTransformInventoryDslHandler,
  TransformSalesDslHandler as DataPipelineTransformSalesDslHandler,
  UpdateInventoryDslHandler as EcommerceUpdateInventoryDslHandler,
  UpdatePaymentRecordsDslHandler as PaymentsUpdatePaymentRecordsDslHandler,
  UpdateTicketStatusDslHandler as CustomerSuccessUpdateTicketStatusDslHandler,
  UpdateUserStatusDslHandler as MicroservicesUpdateUserStatusDslHandler,
  ValidateCartDslHandler as EcommerceValidateCartDslHandler,
  ValidatePaymentEligibilityDslHandler as PaymentsValidatePaymentEligibilityDslHandler,
  ValidateRefundRequestDslHandler as CustomerSuccessValidateRefundRequestDslHandler,
} from './blog_examples/index.js';

// Checkpoint Yield DSL Handlers
export {
  CheckpointYieldAggregatorDslHandler,
  CheckpointYieldAnalyzerDslHandler,
  CheckpointYieldWorkerDslHandler,
} from './checkpoint_yield/index.js';

// Conditional Approval DSL Handlers
export {
  AutoApproveDslHandler,
  FinalizeApprovalDslHandler,
  FinanceReviewDslHandler,
  ManagerApprovalDslHandler,
  RoutingDecisionDslHandler,
  ValidateRequestDslHandler,
} from './conditional_approval/index.js';

// Diamond Workflow DSL Handlers
export {
  DiamondBranchBDslHandler,
  DiamondBranchCDslHandler,
  DiamondEndDslHandler,
  DiamondStartDslHandler,
} from './diamond_workflow/index.js';

// Domain Events DSL Handlers
export {
  ProcessPaymentDslHandler,
  SendNotificationDslHandler,
  UpdateInventoryDslHandler,
  ValidateOrderDslHandler,
} from './domain_events/index.js';

// Linear Workflow DSL Handlers
export {
  LinearStep1DslHandler,
  LinearStep2DslHandler,
  LinearStep3DslHandler,
  LinearStep4DslHandler,
} from './linear_workflow/index.js';

// Resolver Tests DSL Handlers
export {
  AlternateMethodDslHandler,
  AlternateMethodExecuteActionDslHandler,
  MultiMethodDslHandler,
  MultiMethodProcessDslHandler,
  MultiMethodRefundDslHandler,
  MultiMethodValidateDslHandler,
} from './resolver_tests/index.js';

// Error Testing DSL Handlers
export {
  PermanentErrorDslHandler,
  RetryableErrorDslHandler,
  SuccessDslHandler,
} from './test_errors/index.js';

// Test Scenarios DSL Handlers
export { SuccessStepDslHandler } from './test_scenarios/index.js';

// Import all for ALL_DSL_HANDLERS array
import {
  CsvAnalyzerDslHandler,
  CsvBatchProcessorDslHandler,
  CsvResultsAggregatorDslHandler,
} from './batch_processing/index.js';
import {
  AggregateMetricsDslHandler,
  CheckRefundPolicyDslHandler,
  CreateOrderDslHandler,
  CreateUserAccountDslHandler,
  ProcessPaymentDslHandler as EcommerceProcessPaymentDslHandler,
  UpdateInventoryDslHandler as EcommerceUpdateInventoryDslHandler,
  ExecuteRefundWorkflowDslHandler,
  ExtractCustomerDataDslHandler,
  ExtractInventoryDataDslHandler,
  ExtractSalesDataDslHandler,
  GenerateInsightsDslHandler,
  GetManagerApprovalDslHandler,
  InitializePreferencesDslHandler,
  NotifyCustomerDslHandler,
  ProcessGatewayRefundDslHandler,
  SendConfirmationDslHandler,
  SendWelcomeSequenceDslHandler,
  SetupBillingProfileDslHandler,
  TransformCustomersDslHandler,
  TransformInventoryDslHandler,
  TransformSalesDslHandler,
  UpdatePaymentRecordsDslHandler,
  UpdateTicketStatusDslHandler,
  UpdateUserStatusDslHandler,
  ValidateCartDslHandler,
  ValidatePaymentEligibilityDslHandler,
  ValidateRefundRequestDslHandler,
} from './blog_examples/index.js';
import {
  CheckpointYieldAggregatorDslHandler,
  CheckpointYieldAnalyzerDslHandler,
  CheckpointYieldWorkerDslHandler,
} from './checkpoint_yield/index.js';
import {
  AutoApproveDslHandler,
  FinalizeApprovalDslHandler,
  FinanceReviewDslHandler,
  ManagerApprovalDslHandler,
  RoutingDecisionDslHandler,
  ValidateRequestDslHandler,
} from './conditional_approval/index.js';
import {
  DiamondBranchBDslHandler,
  DiamondBranchCDslHandler,
  DiamondEndDslHandler,
  DiamondStartDslHandler,
} from './diamond_workflow/index.js';
import {
  ProcessPaymentDslHandler,
  SendNotificationDslHandler,
  UpdateInventoryDslHandler,
  ValidateOrderDslHandler,
} from './domain_events/index.js';
import {
  LinearStep1DslHandler,
  LinearStep2DslHandler,
  LinearStep3DslHandler,
  LinearStep4DslHandler,
} from './linear_workflow/index.js';
import {
  AlternateMethodDslHandler,
  AlternateMethodExecuteActionDslHandler,
  MultiMethodDslHandler,
  MultiMethodProcessDslHandler,
  MultiMethodRefundDslHandler,
  MultiMethodValidateDslHandler,
} from './resolver_tests/index.js';
import {
  PermanentErrorDslHandler,
  RetryableErrorDslHandler,
  SuccessDslHandler,
} from './test_errors/index.js';
import { SuccessStepDslHandler } from './test_scenarios/index.js';

/**
 * Array of all DSL handler classes for easy registration.
 */
export const ALL_DSL_HANDLERS = [
  // Linear Workflow
  LinearStep1DslHandler,
  LinearStep2DslHandler,
  LinearStep3DslHandler,
  LinearStep4DslHandler,
  // Test Scenarios
  SuccessStepDslHandler,
  // Error Testing
  SuccessDslHandler,
  PermanentErrorDslHandler,
  RetryableErrorDslHandler,
  // Diamond Workflow
  DiamondStartDslHandler,
  DiamondBranchBDslHandler,
  DiamondBranchCDslHandler,
  DiamondEndDslHandler,
  // Conditional Approval
  ValidateRequestDslHandler,
  RoutingDecisionDslHandler,
  AutoApproveDslHandler,
  ManagerApprovalDslHandler,
  FinanceReviewDslHandler,
  FinalizeApprovalDslHandler,
  // Batch Processing
  CsvAnalyzerDslHandler,
  CsvBatchProcessorDslHandler,
  CsvResultsAggregatorDslHandler,
  // Checkpoint Yield
  CheckpointYieldAnalyzerDslHandler,
  CheckpointYieldWorkerDslHandler,
  CheckpointYieldAggregatorDslHandler,
  // Domain Events
  ValidateOrderDslHandler,
  ProcessPaymentDslHandler,
  UpdateInventoryDslHandler,
  SendNotificationDslHandler,
  // Resolver Tests
  MultiMethodDslHandler,
  MultiMethodValidateDslHandler,
  MultiMethodProcessDslHandler,
  MultiMethodRefundDslHandler,
  AlternateMethodDslHandler,
  AlternateMethodExecuteActionDslHandler,
  // Blog Examples (Post 01: E-commerce)
  ValidateCartDslHandler,
  EcommerceProcessPaymentDslHandler,
  EcommerceUpdateInventoryDslHandler,
  CreateOrderDslHandler,
  SendConfirmationDslHandler,
  // Blog Examples (Post 02: Data Pipeline)
  ExtractSalesDataDslHandler,
  ExtractInventoryDataDslHandler,
  ExtractCustomerDataDslHandler,
  TransformSalesDslHandler,
  TransformInventoryDslHandler,
  TransformCustomersDslHandler,
  AggregateMetricsDslHandler,
  GenerateInsightsDslHandler,
  // Blog Examples (Post 03: Microservices)
  CreateUserAccountDslHandler,
  SetupBillingProfileDslHandler,
  InitializePreferencesDslHandler,
  SendWelcomeSequenceDslHandler,
  UpdateUserStatusDslHandler,
  // Blog Examples (Post 04: Team Scaling - Customer Success)
  ValidateRefundRequestDslHandler,
  CheckRefundPolicyDslHandler,
  GetManagerApprovalDslHandler,
  ExecuteRefundWorkflowDslHandler,
  UpdateTicketStatusDslHandler,
  // Blog Examples (Post 04: Team Scaling - Payments)
  ValidatePaymentEligibilityDslHandler,
  ProcessGatewayRefundDslHandler,
  UpdatePaymentRecordsDslHandler,
  NotifyCustomerDslHandler,
];
