#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// demowebshop.tricentis.com/cart — add an item, bump the quantity via
// the update button, then remove it: qty-input value claims and the
// empty-cart message cover the whole lifecycle.
await runEdgeGolden(
  "tc03", "demo web shop — cart quantity + remove lifecycle",
  "https://demowebshop.tricentis.com/books",
  ".product-box-add-to-cart-button",
  async (b) => {
    await b.openPage();
    await b.clickSelector(".item-box .product-box-add-to-cart-button", "add a book to the cart");
    await b.waitSelector(".bar-notification.success", "ajax add-to-cart banner");
    await b.assertElementText(".cart-qty", "(1)", "cart badge shows one item");

    await b.gotoUrl("https://demowebshop.tricentis.com/cart", "open the cart");
    await b.waitSelector(".cart-item-row", "cart row rendered");
    await b.fillSelector(".cart-item-row .qty-input", "3", "bump quantity to 3");
    await b.clickSelector("input[name='updatecart']", "update the cart");
    await b.waitMs(400, "cart reload");
    await b.assertElementAttribute(".cart-item-row .qty-input", "value", "equals", "3", "quantity persisted");

    await b.checkSelector(".cart-item-row input[name='removefromcart']", "tick remove on the row");
    await b.clickSelector("input[name='updatecart']", "update the cart again");
    await b.waitSelector(".order-summary-content", "cart summary re-rendered");
    await b.assertElementAttribute(".order-summary-content", "text", "contains", "Your Shopping Cart is empty!", "cart emptied");
    await b.assertElementText(".cart-qty", "(0)", "cart badge back to zero");
  },
  { label: "dwshop" },
);
