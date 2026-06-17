#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc23", "Automation Exercise TC23 verify checkout address details", async (golden) => {
  const unique = Date.now();
  const name = `Agent QA ${unique}`;
  await golden.openUrl("https://www.automationexercise.com/login", "open signup login page");
  await golden.fillSelector('input[data-qa="signup-name"]', name, "enter signup name");
  await golden.fillSelector('input[data-qa="signup-email"]', `agent-qa-${unique}@example.com`, "enter signup email");
  await golden.domClickSelector('button[data-qa="signup-button"]', "submit signup form");
  await golden.waitText("ENTER ACCOUNT INFORMATION", "account information is visible");
  await golden.domClickSelector("#id_gender1", "select title Mr");
  await golden.fillSelector('input[data-qa="password"]', "Password123!", "enter account password");
  await golden.fillSelector('input[data-qa="first_name"]', "Agent", "enter first name");
  await golden.fillSelector('input[data-qa="last_name"]', "QA", "enter last name");
  await golden.fillSelector('input[data-qa="address"]', "123 Example Street", "enter address");
  await golden.selectSelector('select[data-qa="country"]', "United States", "select country");
  await golden.fillSelector('input[data-qa="state"]', "CA", "enter state");
  await golden.fillSelector('input[data-qa="city"]', "Example City", "enter city");
  await golden.fillSelector('input[data-qa="zipcode"]', "94105", "enter zipcode");
  await golden.fillSelector('input[data-qa="mobile_number"]', "555010023", "enter mobile number");
  await golden.domClickSelector('button[data-qa="create-account"]', "create account");
  await golden.waitSelectorText('h2[data-qa="account-created"]', "Account Created!", "account created");
  await golden.domClickSelector('a[data-qa="continue-button"]', "continue after account creation");
  await golden.openUrl("https://www.automationexercise.com/products", "open products page");
  await golden.domClickSelector('.features_items .productinfo a.add-to-cart[data-product-id="1"]', "add Blue Top to cart");
  await golden.waitSelector("#cartModal.show", "added-to-cart modal is visible");
  await golden.openUrl("https://www.automationexercise.com/view_cart", "open cart after adding product");
  await golden.waitUrl("/view_cart", "cart URL is reached");
  await golden.waitSelectorText("#cart_items .breadcrumb", "Shopping Cart", "cart page is visible");
  await golden.waitSelectorText("#product-1 .cart_description h4 a", "Blue Top", "cart contains Blue Top");
  await golden.domClickSelector(".check_out", "proceed to checkout");
  await golden.waitSelectorText("#address_delivery", "123 Example Street", "delivery address matches registration");
  await golden.waitSelectorText("#address_invoice", "123 Example Street", "billing address matches registration");
  await golden.domClickSelector('a[href="/delete_account"]', "delete account");
  await golden.waitSelectorText('h2[data-qa="account-deleted"]', "Account Deleted!", "account deleted");
});
