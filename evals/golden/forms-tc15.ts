#!/usr/bin/env bun
import { runFormsGolden } from "./forms-lib.ts";

await runFormsGolden("tc15", "Forms TC15 form page loads without errors", async (golden) => {
  await golden.openPage();
  // All five form cards hydrate cleanly.
  await golden.waitSelectorVisible('[data-testid="form-login-inner"]', "login form rendered");
  await golden.waitSelectorVisible('[data-testid="form-personal-inner"]', "personal details form rendered");
  await golden.waitSelectorVisible('[data-testid="form-address-inner"]', "address form rendered");
  await golden.waitSelectorVisible('[data-testid="form-interests-inner"]', "interests form rendered");
  await golden.waitSelectorVisible('[data-testid="registration-form"]', "account setup form rendered");
});
