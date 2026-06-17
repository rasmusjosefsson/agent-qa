#!/usr/bin/env bun
import { runBankGolden } from "./bank-login-lib.ts";

await runBankGolden("tc01", "Bank Login TC01 successful admin login", async (golden) => {
  await golden.openBank();
  await golden.fill("#username", "admin", "fill admin username");
  await golden.fill("#password", "admin123", "fill admin password");
  await golden.clickSelector('[data-testid="login-button"]', "submit admin login");
  await golden.waitUrl("/bank/dashboard", "admin login redirects to dashboard");
  await golden.assertLiveSelectorText(
    '[data-testid="page-title"]',
    "SecureBank Dashboard",
    "dashboard page title is visible",
  );
  await golden.waitSelector('[data-testid="page-title"]', "dashboard page title is visible");
});
