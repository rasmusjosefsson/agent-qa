#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// ecommerce-playground.lambdatest.io — direct product page -> add -> cart
// page. Covers: deep-link nav into product_id=32 (iPod Touch), the POST
// assertion, the cart badge drawer, and the cart page's quantity input
// (`input[name^=quantity]`, name carries the server-side cart row id).
await runEdgeGolden(
  "tc02", "Lambdatest store — product page add, then inspect the cart",
  "https://ecommerce-playground.lambdatest.io/index.php?route=product/product&product_id=32", "h1",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent("button.btn-cart", "add-to-cart control rendered");
    await b.assertElementAttribute("body", "text", "contains", "iPod Touch", "product title rendered");
    await b.waitMs(2500, "let the theme bundle bind the delegated cart handler");
    // clickSelectorForce — see lambdatest-tc01 (hidden sticky-bar twin).
    await b.clickSelectorForce("button.btn-cart", "add the product to the cart");
    await b.assertNetworkStatus({ urlMatches: "checkout(%2F|/)cart(%2F|/)add" }, "equals", 200, "cart/add returned 200");
    await b.waitSelectorText("#cart-total-drawer", "iPod Touch", "cart drawer lists the product");
    await b.gotoUrl("https://ecommerce-playground.lambdatest.io/index.php?route=checkout/cart", "open the cart page");
    await b.waitSelector("input[name^=quantity]", "cart line item rendered");
    await b.assertElementAttribute("input[name^=quantity]", "value", "equals", "1", "one unit in the cart");
    await b.assertElementAttribute("body", "text", "contains", "iPod Touch", "cart row names the product");
  },
);
