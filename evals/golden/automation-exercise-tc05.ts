#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc05", "Automation Exercise TC05 register existing email", async (golden) => {
  const unique = Date.now();
  const email = `agent-qa-${unique}@example.com`;
  await golden.openUrl("https://www.automationexercise.com/login", "open signup login page");
  await golden.fillSelector('input[data-qa="signup-name"]', `Agent QA ${unique}`, "enter setup signup name");
  await golden.fillSelector('input[data-qa="signup-email"]', email, "enter setup signup email");
  await golden.domClickSelector('button[data-qa="signup-button"]', "submit setup signup form");
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
  await golden.fillSelector('input[data-qa="mobile_number"]', "555010005", "enter mobile number");
  await golden.domClickSelector('button[data-qa="create-account"]', "create setup account");
  await golden.waitSelectorText('h2[data-qa="account-created"]', "Account Created!", "setup account created");
  await golden.domClickSelector('a[data-qa="continue-button"]', "continue after setup account creation");
  await golden.domClickSelector('a[href="/logout"]', "logout setup account");
  await golden.waitUrl("/login", "login page reached after logout");
  await golden.fillSelector('input[data-qa="signup-name"]', "Existing User", "enter duplicate signup name");
  await golden.fillSelector('input[data-qa="signup-email"]', email, "enter duplicate signup email");
  await golden.domClickSelector('button[data-qa="signup-button"]', "submit duplicate signup form");
  await golden.waitText("Email Address already exist!", "existing email error is visible");
  await golden.fillSelector('input[data-qa="login-email"]', email, "enter email for cleanup login");
  await golden.fillSelector('input[data-qa="login-password"]', "Password123!", "enter password for cleanup login");
  await golden.domClickSelector('button[data-qa="login-button"]', "login for cleanup");
  await golden.domClickSelector('a[href="/delete_account"]', "delete cleanup account");
  await golden.waitSelectorText('h2[data-qa="account-deleted"]', "Account Deleted!", "cleanup account deleted");
});
