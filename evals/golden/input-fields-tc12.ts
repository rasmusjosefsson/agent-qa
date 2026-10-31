#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc12", "Input Fields TC12 — readonly attribute reported correctly", "https://qaplayground.com/practice/input-fields", '[data-testid="input-readonly"]', async (b) => {
  await b.openPage();
  // IDL `readOnly` is a boolean — claims stringify it as "true"/"false"
  // (getAttribute on a bare `readonly` attribute would be "").
  await b.assertElementAttribute('[data-testid="input-readonly"]', "readonly", "true", "readonly attribute present");
  await b.waitSelectorText('[data-testid="result-s06"]', "Readonly", "readonly state explained");
});
