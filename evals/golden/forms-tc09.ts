#!/usr/bin/env bun
import { runFormsGolden } from "./forms-lib.ts";

await runFormsGolden("tc09", "Forms TC09 reset button clears all fields", async (golden) => {
  await golden.openPage();
  await golden.fill("#password", "pass123", "fill password");
  await golden.fill("#confirmPassword", "pass123", "fill confirm password");
  await golden.clickSelector('[data-testid="reset-form-btn"]', "reset the form");
  // `value` is a live-property read — the input was cleared.
  await golden.assertElementAttribute("#password", "value", "", "password field cleared");
  await golden.assertElementAttribute("#confirmPassword", "value", "", "confirm field cleared");
});
