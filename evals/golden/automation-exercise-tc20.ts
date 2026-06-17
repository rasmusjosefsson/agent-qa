#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc20", "Automation Exercise TC20 search products and cart after login", async (golden) => {
  const unique = Date.now();
  const name = `Agent QA ${unique}`;
  const email = `agent-qa-${unique}@example.com`;
  const password = "Password123!";
  await golden.openUrl("https://www.automationexercise.com/login", "open signup login page");
  await golden.fillSelector('input[data-qa="signup-name"]', name, "enter signup name");
  await golden.fillSelector('input[data-qa="signup-email"]', email, "enter signup email");
  await golden.domClickSelector('button[data-qa="signup-button"]', "submit signup form");
  await golden.waitText("ENTER ACCOUNT INFORMATION", "account information is visible");
  await golden.domClickSelector("#id_gender1", "select title Mr");
  await golden.fillSelector('input[data-qa="password"]', password, "enter account password");
  await golden.fillSelector('input[data-qa="first_name"]', "Agent", "enter first name");
  await golden.fillSelector('input[data-qa="last_name"]', "QA", "enter last name");
  await golden.fillSelector('input[data-qa="address"]', "123 Example Street", "enter address");
  await golden.selectSelector('select[data-qa="country"]', "United States", "select country");
  await golden.fillSelector('input[data-qa="state"]', "CA", "enter state");
  await golden.fillSelector('input[data-qa="city"]', "Example City", "enter city");
  await golden.fillSelector('input[data-qa="zipcode"]', "94105", "enter zipcode");
  await golden.fillSelector('input[data-qa="mobile_number"]', "555010020", "enter mobile number");
  await golden.domClickSelector('button[data-qa="create-account"]', "create account");
  await golden.waitSelectorText('h2[data-qa="account-created"]', "Account Created!", "account created");
  await golden.domClickSelector('a[data-qa="continue-button"]', "continue after account creation");
  await golden.openUrl("https://www.automationexercise.com/products", "open products page");
  await golden.fillSelector("#search_product", "jeans", "enter product search keyword");
  await golden.domClickSelector("#submit_search", "submit product search");
  await golden.waitText("SEARCHED PRODUCTS", "searched products heading is visible");
  await golden.waitSelectorText(".features_items", "Soft Stretch Jeans", "matching jeans result is visible");
  await golden.domClickSelector('.features_items .productinfo a.add-to-cart[data-product-id="33"]', "add searched jeans product to cart");
  await golden.waitSelector("#cartModal.show", "added-to-cart modal is visible");
  await golden.domClickSelector('#cartModal.show a[href="/view_cart"]', "open cart from modal");
  await golden.waitSelectorText("#cart_info", "Soft Stretch Jeans", "searched product is visible in cart");
  await golden.domClickSelector('a[href="/delete_account"]', "delete account");
  await golden.waitSelectorText('h2[data-qa="account-deleted"]', "Account Deleted!", "account deleted");
});
