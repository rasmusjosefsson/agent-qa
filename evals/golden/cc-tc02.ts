#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// coffee-cart.app — the third added drink triggers the promo modal;
// accepting it adds a discounted drink to the cart.
await runEdgeGolden(
  "tc02", "Coffee cart — third drink offers the lucky-day promo",
  "https://coffee-cart.app/", "[data-test='Espresso']",
  async (b) => {
    await b.openPage();
    await b.clickSelector("[data-test='Espresso']", "add espresso");
    await b.waitSelectorText("a[href='/cart']", "cart (1)", "badge counts one");
    await b.clickSelector("[data-test='Espresso_Macchiato']", "add espresso macchiato");
    await b.waitSelectorText("a[href='/cart']", "cart (2)", "badge counts two");
    await b.clickSelector("[data-test='Cappuccino']", "add cappuccino");
    await b.assertElementAttribute("body", "text", "contains", "lucky day", "promo modal opened on the third add");
    await b.clickSelector("button.yes", "accept the promo drink");
    await b.waitSelectorText("a[href='/cart']", "cart (4)", "badge counts promo item");
    await b.clickSelector("a[href='/cart']", "open the cart");
    await b.assertElementAttribute("li.list-item", "text", "contains", "(Discounted)", "promo drink listed at a discount");
  },
);
