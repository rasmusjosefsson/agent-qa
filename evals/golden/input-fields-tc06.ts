#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc06", "Input Fields TC06 — read the input value", "https://qaplayground.com/practice/input-fields", '[data-testid="input-read-value"]', async (b) => {
  await b.openPage();
  await b.clickSelector('[data-testid="btn-read-value"]', "read the value");
  await b.waitSelectorText('[data-testid="result-s03"]', "Value: The Matrix", "read-value result");
});
