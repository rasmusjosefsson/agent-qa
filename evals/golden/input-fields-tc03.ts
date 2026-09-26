#!/usr/bin/env bun
import { runPracticeGolden } from "./practice-lib";

runPracticeGolden("input-fields", "tc03", "Input Fields TC03 — Tab moves focus away from the append field", "https://qaplayground.com/practice/input-fields", '[data-testid="input-append"]', async (b) => {
  await b.openPage();
  await b.focusSelector('[data-testid="input-append"]', "focus append input");
  await b.assertElementAttribute('[data-testid="input-append"]', "focused", "true", "append input focused");
  await b.pressKey("Tab", "tab away");
  await b.assertElementAttribute('[data-testid="input-append"]', "focused", "false", "focus moved off append input");
});
