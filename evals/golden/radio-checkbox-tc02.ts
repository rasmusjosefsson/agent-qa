#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc02", "Radio TC02 — new selection deselects previous", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.checkSelector("#radio-plan-starter", "select Starter");
  await b.checkSelector("#radio-plan-pro", "switch to Pro");
  await b.assertElementAttribute("#radio-plan-pro", "checked", "true", "Pro checked");
  await b.assertElementAttribute("#radio-plan-starter", "checked", "false", "Starter deselected");
});
