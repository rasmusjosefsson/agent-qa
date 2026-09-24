#!/usr/bin/env bun
import { runBankGolden } from "./bank-login-lib.ts";

await runBankGolden("tc02", "Bank Login TC02 invalid login stays on login page", async (golden) => {
  await golden.openBank();
  await golden.fill("#login-username", "wrong", "fill invalid username");
  await golden.fill("#login-password", "wrong123", "fill invalid password");
  await golden.clickSelector('[data-testid="login-submit-btn"]', "submit invalid login");
  await golden.waitSelector('[data-testid="login-submit-btn"]', "login form still present after invalid login");
  await golden.waitUrl("/bank/login", "invalid login stays on bank login page");
});
