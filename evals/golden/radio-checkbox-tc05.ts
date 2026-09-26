#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc05", "Radio TC05 — selection persists across other interactions", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.checkSelector("#radio-plan-pro", "select Pro");
  await b.checkSelector("#chk-accept-terms", "interact elsewhere");
  await b.assertElementAttribute("#radio-plan-pro", "checked", "true", "Pro still selected");
});
