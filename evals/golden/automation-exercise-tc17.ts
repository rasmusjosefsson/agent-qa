#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc17", "Automation Exercise TC17 remove products from cart", async (golden) => {
  await golden.openHome();
  await golden.waitSelectorText("#slider", "AutomationExercise", "home page is visible");
  await golden.openUrl("https://www.automationexercise.com/products", "open products page");
  await golden.domClickSelector('.features_items .productinfo a.add-to-cart[data-product-id="1"]', "add Blue Top to cart");
  await golden.waitSelector("#cartModal.show", "added-to-cart modal is visible");
  await golden.domClickSelector('#cartModal.show a[href="/view_cart"]', "open cart from modal");
  await golden.waitUrl("/view_cart", "cart URL is reached");
  await golden.waitSelectorText("#cart_items .breadcrumb", "Shopping Cart", "cart page is visible");
  await golden.waitSelector("#product-1", "product row is present before removal");
  await golden.domClickSelector('#product-1 a.cart_quantity_delete', "remove product from cart");
  await golden.waitSelector("#empty_cart", "empty cart state is visible after removal");
  await golden.waitText("Cart is empty!", "cart empty message is visible");
});
