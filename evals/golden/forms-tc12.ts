#!/usr/bin/env bun
import { runFormsGolden } from "./forms-lib.ts";

await runFormsGolden("tc12", "Forms TC12 multiple interest checkboxes can be selected", async (golden) => {
  await golden.openPage();
  await golden.checkSelector("#interest-selenium", "check selenium");
  await golden.checkSelector("#interest-playwright", "check playwright");
  await golden.clickSelector('[data-testid="btn-interests-submit"]', "save interests");
  await golden.waitSelectorText('[data-testid="form-interests"]', "Interests saved: Selenium, Playwright", "both interests saved");
});
