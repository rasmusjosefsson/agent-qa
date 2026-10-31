// petstore-tc04 — the full money path: sign in, add to cart, checkout,
// confirm the order — ends on "Thank you, your order has been submitted."
// Spring paths: /account/signon, /cart/addItem, /order/new (form POST for
// both Continue and Confirm — Confirm is input[value=Confirm], not a link).
// Post-login clicks use clickSelectorForce: after the signon POST→302 the
// agent-browser input channel goes stale until a cross-origin nav, so the
// live leg drives via el.click() while recording the same do/click step.

import { runEdgeGolden } from "./edge-pages-lib.ts";

await runEdgeGolden(
  "tc04",
  "JPetStore — sign in and place an order end to end",
  "https://petstore.octoperf.com/account",
  "input[name=username]",
  async (b) => {
    await b.openPage();
    await b.fillSelector("input[name=username]", "j2ee", "fill username");
    await b.fillSelector("input[name=password]", "j2ee", "fill password");
    await b.clickSelector("input[value=Login]", "submit the signon form");
    await b.waitSelector('a[href*="signoff"]', "signed in — Sign Out link visible");

    await b.gotoUrl("https://petstore.octoperf.com/catalog/viewItem?itemId=EST-2", "open a second item");
    await b.waitSelector('a[href*="addItem"]', "item page ready");
    await b.clickSelectorForce('a[href*="addItem"]', "add to cart");
    await b.assertElementAttribute("body", "text", "contains", "Sub Total", "cart subtotal shown");

    await b.waitSelector('a[href*="/order/new"]', "checkout link rendered");
    await b.clickSelectorForce('a[href*="/order/new"]', "proceed to checkout");
    await b.waitSelector("input[name=newOrder]", "order form rendered");
    await b.clickSelectorForce("input[name=newOrder]", "continue to confirmation");
    await b.waitSelector("input[value=Confirm]", "confirm submit rendered");
    await b.assertElementAttribute("body", "text", "contains", "Please confirm the information below", "confirm page asks for confirmation");
    await b.clickSelectorForce("input[value=Confirm]", "confirm the order");
    await b.waitSelectorText("body", "Thank you", "order submitted message appeared");
    await b.assertElementAttribute("body", "text", "contains", "order has been submitted", "order acknowledged");
    await b.assertElementAttribute("body", "text", "contains", "Order #", "order number issued");
  },
);
