#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc07", "Input Fields TC07 — clear button clears the field", "https://qaplayground.com/practice/input-fields", '[data-testid="input-clear"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="input-clear"]', "value", "Inception", "prefilled before clear");
  await b.clickSelector('[data-testid="btn-clear-field"]', "clear the field");
  await b.waitSelectorText('[data-testid="result-s04"]', "Field cleared", "clear confirmed");
});
