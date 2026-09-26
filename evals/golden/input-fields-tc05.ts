#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc05", "Input Fields TC05 — input text matches expected value", "https://qaplayground.com/practice/input-fields", '[data-testid="input-read-value"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="input-read-value"]', "value", "The Matrix", "prefilled value present");
});
