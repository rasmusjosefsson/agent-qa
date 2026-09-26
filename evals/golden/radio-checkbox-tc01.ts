#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc01", "Radio TC01 — radio selects on click", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.checkSelector("#radio-plan-pro", "select Pro plan");
  await b.waitSelectorText('[data-testid="result-s02"]', "Selected: Pro", "selection echoed");
  await b.assertElementAttribute("#radio-plan-pro", "checked", "true", "Pro radio checked");
});
