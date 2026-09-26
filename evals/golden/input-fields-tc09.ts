#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc09", "Input Fields TC09 — disabled input cannot be edited", "https://qaplayground.com/practice/input-fields", '[data-testid="input-disabled"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="input-disabled"]', "disabled", "true", "input is disabled");
  await b.assertElementAttribute('[data-testid="input-disabled"]', "value", "You can't type here", "disabled input keeps its value");
});
