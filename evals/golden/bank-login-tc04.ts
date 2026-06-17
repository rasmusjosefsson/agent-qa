#!/usr/bin/env bun
import { runBankGolden } from "./bank-login-lib.ts";

await runBankGolden("tc04", "Bank Login TC04 Enter submits login form", async (golden) => {
  await golden.openBank();
  await golden.fill("#username", "admin", "fill admin username");
  await golden.fill("#password", "admin123", "fill admin password and keep focus");
  await golden.pressKey("Enter", "press Enter in password field to submit login");
  await golden.waitUrl("/bank/dashboard", "Enter key redirects to dashboard");
});
