#!/usr/bin/env bun
import { runFormsGolden } from "./forms-lib.ts";

await runFormsGolden("tc08", "Forms TC08 success message displays submitted name", async (golden) => {
  await golden.openPage();
  await golden.fill("#firstName", "John", "fill first name");
  await golden.fill("#lastName", "Doe", "fill last name");
  await golden.fill("#phone", "9876543210", "fill phone");
  await golden.fill("#dob", "1990-01-15", "fill date of birth");
  await golden.clickSelector("#gender-male", "select male");
  await golden.clickSelector('[data-testid="btn-personal-submit"]', "save details");
  await golden.waitSelectorText('[data-testid="form-personal"]', "Saved: John Doe", "success shows submitted name");
});
