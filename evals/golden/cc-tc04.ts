#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// coffee-cart.app — cart quantity stepper: + doubles the line, − returns it.
await runEdgeGolden(
  "tc04", "Coffee cart — quantity stepper adjusts the line item",
  "https://coffee-cart.app/", "[data-test='Espresso']",
  async (b) => {
    await b.openPage();
    await b.clickSelector("[data-test='Espresso']", "add espresso");
    await b.clickSelector("a[href='/cart']", "open the cart");
    await b.assertElementAttribute("li.list-item", "text", "contains", "x 1", "line starts at quantity one");
    await b.clickSelector("ul:not(.cart-preview) li.list-item button[aria-label='Add one Espresso']", "increment the quantity");
    await b.assertElementAttribute("li.list-item", "text", "contains", "x 2", "quantity increments to two");
    await b.assertElementAttribute("button.pay", "text", "contains", "$20.00", "total doubles");
    await b.clickSelector("ul:not(.cart-preview) li.list-item button[aria-label='Remove one Espresso']", "decrement the quantity");
    await b.assertElementAttribute("li.list-item", "text", "contains", "x 1", "quantity returns to one");
    await b.assertElementAttribute("button.pay", "text", "contains", "$10.00", "total returns to one unit");
  },
);
