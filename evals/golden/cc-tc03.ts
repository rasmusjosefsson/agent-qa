#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// coffee-cart.app — checkout modal: name/email/promotion checkbox → Submit →
// "Thanks for your purchase" toast and the cart empties.
await runEdgeGolden(
  "tc03", "Coffee cart — checkout modal completes the order",
  "https://coffee-cart.app/", "[data-test='Espresso']",
  async (b) => {
    await b.openPage();
    await b.clickSelector("[data-test='Espresso']", "add espresso");
    await b.clickSelector("a[href='/cart']", "open the cart");
    await b.assertUrlContains("/cart", "cart route loaded");
    await b.clickSelector("button.pay", "open the payment form");
    await b.waitSelector("#name", "payment modal rendered");
    await b.fillSelector("#name", "Devin QA", "enter payer name");
    await b.fillSelector("#email", "devin@example.com", "enter payer email");
    await b.checkSelector("#promotion", "subscribe to promotions");
    await b.clickSelector("button[type='submit']", "submit the payment form");
    await b.assertElementAttribute("body", "text", "contains", "Thanks for your purchase", "confirmation toast shown");
    await b.waitSelectorText("a[href='/cart']", "cart", "cart emptied after purchase");
  },
);
