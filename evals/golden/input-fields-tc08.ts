#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc08", "Input Fields TC08 — field is empty after clear", "https://qaplayground.com/practice/input-fields", '[data-testid="input-clear"]', async (b) => {
  await b.openPage();
  await b.clickSelector('[data-testid="btn-clear-field"]', "clear the field");
  await b.assertElementAttribute('[data-testid="input-clear"]', "value", "", "field empty after clear");
});
