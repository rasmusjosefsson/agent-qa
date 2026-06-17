#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc01", "Automation Exercise TC01 register user", async (golden) => {
  const unique = Date.now();
  const name = `Agent QA ${unique}`;
  await golden.openHome();
  await golden.openUrl("https://www.automationexercise.com/login", "open signup login page");
  await golden.waitSelector('input[data-qa="signup-email"]', "new user signup form is visible");
  await golden.fillSelector('input[data-qa="signup-name"]', name, "enter signup name");
  await golden.fillSelector('input[data-qa="signup-email"]', `agent-qa-${unique}@example.com`, "enter unique signup email");
  await golden.domClickSelector('button[data-qa="signup-button"]', "submit signup form");
  await golden.waitText("ENTER ACCOUNT INFORMATION", "account information is visible");
  await golden.domClickSelector("#id_gender1", "select title Mr");
  await golden.fillSelector('input[data-qa="password"]', "Password123!", "enter password");
  await golden.selectSelector('select[data-qa="days"]', "1", "select day");
  await golden.selectSelector('select[data-qa="months"]', "January", "select month");
  await golden.selectSelector('select[data-qa="years"]', "1990", "select year");
  await golden.domClickSelector("#newsletter", "select newsletter");
  await golden.domClickSelector("#optin", "select special offers");
  await golden.fillSelector('input[data-qa="first_name"]', "Agent", "enter first name");
  await golden.fillSelector('input[data-qa="last_name"]', "QA", "enter last name");
  await golden.fillSelector('input[data-qa="company"]', "Example Co", "enter company");
  await golden.fillSelector('input[data-qa="address"]', "123 Example Street", "enter address");
  await golden.fillSelector('input[data-qa="address2"]', "Suite 1", "enter address line 2");
  await golden.selectSelector('select[data-qa="country"]', "United States", "select country");
  await golden.fillSelector('input[data-qa="state"]', "CA", "enter state");
  await golden.fillSelector('input[data-qa="city"]', "Example City", "enter city");
  await golden.fillSelector('input[data-qa="zipcode"]', "94105", "enter zipcode");
  await golden.fillSelector('input[data-qa="mobile_number"]', "555010001", "enter mobile number");
  await golden.domClickSelector('button[data-qa="create-account"]', "create account");
  await golden.waitSelectorText('h2[data-qa="account-created"]', "Account Created!", "account created is visible");
  await golden.domClickSelector('a[data-qa="continue-button"]', "continue after account creation");
  await golden.waitText(`Logged in as ${name}`, "logged-in username is visible");
  await golden.domClickSelector('a[href="/delete_account"]', "delete account");
  await golden.waitSelectorText('h2[data-qa="account-deleted"]', "Account Deleted!", "account deleted is visible");
  await golden.domClickSelector('a[data-qa="continue-button"]', "continue after account deletion");
});
