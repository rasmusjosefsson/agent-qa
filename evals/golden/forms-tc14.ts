#!/usr/bin/env bun
import { runFormsGolden } from "./forms-lib.ts";

await runFormsGolden("tc14", "Forms TC14 Fill Again returns to empty form from success state", async (golden) => {
  await golden.openPage();
  await golden.fill("#password", "pass123", "fill password");
  await golden.fill("#confirmPassword", "pass123", "fill confirm password");
  await golden.checkSelector("#terms", "accept terms");
  await golden.clickSelector('[data-testid="submit-form-btn"]', "submit account setup");
  await golden.waitSelectorText('[data-testid="form-success-msg"]', "Account Setup Complete!", "success panel shown");
  await golden.clickSelector('[data-testid="btn-fill-again"]', "fill again");
  await golden.assertElementAttribute("#password", "value", "", "form reset to empty");
  await golden.assertElementAttribute("#terms", "checked", "false", "terms unchecked");
});
