#!/usr/bin/env bun
import { runBankGolden } from "./bank-login-lib.ts";

await runBankGolden("tc05", "Bank Login TC05 viewer has restricted access", async (golden) => {
  await golden.openBank();
  await golden.fill("#username", "viewer", "fill viewer username");
  await golden.fill("#password", "viewer123", "fill viewer password");
  await golden.clickSelector('[data-testid="login-button"]', "submit viewer login");
  await golden.waitUrl("/bank/dashboard", "viewer login redirects to dashboard");
  await golden.assertLiveSelectorText('[data-testid="viewer-badge"]', "Read-only", "viewer badge is visible");
  await golden.waitSelector('[data-testid="viewer-badge"]', "viewer badge is visible");
  await golden.assertLiveSelectorText(
    '[data-testid="role-indicator"]',
    "Read-only Viewer",
    "role indicator shows read-only viewer",
  );
  await golden.waitSelector('[data-testid="role-indicator"]', "role indicator shows read-only viewer");
  await golden.navigate("https://qaplayground.com/bank/accounts", "navigate to accounts page as viewer");
  await golden.assertLiveSelectorAbsent(
    '[data-testid="add-account-button"], [data-testid="add-new-account-button"]',
    "viewer cannot see add account button selector",
  );
  await golden.waitSelectorAbsent(
    '[data-testid="add-account-button"], [data-testid="add-new-account-button"]',
    "viewer cannot see add account button selector",
  );
});
