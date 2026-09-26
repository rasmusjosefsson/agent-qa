#!/usr/bin/env bun
import { runFormsGolden } from "./forms-lib.ts";

await runFormsGolden("tc06", "Forms TC06 password mismatch shows confirm password error", async (golden) => {
  await golden.openPage();
  await golden.fill("#password", "pass123", "fill password");
  await golden.fill("#confirmPassword", "different", "fill mismatched confirm password");
  await golden.clickSelector('[data-testid="submit-form-btn"]', "submit mismatched passwords");
  await golden.waitSelectorText('[data-testid="error-confirm-password"]', "Passwords do not match.", "mismatch error shown");
});
