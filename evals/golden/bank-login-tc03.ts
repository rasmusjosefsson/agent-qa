#!/usr/bin/env bun
import { runBankGolden } from "./bank-login-lib.ts";

await runBankGolden("tc03", "Bank Login TC03 password visibility toggle", async (golden) => {
  await golden.openBank();
  await golden.fill("#password", "secret", "fill password before toggling visibility");
  await golden.assertLivePasswordType("password", "password input starts hidden");
  await golden.waitSelector('#password[type="password"]', "password input starts hidden");
  await golden.clickSelector('[data-testid="toggle-password-btn"]', "show password text");
  await golden.assertLivePasswordType("text", "password input changes to text");
  await golden.waitSelector('#password[type="text"]', "password input changes to text");
  await golden.clickSelector('[data-testid="toggle-password-btn"]', "hide password text again");
  await golden.assertLivePasswordType("password", "password input changes back to password");
  await golden.waitSelector('#password[type="password"]', "password input changes back to password");
});
