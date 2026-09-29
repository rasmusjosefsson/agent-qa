#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// ecommerce-playground.lambdatest.io (OpenCart, server-rendered) — full
// journey: search -> results -> product page -> POST checkout/cart/add.
// Covers: GET-form submit via a real submit button (press Enter does NOT
// submit on this theme), URL-encoded route params (route=product%2Fsearch),
// a comma-less tight submit selector (the form's earlier button is the
// visible category dropdown-toggle), hidden sticky-bar `btn-cart` twin
// (first DOM match is 0x0 — exercises the prefer-visible resolution), and
// a delegated jQuery click handler bound ~2.5s after load.
await runEdgeGolden(
  "tc01", "Lambdatest store — search, open product, add to cart",
  "https://ecommerce-playground.lambdatest.io/", "input[name=search]",
  async (b) => {
    await b.openPage();
    await b.fillSelector("input[name=search]", "iPhone", "type the search term");
    await b.clickSelector("form button.type-text", "submit the search form");
    await b.waitSelector(".product-thumb", "results grid rendered");
    await b.assertUrlContains("product%2Fsearch", "landed on the search route");
    await b.assertUrlContains("search=iPhone", "term carried in the URL");
    await b.clickSelector(".product-thumb h4 a", "open the first result");
    await b.waitSelector("h1", "product page rendered");
    await b.assertUrlContains("product/product&product_id=", "on a product page");
    await b.waitMs(2500, "let the theme bundle bind the delegated cart handler");
    // live-drive .click() — agent-browser's trusted click refuses the hidden
    // sticky-bar twin (first DOM match, covered by #main-header); replay
    // resolves the visible copy via prefer-visible.
    await b.clickSelectorForce("button.btn-cart", "add the product to the cart");
    await b.assertNetworkFired({ urlMatches: "route=checkout(%2F|/)cart(%2F|/)add", method: "POST" }, true, "cart/add POST fired");
    await b.assertNetworkStatus({ urlMatches: "checkout(%2F|/)cart(%2F|/)add" }, "equals", 200, "cart/add returned 200");
    await b.waitSelectorText("#cart-total-drawer", "x1", "cart badge shows the item");
  },
);
