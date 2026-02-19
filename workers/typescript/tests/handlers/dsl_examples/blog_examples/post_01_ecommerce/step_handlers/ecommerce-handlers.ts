/**
 * E-commerce Order Processing DSL Step Handlers (TAS-294).
 *
 * Factory API equivalents of the class-based e-commerce handlers.
 * Produces identical output for parity testing.
 */

import { ErrorType } from '../../../../../../src/types/error-type.js';
import { StepHandlerResult } from '../../../../../../src/types/step-handler-result.js';
import {
  PermanentError,
  RetryableError,
  defineHandler,
} from '../../../../../../src/handler/functional.js';

// =============================================================================
// Types (same as verbose)
// =============================================================================

interface CartItem {
  product_id: number;
  quantity: number;
}

interface CustomerInfo {
  email: string;
  name: string;
  phone?: string;
}

interface PaymentInfo {
  method: string;
  token: string;
  amount: number;
}

interface Product {
  id: number;
  name: string;
  price: number;
  stock: number;
}

interface ValidatedItem {
  product_id: number;
  name: string;
  price: number;
  quantity: number;
  line_total: number;
}

// =============================================================================
// Mock Data (same as verbose)
// =============================================================================

const PRODUCTS: Record<number, Product> = {
  1: { id: 1, name: 'Widget A', price: 29.99, stock: 100 },
  2: { id: 2, name: 'Widget B', price: 49.99, stock: 50 },
  3: { id: 3, name: 'Widget C', price: 19.99, stock: 25 },
  4: { id: 4, name: 'Widget D', price: 99.99, stock: 0 },
  5: { id: 5, name: 'Widget E', price: 14.99, stock: 10 },
};

const TAX_RATE = 0.08;
const SHIPPING_COST = 5.99;

// =============================================================================
// Utility Functions (same as verbose)
// =============================================================================

function generateId(prefix: string): string {
  const hex = Array.from({ length: 12 }, () => Math.floor(Math.random() * 16).toString(16)).join(
    ''
  );
  return `${prefix}_${hex}`;
}

function generateOrderNumber(): string {
  const today = new Date().toISOString().slice(0, 10).replace(/-/g, '');
  const suffix = Array.from({ length: 8 }, () => Math.floor(Math.random() * 16).toString(16))
    .join('')
    .toUpperCase();
  return `ORD-${today}-${suffix}`;
}

// =============================================================================
// Handlers
// =============================================================================

/**
 * Step 1: Validate cart items, check availability, calculate totals.
 */
