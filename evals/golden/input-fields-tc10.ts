#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc10", "Input Fields TC10 — disabled input reports not enabled", "https://qaplayground.com/practice/input-fields", '[data-testid="input-disabled"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="input-disabled"]', "disabled", "true", "disabled property true");
  await b.waitSelectorText('[data-testid="result-s05"]', "Input is disabled", "disabled state explained");
});
