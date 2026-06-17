#!/usr/bin/env bun
import { runBankGolden } from "./bank-login-lib.ts";

await runBankGolden("tc02", "Bank Login TC02 invalid credentials show alert", async (golden) => {
  await golden.openBank();
  await golden.fill("#username", "wrong", "fill invalid username");
  await golden.fill("#password", "wrong123", "fill invalid password");
  await golden.clickSelector('[data-testid="login-button"]', "submit invalid login");
  await golden.assertLiveSelectorText(
    '[data-testid="login-alert"]',
    "Invalid username or password",
    "invalid credentials alert is visible",
  );
  await golden.waitSelector('[data-testid="login-alert"]', "invalid credentials alert is visible");
  await golden.waitUrl("/bank", "invalid login stays on bank login page");
});
