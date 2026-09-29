// petstore-tc04 — the full money path: sign in, add to cart, checkout,
// confirm the order — ends on "Thank you, your order has been submitted."
// Exercises auth-gated navigation across four server round-trips.
// Post-login clicks use clickSelectorForce: after the signon POST→302 the
// agent-browser input channel goes stale until a cross-origin nav, so the
// live leg drives via el.click() while recording the same do/click step.

import { runEdgeGolden } from "./edge-pages-lib.ts";

await runEdgeGolden(
  "tc04",
  "JPetStore — sign in and place an order end to end",
  "https://petstore.octoperf.com/actions/Account.action?signonForm=",
  "input[name=username]",
  async (b) => {
    await b.openPage();
    await b.fillSelector("input[name=username]", "j2ee", "fill username");
    await b.fillSelector("input[name=password]", "j2ee", "fill password");
    await b.clickSelector("input[value=Login]", "submit the signon form");
    await b.waitSelector('a[href*="signoff"]', "signed in — Sign Out link visible");

    await b.gotoUrl("https://petstore.octoperf.com/actions/Catalog.action?viewItem=&itemId=EST-2", "open a second item");
    await b.waitSelector('a[href*="addItemToCart"]', "item page ready");
    await b.clickSelectorForce('a[href*="addItemToCart"]', "add to cart");
    await b.assertElementAttribute("body", "text", "contains", "Sub Total", "cart subtotal shown");

    await b.waitSelector('a[href*="newOrderForm"]', "checkout link rendered");
    await b.clickSelectorForce('a[href*="newOrderForm"]', "proceed to checkout");
    await b.waitSelector("input[name=newOrder]", "order form rendered");
    await b.clickSelectorForce("input[name=newOrder]", "continue to confirmation");
    await b.waitSelector('a[href*="confirmed=true"]', "confirm link rendered");
    await b.assertElementAttribute("body", "text", "contains", "Please confirm the information below", "confirm page asks for confirmation");
    await b.clickSelectorForce('a[href*="confirmed=true"]', "confirm the order");
    await b.waitSelectorText("body", "Thank you", "order submitted message appeared");
    await b.assertElementAttribute("body", "text", "contains", "order has been submitted", "order acknowledged");
    await b.assertElementAttribute("body", "text", "contains", "Order #", "order number issued");
  },
);
