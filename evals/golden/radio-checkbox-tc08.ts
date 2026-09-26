#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc08", "Radio TC08 — multiple checkboxes selected together", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.checkSelector("#skill-playwright", "check Playwright");
  await b.checkSelector("#skill-cypress", "check Cypress");
  await b.waitSelectorText('[data-testid="result-s03"]', "Playwright, Cypress", "both skills echoed");
  await b.assertElementAttribute("#skill-playwright", "checked", "true", "Playwright checked");
  await b.assertElementAttribute("#skill-cypress", "checked", "true", "Cypress checked");
});
