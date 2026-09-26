#!/usr/bin/env bun
import { runFormsGolden } from "./forms-lib.ts";

await runFormsGolden("tc13", "Forms TC13 form fields retain values after validation failure", async (golden) => {
  await golden.openPage();
  await golden.fill("#password", "pass123", "fill password only");
  // Submit with empty confirm + unchecked terms — validation fails.
  await golden.clickSelector('[data-testid="submit-form-btn"]', "submit incomplete form");
  // Live property read proves the typed password survived the failed submit.
  await golden.assertElementAttribute("#password", "value", "pass123", "password retained after failed submit");
  await golden.waitSelectorText("#registrationForm", "You must accept the Terms & Conditions.", "terms error shown");
});
