#!/usr/bin/env bun
import { runAutomationExerciseGolden } from "./automation-exercise-lib.ts";

await runAutomationExerciseGolden("tc02", "Automation Exercise TC02 login correct user", async (golden) => {
  const unique = Date.now();
  const name = `Agent QA ${unique}`;
  const email = `agent-qa-${unique}@example.com`;
  const password = "Password123!";
  await golden.openHome();
  await golden.openUrl("https://www.automationexercise.com/login", "open signup login page");
  await golden.fillSelector('input[data-qa="signup-name"]', name, "enter setup signup name");
  await golden.fillSelector('input[data-qa="signup-email"]', email, "enter setup signup email");
  await golden.domClickSelector('button[data-qa="signup-button"]', "submit setup signup form");
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
  await golden.fillSelector('input[data-qa="mobile_number"]', "555010002", "enter mobile number");
  await golden.domClickSelector('button[data-qa="create-account"]', "create setup account");
  await golden.waitSelectorText('h2[data-qa="account-created"]', "Account Created!", "setup account created");
  await golden.domClickSelector('a[data-qa="continue-button"]', "continue after setup account creation");
  await golden.domClickSelector('a[href="/logout"]', "logout setup account");
  await golden.waitUrl("/login", "login page reached after logout");
  await golden.fillSelector('input[data-qa="login-email"]', email, "enter correct email");
  await golden.fillSelector('input[data-qa="login-password"]', password, "enter correct password");
  await golden.domClickSelector('button[data-qa="login-button"]', "login with correct credentials");
  await golden.waitText(`Logged in as ${name}`, "logged-in username is visible");
  await golden.domClickSelector('a[href="/delete_account"]', "delete account");
  await golden.waitSelectorText('h2[data-qa="account-deleted"]', "Account Deleted!", "account deleted is visible");
});
