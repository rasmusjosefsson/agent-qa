#!/usr/bin/env bun
import { runBankGolden } from "./bank-login-lib.ts";

await runBankGolden("tc01", "Bank Login TC01 successful admin login", async (golden) => {
  await golden.openBank();
  await golden.fill("#login-username", "admin_user", "fill admin username");
  await golden.fill("#login-password", "admin_sauce", "fill admin password");
  await golden.clickSelector('[data-testid="login-submit-btn"]', "submit admin login");
  await golden.waitUrl("/bank/dashboard", "admin login redirects to dashboard");
  await golden.waitLiveSelector('[data-testid="dashboard-welcome-message"]');
  await golden.assertLiveSelectorText(
    '[data-testid="dashboard-welcome-message"]',
    "Welcome back",
    "dashboard page title is visible",
  );
  await golden.waitSelector('[data-testid="dashboard-welcome-message"]', "dashboard page title is visible");
});
