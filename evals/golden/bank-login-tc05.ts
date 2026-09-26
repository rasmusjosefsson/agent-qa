#!/usr/bin/env bun
import { runBankGolden } from "./bank-login-lib.ts";

await runBankGolden("tc05", "Bank Login TC05 failed login keeps form usable then succeeds", async (golden) => {
  await golden.openBank();
  await golden.fill("#login-username", "wrong", "fill invalid username");
  await golden.fill("#login-password", "wrong123", "fill invalid password");
  await golden.clickSelector('[data-testid="login-submit-btn"]', "submit invalid login");
  await golden.waitSelector('[data-testid="login-submit-btn"]', "login form still usable after failed login");
  await golden.fill("#login-username", "standard_user", "fill real username after failure");
  await golden.fill("#login-password", "bank_sauce", "fill real password after failure");
  await golden.clickSelector('[data-testid="login-submit-btn"]', "resubmit login after failure");
  await golden.waitUrl("/bank/dashboard", "recovery login redirects to dashboard");
  await golden.waitSelector('[data-testid="dashboard-welcome-message"]', "dashboard welcome visible");
});
