#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc12", "Automation Exercise TC12 add products in cart", async (golden) => {
  await golden.openHome();
  await golden.waitSelectorText("#slider", "AutomationExercise", "home page is visible");
  await golden.openUrl("https://www.automationexercise.com/products", "open products page");
  await golden.waitUrl("/products", "products URL is reached");
  await golden.waitText("ALL PRODUCTS", "all products heading is visible");
  await golden.waitSelector('.features_items .productinfo a.add-to-cart[data-product-id="1"]', "first add-to-cart button is visible");
  await golden.waitSelector('.features_items .productinfo a.add-to-cart[data-product-id="2"]', "second add-to-cart button is visible");
  await golden.domClickSelector('.features_items .productinfo a.add-to-cart[data-product-id="1"]', "add first product to cart");
  await golden.waitSelector("#cartModal.show", "added-to-cart modal is visible for first product");
  await golden.domClickSelector("#cartModal.show .close-modal", "continue shopping after first product");
  await golden.domClickSelector('.features_items .productinfo a.add-to-cart[data-product-id="2"]', "add second product to cart");
  await golden.waitSelector('#cartModal.show a[href="/view_cart"]', "view cart link is visible in modal");
  await golden.domClickSelector('#cartModal.show a[href="/view_cart"]', "open cart from modal");
  await golden.waitUrl("/view_cart", "cart URL is reached");
  await golden.waitSelectorText("#cart_items .breadcrumb", "Shopping Cart", "cart page is visible");
  await golden.waitSelectorText("#product-1 .cart_description h4 a", "Blue Top", "first cart product name is correct");
  await golden.waitSelectorText("#product-1 .cart_price p", "Rs. 500", "first cart product price is correct");
  await golden.waitSelectorText("#product-1 .cart_quantity button", "1", "first cart product quantity is correct");
  await golden.waitSelectorText("#product-1 .cart_total .cart_total_price", "Rs. 500", "first cart product total is correct");
  await golden.waitSelectorText("#product-2 .cart_description h4 a", "Men Tshirt", "second cart product name is correct");
  await golden.waitSelectorText("#product-2 .cart_price p", "Rs. 400", "second cart product price is correct");
  await golden.waitSelectorText("#product-2 .cart_quantity button", "1", "second cart product quantity is correct");
  await golden.waitSelectorText("#product-2 .cart_total .cart_total_price", "Rs. 400", "second cart product total is correct");
});
