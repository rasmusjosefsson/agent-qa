#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// demowebshop.tricentis.com — the marquee flow: guest checkout through
// nopCommerce's six-step ajax accordion (billing → shipping address →
// shipping method → payment method → payment info → confirm) ending
// at /checkout/completed/ with a minted order number. Each continue
// button is waited on via the next step's tab/content so the ajax
// accordion is the system under test.
await runEdgeGolden(
  "tc04", "demo web shop — guest checkout accordion",
  "https://demowebshop.tricentis.com/books",
  ".product-box-add-to-cart-button",
  async (b) => {
    await b.openPage();
    await b.clickSelector(".item-box .product-box-add-to-cart-button", "add a book to the cart");
    await b.waitSelector(".bar-notification.success", "ajax add-to-cart banner");

    await b.gotoUrl("https://demowebshop.tricentis.com/cart", "open the cart");
    await b.checkSelector("#termsofservice", "agree to terms of service");
    await b.clickSelector("#checkout", "start checkout");
    await b.waitSelector(".checkout-as-guest-button", "guest-or-register page");
    await b.clickSelector(".checkout-as-guest-button", "check out as guest");
    await b.waitSelector("#BillingNewAddress_FirstName", "billing step expanded");

    await b.fillSelector("#BillingNewAddress_FirstName", "Rasmus", "billing first name");
    await b.fillSelector("#BillingNewAddress_LastName", "Testsson", "billing last name");
    await b.fillUnique("#BillingNewAddress_Email", "dw-{{vars._unique}}@qa.test", "billing email");
    await b.selectOption("#BillingNewAddress_CountryId", "1", "billing country: United States");
    await b.fillSelector("#BillingNewAddress_City", "Stockholm", "billing city");
    await b.fillSelector("#BillingNewAddress_Address1", "Testgatan 1", "billing address");
    await b.fillSelector("#BillingNewAddress_ZipPostalCode", "11122", "billing zip");
    await b.fillSelector("#BillingNewAddress_PhoneNumber", "0701234567", "billing phone");
    await b.clickSelector("#billing-buttons-container input", "billing continue");
    await b.waitSelector("#co-shipping-form", "shipping address step expanded");
    await b.clickSelector("#shipping-buttons-container input", "shipping address continue");
    // Option 0 is pre-checked on both radio groups — picking it would
    // be an honest no-op; take a non-default so the click actually flips.
    await b.waitSelector("#shippingoption_1", "shipping methods rendered");
    await b.checkSelector("#shippingoption_1", "pick Next Day Air");
    await b.clickSelector("#opc-shipping_method .shipping-method-next-step-button", "shipping method continue");
    await b.waitSelector("#paymentmethod_1", "payment methods rendered");
    await b.checkSelector("#paymentmethod_1", "pick check / money order");
    await b.clickSelector("#opc-payment_method .payment-method-next-step-button", "payment method continue");
    await b.waitSelector("#opc-payment_info .info", "payment info rendered");
    await b.assertElementAttribute("#opc-payment_info .info", "text", "contains", "Mail Personal or Business Check", "payment info confirms check order");
    await b.clickSelector("#opc-payment_info .payment-info-next-step-button", "payment info continue");
    await b.waitSelector("#opc-confirm_order .confirm-order-next-step-button", "confirm step expanded");
    await b.clickSelector("#opc-confirm_order .confirm-order-next-step-button", "place the order");
    await b.waitSelector(".order-completed .title", "order completion page");
    await b.assertElementAttribute(".order-completed .details li", "text", "matches", "Order number: \\d+", "order number minted");
  },
  { label: "dwshop" },
);
