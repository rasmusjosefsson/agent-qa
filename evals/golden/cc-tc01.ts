#!/usr/bin/env bun
import { runEdgeGolden } from "./edge-pages-lib";

// coffee-cart.app — add an item, open the cart, remove it.
// Covers: SPA menu grid, cart badge text, in-app router navigation,
// per-row aria-label actions, empty-cart state.
await runEdgeGolden(
  "tc01", "Coffee cart — add espresso, open cart, remove it",
  "https://coffee-cart.app/", "[data-test='Espresso']",
  async (b) => {
    await b.openPage();
    await b.assertElementPresent("[data-test='Espresso']", "espresso card rendered");
    await b.assertElementAttribute("[data-test='Cafe_Breve']", "aria-label", "equals", "Cafe Breve", "menu grid fully rendered");
    await b.clickSelector("[data-test='Espresso']", "add an espresso to the cart");
    await b.waitSelectorText("a[href='/cart']", "cart (1)", "cart badge counts the item");
    await b.clickSelector("a[href='/cart']", "open the cart");
    await b.assertUrlContains("/cart", "cart route loaded");
    await b.assertElementAttribute("li.list-item", "text", "contains", "Espresso", "espresso line item in the cart");
    await b.assertElementAttribute("button.pay", "text", "contains", "$10.00", "total reflects the espresso price");
    await b.clickSelector("button[aria-label='Remove all Espresso']", "remove the line item");
    await b.waitSelectorText("a[href='/cart']", "cart (0)", "cart badge returns to zero");
    await b.assertElementAttribute("body", "text", "contains", "No coffee, go add some", "empty-cart state rendered");
  },
);
