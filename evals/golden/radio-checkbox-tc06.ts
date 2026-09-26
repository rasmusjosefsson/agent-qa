#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("radio-checkbox", "tc06", "Radio TC06 — checkbox can be checked", "https://qaplayground.com/practice/radio-checkbox", '[data-testid="radio-checkbox-page"]', async (b) => {
  await b.openPage();
  await b.checkSelector("#chk-accept-terms", "accept terms");
  await b.assertElementAttribute("#chk-accept-terms", "checked", "true", "terms checked");
  await b.waitSelectorText('[data-testid="result-s01"]', "Checked", "check state echoed");
});