export const ValidateCartDslHandler = defineHandler(
  'EcommerceDsl.StepHandlers.ValidateCartDslHandler',
  { inputs: { cartItems: 'cart_items' } },
  async ({ cartItems }) => {
    const items = cartItems as CartItem[] | undefined;

    if (!items || items.length === 0) {
      return StepHandlerResult.failure(
        'Cart items are required but were not provided',
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    const validatedItems: ValidatedItem[] = [];
    let subtotal = 0;

    for (const item of items) {
      const product = PRODUCTS[item.product_id];

      if (!product) {
        return StepHandlerResult.failure(
          `Product ${item.product_id} not found`,
          ErrorType.PERMANENT_ERROR,
          false
        );
      }

      if (product.stock < item.quantity) {
        return StepHandlerResult.failure(
          `Insufficient stock for ${product.name}. Available: ${product.stock}, Requested: ${item.quantity}`,
          ErrorType.RETRYABLE_ERROR,
          true
        );
      }

      const lineTotal = product.price * item.quantity;
      subtotal += lineTotal;

      validatedItems.push({
        product_id: product.id,
        name: product.name,
        price: product.price,
        quantity: item.quantity,
        line_total: Math.round(lineTotal * 100) / 100,
      });
    }

    const tax = Math.round(subtotal * TAX_RATE * 100) / 100;
    const total = Math.round((subtotal + tax + SHIPPING_COST) * 100) / 100;
    const itemCount = validatedItems.reduce((sum, item) => sum + item.quantity, 0);

    return StepHandlerResult.success(
      {
        validated_items: validatedItems,
        subtotal: Math.round(subtotal * 100) / 100,
        tax,
        shipping: SHIPPING_COST,
        total,
        item_count: itemCount,
      },
      {
        operation: 'validate_cart',
        products_checked: validatedItems.length,
        total_items: itemCount,
      }
    );
  }
);

/**
 * Step 2: Process customer payment.
 */
export const ProcessPaymentDslHandler = defineHandler(
  'EcommerceDsl.StepHandlers.ProcessPaymentDslHandler',
  {
    inputs: { paymentInfo: 'payment_info' },
    depends: { cartResult: 'validate_cart' },
  },
  async ({ paymentInfo, context }) => {
    const payment = paymentInfo as PaymentInfo | undefined;
    const cartTotal = context.getDependencyField('validate_cart', 'total') as number | null;

    if (!payment) {
      return StepHandlerResult.failure(
        'Payment information is required but was not provided',
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    if (cartTotal === null || cartTotal === undefined) {
      return StepHandlerResult.failure(
        'Cart total is required but was not found from validate_cart step',
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    // Simulate payment processing
    const errorResponses: Record<string, { status: string; error: string }> = {
      tok_test_declined: { status: 'card_declined', error: 'Card was declined by issuer' },
      tok_test_insufficient_funds: { status: 'insufficient_funds', error: 'Insufficient funds' },
      tok_test_network_error: { status: 'timeout', error: 'Payment gateway timeout' },
    };

    if (payment.token in errorResponses) {
      const resp = errorResponses[payment.token];
      if (resp.status === 'timeout') {
        return StepHandlerResult.failure(
          `Payment gateway timeout: ${resp.error}`,
          ErrorType.RETRYABLE_ERROR,
          true
        );
      }
      return StepHandlerResult.failure(
        resp.status === 'card_declined'
          ? `Payment declined: ${resp.error}`
          : `Payment failed: ${resp.error}`,
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    return StepHandlerResult.success(
      {
        payment_id: generateId('pay'),
        transaction_id: generateId('txn'),
        status: 'succeeded',
        amount_charged: cartTotal,
        currency: 'USD',
        payment_method: payment.method,
      },
      {
        operation: 'process_payment',
        gateway: 'MockPaymentGateway',
      }
    );
  }
);

/**
 * Step 3: Reserve inventory for order items.
 */
export const UpdateInventoryDslHandler = defineHandler(
  'EcommerceDsl.StepHandlers.UpdateInventoryDslHandler',
  { depends: { cartValidation: 'validate_cart' } },
  async ({ cartValidation }) => {
    const result = cartValidation as Record<string, unknown> | null;

    if (!result || !result.validated_items) {
      return StepHandlerResult.failure(
        'Validated cart items are required but were not found from validate_cart step',
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    const validatedItems = result.validated_items as ValidatedItem[];
    const updatedProducts: Array<{
      product_id: number;
      name: string;
      previous_stock: number;
      new_stock: number;
      quantity_reserved: number;
      reservation_id: string;
    }> = [];

    let totalItemsReserved = 0;

    for (const item of validatedItems) {
      const product = PRODUCTS[item.product_id];

      if (!product) {
        return StepHandlerResult.failure(
          `Product ${item.product_id} not found`,
          ErrorType.PERMANENT_ERROR,
          false
        );
      }

      if (product.stock < item.quantity) {
        return StepHandlerResult.failure(
          `Stock not available for ${product.name}. Available: ${product.stock}, Needed: ${item.quantity}`,
          ErrorType.RETRYABLE_ERROR,
          true
        );
      }

      updatedProducts.push({
        product_id: product.id,
        name: product.name,
        previous_stock: product.stock,
        new_stock: product.stock - item.quantity,
        quantity_reserved: item.quantity,
        reservation_id: generateId('rsv'),
      });

      totalItemsReserved += item.quantity;
    }

    return StepHandlerResult.success(
      {
        updated_products: updatedProducts,
        total_items_reserved: totalItemsReserved,
        inventory_log_id: generateId('log'),
        updated_at: new Date().toISOString(),
      },
      {
        operation: 'update_inventory',
        products_updated: updatedProducts.length,
      }
    );
  }
);

/**
 * Step 4: Create order record.
 */
export const CreateOrderDslHandler = defineHandler(
  'EcommerceDsl.StepHandlers.CreateOrderDslHandler',
  {
    inputs: { customerInfo: 'customer_info' },
    depends: {
      cartValidation: 'validate_cart',
      paymentResult: 'process_payment',
      inventoryResult: 'update_inventory',
    },
  },
  async ({ customerInfo, cartValidation, paymentResult, inventoryResult }) => {
    const customer = customerInfo as CustomerInfo | undefined;
    const cart = cartValidation as Record<string, unknown> | null;
    const payment = paymentResult as Record<string, unknown> | null;
    const inventory = inventoryResult as Record<string, unknown> | null;

    if (!customer) {
      return StepHandlerResult.failure(
        'Customer information is required but was not provided',
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    if (!cart || !cart.validated_items) {
      return StepHandlerResult.failure(
        'Cart validation results are required but were not found from validate_cart step',
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    if (!payment || !payment.payment_id) {
      return StepHandlerResult.failure(
        'Payment results are required but were not found from process_payment step',
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    if (!inventory || !inventory.updated_products) {
      return StepHandlerResult.failure(
        'Inventory results are required but were not found from update_inventory step',
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    const orderId = Math.floor(Math.random() * 9000) + 1000;
    const orderNumber = generateOrderNumber();
    const now = new Date().toISOString();

    const deliveryDate = new Date();
    deliveryDate.setDate(deliveryDate.getDate() + 7);
    const estimatedDelivery = deliveryDate.toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
    });

    return StepHandlerResult.success(
      {
        order_id: orderId,
        order_number: orderNumber,
        status: 'confirmed',
        total_amount: cart.total,
        customer_email: customer.email,
        created_at: now,
        estimated_delivery: estimatedDelivery,
      },
      {
        operation: 'create_order',
        order_id: orderId,
        order_number: orderNumber,
        item_count: cart.item_count,
      }
    );
  }
);

/**
 * Step 5: Send order confirmation email.
 */
export const SendConfirmationDslHandler = defineHandler(
  'EcommerceDsl.StepHandlers.SendConfirmationDslHandler',
  {
    inputs: { customerInfo: 'customer_info' },
    depends: {
      cartResult: 'validate_cart',
      orderResult: 'create_order',
    },
  },
  async ({ customerInfo, context }) => {
    const customer = customerInfo as CustomerInfo | undefined;
    const orderNumber = context.getDependencyField('create_order', 'order_number') as string | null;
    const totalAmount = context.getDependencyField('create_order', 'total_amount') as number | null;
    const estimatedDelivery = context.getDependencyField('create_order', 'estimated_delivery') as
      | string
      | null;
    const validatedItems = context.getDependencyField('validate_cart', 'validated_items') as
      | ValidatedItem[]
      | null;

    if (!customer || !customer.email) {
      return StepHandlerResult.failure(
        'Customer email is required but was not provided',
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    if (!orderNumber) {
      return StepHandlerResult.failure(
        'Order number is required but was not found from create_order step',
        ErrorType.PERMANENT_ERROR,
        false
      );
    }

    const emailId = generateId('eml');
    const now = new Date().toISOString();

    return StepHandlerResult.success(
      {
        email_id: emailId,
        recipient: customer.email,
        subject: `Order Confirmation - ${orderNumber}`,
        status: 'sent',
        sent_at: now,
        template: 'order_confirmation',
        template_data: {
          customer_name: customer.name,
          order_number: orderNumber,
          total_amount: totalAmount,
          estimated_delivery: estimatedDelivery,
          items: validatedItems,
        },
      },
      {
        operation: 'send_confirmation',
        email_service: 'MockEmailService',
        recipient: customer.email,
      }
    );
  }
);
