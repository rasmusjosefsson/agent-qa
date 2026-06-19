#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc14", "Automation Exercise TC14 place order register while checkout", async (golden) => {
  const unique = Date.now();
  const name = `Agent QA ${unique}`;
  const email = `agent-qa-${unique}@example.com`;

  await golden.openHome();
  await golden.waitSelectorText("#slider", "AutomationExercise", "home page is visible");
  await golden.domClickSelector('.features_items .productinfo a.add-to-cart[data-product-id="1"]', "add Blue Top to cart");
  await golden.waitSelector("#cartModal.show", "added-to-cart modal is visible");
  await golden.domClickSelector('#cartModal.show a[href="/view_cart"]', "open cart from modal");
  await golden.waitUrl("/view_cart", "cart URL is reached");
  await golden.waitSelectorText("#cart_items .breadcrumb", "Shopping Cart", "cart page is visible");
  await golden.waitSelectorText("#product-1 .cart_description h4 a", "Blue Top", "cart contains Blue Top");
  await golden.domClickSelector(".check_out", "proceed to checkout while logged out");
  await golden.waitSelector("#checkoutModal.show", "register login modal is visible");
  await golden.domClickSelector('#checkoutModal.show a[href="/login"]', "open signup login from checkout modal");
  await golden.waitUrl("/login", "login page is reached");
  await golden.waitSelector('input[data-qa="signup-email"]', "signup form is visible");
  await golden.fillSelector('input[data-qa="signup-name"]', name, "enter signup name");
  await golden.fillSelector('input[data-qa="signup-email"]', email, "enter unique signup email");
  await golden.domClickSelector('button[data-qa="signup-button"]', "submit signup form");
  await golden.waitText("ENTER ACCOUNT INFORMATION", "account information form is visible");
  await golden.domClickSelector("#id_gender1", "select title Mr");
  await golden.fillSelector('input[data-qa="password"]', "Password123!", "enter account password");
  await golden.selectSelector('select[data-qa="days"]', "1", "select birth day");
  await golden.selectSelector('select[data-qa="months"]', "January", "select birth month");
  await golden.selectSelector('select[data-qa="years"]', "1990", "select birth year");
  await golden.domClickSelector("#newsletter", "select newsletter checkbox");
  await golden.domClickSelector("#optin", "select offers checkbox");
  await golden.fillSelector('input[data-qa="first_name"]', "Agent", "enter first name");
  await golden.fillSelector('input[data-qa="last_name"]', "QA", "enter last name");
  await golden.fillSelector('input[data-qa="company"]', "Example Co", "enter company");
  await golden.fillSelector('input[data-qa="address"]', "123 Example Street", "enter address");
  await golden.fillSelector('input[data-qa="address2"]', "Suite 14", "enter address line 2");
  await golden.selectSelector('select[data-qa="country"]', "United States", "select country");
  await golden.fillSelector('input[data-qa="state"]', "CA", "enter state");
  await golden.fillSelector('input[data-qa="city"]', "Example City", "enter city");
  await golden.fillSelector('input[data-qa="zipcode"]', "94105", "enter zipcode");
  await golden.fillSelector('input[data-qa="mobile_number"]', "555010014", "enter mobile number");
  await golden.domClickSelector('button[data-qa="create-account"]', "create account");
  await golden.waitSelectorText('h2[data-qa="account-created"]', "Account Created!", "account created confirmation is visible");
  await golden.domClickSelector('a[data-qa="continue-button"]', "continue after account creation");
  await golden.waitText(`Logged in as ${name}`, "logged-in username is visible");
  await golden.openUrl("https://www.automationexercise.com/view_cart", "return to cart after signup");
  await golden.waitUrl("/view_cart", "cart URL is reached after signup");
  await golden.domClickSelector(".check_out", "proceed to checkout as registered user");
  await golden.waitSelectorText("#address_delivery", "Agent QA", "delivery address is visible");
  await golden.waitSelectorText("#cart_info", "Blue Top", "review order contains Blue Top");
  await golden.waitText("Review Your Order", "review your order heading is visible");
  await golden.fillSelector('#ordermsg textarea[name="message"]', "TC14 checkout comment", "enter order comment");
  await golden.domClickSelector('a[href="/payment"].check_out', "place order");
  await golden.waitUrl("/payment", "payment URL is reached");
  await golden.fillSelector('input[data-qa="name-on-card"]', "Agent QA", "enter name on card");
  await golden.fillSelector('input[data-qa="card-number"]', "4111111111111111", "enter card number");
  await golden.fillSelector('input[data-qa="cvc"]', "123", "enter cvc");
  await golden.fillSelector('input[data-qa="expiry-month"]', "12", "enter expiry month");
  await golden.fillSelector('input[data-qa="expiry-year"]', "2030", "enter expiry year");
  await golden.domClickSelector('button[data-qa="pay-button"]', "pay and confirm order");
  await golden.assertLiveCondition(
    `(() => { const text = document.body.innerText; if (!text.includes('Your order has been placed successfully!') && !text.includes('Congratulations! Your order has been confirmed!')) throw new Error('order success message missing'); return true; })()`,
    "order success message is visible",
  );
  await golden.waitText("Your order has been placed successfully!", "order success message is replay-visible");
  await golden.domClickSelector('a[href="/delete_account"]', "delete account");
  await golden.waitSelectorText('h2[data-qa="account-deleted"]', "Account Deleted!", "account deleted confirmation is visible");
  await golden.domClickSelector('a[data-qa="continue-button"]', "continue after account deletion");
});
