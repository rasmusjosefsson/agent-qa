#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc13", "Automation Exercise TC13 product quantity in cart", async (golden) => {
  await golden.openHome();
  await golden.waitSelectorText("#slider", "AutomationExercise", "home page is visible");
  await golden.waitSelector('.features_items .choose a[href="/product_details/1"]', "first product view link is visible");
  await golden.domClickSelector('.features_items .choose a[href="/product_details/1"]', "open first product detail from home page");
  await golden.waitUrl("/product_details/1", "product detail URL is reached");
  await golden.waitSelectorText(".product-information h2", "Blue Top", "product detail name is visible");
  await golden.waitText("Availability:", "product detail availability is visible");
  await golden.waitSelector("#quantity", "quantity input is visible");
  await golden.fillSelector("#quantity", "4", "set product quantity to four");
  await golden.domClickSelector(".product-information button.cart", "add product quantity four to cart");
  await golden.waitSelector("#cartModal.show", "added-to-cart modal is visible");
  await golden.domClickSelector('#cartModal.show a[href="/view_cart"]', "open cart from added-to-cart modal");
  await golden.waitUrl("/view_cart", "cart URL is reached");
  await golden.waitSelectorText("#cart_items .breadcrumb", "Shopping Cart", "cart page is visible");
  await golden.waitSelectorText("#product-1 .cart_description h4 a", "Blue Top", "cart product name is correct");
  await golden.waitSelectorText("#product-1 .cart_price p", "Rs. 500", "cart product price is correct");
  await golden.waitSelectorText("#product-1 .cart_quantity button", "4", "cart product quantity is exactly four");
  await golden.waitSelectorText("#product-1 .cart_total .cart_total_price", "Rs. 2000", "cart product total matches quantity four");
});
