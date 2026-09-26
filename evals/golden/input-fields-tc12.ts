#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc12", "Input Fields TC12 — readonly attribute reported correctly", "https://qaplayground.com/practice/input-fields", '[data-testid="input-readonly"]', async (b) => {
  await b.openPage();
  await b.assertElementAttribute('[data-testid="input-readonly"]', "readonly", "", "readonly attribute present");
  await b.waitSelectorText('[data-testid="result-s06"]', "Readonly", "readonly state explained");
});
