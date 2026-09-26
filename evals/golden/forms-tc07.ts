#!/usr/bin/env bun
import { runFormsGolden } from "./forms-lib.ts";

await runFormsGolden("tc07", "Forms TC07 T&C checkbox required error appears", async (golden) => {
  await golden.openPage();
  await golden.fill("#password", "pass123", "fill password");
  await golden.fill("#confirmPassword", "pass123", "fill matching confirm password");
  // Deliberately skip #terms, then submit.
  await golden.clickSelector('[data-testid="submit-form-btn"]', "submit without accepting terms");
  // The terms error has no testid (CSS-module class) — assert on the form scope.
  await golden.waitSelectorText("#registrationForm", "You must accept the Terms & Conditions.", "terms required error shown");
});
